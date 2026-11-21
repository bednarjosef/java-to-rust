//! A thread's closure between `pthread_create` and the thread's registration: it is then only in
//! memory of the C library, which collections do not read, and an entry of `gc::STARTING` keeps
//! it. The scenario runs in a child process of this test with `JAVA_GC_START_DELAY`, which holds
//! each starting thread before it registers, so that the collections here run in that time.

use java_runtime as java;
use std::hint::black_box;
use std::thread::JoinHandle;

#[global_allocator]
static ALLOCATOR: java::Allocator = java::Allocator;

struct Payload {
    value: u64,
    _padding: [u64; 3],
}

struct Reference {
    referent: java::WeakField<Option<java::Arc<Payload>>>,
}

const THREADS: usize = 16;

/// How long a starting thread is held, in milliseconds: long against the few milliseconds the
/// collections here take.
const DELAY: &str = "1000";

/// Starts a thread whose closure holds the only handle of a payload, and returns it with a weak
/// reference to the payload, which a collection clears if it does not find the closure.
#[inline(never)]
fn start(value: u64) -> (JoinHandle<u64>, java::Arc<Reference>) {
    let payload = java::Arc::new(Payload { value, _padding: [0; 3] });
    let reference = java::Arc::new(Reference { referent: java::WeakField::new(Some(payload.clone())) });
    java::register_reference(&reference, &reference.referent, 2, false);
    (std::thread::spawn(move || payload.value), reference)
}

/// Takes a new span in every small class of slots that may hold pointers. A span that handed
/// slots out to Rust code keeps them all until the thread takes the next one, the closures of
/// the threads started among them, and a span of objects keeps the last it handed out.
#[inline(never)]
fn take_new_spans() {
    for words in 1..=128usize {
        for _ in 0..(32 << 10) / (words * 8) + 1 {
            std::mem::forget(black_box(Vec::<usize>::with_capacity(words)));
        }
    }
    for value in 0..1024 {
        black_box(java::Arc::new(Payload { value, _padding: [0; 3] }));
    }
}

/// Overwrites the stack below the caller's frame, where copies of the closures' addresses and
/// the payloads' handles may be.
#[inline(never)]
fn scrub() {
    black_box([0u64; 4096]);
}

/// Starts the threads and collects while they are held; whether they were still held when the
/// collections were done, so that these ran before any of them registered.
fn collect_while_threads_start() -> bool {
    let started: Vec<(JoinHandle<u64>, java::Arc<Reference>)> = (0..THREADS as u64).map(start).collect();
    take_new_spans();
    scrub();
    for _ in 0..3 {
        java::gc::collect_now();
    }
    let held = started.iter().all(|(thread, _)| !thread.is_finished());
    let cleared = started.iter().filter(|(_, reference)| reference.referent.get().is_none()).count();
    // Before the threads run: a closure that was collected is not run.
    assert_eq!(cleared, 0, "the closures of {cleared} of {THREADS} starting threads were collected");
    for (value, (thread, _)) in started.into_iter().enumerate() {
        assert_eq!(thread.join().unwrap(), value as u64);
    }
    held
}

fn scenario() {
    let _main = java::main_thread();
    // On a machine so loaded that the threads registered before the collections were done, the
    // collections showed nothing: again, a few times.
    for _ in 0..5 {
        if collect_while_threads_start() {
            return;
        }
    }
    panic!("the threads registered before the collections ran, five times");
}

#[test]
fn child() {
    if std::env::var("THREAD_STARTS_SCENARIO").is_ok() {
        scenario();
    }
}

#[test]
fn the_closure_of_a_starting_thread_lives_through_collections() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
        .env("THREAD_STARTS_SCENARIO", "1")
        .env("JAVA_GC_START_DELAY", DELAY)
        .status()
        .unwrap();
    assert!(status.success(), "the scenario failed");
}
