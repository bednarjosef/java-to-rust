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

use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
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
/// by the lock's address, so that a lock is two words.
struct Bucket {
    mutex: Mutex<()>,
    condvar: Condvar,
    waiters: AtomicUsize,
}

const BUCKETS: usize = 64;

static PARKING: [Bucket; BUCKETS] =
    [const { Bucket { mutex: Mutex::new(()), condvar: Condvar::new(), waiters: AtomicUsize::new(0) } };
        BUCKETS];

fn bucket(address: usize) -> &'static Bucket {
    &PARKING[(address >> 4) % BUCKETS]
}

/// A reentrant lock: the token of the thread that holds it (0 when free), and how many times that
/// thread has taken it.
pub(crate) struct RawLock {
    owner: AtomicUsize,
    depth: Cell<usize>,
}

// `depth` is only read and written by the thread that holds the lock.
unsafe impl Sync for RawLock {}

impl RawLock {
    pub(crate) const fn new() -> RawLock {
        RawLock { owner: AtomicUsize::new(0), depth: Cell::new(0) }
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
        let started = Instant::now();
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
