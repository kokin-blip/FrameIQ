use frameiq_application::Application;
use frameiq_contracts::*;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
fn terminal(app: &Application, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    loop {
        let job = app.get_job(id)?;
        match job.status {
            JobStatus::Succeeded => return Ok(()),
            JobStatus::Failed | JobStatus::Cancelled => {
                return Err(job
                    .error
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Job cancelled".into())
                    .into())
            }
            _ => {}
        }
        if start.elapsed() > Duration::from_secs(300) {
            return Err("Job timed out".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn p95(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)]
}
// Sample the coordinator plus its currently live descendants. This is observed
// RSS, not allocator accounting or an OS high-water mark; short-lived peaks can
// occur between samples and shared pages may be counted more than once.
struct MemorySampler {
    stop: Arc<AtomicBool>,
    values: Arc<Mutex<Vec<u64>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl MemorySampler {
    fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let values = Arc::new(Mutex::new(Vec::new()));
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
                let sum = tree
                    .iter()
                    .filter_map(|id| system.process(*id))
                    .map(|p| p.memory())
                    .sum();
                samples.lock().unwrap().push(sum);
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        Self {
            stop,
            values,
            thread: Some(thread),
        }
    }
    fn finish(&mut self) -> Vec<u64> {
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(4..=5).contains(&args.len()) {
        return Err(
            "Usage: benchmark <video> <worker> <report.json> [sustained-iterations:1..10000]"
                .into(),
        );
    }
    let iterations = args
        .get(4)
        .map(|s| s.parse::<u32>())
        .transpose()?
        .unwrap_or(20);
    if !(1..=10000).contains(&iterations) {
        return Err("Sustained iterations must be 1..10000".into());
    }
    let mut memory = MemorySampler::start();
    let data = tempfile::tempdir()?;
    let app = Application::new(data.path(), PathBuf::from(&args[2]), Arc::new(|_| {}))?;
    let project = app.create_project("Benchmark")?;
    let start = Instant::now();
    let imported = app.import_video(&project.id, &PathBuf::from(&args[1]), None)?;
    terminal(&app, &imported.job_id)?;
    let import_ms = start.elapsed().as_secs_f64() * 1000.;
    let source = app.sources(&project.id)?[0].clone();
    let count = source.frame_count.parse::<u64>()?;
    if count == 0 {
        return Err("Source contains no indexed frames".into());
    }
    let mut uncached = Vec::new();
    let mut cached = Vec::new();
    for i in 0..10 {
        let ordinal = (count.saturating_sub(1) * i / 9).to_string();
        let start = Instant::now();
        app.preview(&source.id, &ordinal)?;
        uncached.push(start.elapsed().as_secs_f64() * 1000.);
        for _ in 0..5 {
            let start = Instant::now();
            app.preview(&source.id, &ordinal)?;
            cached.push(start.elapsed().as_secs_f64() * 1000.);
        }
    }
    let start = Instant::now();
    let analysis = app.analyze(&source.id)?;
    terminal(&app, &analysis.job_id)?;
    let analysis_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut rerank = Vec::new();
    let mut candidates = 0;
    for _ in 0..10 {
        let start = Instant::now();
        if let Some(result) = app.recommendations(
            &source.id,
            RankingConfigV1 {
                count: 8,
                sharpness_weight: 50.,
                exposure_weight: 30.,
                blur_weight: 20.,
            },
        )? {
            candidates = result.frames.len();
        }
        rerank.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let measurements = app.quality_measurements(&source.id)?;
    let rank_2000 = if measurements.len() >= 2000 {
        let mut times = Vec::new();
        for _ in 0..10 {
            let start = Instant::now();
            frameiq_analysis::rank(&measurements[..2000], &RankingConfigV1::default())?;
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        Some(p95(&mut times))
    } else {
        None
    };
    let sustained_start = Instant::now();
    let mut sustained_times = Vec::new();
    for i in 0..iterations {
        let start = Instant::now();
        app.preview(
            &source.id,
            &(count.saturating_sub(1) * (i as u64 % 10) / 9).to_string(),
        )?;
        app.recommendations(&source.id, RankingConfigV1::default())?;
        sustained_times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let sustained_ms = sustained_start.elapsed().as_secs_f64() * 1000.;
    let cancellation = app.analyze(&source.id)?;
    let start = Instant::now();
    app.cancel(&cancellation.job_id)?;
    let cancel_ack_ms = start.elapsed().as_secs_f64() * 1000.;
    while !matches!(
        app.get_job(&cancellation.job_id)?.status,
        JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
    ) {
        if start.elapsed() > Duration::from_secs(10) {
            return Err("Cancellation did not reach a terminal state".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let cancel_terminal_ms = start.elapsed().as_secs_f64() * 1000.;
    let cancellation_status = app.get_job(&cancellation.job_id)?.status;
    let samples = memory.finish();
    let report = serde_json::json!({
        "schemaVersion":2,"buildProfile":if cfg!(debug_assertions){"debug"}else{"release"},
        "hardware":app.hardware(),"sourceMetadata":source.metadata,"frames":source.frame_count,
        "workerBuildProfile":"Not asserted; supplied worker binary is independent of coordinator profile",
        "importMs":import_ms,"uncachedSeekP95Ms":p95(&mut uncached),"cachedDisplayP95Ms":p95(&mut cached),
        "analysisMs":analysis_ms,"rerankP95Ms":p95(&mut rerank),"recommendationCount":candidates,
        "measuredCandidateCount":measurements.len(),"rank2000P95Ms":rank_2000,
        "sustained":{"iterations":iterations,"durationMs":sustained_ms,"iterationP95Ms":p95(&mut sustained_times)},
        "cancellation":{"acknowledgementMs":cancel_ack_ms,"terminalMs":cancel_terminal_ms,"status":cancellation_status,
            "scope":"Cached analysis cancellation; a succeeded race does not establish cancellation latency."},
        "memory":{"samplePeriodMs":50,"sampleCount":samples.len(),"peakObservedTreeRssBytes":samples.iter().max(),
            "firstObservedTreeRssBytes":samples.first(),"lastObservedTreeRssBytes":samples.last(),
            "scope":"Coordinator and live descendant RSS; sampled, shared pages can be double counted; not an OS high-water mark."},
        "scope":"Backend timings on supplied media; no UI latency or accuracy claim. rank2000 is null unless 2000 real cached measurements exist. Sustained duration is reported rather than assumed to be a long-run test."
    });
    std::fs::write(&args[3], serde_json::to_vec_pretty(&report)?)?;
    println!("Wrote {}", args[3]);
    Ok(())
}
