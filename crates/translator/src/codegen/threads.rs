//! The modules of a round written on several threads. Each thread has a generator of its own,
//! with its own resolver (`modules::enter_names`, over the program's names found once) and caches, and takes the next module not yet taken,
//! in the order they are written, until none is left; what it writes of a module depends on the
//! module alone (`Gen::module`), so the modules are the same whichever thread writes them, and
//! they are put back in their order. A module of many free functions (the crate root, with the
//! reflection table's data) is taken in parts of its functions (`Plan::parts`), and a part
//! depends on its functions alone but for the blank line before its first, which joining the
//! parts puts back (`modules::join_parts`). What the generators found besides the text (`Found`) is
//! the same as one generator would find, as each part of it only grows. A panic outside the
//! methods, which a census does not catch (`catch_panics`), comes out of the first module in
//! their order that panicked, with its place in the translator, as it would with one thread.
//!
//! The generator writes on eight threads, or as many as the machine has if fewer, unless
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

/// How many threads write modules: `JAVA_TO_RUST_GENERATOR_THREADS`, or eight (`DEFAULT_THREADS`),
/// or as many as the machine has if fewer.
fn threads() -> usize {
    THREADS.get().unwrap_or_else(|| {
        std::env::var("JAVA_TO_RUST_GENERATOR_THREADS").ok().and_then(|n| n.parse().ok()).unwrap_or_else(
            || std::thread::available_parallelism().map_or(1, |n| n.get()).min(DEFAULT_THREADS),
        )
    })
}

/// The threads that write modules by default: on the Minecraft server's census, eight write its
/// first round in 6.3 s and four in 8.5 s, at the same peak.
const DEFAULT_THREADS: usize = 8;

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
    // On several threads, a module of many functions is written in parts (`Plan::parts`), so that
    // the crate root is not one thread's work while the others wait.
    let cut = threads() > 1 && round.only_enums.is_none() && first.is_none();
    let units: Vec<(usize, std::ops::Range<usize>)> = indexes
        .iter()
        .flat_map(|&index| setup.plans[index].parts(cut).into_iter().map(move |part| (index, part)))
        .collect();
    let threads = threads().clamp(1, units.len().max(1));
    let queue = Queue { units: &units, next: AtomicUsize::new(0), stop: AtomicUsize::new(usize::MAX) };
    let write = || write_some(class, facts, setup, round, first, &queue);
    let results: Vec<Result<Written, Panicked>> = if threads == 1 {
        vec![write()]
    } else {
        let (shared, catching) = (shared_strings(), CATCHING.with(|c| c.get()));
        let names = modules::entered();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    let thread = std::thread::Builder::new().name("generator".to_string()).stack_size(STACK);
                    let run = || {
                        SHARED_STRINGS.set(shared);
                        catch_panics(catching);
                        let _modules = modules::enter_names(&names);
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
    let mut parts: Vec<Option<(modules::Written, Found, std::time::Duration)>> =
        units.iter().map(|_| None).collect();
    let mut panics = Vec::new();
    for result in results {
        match result {
            Ok(written) => {
                for (at, module, found, time) in written.modules {
                    parts[at] = Some((module, found, time));
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
    // The parts of each module, which are next to each other in `units`, joined.
    let mut modules = Vec::with_capacity(indexes.len());
    let mut times = Vec::with_capacity(indexes.len());
    let mut parts = units.iter().zip(parts).peekable();
    while let Some((&(index, _), part)) = parts.next() {
        let (module, found, time) = part.expect("every part is written");
        let (mut written, mut founds, mut spent) = (vec![module], vec![found], time);
        while let Some((_, part)) = parts.next_if(|((next, _), _)| *next == index) {
            let (module, found, time) = part.expect("every part is written");
            written.push(module);
            founds.push(found);
            spent += time;
        }
        let found = if founds.len() == 1 { founds.pop().expect("a part") } else { Found::all(&founds) };
        modules.push((modules::join_parts(class, written), found));
        times.push((spent, index));
    }
    slowest_timing(setup, times);
    modules
}

/// `f` of each of `items`, in their order, on the generator's threads (`threads`), each with
/// the program's resolver (`modules::enter_names`) as a thread that writes modules has.
pub(super) fn on_each<T: Send, U: Send>(items: Vec<T>, f: impl Fn(T) -> U + Sync) -> Vec<U> {
    let threads = threads().clamp(1, items.len().max(1));
    if threads == 1 {
        return items.into_iter().map(f).collect();
    }
    let slots: Vec<std::sync::Mutex<Option<T>>> =
        items.into_iter().map(|item| std::sync::Mutex::new(Some(item))).collect();
    let next = AtomicUsize::new(0);
    let (shared, names) = (shared_strings(), modules::entered());
    let done: Vec<Vec<(usize, U)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let thread = std::thread::Builder::new().name("generator".to_string()).stack_size(STACK);
                let run = || {
                    SHARED_STRINGS.set(shared);
                    let _modules = modules::enter_names(&names);
                    let mut out = Vec::new();
                    loop {
                        let at = next.fetch_add(1, Ordering::Relaxed);
                        let Some(slot) = slots.get(at) else { break };
                        let item = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
                        out.push((at, f(item.expect("each item is taken once"))));
                    }
                    out
                };
                thread.spawn_scoped(scope, run).expect("a thread for the generator")
            })
            .collect();
        let joined = handles.into_iter().map(|handle| handle.join());
        joined.map(|done| done.unwrap_or_else(|payload| std::panic::resume_unwind(payload))).collect()
    });
    let mut out: Vec<Option<U>> = slots.iter().map(|_| None).collect();
    for (at, u) in done.into_iter().flatten() {
        out[at] = Some(u);
    }
    out.into_iter().map(|u| u.expect("each item is done")).collect()
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

/// The modules that the generators of a round write, by index and the range of its methods that
/// the part holds (`units`): the place among them of the next one that no generator took
/// (`next`), and the earliest where one panicked (`stop`).
struct Queue<'a> {
    units: &'a [(usize, std::ops::Range<usize>)],
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
    let Queue { units, next, stop } = queue;
    let mut g = Gen::new(class, facts, setup, round);
    let mut written = Vec::new();
    loop {
        let at = next.fetch_add(1, Ordering::Relaxed);
        if at >= units.len() || at > stop.load(Ordering::Relaxed) {
            break;
        }
        let (index, ref part) = units[at];
        let kept = first.map(|first| &first[index]);
        let start = std::time::Instant::now();
        let write = || g.module(&setup.plans[index], &setup.live, kept, part.clone());
        match std::panic::catch_unwind(AssertUnwindSafe(write)) {
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
