//! The modules of a round written on several threads. Each thread has a generator of its own,
//! with its own resolver (`modules::enter`) and caches, and takes the next module not yet taken,
//! in the order they are written, until none is left; what it writes of a module depends on the
//! module alone (`Gen::module`), so the modules are the same whichever thread writes them, and
//! they are put back in their order. What the generators found besides the text (`Found`) is
//! the same as one generator would find, as each part of it only grows. A panic outside the
//! methods, which a census does not catch (`catch_panics`), comes out of the first module in
//! their order that panicked, with its place in the translator, as it would with one thread.
//!
//! The generator writes on one thread unless `JAVA_TO_RUST_GENERATOR_THREADS` asks for more:
//! the translator allocates from one arena of glibc's (`main.rs`), and threads that allocate as
//! much as the generator does wait for each other's turns at it (see "Generator threads" in
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

/// How many threads write modules: `JAVA_TO_RUST_GENERATOR_THREADS`, or one.
fn threads() -> usize {
    THREADS.get().unwrap_or_else(|| {
        std::env::var("JAVA_TO_RUST_GENERATOR_THREADS").ok().and_then(|n| n.parse().ok()).unwrap_or(1)
    })
}

/// A panic while a module was written, with the module's index, where in the translator it
/// happened, and the method being written then (`WRITING`).
struct Panicked {
    index: usize,
    payload: Box<dyn std::any::Any + Send>,
    place: Option<String>,
    writing: Option<Pos>,
}

/// The modules of `setup.plans`, written in `round` (with the first round's modules `first` in
/// the last), and what writing them found.
pub(super) fn write_modules(
    class: &Class,
    facts: &Facts,
    setup: &Setup,
    round: &Round,
    first: Option<&[modules::Written]>,
) -> (Vec<modules::Written>, Found) {
    let threads = threads().clamp(1, setup.plans.len().max(1));
    let next = AtomicUsize::new(0);
    let stop = AtomicUsize::new(usize::MAX);
    let write = || write_some(class, facts, setup, round, first, &next, &stop);
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
    let mut modules: Vec<Option<modules::Written>> = setup.plans.iter().map(|_| None).collect();
    let mut found = Found::default();
    let mut panics = Vec::new();
    for result in results {
        match result {
            Ok(written) => {
                for (index, module) in written.modules {
                    modules[index] = Some(module);
                }
                found.add(written.found);
            }
            Err(panicked) => panics.push(panicked),
        }
    }
    if let Some(Panicked { payload, place, writing, .. }) = panics.into_iter().min_by_key(|p| p.index) {
        WRITING.with(|w| w.set(writing));
        crate::lower::panic_carried(place);
        std::panic::resume_unwind(payload);
    }
    (modules.into_iter().map(|module| module.expect("every module is written")).collect(), found)
}

/// What one generator wrote: modules by their index, and what it found.
struct Written {
    modules: Vec<(usize, modules::Written)>,
    found: Found,
}

/// Writes modules not yet taken (`next`) with a generator of this thread, until none is left or
/// another thread's generator panicked on an earlier module (`stop`).
fn write_some(
    class: &Class,
    facts: &Facts,
    setup: &Setup,
    round: &Round,
    first: Option<&[modules::Written]>,
    next: &AtomicUsize,
    stop: &AtomicUsize,
) -> Result<Written, Panicked> {
    let mut g = Gen::new(class, facts, setup, round);
    let mut written = Vec::new();
    loop {
        let index = next.fetch_add(1, Ordering::Relaxed);
        if index >= setup.plans.len() || index > stop.load(Ordering::Relaxed) {
            break;
        }
        let kept = first.map(|first| &first[index]);
        match std::panic::catch_unwind(AssertUnwindSafe(|| g.module(&setup.plans[index], &setup.live, kept)))
        {
            Ok(module) => written.push((index, module)),
            Err(payload) => {
                stop.fetch_min(index, Ordering::Relaxed);
                let place = crate::lower::panic_place();
                let writing = WRITING.with(|w| w.get());
                return Err(Panicked { index, payload, place, writing });
            }
        }
    }
    Ok(Written { modules: written, found: g.take_found() })
}
