//! Running the programs of a test on several threads.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Calls `f` on each index below `n` on one thread per CPU (or `CORPUS_THREADS` threads, to keep
/// memory down when several test runs share a machine), and returns the results in order.
pub fn parallel<R: Send>(n: usize, f: impl Fn(usize) -> R + Sync) -> Vec<R> {
    let threads = std::env::var("CORPUS_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<R>>> = (0..n).map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.min(n) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    *results[i].lock().unwrap() = Some(f(i));
                }
            });
        }
    });
    results.into_iter().map(|r| r.into_inner().unwrap().expect("every index is run")).collect()
}
