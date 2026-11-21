//! The static fields of a program. A program that starts no thread keeps each in a `Global`: a
//! plain `static` holding a `Cell` (a primitive) or a `RefCell`, read and written with the text of
//! a thread-local cell, inside a `LazyGlobal` when its initializer is not a constant. Unlike a
//! thread-local, it is an address the compiler knows, which it can keep in a register through a
//! loop that writes only arrays and objects. With threads, the statics that threads share are
//! process-wide and synchronized, and the others stay thread-local cells, which only the main
//! thread uses:
//!
//! - a primitive is a `java::Atomic<T>`, read with `get()` (acquire) and written with `set(v)`
//!   (release), the same text as a thread-local `Cell`'s; on x86-64 both are plain moves; a
//!   `volatile` one is read and written with `get_volatile` and `set_volatile`, sequentially
//!   consistent;
//! - an object or a string is a `java::SharedField<T>` (`field.rs`), read and written by value
//!   with no lock in the same way, behind a `LazyLock` when its initializer is not a constant;
//! - one that only its class's static initialization assigns, once (a `static final`), is a
//!   `java::Final<T>`, which a thread reads with no lock: `get()` lends the value in place;
//! - the flag that a class's static initialization has run is a `java::ClassInit`, which runs it
//!   once for the whole process, as JLS 12.4.2 describes.

use std::cell::{OnceCell, RefCell, UnsafeCell};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::atomic::{
    AtomicBool, AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicPtr, AtomicU16, AtomicU32, AtomicU64,
    AtomicUsize, Ordering,
};
use std::sync::{Condvar, Mutex, OnceLock};

/// A static field of a program that starts no thread: a `Cell` or a `RefCell` (`C`), which
/// `Deref` reaches. `with_borrow`, `with_borrow_mut` and `set` of a `RefCell` are those of a
/// thread-local one; `cloned` reads a `RefCell`'s value without writing its borrow flag.
pub struct Global<C>(C);

// Only the program's one thread runs Java code. The collector reads statics while that thread
// waits for it.
unsafe impl<C> Sync for Global<C> {}

impl<C> Global<C> {
    pub const fn new(cell: C) -> Global<C> {
        Global(cell)
    }
}

impl<C> Deref for Global<C> {
    type Target = C;

    #[inline(always)]
    fn deref(&self) -> &C {
        &self.0
    }
}

/// The object that a lambda or method reference capturing nothing is, where it is a struct of no
/// fields (or one in a `Monitored`): one per place it is written, a constant in a `static` there,
/// whose handle is the static's address. Nothing makes it at run time, and taking it is loading
/// that address. The collector never frees it, as it is not in the heap, and scans it with the
/// program's static data (`gc.rs`), which holds the lock a `Monitored` makes when a thread first
/// takes it; a weak reference to it is never cleared. Only the translation makes one, of an
/// object that threads may share. A byte after the value gives a struct of no size an address of
/// its own, as `Slot`'s does: Java's `==` tells two places' objects apart. The byte is a cell, so
/// the static is in writable data, which neither LLVM's merging of constants nor a linker's
/// folding of identical sections joins with another static of the same bytes.
#[repr(C)]
pub struct StaticObject<T> {
    value: T,
    _byte: std::cell::UnsafeCell<u8>,
}

// Only for objects of no fields whose handles threads may share, as above.
unsafe impl<T> Sync for StaticObject<T> {}

impl<T> StaticObject<T> {
    pub const fn new(value: T) -> StaticObject<T> {
        StaticObject { value, _byte: std::cell::UnsafeCell::new(0) }
    }

    /// The object's handle.
    #[inline(always)]
    pub fn handle(&'static self) -> crate::Arc<T> {
        crate::Arc::of_static(&self.value)
    }
}

/// The object that a lambda or method reference capturing nothing is, elsewhere: one per place it is
/// written, made the first time the place runs, in a `static` at that place. The translation
/// puts one there only where every thread may use the object, which has no fields: in a program
/// that starts no thread, or where the object is not a `RefCell`, whose borrow flag is not atomic;
/// elsewhere the place keeps a thread-local `OnceCell`. A thread-local takes its 8 bytes
/// in every thread's static thread-local storage, which the C library zeroes for each thread it
/// starts and the collector scans at each collection: the Minecraft server's 14,300 places took
/// 114 KB of each of its 60 threads' 120 KB.
pub struct PlaceObject<T>(OnceLock<T>);

// Only where the translation found the object safe to share, as above.
unsafe impl<T> Sync for PlaceObject<T> {}

impl<T: Clone> PlaceObject<T> {
    pub const fn new() -> PlaceObject<T> {
        PlaceObject(OnceLock::new())
    }

    /// The place's object, made by `make` the first time.
    #[inline(always)]
    pub fn get(&self, make: impl FnOnce() -> T) -> T {
        self.0.get_or_init(make).clone()
    }
}

impl<T: Clone> Default for PlaceObject<T> {
    fn default() -> PlaceObject<T> {
        PlaceObject::new()
    }
}

/// A `Global` whose value is computed on first use, by `init`.
pub struct LazyGlobal<C> {
    cell: OnceCell<C>,
    init: fn() -> C,
}

// As for `Global`.
unsafe impl<C> Sync for LazyGlobal<C> {}

impl<C> LazyGlobal<C> {
    pub const fn new(init: fn() -> C) -> LazyGlobal<C> {
        LazyGlobal { cell: OnceCell::new(), init }
    }
}

impl<C> Deref for LazyGlobal<C> {
    type Target = C;

    #[inline(always)]
    fn deref(&self) -> &C {
        self.cell.get_or_init(self.init)
    }
}

macro_rules! like_thread_local {
    ($($global:ident),*) => {$(
        impl<T> $global<RefCell<T>> {
            #[inline(always)]
            pub fn with_borrow<R>(&self, f: impl FnOnce(&T) -> R) -> R {
                f(&(**self).borrow())
            }

            #[inline(always)]
            pub fn with_borrow_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
                f(&mut (**self).borrow_mut())
            }

            /// A copy of the value, as `with_borrow(|v| v.clone())` makes it, but with the
            /// borrow flag tested and not written: a `borrow` adds to the flag and takes away
            /// from it again, stores to a static that the compiler cannot tell apart from the
            /// objects a loop reads, so a loop that reads the static kept reloading what it read
            /// through the value and was not unrolled. A clone of the translation's values runs
            /// no Java code, so nothing writes the static while it is lent.
            #[inline(always)]
            pub fn cloned(&self) -> T
            where
                T: Clone,
            {
                match unsafe { (**self).try_borrow_unguarded() } {
                    Ok(value) => value.clone(),
                    Err(_) => already_borrowed(),
                }
            }

            #[inline(always)]
            pub fn set(&self, value: T) {
                drop((**self).replace(value));
            }
        }
    )*};
}

like_thread_local!(Global, LazyGlobal);

/// A static read while it is being written, which `RefCell::borrow` refuses with this panic.
#[cold]
#[inline(never)]
fn already_borrowed() -> ! {
    panic!("already mutably borrowed")
}

/// A primitive static field shared by threads: every read acquires and every write releases, so
/// a thread that sees a value sees everything the writing thread did before it. The value is
/// only ever reached through the atomic of its size (`Primitive`), so that `new` can be one
/// `const fn` for every type: `static N: java::Atomic<i32> = java::Atomic::new(0);`.
#[repr(align(8))]
pub struct Atomic<T: Primitive>(UnsafeCell<T>);

// Every access is atomic.
unsafe impl<T: Primitive> Sync for Atomic<T> {}

/// The primitive types, each read and written through the atomic of its size (floats as their
/// bits).
pub trait Primitive: Copy {
    /// # Safety
    /// `at` points to a live value that is only accessed atomically, aligned for its atomic.
    unsafe fn load(at: *mut Self, order: Ordering) -> Self;
    /// # Safety
    /// As for `load`.
    unsafe fn store(at: *mut Self, value: Self, order: Ordering);
}

macro_rules! primitive {
    ($($t:ty => $cell:ty, $bits:ty;)*) => {$(
        impl Primitive for $t {
            #[inline]
            unsafe fn load(at: *mut $t, order: Ordering) -> $t {
                let bits = unsafe { <$cell>::from_ptr(at as *mut $bits) }.load(order);
                <$t>::from_bits_of(bits)
            }

            #[inline]
            unsafe fn store(at: *mut $t, value: $t, order: Ordering) {
                unsafe { <$cell>::from_ptr(at as *mut $bits) }.store(value.bits_of(), order)
            }
        }
    )*};
}

/// A primitive as the bits its atomic holds, and back.
trait Bits<B> {
    fn bits_of(self) -> B;
    fn from_bits_of(bits: B) -> Self;
}

macro_rules! same_bits {
    ($($t:ty),*) => {$(
        impl Bits<$t> for $t {
            #[inline]
            fn bits_of(self) -> $t {
                self
            }

            #[inline]
            fn from_bits_of(bits: $t) -> $t {
                bits
            }
        }
    )*};
}

same_bits!(i32, i64, i8, i16, u16, bool);

impl Bits<u32> for f32 {
    #[inline]
    fn bits_of(self) -> u32 {
        self.to_bits()
    }

    #[inline]
    fn from_bits_of(bits: u32) -> f32 {
        f32::from_bits(bits)
    }
}

impl Bits<u64> for f64 {
    #[inline]
    fn bits_of(self) -> u64 {
        self.to_bits()
    }

    #[inline]
    fn from_bits_of(bits: u64) -> f64 {
        f64::from_bits(bits)
    }
}

primitive! {
    i32 => AtomicI32, i32;
    i64 => AtomicI64, i64;
    i8 => AtomicI8, i8;
    i16 => AtomicI16, i16;
    u16 => AtomicU16, u16;
    bool => AtomicBool, bool;
    f32 => AtomicU32, u32;
    f64 => AtomicU64, u64;
}

impl<T: Primitive> Atomic<T> {
    pub const fn new(value: T) -> Atomic<T> {
        Atomic(UnsafeCell::new(value))
    }

    #[inline]
    pub fn get(&self) -> T {
        // The cell is aligned to 8, enough for every atomic, and only accessed atomically.
        unsafe { T::load(self.0.get(), Ordering::Acquire) }
    }

    #[inline]
    pub fn set(&self, value: T) {
        unsafe { T::store(self.0.get(), value, Ordering::Release) }
    }

    /// A read of a `volatile` static, sequentially consistent as Java's is.
    #[inline]
    pub fn get_volatile(&self) -> T {
        unsafe { T::load(self.0.get(), Ordering::SeqCst) }
    }

    /// A write of a `volatile` static, sequentially consistent as Java's is.
    #[inline]
    pub fn set_volatile(&self, value: T) {
        unsafe { T::store(self.0.get(), value, Ordering::SeqCst) }
    }
}

/// A `static final` object, array or string that threads share, which only its class's static
/// initialization assigns, once (`set`). Until then it holds the value it starts with, Java's
/// default (or, for a class initialized lazily, its initializer's value, which nothing replaces),
/// as the initializer's own reads before the assignment see it. A reader lends the value with no
/// lock (`get`): once the class is initialized it never changes.
///
/// A read is one load of `current`, the address of the value it lends, and a test that it is set:
/// the static needs no `LazyLock` around it, though the value it starts with may be built at run
/// time (`lazy`), since that value is built on the first read that finds `current` unset, out of
/// line, and kept in `default`. `set` writes the assigned value into `value` and then points
/// `current` at it with release, which publishes it to every thread that reads `current`. Neither
/// value moves or changes once `current` points at it, so a lent value stays valid. A thread that
/// reads the static during another thread's initialization (code of the class itself, which waits
/// for no initialization) sees one of the two values.
///
/// An assignment made before the program starts its first thread also writes the value's address
/// to `settled`, a plain field, and a read tries `settled` first. Every later thread starts after
/// that write, so it sees it with no ordering of its own, and once a thread has started nothing
/// writes `settled` again, so its plain reads race with nothing. A plain read is one the compiler
/// may keep from one use to the next, which it does not do with the acquire load of `current`:
/// a loop that reads the static on each pass then reads it once.
///
/// A static of an eager class (one that runs its `<clinit>` on first use) is prepared instead:
/// `main` calls `prepare` on each before any other code runs, which points `current` at the value
/// the static starts with, so that a read (`get_prepared`) is the acquire load of `current` alone,
/// with no test. A test and its out-of-line call on each read split the code around the read and
/// keep values from staying in registers across it; in a thread started after the class's
/// initialization, where `settled` is never set, NoiseTree's noise function, which reads its
/// gradient table eight times, ran 290 instructions a call with the test and 221 without.
pub struct Final<T> {
    settled: UnsafeCell<*const T>,
    current: AtomicPtr<T>,
    value: UnsafeCell<MaybeUninit<T>>,
    assigned: AtomicBool,
    default: OnceLock<T>,
    start: Start<T>,
}

/// The value a `Final` starts with: a constant, or a function that builds it.
enum Start<T> {
    Value(T),
    Built(fn() -> T),
}

// `value` is written once, before `current` publishes it, and only read after; the rest is `Sync`.
unsafe impl<T: Send + Sync> Sync for Final<T> {}

impl<T> Final<T> {
    pub const fn new(default: T) -> Final<T> {
        Final::starting(Start::Value(default))
    }

    /// A `Final` that starts with the value `make` builds, on its first read.
    pub const fn lazy(make: fn() -> T) -> Final<T> {
        Final::starting(Start::Built(make))
    }

    const fn starting(start: Start<T>) -> Final<T> {
        Final {
            settled: UnsafeCell::new(std::ptr::null()),
            current: AtomicPtr::new(std::ptr::null_mut()),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            assigned: AtomicBool::new(false),
            default: OnceLock::new(),
            start,
        }
    }

    /// The value: the one assigned, or the one it starts with.
    #[inline(always)]
    pub fn get(&self) -> &T {
        // Written only before the first thread starts (see `set`).
        let settled = unsafe { *self.settled.get() };
        if !settled.is_null() {
            // As in `unsettled`: `settled` points at `value`.
            return unsafe { &*settled };
        }
        self.unsettled()
    }

    /// The value of a prepared static (see the type's documentation): `current` is set before
    /// any thread starts and never unset.
    #[inline(always)]
    pub fn get_prepared(&self) -> &T {
        let p = self.current.load(Ordering::Acquire);
        debug_assert!(!p.is_null(), "a static final read before it was prepared");
        // `prepare` set `current`, before any thread started; it points at `value` or a starting
        // value, as in `unsettled`.
        unsafe { &*p }
    }

    /// The value of a static whose class `main` initializes before any other code runs: the
    /// address in `settled`, a plain load with no test, which the compiler may keep from one read
    /// to the next. `main` checks that it is set (`check_settled`) before any code that reads it
    /// this way runs, and nothing writes it once a thread has started.
    #[inline(always)]
    pub fn get_settled(&self) -> &T {
        // Written only before the first thread starts (see `set`).
        let settled = unsafe { *self.settled.get() };
        debug_assert!(!settled.is_null(), "a static final read before it was settled");
        // `check_settled` found it set, and it points at `value`, as in `get`.
        unsafe { &*settled }
    }

    /// Panics unless `settled` is set: `main` calls it for each static read with `get_settled`,
    /// once the class's initialization has run.
    #[cold]
    #[inline(never)]
    pub fn check_settled(&self) {
        if unsafe { *self.settled.get() }.is_null() {
            panic!("a static final that main initializes was not settled");
        }
    }

    /// Points `current` at the starting value, unless it is set: `main` calls it for each static
    /// read with `get_prepared`, before any other code runs.
    #[cold]
    #[inline(never)]
    pub fn prepare(&self) {
        if self.current.load(Ordering::Acquire).is_null() {
            self.first_read();
        }
    }

    /// A read that finds `settled` unset: one out-of-line call, so that a function that reads the
    /// static several times reaches each later read along two paths, of which the call is the
    /// only one that has not read `settled` already, and the compiler can then read it once.
    #[cold]
    #[inline(never)]
    fn unsettled(&self) -> &T {
        let p = self.current.load(Ordering::Acquire);
        if p.is_null() {
            return self.first_read();
        }
        // `current` points at `value` or a starting value, neither of which moves or changes
        // while the `Final` lives.
        unsafe { &*p }
    }

    /// A read before `current` is set: points it at the starting value, unless `set` has pointed
    /// it at the assigned one meanwhile.
    #[cold]
    #[inline(never)]
    fn first_read(&self) -> &T {
        let start: *const T = match &self.start {
            Start::Value(v) => v,
            Start::Built(make) => self.default.get_or_init(*make),
        };
        let _ = self.current.compare_exchange(
            std::ptr::null_mut(),
            start as *mut T,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        // As in `get`.
        unsafe { &*self.current.load(Ordering::Acquire) }
    }

    /// The assignment of the class's static initialization. Java assigns a `final` once, so a
    /// second assignment is a translation bug.
    pub fn set(&self, value: T) {
        if self.assigned.swap(true, Ordering::AcqRel) {
            panic!("a static final assigned twice");
        }
        // Only this call writes `value`, and no reader reaches it until `current` points at it.
        unsafe { (*self.value.get()).write(value) };
        self.current.store(self.value.get() as *mut T, Ordering::Release);
        // No other thread runs yet: every thread that reads `settled` starts after this write.
        if !crate::thread::threads_started() {
            unsafe { *self.settled.get() = self.value.get() as *const T };
        }
    }

    /// Runs `f` on the value where it is, with no lock and no copy.
    #[inline(always)]
    pub fn with_borrow<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(self.get())
    }
}

impl<T> Drop for Final<T> {
    fn drop(&mut self) {
        if *self.assigned.get_mut() {
            // `set` wrote it, and nothing lends it once the `Final` is dropped.
            unsafe { self.value.get_mut().assume_init_drop() };
        }
    }
}

/// The state of a class's static initialization, for the whole process: not started, being run
/// by the thread with a token (the address of a thread-local, never 0, 1 or 2), done, or done and
/// succeeded (`READY`, for an initialization that keeps its own state of success, see
/// `ClassInit::run`).
const NOT_STARTED: usize = 0;
const DONE: usize = 1;
const READY: usize = 2;

/// Whether a class's static initialization has run, for a program with threads: the first thread
/// that uses the class runs it, another thread that uses the class meanwhile waits until it is
/// done, and the thread that runs it sees the class as initialized when its initializer uses the
/// class again (it reads the statics not yet assigned at their defaults), as JLS 12.4.2 says.
/// (`std::sync::Once` would deadlock on that last case.)
///
/// The test that a use of the class makes first is one acquire load of `state`: `done` for a
/// class whose initialization cannot fail, `ready` for one that keeps its own state of success or
/// failure, whose run records which it was as it finishes. Either sees, once true, everything the
/// thread that ran the initialization wrote, as the finish stores with release.
pub struct ClassInit {
    state: AtomicUsize,
    lock: Mutex<()>,
    done: Condvar,
}

impl ClassInit {
    pub const fn new() -> ClassInit {
        ClassInit { state: AtomicUsize::new(NOT_STARTED), lock: Mutex::new(()), done: Condvar::new() }
    }

    /// Whether the calling thread is to run the initialization now: `false` once it is done, or
    /// while this thread runs it; another thread's run is waited for first.
    #[inline]
    pub fn start(&self) -> bool {
        if matches!(self.state.load(Ordering::Acquire), DONE | READY) {
            return false;
        }
        self.start_slow()
    }

    /// Whether the initialization is done: a test without side effects, for the fast path of a
    /// class's `init` function.
    #[inline]
    pub fn done(&self) -> bool {
        matches!(self.state.load(Ordering::Acquire), DONE | READY)
    }

    /// Whether the initialization is done and succeeded (see `run`): the whole fast path of the
    /// `init` function of a class whose `<clinit>` keeps its own state, one load.
    #[inline]
    pub fn ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == READY
    }

    #[cold]
    fn start_slow(&self) -> bool {
        let me = crate::locked::thread_token();
        let mut guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match self.state.load(Ordering::Acquire) {
                NOT_STARTED => {
                    self.state.store(me, Ordering::Release);
                    return true;
                }
                DONE | READY => return false,
                owner if owner == me => return false,
                _ => guard = self.done.wait(guard).unwrap_or_else(|e| e.into_inner()),
            }
        }
    }

    /// Records that the initialization the calling thread started is done, and wakes the threads
    /// that wait for it.
    pub fn finish(&self) {
        self.finish_as(DONE);
    }

    fn finish_as(&self, state: usize) {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.state.store(state, Ordering::Release);
        self.done.notify_all();
    }

    /// `start`, for an initialization that keeps its own state of success or failure in `kept`
    /// (a class whose `<clinit>` the translator rewrites to rethrow: 0 not started, 1 started or
    /// succeeded, 2 failed): the run that the calling thread is to do now, which finishes when it
    /// is dropped, at whichever return ends it, failed or not; as `READY` when `kept` is then 1,
    /// so that `ready` alone tells a later use that it may go on. `None` once it is done, or while
    /// this thread runs it; another thread's run is waited for first.
    #[inline]
    pub fn run<'a>(&'a self, kept: &'a Atomic<i32>) -> Option<InitRun<'a>> {
        self.start().then_some(InitRun { init: self, kept })
    }
}

/// An initialization that the calling thread runs (`ClassInit::run`): done when dropped, and
/// ready when its kept state says that it succeeded.
pub struct InitRun<'a> {
    init: &'a ClassInit,
    kept: &'a Atomic<i32>,
}

impl Drop for InitRun<'_> {
    fn drop(&mut self) {
        // Only the calling thread writes the kept state while the run is in progress.
        self.init.finish_as(if self.kept.get() == 1 { READY } else { DONE });
    }
}

impl Default for ClassInit {
    fn default() -> ClassInit {
        ClassInit::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn atomics_read_what_was_written() {
        static N: Atomic<i32> = Atomic::new(-3);
        static D: Atomic<f64> = Atomic::new(0.5);
        static C: Atomic<u16> = Atomic::new(65);
        N.set(N.get().wrapping_mul(2));
        D.set(D.get() * 3.0);
        assert_eq!((N.get(), D.get(), C.get()), (-6, 1.5, 65));
    }

    #[test]
    fn a_final_reads_its_default_until_it_is_set_and_then_its_value() {
        static NAME: Final<Option<&str>> = Final::new(None);
        assert_eq!(*NAME.get(), None);
        NAME.set(Some("set"));
        let read: Vec<_> = (0..4).map(|_| std::thread::spawn(|| *NAME.get())).collect();
        for t in read {
            assert_eq!(t.join().unwrap(), Some("set"));
        }
        assert!(NAME.with_borrow(|v| v.is_some()));
    }

    #[test]
    #[should_panic(expected = "a static final assigned twice")]
    fn a_final_is_assigned_once() {
        let once = Final::new(0);
        once.set(1);
        once.set(2);
    }

    #[test]
    fn a_class_is_initialized_once_and_others_wait_for_it() {
        crate::locked::begin_threads();
        static INIT: ClassInit = ClassInit::new();
        static VALUE: Atomic<i32> = Atomic::new(0);
        let runs = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let runs = runs.clone();
                std::thread::spawn(move || {
                    if INIT.start() {
                        runs.fetch_add(1, Ordering::SeqCst);
                        // Recursive initialization from the same thread sees it as started.
                        assert!(!INIT.start());
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        VALUE.set(42);
                        INIT.finish();
                    }
                    VALUE.get()
                })
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap(), 42);
        }
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_run_that_fails_is_done_for_the_threads_that_wait() {
        crate::locked::begin_threads();
        static INIT: ClassInit = ClassInit::new();
        static STATE: Atomic<i32> = Atomic::new(0);
        fn init() -> Result<(), &'static str> {
            let _run = INIT.run(&STATE);
            match STATE.get() {
                2 => return Err("no class"),
                1 => return Ok(()),
                _ => STATE.set(1),
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            STATE.set(2);
            Err("failed")
        }
        let threads: Vec<_> = (0..4).map(|_| std::thread::spawn(init)).collect();
        let mut results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        results.sort();
        assert_eq!(results, [Err("failed"), Err("no class"), Err("no class"), Err("no class")]);
        assert!(INIT.done() && !INIT.ready());
    }

    #[test]
    fn a_run_that_succeeds_is_ready_for_the_threads_that_wait() {
        crate::locked::begin_threads();
        static INIT: ClassInit = ClassInit::new();
        static STATE: Atomic<i32> = Atomic::new(0);
        static VALUE: Atomic<i32> = Atomic::new(0);
        fn init() -> Result<i32, &'static str> {
            if INIT.ready() {
                return Ok(VALUE.get());
            }
            let _run = INIT.run(&STATE);
            match STATE.get() {
                2 => return Err("no class"),
                1 => return Ok(VALUE.get()),
                _ => STATE.set(1),
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            VALUE.set(7);
            Ok(VALUE.get())
        }
        let threads: Vec<_> = (0..4).map(|_| std::thread::spawn(init)).collect();
        for t in threads {
            assert_eq!(t.join().unwrap(), Ok(7));
        }
        assert!(INIT.ready() && INIT.done());
    }
}
