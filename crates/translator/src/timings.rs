//! The time each phase of a translation takes, and the memory it has needed by then, printed to
//! standard error when timings are on (`--timings` on the command line, or the environment
//! variable `JAVA_TO_RUST_TIMINGS`).
//!
//! The memory is the translator's peak resident memory so far and what it holds now, and the
//! peak of the javac process that gives it the trees (`javac::Session`), read from Linux's
//! `/proc/PID/status` while javac runs. Elsewhere the memory is left out.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

static ON: AtomicBool = AtomicBool::new(false);

/// The process id of the running javac, 0 when none runs.
static JAVAC: AtomicU32 = AtomicU32::new(0);

/// The largest peak, in kB, that a javac process of this translation reached.
static JAVAC_PEAK: AtomicU64 = AtomicU64::new(0);

/// Turns the timings on for the rest of the process.
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

/// Whether timings are on, by `enable` or by the environment.
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed) || std::env::var_os("JAVA_TO_RUST_TIMINGS").is_some()
}

/// Runs `f` and, when timings are on, prints how long it took as `phase`, with the memory used
/// so far.
pub fn phase<T>(phase: &str, f: impl FnOnce() -> T) -> T {
    if !enabled() {
        return f();
    }
    let start = Instant::now();
    let result = f();
    eprintln!("timing: {phase}: {:.3} s{}", start.elapsed().as_secs_f64(), memory());
    result
}

/// The memory used so far, as `, peak 1.23 GB (now 0.98 GB), javac peak 2.34 GB`, or nothing where
/// `/proc` is not there.
pub fn memory() -> String {
    note_javac();
    let Some((peak, now)) = status(std::process::id()) else { return String::new() };
    let mut out = format!(", peak {} (now {})", gigabytes(peak), gigabytes(now));
    let javac = JAVAC_PEAK.load(Ordering::Relaxed);
    if javac > 0 {
        out += &format!(", javac peak {}", gigabytes(javac));
    }
    out
}

/// Records that javac runs as process `pid` (or, with `None`, that it is about to end, after
/// reading its peak a last time).
pub fn javac_process(pid: Option<u32>) {
    if pid.is_none() {
        note_javac();
    }
    JAVAC.store(pid.unwrap_or(0), Ordering::Relaxed);
}

/// Reads the peak of the running javac, if timings are on and one runs.
pub fn note_javac() {
    let pid = JAVAC.load(Ordering::Relaxed);
    if pid == 0 || !enabled() {
        return;
    }
    if let Some((peak, _)) = status(pid) {
        JAVAC_PEAK.fetch_max(peak, Ordering::Relaxed);
    }
}

/// The peak and current resident memory of process `pid`, in kB (`VmHWM` and `VmRSS`).
fn status(pid: u32) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let field = |name: &str| {
        let line = text.lines().find_map(|l| l.strip_prefix(name))?;
        line.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()
    };
    Some((field("VmHWM:")?, field("VmRSS:")?))
}

fn gigabytes(kb: u64) -> String {
    format!("{:.2} GB", kb as f64 / (1024.0 * 1024.0))
}
