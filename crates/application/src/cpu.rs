//! Shared native CPU slots. Workers use one compute thread per active request;
//! pending interactive decoding is admitted before background work.
use frameiq_contracts::{AppError, AppResult};
use std::sync::Mutex;
#[derive(Default)]
struct Usage {
    active: usize,
    interactive_waiters: usize,
}
pub struct Budget {
    maximum: usize,
    usage: Mutex<Usage>,
}
pub struct Permit<'a> {
    budget: &'a Budget,
}
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        if let Ok(mut usage) = self.budget.usage.lock() {
            usage.active -= 1;
        }
    }
}
impl Budget {
    pub fn new(maximum: usize) -> Self {
        Self {
            maximum: maximum.max(1),
            usage: Mutex::new(Usage::default()),
        }
    }
    pub fn acquire(&self, interactive: bool, cancel: impl Fn() -> bool) -> AppResult<Permit<'_>> {
        let interrupted = || AppError::new("INTERNAL_ERROR", "CPU scheduler interrupted.");
        if interactive {
            self.usage
                .lock()
                .map_err(|_| interrupted())?
                .interactive_waiters += 1;
        }
        loop {
            let mut usage = self.usage.lock().map_err(|_| interrupted())?;
            if cancel() {
                if interactive {
                    usage.interactive_waiters -= 1;
                }
                return Err(AppError::new(
                    "CANCELLED",
                    "Queued worker request cancelled.",
                ));
            }
            if usage.active < self.maximum && (interactive || usage.interactive_waiters == 0) {
                if interactive {
                    usage.interactive_waiters -= 1;
                }
                usage.active += 1;
                return Ok(Permit { budget: self });
            }
            drop(usage);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    };
    #[test]
    fn cancellation_releases_waiter_and_interactive_precedes_background() {
        let budget = Arc::new(Budget::new(1));
        let held = budget.acquire(false, || false).unwrap();
        let (tx, rx) = mpsc::channel();
        let b = budget.clone();
        let t = tx.clone();
        let background = std::thread::spawn(move || {
            let _p = b.acquire(false, || false).unwrap();
            t.send("background").unwrap();
        });
        let b = budget.clone();
        let t = tx.clone();
        let interactive = std::thread::spawn(move || {
            let _p = b.acquire(true, || false).unwrap();
            t.send("interactive").unwrap();
        });
        let start = std::time::Instant::now();
        while budget.usage.lock().unwrap().interactive_waiters == 0 {
            assert!(start.elapsed().as_secs() < 2);
            std::thread::yield_now();
        }
        drop(held);
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            "interactive"
        );
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            "background"
        );
        interactive.join().unwrap();
        background.join().unwrap();
        let held = budget.acquire(false, || false).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let b = budget.clone();
        let c = cancel.clone();
        let waiting = std::thread::spawn(move || {
            b.acquire(true, || c.load(Ordering::SeqCst))
                .err()
                .unwrap()
                .code
        });
        let start = std::time::Instant::now();
        while budget.usage.lock().unwrap().interactive_waiters == 0 {
            assert!(start.elapsed().as_secs() < 2);
            std::thread::yield_now();
        }
        cancel.store(true, Ordering::SeqCst);
        assert_eq!(waiting.join().unwrap(), "CANCELLED");
        assert_eq!(budget.usage.lock().unwrap().interactive_waiters, 0);
        drop(held);
        assert!(budget.acquire(false, || false).is_ok());
    }
}
