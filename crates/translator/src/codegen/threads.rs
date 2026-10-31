//! The modules of a round written on several threads. Each thread has a generator of its own,
//! with its own resolver (`modules::enter`) and caches, and takes the next module not yet taken,
//! in the order they are written, until none is left; what it writes of a module depends on the
//! module alone (`Gen::module`), so the modules are the same whichever thread writes them, and
//! they are put back in their order. What the generators found besides the text (`Found`) is
//! the same as one generator would find, as each part of it only grows. A panic outside the
//! methods, which a census does not catch (`catch_panics`), comes out of the first module in
//! their order that panicked, with its place in the translator, as it would with one thread.
//!
//! The generator writes on four threads, or as many as the machine has if fewer, unless
//! `JAVA_TO_RUST_GENERATOR_THREADS` says how many (see "Generator threads" in
//! docs/performance.md).

use super::*;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The stack of each thread: as large as that of the translator's own (`main.rs`), as the
/// generator recurses over the code.
const STACK: usize = 1 << 30;

thread_local! {
    /// How many threads the generator of this thread writes modules on (`on_threads`).
    static THREADS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Runs `f` with the generator writing modules on `threads` threads: for a test that it writes
/// the same on one thread as on several.
pub fn on_threads<T>(threads: usize, f: impl FnOnce() -> T) -> T {
    let before = THREADS.replace(Some(threads));
    let out = f();
    THREADS.set(before);
    out
}

/// How many threads write modules: `JAVA_TO_RUST_GENERATOR_THREADS`, or four (`DEFAULT_THREADS`),
/// or as many as the machine has if fewer.
fn threads() -> usize {
    THREADS.get().unwrap_or_else(|| {
        std::env::var("JAVA_TO_RUST_GENERATOR_THREADS").ok().and_then(|n| n.parse().ok()).unwrap_or_else(
            || std::thread::available_parallelism().map_or(1, |n| n.get()).min(DEFAULT_THREADS),
        )
    })
}

/// The threads that write modules by default: past four, the crate root, written by one thread,
/// is most of a round on the Minecraft server.
const DEFAULT_THREADS: usize = 4;

/// A panic while a module was written, with the module's place among those written, where in
/// the translator it happened, and the method being written then (`WRITING`).
struct Panicked {
    index: usize,
    payload: Box<dyn std::any::Any + Send>,
    place: Option<String>,
    writing: Option<Pos>,
}

/// The modules of `setup.plans`, or those of them at the indexes `only`, written in `round` (with
/// the first round's modules `first` in the last), each with what writing it found, in their
/// order.
pub(super) fn write_modules(
    class: &Class,
    facts: &Facts,
    setup: &Setup,
    round: &Round,
    first: Option<&[modules::Written]>,
    only: Option<&[usize]>,
) -> Vec<(modules::Written, Found)> {
    let all: Vec<usize>;
    let indexes = match only {
        Some(only) => only,
        None => {
            all = (0..setup.plans.len()).collect();
            &all
        }
    };
    let threads = threads().clamp(1, indexes.len().max(1));
    let queue = Queue { indexes, next: AtomicUsize::new(0), stop: AtomicUsize::new(usize::MAX) };
    let write = || write_some(class, facts, setup, round, first, &queue);
    let results: Vec<Result<Written, Panicked>> = if threads == 1 {
        vec![write()]
    } else {
        let (shared, catching) = (shared_strings(), CATCHING.with(|c| c.get()));
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    let thread = std::thread::Builder::new().name("generator".to_string()).stack_size(STACK);
                    let run = || {
                        SHARED_STRINGS.set(shared);
                        catch_panics(catching);
                        let _modules = modules::enter(class);
                        write()
                    };
                    thread.spawn_scoped(scope, run).expect("a thread for the generator")
                })
                .collect();
            let joined = handles.into_iter().map(|handle| handle.join());
            // A panic outside the modules, which no module's writing caught.
            joined
                .map(|done| {
                    done.unwrap_or_else(|payload| {
                        Err(Panicked { index: usize::MAX, payload, place: None, writing: None })
                    })
                })
                .collect()
        })
    };
    let mut modules: Vec<Option<(modules::Written, Found)>> = indexes.iter().map(|_| None).collect();
    let mut panics = Vec::new();
    let mut times = Vec::new();
    for result in results {
        match result {
            Ok(written) => {
                for (at, module, found, time) in written.modules {
                    modules[at] = Some((module, found));
                    times.push((time, indexes[at]));
                }
            }
            Err(panicked) => panics.push(panicked),
        }
    }
    if let Some(Panicked { payload, place, writing, .. }) = panics.into_iter().min_by_key(|p| p.index) {
        WRITING.with(|w| w.set(writing));
        crate::lower::panic_carried(place);
        std::panic::resume_unwind(payload);
    }
    slowest_timing(setup, times);
    modules.into_iter().map(|module| module.expect("every module is written")).collect()
}

/// The `--timings` line of the modules of a round that took longest to write, by package, which
/// shows how much writing them on more threads could gain: a round takes at least as long as
/// its slowest module.
fn slowest_timing(setup: &Setup, mut times: Vec<(std::time::Duration, usize)>) {
    if !crate::timings::enabled() || times.is_empty() {
        return;
    }
    let all: std::time::Duration = times.iter().map(|(time, _)| *time).sum();
    times.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let slowest: Vec<String> = times
        .iter()
        .take(3)
        .map(|&(time, index)| {
            let package = setup.plans[index].package();
            let name = if package.is_empty() { "the crate root" } else { package };
            format!("{name} {:.3} s", time.as_secs_f64())
        })
        .collect();
    eprintln!(
        "timing: codegen: {} modules written in {:.3} s, the slowest {}",
        times.len(),
        all.as_secs_f64(),
        slowest.join(", ")
    );
}

/// What one generator wrote: modules by their place among those written, each with what writing
/// it found.
struct Written {
    modules: Vec<(usize, modules::Written, Found, std::time::Duration)>,
}

/// The modules that the generators of a round write, by index (`indexes`): the place among them of
/// the next one that no generator took (`next`), and the earliest where one panicked (`stop`).
struct Queue<'a> {
    indexes: &'a [usize],
    next: AtomicUsize,
    stop: AtomicUsize,
}

/// Writes modules of `queue` not yet taken with a generator of this thread, until none is left or
/// another thread's generator panicked on an earlier module.
fn write_some(
    class: &Class,
    facts: &Facts,
    setup: &Setup,
    round: &Round,
    first: Option<&[modules::Written]>,
    queue: &Queue,
) -> Result<Written, Panicked> {
    let Queue { indexes, next, stop } = queue;
    let mut g = Gen::new(class, facts, setup, round);
    let mut written = Vec::new();
    loop {
        let at = next.fetch_add(1, Ordering::Relaxed);
        if at >= indexes.len() || at > stop.load(Ordering::Relaxed) {
            break;
        }
        let index = indexes[at];
        let kept = first.map(|first| &first[index]);
        let start = std::time::Instant::now();
        match std::panic::catch_unwind(AssertUnwindSafe(|| g.module(&setup.plans[index], &setup.live, kept)))
        {
            // What writing a module found is the module's own: nothing that the generator found
            // before is read while it writes (`Found`).
            Ok(module) => written.push((at, module, g.take_found(), start.elapsed())),
            Err(payload) => {
                stop.fetch_min(at, Ordering::Relaxed);
                let place = crate::lower::panic_place();
                let writing = WRITING.with(|w| w.get());
                return Err(Panicked { index: at, payload, place, writing });
            }
        }
    }
    Ok(Written { modules: written })
}
