//! The natives of `java.lang.Thread`, which the translator's own `Thread` class calls (see
//! `java/Threads.java` in the translator): starting a thread, joining it, its name, its id, and
//! the thread object that `Thread.currentThread()` returns.
//!
//! Each started thread has a record, kept by its id in a registry while it runs, that holds what
//! the JVM keeps for it: its name (for the message of an exception that ends it), whether it is
//! alive, and the permit of `park`/`unpark`. The program's `Thread` object holds the id, so the
//! translation's `Thread` is an ordinary class of the program and the record needs no type of it.
//!
//! The JVM ends when the last thread that is not a daemon ends. So `main` holds a `MainThread`
//! (`let _main = java::main_thread();`), which on its way out waits for the other non-daemon
//! threads, then writes out what is left of the output; a daemon thread still running then ends
//! with the process.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The id of the main thread: the JVM's, whose own threads come first.
const MAIN_ID: i64 = 3;

/// Whether a thread has been started: from then on the output buffer is the process's (see
/// `out.rs`).
static STARTED: AtomicBool = AtomicBool::new(false);

pub(crate) fn threads_started() -> bool {
    STARTED.load(Ordering::Acquire)
}

/// The non-daemon threads that have been started and have not ended.
static LIVE: Mutex<usize> = Mutex::new(0);
static ALL_ENDED: Condvar = Condvar::new();

static NEXT_NUMBER: AtomicI32 = AtomicI32::new(0);
static NEXT_ID: AtomicI64 = AtomicI64::new(MAIN_ID + 1);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the runtime keeps for a thread.
pub(crate) struct Record {
    name: Mutex<String>,
    alive: Mutex<bool>,
    ended: Condvar,
    /// The permit of `park` and `unpark`.
    permit: Mutex<bool>,
    unparked: Condvar,
}

impl Record {
    fn new(name: String, alive: bool) -> Arc<Record> {
        Arc::new(Record {
            name: Mutex::new(name),
            alive: Mutex::new(alive),
            ended: Condvar::new(),
            permit: Mutex::new(false),
            unparked: Condvar::new(),
        })
    }

    pub(crate) fn name(&self) -> String {
        lock(&self.name).clone()
    }
}

/// The records of the threads that run, by id.
static RECORDS: LazyLock<Mutex<HashMap<i64, Arc<Record>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn record(tid: i64) -> Option<Arc<Record>> {
    lock(&RECORDS).get(&tid).cloned()
}

thread_local! {
    /// The record of the thread; `None` on the main thread until it needs one.
    static RECORD: RefCell<Option<Arc<Record>>> = const { RefCell::new(None) };
    /// The program's `Thread` object of the thread, once `currentThread()` or the start of the
    /// thread has made it.
    static CURRENT: RefCell<Option<Box<dyn Any>>> = const { RefCell::new(None) };
    /// Whether the thread was started by the program, rather than being the main thread.
    static WORKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The record of the calling thread, made for the main thread on first use.
pub(crate) fn current_record() -> Arc<Record> {
    RECORD.with_borrow_mut(|r| r.get_or_insert_with(main_record).clone())
}

/// Whether the calling thread is one that the program started.
pub(crate) fn on_worker() -> bool {
    WORKER.try_with(|w| w.get()).unwrap_or(false)
}

/// The number of the next thread created without a name: `Thread-0`, `Thread-1`, ...
pub fn next_thread_number() -> i32 {
    NEXT_NUMBER.fetch_add(1, Ordering::Relaxed)
}

/// The id of the next thread created.
pub fn next_thread_id() -> i64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// The id of the main thread, which `Thread`'s constructor for it takes.
pub fn main_thread_id() -> i64 {
    MAIN_ID
}

/// `Thread.start()`: runs `body` on a new thread, which the record of id `tid` describes. The
/// first start makes the output buffer the process's.
pub fn start_thread(tid: i64, name: String, daemon: bool, body: impl FnOnce() + Send + 'static) {
    if !STARTED.swap(true, Ordering::AcqRel) {
        crate::out::share_output();
    }
    let record = Record::new(name, true);
    lock(&RECORDS).insert(tid, record.clone());
    if !daemon {
        *lock(&LIVE) += 1;
    }
    let spawned = std::thread::Builder::new().spawn(move || {
        WORKER.with(|w| w.set(true));
        RECORD.with_borrow_mut(|r| *r = Some(record.clone()));
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(body));
        CURRENT.with_borrow_mut(|c| *c = None);
        crate::release_thread_cache();
        if let Err(payload) = outcome
            && !payload.is::<Uncaught>()
        {
            // A panic is a bug in the translation, whose message the hook has printed: the
            // program ends, as it would on the main thread.
            crate::flush();
            std::process::exit(101);
        }
        lock(&RECORDS).remove(&tid);
        *lock(&record.alive) = false;
        record.ended.notify_all();
        if !daemon {
            let mut live = lock(&LIVE);
            *live -= 1;
            if *live == 0 {
                ALL_ENDED.notify_all();
            }
        }
    });
    if let Err(e) = spawned {
        crate::throw("java.lang.OutOfMemoryError", &format!("unable to create native thread: {e}"));
    }
}

/// The payload with which a thread that an exception ends unwinds to its start, once the message
/// is printed.
struct Uncaught;

/// Ends the calling thread, started by the program, because of an exception that nothing caught:
/// the message is printed, and the other threads go on.
pub(crate) fn end_worker(e: impl std::fmt::Display) -> ! {
    let name = current_record().name();
    crate::flush();
    eprintln!("Exception in thread \"{name}\" {e}");
    std::panic::resume_unwind(Box::new(Uncaught))
}

/// `isAlive()` of the thread with id `tid`.
pub fn thread_alive(tid: i64) -> bool {
    record(tid).is_some_and(|r| *lock(&r.alive))
}

/// `join(millis)` of the thread with id `tid`: waits until it ends, or for at most `millis`
/// milliseconds when that is not 0.
pub fn join_thread(tid: i64, millis: i64) {
    let Some(record) = record(tid) else { return };
    let deadline = (millis > 0).then(|| Instant::now() + Duration::from_millis(millis as u64));
    let mut alive = lock(&record.alive);
    while *alive {
        match deadline {
            None => alive = record.ended.wait(alive).unwrap_or_else(|e| e.into_inner()),
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    return;
                }
                alive = record.ended.wait_timeout(alive, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
            }
        }
    }
}

/// `setName(name)` of a running thread with id `tid`: the name its uncaught exception shows.
pub fn rename_thread(tid: i64, name: &str) {
    let record = if tid == MAIN_ID { Some(current_record()) } else { record(tid) };
    if let Some(record) = record {
        *lock(&record.name) = name.to_string();
    }
}

/// Whether the calling thread has its `Thread` object yet: the main thread gets one when it first
/// asks `Thread.currentThread()`.
pub fn has_current_thread() -> bool {
    CURRENT.with_borrow(Option::is_some)
}

/// Makes `thread` the `Thread` object of the calling thread.
pub fn set_current_thread<T: 'static>(thread: T) {
    CURRENT.with_borrow_mut(|c| *c = Some(Box::new(thread)));
}

/// `Thread.currentThread()`, once `set_current_thread` has given the thread its object.
pub fn current_thread<T: Clone + 'static>() -> T {
    CURRENT.with_borrow(|c| {
        c.as_ref().and_then(|t| t.downcast_ref::<T>()).cloned().expect("the thread has its `Thread` object")
    })
}

/// `LockSupport.park()`: waits until the calling thread's permit is given, and takes it.
pub fn park(timeout: Option<Duration>) {
    let record = current_record();
    let deadline = timeout.map(|t| Instant::now() + t);
    let mut permit = lock(&record.permit);
    while !*permit {
        match deadline {
            None => permit = record.unparked.wait(permit).unwrap_or_else(|e| e.into_inner()),
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                permit =
                    record.unparked.wait_timeout(permit, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
            }
        }
    }
    *permit = false;
}

/// `LockSupport.unpark(thread)`: gives the thread with id `tid` its permit.
pub fn unpark(tid: i64) {
    let record = if tid == MAIN_ID { Some(main_record()) } else { record(tid) };
    if let Some(record) = record {
        *lock(&record.permit) = true;
        record.unparked.notify_all();
    }
}

/// The main thread's record, from any thread.
fn main_record() -> Arc<Record> {
    let found = record(MAIN_ID);
    found.unwrap_or_else(|| {
        let mut records = lock(&RECORDS);
        records.entry(MAIN_ID).or_insert_with(|| Record::new("main".to_string(), true)).clone()
    })
}

/// Held by `main` while it runs: when `main` returns, the program waits for its other non-daemon
/// threads, as the JVM does, then writes out its output.
pub struct MainThread;

/// What `main` holds while it runs (see `MainThread`).
pub fn main_thread() -> MainThread {
    MainThread
}

impl Drop for MainThread {
    fn drop(&mut self) {
        // A panic is a translation bug: the program ends with it.
        if !std::thread::panicking() {
            await_threads();
        }
        crate::flush();
    }
}

/// Waits until every non-daemon thread the program started has ended.
pub(crate) fn await_threads() {
    if !threads_started() {
        return;
    }
    crate::flush();
    let mut live = lock(&LIVE);
    while *live > 0 {
        live = ALL_ENDED.wait(live).unwrap_or_else(|e| e.into_inner());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_started_thread_runs_and_is_joined() {
        let tid = next_thread_id();
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        start_thread(tid, "worker".into(), false, move || {
            std::thread::sleep(Duration::from_millis(20));
            flag.store(true, Ordering::SeqCst);
        });
        assert!(thread_alive(tid));
        join_thread(tid, 0);
        assert!(done.load(Ordering::SeqCst));
        assert!(!thread_alive(tid));
    }

    #[test]
    fn a_timed_join_returns_while_the_thread_runs() {
        let tid = next_thread_id();
        start_thread(tid, "sleeper".into(), true, || std::thread::sleep(Duration::from_millis(300)));
        join_thread(tid, 10);
        assert!(thread_alive(tid));
        join_thread(tid, 0);
    }

    #[test]
    fn unpark_before_park_is_kept() {
        let tid = next_thread_id();
        let (tx, rx) = std::sync::mpsc::channel();
        start_thread(tid, "parker".into(), false, move || {
            rx.recv().unwrap();
            park(None);
        });
        unpark(tid);
        tx.send(()).unwrap();
        join_thread(tid, 0);
    }

    #[test]
    fn the_current_thread_object_is_per_thread() {
        set_current_thread(7i32);
        assert!(has_current_thread());
        let tid = next_thread_id();
        let (tx, rx) = std::sync::mpsc::channel();
        start_thread(tid, "other".into(), false, move || {
            tx.send(has_current_thread()).unwrap();
        });
        assert!(!rx.recv().unwrap());
        assert_eq!(current_thread::<i32>(), 7);
        join_thread(tid, 0);
    }
}
