//! `java::Arc<T>`: the handle of an object that threads can reach, as `std::sync::Arc`, whose
//! count changes take plain loads and stores until the process starts a second thread.
//!
//! A program that may start a thread holds most of its objects in `Arc`s, and copying and dropping
//! a handle is most of what such a program does: `std::sync::Arc` makes each copy and drop an
//! atomic read-modify-write, which costs a store-buffer drain every time even when only one thread
//! runs. javac starts its one thread only when a file manager closes in the background, which a
//! normal compilation never does; plain counts make it about an eighth faster.
//!
//! The count is an `AtomicUsize`. While only one thread runs (`locked::alone`), a copy loads it and
//! stores it plus one, with relaxed orderings, which compile to a plain increment. That is sound
//! for the same reason as the locks' plain path (`locked.rs`, `THREADS`): the flag turns on only in
//! the thread that is about to spawn the second thread, before it does, so a thread that reads it
//! off is the only thread there is, and the spawn orders every plain store before anything the new
//! thread does. From then on every change is atomic, as `std::sync::Arc`'s.
//!
//! There are no weak references, and a handle of a `dyn` type cannot be made from one of a sized
//! type (that needs the unstable `CoerceUnsized`): closures of functional interfaces that threads
//! share stay `std::sync::Arc<dyn Fn ..>`.

use crate::locked::alone;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering, fence};

/// More references than this abort the process, as `std::sync::Arc` does: counting on would wrap.
const MAX_REFS: usize = isize::MAX as usize;

struct Inner<T: ?Sized> {
    count: AtomicUsize,
    value: T,
}

/// A counted reference to a `T` that threads can share. See the module comment.
pub struct Arc<T: ?Sized> {
    ptr: NonNull<Inner<T>>,
    owns: PhantomData<Inner<T>>,
}

// As `std::sync::Arc`: a handle moves to or is shared with another thread only if the value may be
// shared and dropped there.
unsafe impl<T: ?Sized + Sync + Send> Send for Arc<T> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for Arc<T> {}

impl<T> Arc<T> {
    #[inline]
    pub fn new(value: T) -> Arc<T> {
        let inner = Box::new(Inner { count: AtomicUsize::new(1), value });
        Arc { ptr: NonNull::from(Box::leak(inner)), owns: PhantomData }
    }
}

impl<T: ?Sized> Arc<T> {
    #[inline]
    fn inner(&self) -> &Inner<T> {
        // The handle keeps the allocation alive.
        unsafe { self.ptr.as_ref() }
    }

    /// Whether `a` and `b` are handles of the same object (Java's `==`).
    #[inline]
    pub fn ptr_eq(a: &Arc<T>, b: &Arc<T>) -> bool {
        std::ptr::addr_eq(a.ptr.as_ptr(), b.ptr.as_ptr())
    }

    /// The address of the value, which identifies the object while it lives.
    #[inline]
    pub fn as_ptr(this: &Arc<T>) -> *const T {
        &this.inner().value
    }

    /// How many handles there are of the object.
    #[inline]
    pub fn strong_count(this: &Arc<T>) -> usize {
        this.inner().count.load(Ordering::Acquire)
    }

    /// Always 0: there are no weak references (kept for the shape of `std::sync::Arc`'s).
    #[inline]
    pub fn weak_count(_: &Arc<T>) -> usize {
        0
    }

    /// The value to change, when this is the only handle of it.
    #[inline]
    pub fn get_mut(this: &mut Arc<T>) -> Option<&mut T> {
        if this.inner().count.load(Ordering::Acquire) == 1 {
            // The only handle, borrowed mutably: nothing else can reach the value.
            Some(unsafe { &mut this.ptr.as_mut().value })
        } else {
            None
        }
    }

    #[cold]
    #[inline(never)]
    fn drop_slow(&mut self) {
        // The last handle: nothing else can reach the allocation.
        drop(unsafe { Box::from_raw(self.ptr.as_ptr()) });
    }
}

impl<T: ?Sized> Clone for Arc<T> {
    #[inline]
    fn clone(&self) -> Arc<T> {
        count_up(&self.inner().count);
        Arc { ptr: self.ptr, owns: PhantomData }
    }
}

impl<T: ?Sized> Drop for Arc<T> {
    #[inline]
    fn drop(&mut self) {
        if count_down(&self.inner().count) {
            self.drop_slow();
        }
    }
}

// The count changes are not generic, so each is compiled once rather than once per class, and only
// the plain path is inlined where a handle is copied or dropped: there are hundreds of thousands of
// such places in a large program.

/// One more handle.
#[inline]
fn count_up(count: &AtomicUsize) {
    if alone() {
        let old = count.load(Ordering::Relaxed);
        if old > MAX_REFS {
            std::process::abort();
        }
        count.store(old + 1, Ordering::Relaxed);
    } else {
        count_up_atomic(count);
    }
}

#[cold]
#[inline(never)]
fn count_up_atomic(count: &AtomicUsize) {
    if count.fetch_add(1, Ordering::Relaxed) > MAX_REFS {
        std::process::abort();
    }
}

/// One handle fewer: whether it was the last one, whose value is to be freed.
#[inline]
fn count_down(count: &AtomicUsize) -> bool {
    if alone() {
        let old = count.load(Ordering::Relaxed);
        count.store(old - 1, Ordering::Relaxed);
        old == 1
    } else {
        count_down_atomic(count)
    }
}

#[cold]
#[inline(never)]
fn count_down_atomic(count: &AtomicUsize) -> bool {
    if count.fetch_sub(1, Ordering::Release) != 1 {
        return false;
    }
    // Every other handle's changes to the value happen before it is freed.
    fence(Ordering::Acquire);
    true
}

impl<T: ?Sized> Deref for Arc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.inner().value
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for Arc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    struct Noted<'a>(&'a AtomicBool);

    impl Drop for Noted<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn the_value_is_freed_with_its_last_handle() {
        let freed = AtomicBool::new(false);
        let a = Arc::new(Noted(&freed));
        let b = a.clone();
        assert_eq!(Arc::strong_count(&a), 2);
        assert!(Arc::ptr_eq(&a, &b));
        drop(a);
        assert!(!freed.load(Ordering::SeqCst));
        drop(b);
        assert!(freed.load(Ordering::SeqCst));
    }

    #[test]
    fn only_the_last_handle_changes_the_value() {
        let mut a = Arc::new(1);
        let b = a.clone();
        assert!(Arc::get_mut(&mut a).is_none());
        drop(b);
        *Arc::get_mut(&mut a).unwrap() += 1;
        assert_eq!(*a, 2);
    }

    #[test]
    fn threads_copy_and_drop_handles_of_one_object() {
        crate::locked::begin_threads();
        let a = Arc::new(7);
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let a = a.clone();
                std::thread::spawn(move || {
                    for _ in 0..10_000 {
                        drop(a.clone());
                    }
                    *a
                })
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap(), 7);
        }
        assert_eq!(Arc::strong_count(&a), 1);
    }
}
