//! `java::Locked<T>`: the data lock of an object that several threads can reach, which takes the
//! place of the `RefCell` of a thread-local object (`Arc<java::Locked<C>>` for `Rc<RefCell<C>>`).
//!
//! It is a `RefCell` behind a reentrant lock. `borrow()` and `borrow_mut()` take the lock (or
//! count once more when the calling thread holds it already) and then borrow the cell, and their
//! guards release both. So the translation keeps the shapes and the discipline of its `RefCell`
//! code: one statement may borrow the same object several times (`o.borrow_mut().n =
//! o.borrow().n + 1`), which a plain `Mutex` would deadlock on, and a borrow that conflicts with
//! another of the same thread panics as `RefCell`'s does. A thread that finds the object locked by
//! another waits until that thread's borrows end.
//!
//! The translation never holds a borrow while it waits for anything else, so a data lock cannot be
//! part of a deadlock (see docs/multi-threading.md, 1.3). In a build with debug assertions (the
//! corpus test's) a wait that lasts longer than `WATCHDOG` is that rule broken, and panics with a
//! message that says so instead of hanging.
//!
//! The same lock is the object's monitor (`monitor.rs`): `synchronized` holds it for a whole
//! region, which may be long and may wait for other monitors, as Java's does. A thread that waits
//! for a lock held as a monitor is not watched.

use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a thread may wait for a data lock in a build with debug assertions before the wait is
/// taken for a translation bug.
const WATCHDOG: Duration = Duration::from_secs(20);

thread_local! {
    /// A byte whose address identifies the thread while it lives.
    static TOKEN: u8 = const { 0 };
}

/// The calling thread's token: never 0, and different from every other live thread's.
#[inline]
pub(crate) fn thread_token() -> usize {
    TOKEN.with(|t| t as *const u8 as usize)
}

/// Where threads that wait for a lock sleep: a few condition variables shared by all locks, picked
/// by the lock's address, so that a lock stays small. The mutex guards the wait sets of the
/// bucket's monitors: one entry per thread in `Object.wait`.
struct Bucket {
    mutex: Mutex<Vec<Waiter>>,
    condvar: Condvar,
    waiters: AtomicUsize,
}

/// A thread in `Object.wait` on the monitor at `lock`, and whether a `notify` has picked it. Each
/// waiter has its own entry, so a notification is for one waiter and no later one can take it.
struct Waiter {
    lock: usize,
    ticket: u64,
    notified: bool,
}

static TICKETS: AtomicU64 = AtomicU64::new(0);

const BUCKETS: usize = 64;

static PARKING: [Bucket; BUCKETS] = [const {
    Bucket { mutex: Mutex::new(Vec::new()), condvar: Condvar::new(), waiters: AtomicUsize::new(0) }
}; BUCKETS];

fn bucket_index(address: usize) -> usize {
    (address >> 4) % BUCKETS
}

fn bucket(address: usize) -> &'static Bucket {
    &PARKING[bucket_index(address)]
}

/// Wakes the threads that wait in bucket `index`, for an interrupt of one of them.
pub(crate) fn wake_bucket(index: usize) {
    let bucket = &PARKING[index];
    let _guard = bucket.mutex.lock().unwrap_or_else(|e| e.into_inner());
    bucket.condvar.notify_all();
}

/// How `Object.wait` ended.
pub(crate) enum Waited {
    /// Notified, or the timeout passed.
    Done,
    /// The thread does not hold the monitor.
    NotOwner,
    /// An interrupt of the thread, before or during the wait.
    Interrupted,
}

/// A reentrant lock: the token of the thread that holds it (0 when free), and how many times that
/// thread has taken it, of which how many as the object's monitor. (The monitor's wait set is in
/// the lock's bucket.)
pub struct RawLock {
    owner: AtomicUsize,
    depth: Cell<usize>,
    entered: AtomicUsize,
}

// `depth` is only read and written by the thread that holds the lock.
unsafe impl Sync for RawLock {}

impl RawLock {
    pub(crate) const fn new() -> RawLock {
        RawLock { owner: AtomicUsize::new(0), depth: Cell::new(0), entered: AtomicUsize::new(0) }
    }

    #[inline]
    pub(crate) fn acquire(&self) {
        let me = thread_token();
        // Only this thread ever stores its own token, so seeing it means this thread holds the lock.
        if self.owner.load(Ordering::Relaxed) == me {
            self.depth.set(self.depth.get() + 1);
            return;
        }
        if self.owner.compare_exchange(0, me, Ordering::Acquire, Ordering::Relaxed).is_err() {
            self.wait(me);
        }
        self.depth.set(1);
    }

    #[cold]
    fn wait(&self, me: usize) {
        let mut started = Instant::now();
        let bucket = bucket(self as *const RawLock as usize);
        let mut spins = 0u32;
        loop {
            if self.owner.load(Ordering::Relaxed) == 0
                && self.owner.compare_exchange(0, me, Ordering::Acquire, Ordering::Relaxed).is_ok()
            {
                return;
            }
            spins += 1;
            if spins < 64 {
                std::hint::spin_loop();
                continue;
            }
            if spins < 128 {
                std::thread::yield_now();
                continue;
            }
            let guard = bucket.mutex.lock().unwrap_or_else(|e| e.into_inner());
            bucket.waiters.fetch_add(1, Ordering::SeqCst);
            // `release` stores 0 before it looks for waiters, and this thread counted itself before
            // it looks at the owner, so one of the two sees the other: no wakeup is lost. The
            // timeout is only a second line of defence.
            if self.owner.load(Ordering::SeqCst) != 0 {
                let _ = bucket.condvar.wait_timeout(guard, Duration::from_millis(50));
            } else {
                drop(guard);
            }
            bucket.waiters.fetch_sub(1, Ordering::SeqCst);
            // A monitor may be held for long, as Java holds it.
            if self.entered.load(Ordering::Relaxed) > 0 {
                started = Instant::now();
            }
            if cfg!(debug_assertions) && started.elapsed() > WATCHDOG {
                panic!(
                    "translation bug: a thread waited {} seconds for the data lock of an object, which \
                     another thread holds across a call that blocks",
                    WATCHDOG.as_secs()
                );
            }
        }
    }

    #[inline]
    pub(crate) fn release(&self) {
        let depth = self.depth.get() - 1;
        self.depth.set(depth);
        if depth == 0 {
            self.owner.store(0, Ordering::SeqCst);
            let bucket = bucket(self as *const RawLock as usize);
            if bucket.waiters.load(Ordering::SeqCst) > 0 {
                let _guard = bucket.mutex.lock().unwrap_or_else(|e| e.into_inner());
                bucket.condvar.notify_all();
            }
        }
    }
}

impl RawLock {
    /// Enters the lock as the object's monitor: `synchronized`.
    #[inline]
    pub(crate) fn enter(&self) {
        self.acquire();
        self.entered.fetch_add(1, Ordering::Relaxed);
    }

    /// Exits the monitor that `enter` entered.
    #[inline]
    pub(crate) fn exit(&self) {
        self.entered.fetch_sub(1, Ordering::Relaxed);
        self.release();
    }

    /// Whether the calling thread holds the monitor: `Thread.holdsLock`.
    pub(crate) fn held_by_current(&self) -> bool {
        self.owner.load(Ordering::Relaxed) == thread_token() && self.entered.load(Ordering::Relaxed) > 0
    }

    /// `Object.wait`: gives up the monitor, every entry of it, until another thread notifies this
    /// one or the timeout passes, then takes it again as it was held. `false`, with nothing done,
    /// when the calling thread does not hold the monitor.
    pub(crate) fn wait_notified(&self, timeout: Option<Duration>) -> Waited {
        if !self.held_by_current() {
            return Waited::NotOwner;
        }
        if crate::thread::take_interrupt() {
            return Waited::Interrupted;
        }
        let address = self as *const RawLock as usize;
        let bucket = bucket(address);
        let lock = || bucket.mutex.lock().unwrap_or_else(|e| e.into_inner());
        let ticket = TICKETS.fetch_add(1, Ordering::Relaxed);
        // In the wait set while the monitor is still held, so a `notify` after this sees the waiter.
        lock().push(Waiter { lock: address, ticket, notified: false });
        let (depth, entered) = (self.depth.get(), self.entered.load(Ordering::Relaxed));
        // Every hold of the lock is a monitor's: a borrow of the object's fields across the wait
        // would let another thread in while it lasts.
        assert!(depth == entered, "translation bug: `wait` while a borrow of the object is held");
        self.entered.store(0, Ordering::Relaxed);
        self.depth.set(1);
        self.release();
        let deadline = timeout.map(|t| Instant::now() + t);
        crate::thread::waiting_in(Some(bucket_index(address)));
        let mut guard = lock();
        let interrupted = loop {
            let at = guard.iter().position(|w| w.ticket == ticket).expect("a waiter leaves its own entry");
            let now = Instant::now();
            // `interrupt` sets the status, then notifies under this mutex: no wakeup is lost. A
            // waiter both notified and interrupted returns normally with the status set.
            let interrupted = !guard[at].notified && crate::thread::take_interrupt();
            if guard[at].notified || interrupted || deadline.is_some_and(|d| now >= d) {
                guard.swap_remove(at);
                break interrupted;
            }
            let step =
                deadline.map_or(Duration::from_millis(50), |d| (d - now).min(Duration::from_millis(50)));
            guard = bucket.condvar.wait_timeout(guard, step).unwrap_or_else(|e| e.into_inner()).0;
        };
        drop(guard);
        crate::thread::waiting_in(None);
        self.acquire();
        self.depth.set(depth);
        self.entered.store(entered, Ordering::Relaxed);
        if interrupted { Waited::Interrupted } else { Waited::Done }
    }

    /// `Object.notify`, or with `all` `notifyAll`: wakes one of the threads waiting in `wait`,
    /// or all of them. `false` when the calling thread does not hold the monitor.
    pub(crate) fn notify(&self, all: bool) -> bool {
        if !self.held_by_current() {
            return false;
        }
        let address = self as *const RawLock as usize;
        let bucket = bucket(address);
        let mut waiters = bucket.mutex.lock().unwrap_or_else(|e| e.into_inner());
        let waiting = waiters.iter_mut().filter(|w| w.lock == address && !w.notified);
        let picked = if all {
            waiting.fold(false, |_, w| {
                w.notified = true;
                true
            })
        } else {
            // The one that has waited longest.
            match waiting.min_by_key(|w| w.ticket) {
                Some(w) => {
                    w.notified = true;
                    true
                }
                None => false,
            }
        };
        if picked {
            bucket.condvar.notify_all();
        }
        true
    }
}

/// One acquisition of a `RawLock`, released when dropped.
struct Hold<'a>(&'a RawLock);

impl<'a> Hold<'a> {
    fn new(lock: &'a RawLock) -> Hold<'a> {
        lock.acquire();
        Hold(lock)
    }
}

impl Drop for Hold<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// The data of an object that several threads reach, behind its lock.
pub struct Locked<T: ?Sized> {
    lock: RawLock,
    value: RefCell<T>,
}

// The cell is only borrowed by the thread that holds the lock, as a `Mutex<T>` gives its value to
// one thread at a time.
unsafe impl<T: ?Sized + Send> Send for Locked<T> {}
unsafe impl<T: ?Sized + Send> Sync for Locked<T> {}

impl<T> Locked<T> {
    pub const fn new(value: T) -> Locked<T> {
        Locked { lock: RawLock::new(), value: RefCell::new(value) }
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }

    /// Replaces the data, locked: a static field's store, as `LocalKey::set` of a thread-local
    /// `RefCell`. The old value is dropped once the lock is released.
    pub fn set(&self, value: T) {
        let old = std::mem::replace(&mut *self.borrow_mut(), value);
        drop(old);
    }
}

impl<T: ?Sized> Locked<T> {
    /// Locks the data and borrows it to read, as `RefCell::borrow`.
    #[inline]
    pub fn borrow(&self) -> Ref<'_, T> {
        let hold = Hold::new(&self.lock);
        Ref { value: self.value.borrow(), _hold: hold }
    }

    /// Locks the data and borrows it to change, as `RefCell::borrow_mut`.
    #[inline]
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        let hold = Hold::new(&self.lock);
        RefMut { value: self.value.borrow_mut(), _hold: hold }
    }

    /// Runs `f` on the data, locked: a static field's read, as `LocalKey::with_borrow`.
    #[inline]
    pub fn with_borrow<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.borrow())
    }

    /// Runs `f` on the data, locked to change: a static field's write.
    #[inline]
    pub fn with_borrow_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.borrow_mut())
    }

    /// The lock, which is the object's monitor too.
    pub(crate) fn raw(&self) -> &RawLock {
        &self.lock
    }

    /// The data of an object that nothing else holds, as `RefCell::get_mut`: no lock is needed.
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// The address of the data, which is the object's identity (as `RefCell::as_ptr`).
    pub fn as_ptr(&self) -> *mut T {
        self.value.as_ptr()
    }
}

impl<T: Default> Default for Locked<T> {
    fn default() -> Locked<T> {
        Locked::new(T::default())
    }
}

/// A borrow of a `Locked` to read. The cell's borrow ends before the lock is released (fields drop
/// in order).
pub struct Ref<'a, T: ?Sized> {
    value: std::cell::Ref<'a, T>,
    _hold: Hold<'a>,
}

impl<'a, T: ?Sized> Ref<'a, T> {
    /// A borrow of a part of the data, as `std::cell::Ref::map`: the fields struct of a superclass.
    pub fn map<U: ?Sized>(r: Ref<'a, T>, f: impl FnOnce(&T) -> &U) -> Ref<'a, U> {
        Ref { value: std::cell::Ref::map(r.value, f), _hold: r._hold }
    }
}

impl<T: ?Sized> Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.value
    }
}

/// A borrow of a `Locked` to change.
pub struct RefMut<'a, T: ?Sized> {
    value: std::cell::RefMut<'a, T>,
    _hold: Hold<'a>,
}

impl<'a, T: ?Sized> RefMut<'a, T> {
    /// A borrow of a part of the data, as `std::cell::RefMut::map`.
    pub fn map<U: ?Sized>(r: RefMut<'a, T>, f: impl FnOnce(&mut T) -> &mut U) -> RefMut<'a, U> {
        RefMut { value: std::cell::RefMut::map(r.value, f), _hold: r._hold }
    }
}

impl<T: ?Sized> Deref for RefMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: ?Sized> DerefMut for RefMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_thread_borrows_its_own_locked_data_again() {
        let o = Locked::new(1);
        let n = *o.borrow() + *o.borrow();
        *o.borrow_mut() += n;
        assert_eq!(*o.borrow(), 3);
        struct Pair {
            base: i32,
        }
        let p = Locked::new(Pair { base: 4 });
        RefMut::map(p.borrow_mut(), |p| &mut p.base).clone_from(&7);
        assert_eq!(*Ref::map(p.borrow(), |p| &p.base), 7);
    }

    #[test]
    #[should_panic(expected = "already")]
    fn a_conflicting_borrow_panics_as_in_a_ref_cell() {
        let o = Locked::new(1);
        let _a = o.borrow();
        let _b = o.borrow_mut();
    }

    #[test]
    fn threads_take_turns() {
        let racy = Arc::new(Locked::new(0i64));
        let guarded = Arc::new(Locked::new(0i64));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let (racy, guarded) = (racy.clone(), guarded.clone());
                std::thread::spawn(move || {
                    for _ in 0..20_000 {
                        let n = *racy.borrow();
                        *racy.borrow_mut() = n + 1;
                        let mut c = guarded.borrow_mut();
                        *c += 1;
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        // The two-step increment may lose updates of other threads, as Java's racy `n++` may;
        // the one under a single borrow never does.
        let total = *racy.borrow();
        assert!((1..=80_000).contains(&total), "{total}");
        assert_eq!(*guarded.borrow(), 80_000);
    }

    #[test]
    fn a_held_lock_makes_others_wait() {
        let o = Arc::new(Locked::new(Vec::new()));
        let held = o.borrow_mut();
        let other = {
            let o = o.clone();
            std::thread::spawn(move || o.borrow_mut().push(2))
        };
        std::thread::sleep(Duration::from_millis(50));
        drop(held);
        o.borrow_mut().push(1);
        other.join().unwrap();
        let mut v = o.borrow().clone();
        v.sort();
        assert_eq!(v, vec![1, 2]);
    }
}
