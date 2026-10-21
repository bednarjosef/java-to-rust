//! The natives of `java.lang.Thread`, which the translator's own `Thread` class calls (see
//! `java/Threads.java` in the translator): starting a thread, joining it, its name, its id, and
//! the thread object that `Thread.currentThread()` returns.
//!
//! Each started thread has a record, kept by its id in a registry while it runs, that holds what
//! the JVM keeps for it: its name (for the message of an exception that ends it), whether it is
//! alive, the permit of `park`/`unpark`, and its interrupt status with where it waits, so that
//! `interrupt` can wake it from `sleep`, `join` or `wait`. The program's `Thread` object holds the id, so the
//! translation's `Thread` is an ordinary class of the program and the record needs no type of it.
//!
//! The JVM ends when the last thread that is not a daemon ends. So `main` holds a `MainThread`
//! (`let _main = java::main_thread();`), which on its way out waits for the other non-daemon
//! threads, then writes out what is left of the output; a daemon thread still running then ends
//! with the process.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicUsize, Ordering};
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
    /// The permit of `park` and `unpark`. `sleep` waits on its condition variable too, which an
    /// interrupt notifies.
    permit: Mutex<bool>,
    unparked: Condvar,
    /// The interrupt status, set by `interrupt` and cleared by what observes it.
    interrupted: AtomicBool,
    /// The parking bucket of the monitor the thread waits on in `wait`, plus one (0 when none).
    bucket: AtomicUsize,
    /// The thread that this one waits for in `join`.
    joining: Mutex<Option<Arc<Record>>>,
}

impl Record {
    fn new(name: String, alive: bool) -> Arc<Record> {
        Arc::new(Record {
            name: Mutex::new(name),
            alive: Mutex::new(alive),
            ended: Condvar::new(),
            permit: Mutex::new(false),
            unparked: Condvar::new(),
            interrupted: AtomicBool::new(false),
            bucket: AtomicUsize::new(0),
            joining: Mutex::new(None),
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

/// The threads, by id, that are interrupted and have no record: interrupted before they started,
/// or ended with the status set, which Java keeps in the `Thread` object. Changed only while
/// `RECORDS` is locked, so a thread's status moves between its record and this set atomically.
static IDLE_INTERRUPTED: LazyLock<Mutex<HashSet<i64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Whether the program has ever interrupted a thread; until it has, no status needs reading. (What
/// waits still registers where an interrupt would wake it, since one may come during the wait.)
static INTERRUPTS: AtomicBool = AtomicBool::new(false);

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
    {
        let mut records = lock(&RECORDS);
        // An interrupt before the start is the status the thread starts with.
        if lock(&IDLE_INTERRUPTED).remove(&tid) {
            record.interrupted.store(true, Ordering::SeqCst);
        }
        records.insert(tid, record.clone());
    }
    if !daemon {
        *lock(&LIVE) += 1;
    }
    let spawned = std::thread::Builder::new().spawn(move || {
        WORKER.with(|w| w.set(true));
        RECORD.with_borrow_mut(|r| *r = Some(record.clone()));
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(body));
        CURRENT.with_borrow_mut(|c| *c = None);
        if let Err(payload) = outcome
            && !payload.is::<Uncaught>()
        {
            // A panic is a bug in the translation, whose message the hook has printed: the
            // program ends, as it would on the main thread.
            crate::flush();
            std::process::exit(101);
        }
        {
            let mut records = lock(&RECORDS);
            records.remove(&tid);
            // The `Thread` object keeps the status after the thread ends.
            if record.interrupted.load(Ordering::SeqCst) {
                lock(&IDLE_INTERRUPTED).insert(tid);
            }
        }
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

/// What the translation of a program that sets an uncaught-exception handler gives the runtime
/// (`Thread.installUncaught0()`): a function that dispatches an exception that nothing caught to
/// the handlers, on the thread it ends (`false` when the value is not one it can make an object
/// of), and one that names the class of an exception.
type Dispatch = fn(&dyn Any) -> bool;
type ClassName = fn(&dyn Any) -> Option<String>;

static UNCAUGHT_HOOK: Mutex<Option<(Dispatch, ClassName)>> = Mutex::new(None);

thread_local! {
    /// Whether the thread runs its uncaught-exception handler.
    static DISPATCHING: Cell<bool> = const { Cell::new(false) };
    /// The exception that the handler is given, as `uncaught0()` returns it.
    static PENDING: RefCell<Option<Box<dyn Any>>> = const { RefCell::new(None) };
}

/// `Thread.installUncaught0()`: from now on, an exception that nothing catches goes to the
/// handlers.
pub fn set_uncaught_hook(dispatch: Dispatch, class_name: ClassName) {
    *lock(&UNCAUGHT_HOOK) = Some((dispatch, class_name));
}

/// Makes `e` the exception that `uncaught0()` returns on this thread.
pub fn set_pending_uncaught<T: 'static>(e: T) {
    PENDING.with_borrow_mut(|p| *p = Some(Box::new(e)));
}

/// `uncaught0()`: the exception that ends the calling thread, which its handler is given.
pub fn pending_uncaught<T: Clone + 'static>() -> T {
    PENDING.with_borrow(|p| p.as_ref().and_then(|e| e.downcast_ref::<T>()).cloned())
        .expect("the runtime sets the exception before it runs the handler")
}

/// `printUncaught0(name, e)`: what the thread group prints for an exception without a handler.
pub fn print_uncaught(name: &str, e: &impl std::fmt::Display) {
    crate::flush();
    eprintln!("Exception in thread \"{name}\" {e}");
}

/// Hands an exception that nothing caught to the program's handlers, once it has set one; `true`
/// when they had it, and the thread is to end without printing it. An exception that the
/// handler itself lets escape ends the thread with the JVM's message naming its class.
pub(crate) fn dispatch_uncaught<E: std::fmt::Display + 'static>(e: &E) -> bool {
    let Some((dispatch, class_name)) = *lock(&UNCAUGHT_HOOK) else { return false };
    if DISPATCHING.get() {
        let any = e as &dyn Any;
        let class = class_name(any)
            .or_else(|| any.downcast_ref::<crate::Exception>().map(|x| x.class.to_string()))
            .unwrap_or_else(|| e.to_string());
        let name = current_record().name();
        crate::flush();
        eprintln!("\nException: {class} thrown from the UncaughtExceptionHandler in thread \"{name}\"");
        return true;
    }
    DISPATCHING.set(true);
    let handled = dispatch(e as &dyn Any);
    DISPATCHING.set(false);
    handled
}

/// Ends the calling thread once the handlers have had its exception: a thread the program started
/// ends, and the main thread ends the program with 1 once the other threads have ended.
pub(crate) fn end_dispatched() -> ! {
    crate::flush();
    if on_worker() {
        std::panic::resume_unwind(Box::new(Uncaught))
    }
    await_threads();
    crate::flush();
    std::process::exit(1)
}

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
    if let Err(e) = try_join_thread(tid, millis) {
        crate::uncaught::<()>(e);
    }
}

const INTERRUPTED: &str = "java.lang.InterruptedException";

/// `join(millis)`, where the program observes its `InterruptedException`: an interrupt of the
/// calling thread, before or while it waits for a thread that is alive, ends the wait with it
/// (Java's `join` is a `wait` on the `Thread` object while the thread is alive).
pub fn try_join_thread(tid: i64, millis: i64) -> Result<(), crate::Exception> {
    let Some(record) = record(tid) else { return Ok(()) };
    // Registered whether or not the program has interrupted a thread yet: an interrupt that comes
    // while this thread waits must find where to wake it.
    let me = current_record();
    *lock(&me.joining) = Some(record.clone());
    let deadline = (millis > 0).then(|| Instant::now() + Duration::from_millis(millis as u64));
    let mut alive = lock(&record.alive);
    let mut outcome = Ok(());
    while *alive {
        // `interrupt` sets the status, then notifies under this mutex: no wakeup is lost.
        if me.interrupted.swap(false, Ordering::SeqCst) {
            outcome = Err(crate::Exception::bare(INTERRUPTED));
            break;
        }
        match deadline {
            None => alive = record.ended.wait(alive).unwrap_or_else(|e| e.into_inner()),
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                alive = record.ended.wait_timeout(alive, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
            }
        }
    }
    drop(alive);
    *lock(&me.joining) = None;
    outcome
}

/// Whether some thread has been interrupted (see `INTERRUPTS`).
fn interrupts_used() -> bool {
    INTERRUPTS.load(Ordering::SeqCst)
}

/// `Thread.sleep(millis)`, where the program observes its exceptions: a negative time, or an
/// interrupt of the calling thread before or while it sleeps.
pub fn try_sleep(millis: i64) -> Result<(), crate::Exception> {
    if millis < 0 {
        return Err(crate::Exception::new("java.lang.IllegalArgumentException", "timeout value is negative"));
    }
    sleep_for(Duration::from_millis(millis as u64))
}

/// `Thread.sleep(millis, nanos)`, where the program observes its exceptions.
pub fn try_sleep_nanos(millis: i64, nanos: i32) -> Result<(), crate::Exception> {
    if millis < 0 {
        return Err(crate::Exception::new("java.lang.IllegalArgumentException", "timeout value is negative"));
    }
    if !(0..=999_999).contains(&nanos) {
        return Err(crate::Exception::new(
            "java.lang.IllegalArgumentException",
            "nanosecond timeout value out of range",
        ));
    }
    sleep_for(Duration::new(millis as u64, nanos as u32))
}

/// Always on the record, even before the program's first interrupt: one that comes while the
/// thread sleeps must wake it.
fn sleep_for(time: Duration) -> Result<(), crate::Exception> {
    if interruptible_sleep(time) {
        return Err(crate::Exception::new(INTERRUPTED, "sleep interrupted"));
    }
    Ok(())
}

/// Sleeps for `time` on the record's condition variable, which an interrupt notifies; `true`,
/// with the status cleared, when an interrupt ended it.
pub(crate) fn interruptible_sleep(time: Duration) -> bool {
    let me = current_record();
    let deadline = Instant::now().checked_add(time);
    let mut permit = lock(&me.permit);
    loop {
        if me.interrupted.swap(false, Ordering::SeqCst) {
            return true;
        }
        let now = Instant::now();
        let left = match deadline {
            Some(d) if now >= d => return false,
            Some(d) => d - now,
            None => Duration::from_secs(3600),
        };
        permit = me.unparked.wait_timeout(permit, left).unwrap_or_else(|e| e.into_inner()).0;
    }
}

/// `t.interrupt()` of the thread with id `tid`: sets its interrupt status and wakes it from
/// `sleep`, `join`, `wait` or `park`. A thread without a record keeps the status for its start,
/// or for `isInterrupted` after its end.
pub fn interrupt_thread(tid: i64) {
    INTERRUPTS.store(true, Ordering::SeqCst);
    let record = {
        let mut records = lock(&RECORDS);
        if tid == MAIN_ID {
            records.entry(MAIN_ID).or_insert_with(|| Record::new("main".to_string(), true)).clone()
        } else {
            match records.get(&tid) {
                Some(r) => r.clone(),
                None => {
                    lock(&IDLE_INTERRUPTED).insert(tid);
                    return;
                }
            }
        }
    };
    {
        let _permit = lock(&record.permit);
        record.interrupted.store(true, Ordering::SeqCst);
        record.unparked.notify_all();
    }
    let bucket = record.bucket.load(Ordering::SeqCst);
    if bucket > 0 {
        crate::locked::wake_bucket(bucket - 1);
    }
    let joining = lock(&record.joining).clone();
    if let Some(target) = joining {
        let _alive = lock(&target.alive);
        target.ended.notify_all();
    }
}

/// `Thread.interrupted()`: the calling thread's interrupt status, which it clears.
pub fn thread_interrupted() -> bool {
    take_interrupt()
}

/// `t.isInterrupted()` of the thread with id `tid`.
pub fn is_interrupted(tid: i64) -> bool {
    if !interrupts_used() {
        return false;
    }
    let records = lock(&RECORDS);
    match records.get(&tid) {
        Some(r) => r.interrupted.load(Ordering::SeqCst),
        None => lock(&IDLE_INTERRUPTED).contains(&tid),
    }
}

/// Whether the calling thread is interrupted, which then clears its status.
pub(crate) fn take_interrupt() -> bool {
    interrupts_used() && current_record().interrupted.swap(false, Ordering::SeqCst)
}

/// Records the parking bucket where the calling thread is about to wait in `wait` (`None` once
/// it no longer waits), so that an interrupt wakes it.
pub(crate) fn waiting_in(bucket: Option<usize>) {
    current_record().bucket.store(bucket.map_or(0, |b| b + 1), Ordering::SeqCst);
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

/// `LockSupport.park()`: waits until the calling thread's permit is given, and takes it, or until
/// the thread is interrupted, which leaves its status set.
pub fn park(timeout: Option<Duration>) {
    let record = current_record();
    let deadline = timeout.map(|t| Instant::now() + t);
    let mut permit = lock(&record.permit);
    while !*permit {
        if record.interrupted.load(Ordering::SeqCst) {
            return;
        }
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
    fn an_interrupt_ends_a_sleep_and_a_join() {
        // Whether the interrupt comes before the sleep or during it, the sleep ends with it.
        let sleeper = next_thread_id();
        let (ready, started) = std::sync::mpsc::channel();
        let (tx, rx) = std::sync::mpsc::channel();
        start_thread(sleeper, "sleeper".into(), false, move || {
            ready.send(()).unwrap();
            let slept = try_sleep(60_000);
            tx.send((slept, thread_interrupted())).unwrap();
        });
        started.recv().unwrap();
        interrupt_thread(sleeper);
        let (slept, status) = rx.recv().unwrap();
        assert_eq!(slept.unwrap_err().message.as_deref(), Some("sleep interrupted"));
        assert!(!status, "the exception clears the status");
        join_thread(sleeper, 0);

        let joiner = next_thread_id();
        let main_waits = next_thread_id();
        start_thread(main_waits, "long".into(), true, || std::thread::sleep(Duration::from_secs(60)));
        let (ready, started) = std::sync::mpsc::channel();
        let (tx, rx) = std::sync::mpsc::channel();
        start_thread(joiner, "joiner".into(), false, move || {
            ready.send(()).unwrap();
            tx.send(try_join_thread(main_waits, 0).is_err()).unwrap();
        });
        started.recv().unwrap();
        interrupt_thread(joiner);
        assert!(rx.recv().unwrap());
        join_thread(joiner, 0);
    }

    #[test]
    fn the_status_is_kept_before_the_start_and_after_the_end() {
        let tid = next_thread_id();
        interrupt_thread(tid);
        assert!(is_interrupted(tid));
        let (tx, rx) = std::sync::mpsc::channel();
        start_thread(tid, "early".into(), false, move || tx.send(thread_interrupted()).unwrap());
        assert!(rx.recv().unwrap());
        join_thread(tid, 0);
        assert!(!is_interrupted(tid));
        interrupt_thread(tid);
        assert!(is_interrupted(tid));
    }

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
