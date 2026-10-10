use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
// Sample the coordinator plus its currently live descendants. This is observed
// RSS, not allocator accounting or an OS high-water mark; short-lived peaks can
// occur between samples and shared pages may be counted more than once.
pub struct MemorySampler {
    stop: Arc<AtomicBool>,
    values: Arc<Mutex<Vec<u64>>>,
    pub checkpoints: Arc<Mutex<Vec<serde_json::Value>>>,
    pub maximum_descendants: Arc<AtomicUsize>,
    pub observed_pids: Arc<Mutex<std::collections::HashSet<sysinfo::Pid>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl MemorySampler {
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let values = Arc::new(Mutex::new(Vec::new()));
        let maximum_descendants = Arc::new(AtomicUsize::new(0));
        let maximum = maximum_descendants.clone();
        let observed_pids = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let observed = observed_pids.clone();
        let checkpoints = Arc::new(Mutex::new(Vec::new()));
        let points = checkpoints.clone();
        let sampled_start = Instant::now();
        let quit = stop.clone();
        let samples = values.clone();
        let root = sysinfo::Pid::from_u32(std::process::id());
        let thread = std::thread::spawn(move || {
            let mut system = sysinfo::System::new();
            while !quit.load(Ordering::Relaxed) {
                system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                let mut tree = std::collections::HashSet::from([root]);
                loop {
                    let before = tree.len();
                    for (id, process) in system.processes() {
                        if process.parent().is_some_and(|p| tree.contains(&p)) {
                            tree.insert(*id);
                        }
                    }
                    if before == tree.len() {
                        break;
                    }
                }
                observed
                    .lock()
                    .unwrap()
                    .extend(tree.iter().copied().filter(|id| *id != root));
                maximum.fetch_max(tree.len().saturating_sub(1), Ordering::Relaxed);
                let sum = tree
                    .iter()
                    .filter_map(|id| system.process(*id))
                    .map(|p| p.memory())
                    .sum();
                let cpu: f32 = tree
                    .iter()
                    .filter_map(|id| system.process(*id))
                    .map(|p| p.cpu_usage())
                    .sum();
                let mut samples = samples.lock().unwrap();
                if samples.len() % 20 == 0 {
                    points.lock().unwrap().push(serde_json::json!({"elapsedMs":sampled_start.elapsed().as_millis(),"rssBytes":sum,"cpuPercent":cpu,"descendantCount":tree.len().saturating_sub(1)}));
                }
                samples.push(sum);
                drop(samples);
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        Self {
            stop,
            values,
            checkpoints,
            maximum_descendants,
            observed_pids,
            thread: Some(thread),
        }
    }
    pub fn finish(&mut self) -> Vec<u64> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.values.lock().unwrap().clone()
    }
}
impl Drop for MemorySampler {
    fn drop(&mut self) {
        self.finish();
    }
}
