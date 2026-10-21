//! The time each phase of a translation takes, printed to standard error when timings are on
//! (`--timings` on the command line, or the environment variable `JAVA_TO_RUST_TIMINGS`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static ON: AtomicBool = AtomicBool::new(false);

/// Turns the timings on for the rest of the process.
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

/// Whether timings are on, by `enable` or by the environment.
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed) || std::env::var_os("JAVA_TO_RUST_TIMINGS").is_some()
}

/// Runs `f` and, when timings are on, prints how long it took as `phase`.
pub fn phase<T>(phase: &str, f: impl FnOnce() -> T) -> T {
    if !enabled() {
        return f();
    }
    let start = Instant::now();
    let result = f();
    eprintln!("timing: {phase}: {:.3} s", start.elapsed().as_secs_f64());
    result
}
