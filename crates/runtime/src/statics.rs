//! The static fields of a program whose threads share them. A single-threaded program keeps its
//! statics in thread-local cells; with threads, each is a process-wide `static`:
//!
//! - a primitive is a `java::Atomic<T>`, read with `get()` (acquire) and written with `set(v)`
//!   (release), the same text as a thread-local `Cell`'s; on x86-64 both are plain moves;
//! - an object or a string is a `java::Locked<T>` (`with_borrow`, `with_borrow_mut`, as a
//!   thread-local `RefCell`'s), behind a `LazyLock` when its initializer is not a constant;
//! - the flag that a class's static initialization has run is a `java::ClassInit`, which runs it
//!   once for the whole process, as JLS 12.4.2 describes.

use std::cell::UnsafeCell;
use std::sync::atomic::{
    AtomicBool, AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicU16, AtomicU32, AtomicU64, AtomicUsize,
    Ordering,
};
use std::sync::{Condvar, Mutex};

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
            fn bits_of(self) -> $t {
                self
            }

            fn from_bits_of(bits: $t) -> $t {
                bits
            }
        }
    )*};
}

same_bits!(i32, i64, i8, i16, u16, bool);

impl Bits<u32> for f32 {
    fn bits_of(self) -> u32 {
        self.to_bits()
    }

    fn from_bits_of(bits: u32) -> f32 {
        f32::from_bits(bits)
    }
}

impl Bits<u64> for f64 {
    fn bits_of(self) -> u64 {
        self.to_bits()
    }

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
}

/// The state of a class's static initialization, for the whole process: not started, being run
/// by the thread with a token, or done.
const NOT_STARTED: usize = 0;
const DONE: usize = 1;

/// Whether a class's static initialization has run, for a program with threads: the first thread
/// that uses the class runs it, another thread that uses the class meanwhile waits until it is
/// done, and the thread that runs it sees the class as initialized when its initializer uses the
/// class again (it reads the statics not yet assigned at their defaults), as JLS 12.4.2 says.
/// (`std::sync::Once` would deadlock on that last case.)
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
        if self.state.load(Ordering::Acquire) == DONE {
            return false;
        }
        self.start_slow()
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
                DONE => return false,
                owner if owner == me => return false,
                _ => guard = self.done.wait(guard).unwrap_or_else(|e| e.into_inner()),
            }
        }
    }

    /// Records that the initialization the calling thread started is done, and wakes the threads
    /// that wait for it.
    pub fn finish(&self) {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.state.store(DONE, Ordering::Release);
        self.done.notify_all();
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
}
