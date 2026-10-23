//! `java::Arc<T>`: the handle of an object that threads can reach, as `std::sync::Arc`, whose
//! count changes take plain loads and stores until the process starts a second thread.
//!
//! A program that may start a thread holds most of its objects in `Arc`s, and copying and dropping
//! a handle is most of what such a program does: `std::sync::Arc` makes each copy and drop an
//! atomic read-modify-write, which costs a store-buffer drain every time even when only one thread
//! runs. javac starts its one thread only when a file manager closes in the background, which a
//! normal compilation never does; plain counts make it about an eighth faster.
//!
//! The count is an `AtomicU32`, in a header before the value that also names the function that
//! frees the allocation (`Inner`). While only one thread runs (`locked::alone`), a copy loads it and
//! stores it plus one, with relaxed orderings, which compile to a plain increment. That is sound
//! for the same reason as the locks' plain path (`locked.rs`, `THREADS`): the flag turns on only in
//! the thread that is about to spawn the second thread, before it does, so a thread that reads it
//! off is the only thread there is, and the spawn orders every plain store before anything the new
//! thread does. From then on every change is atomic, as `std::sync::Arc`'s.
//!
//! There are no weak references, and the value is sized: a handle of a `dyn` type could not be
//! made from one of a sized type anyway (that needs the unstable `CoerceUnsized`), so closures of
//! functional interfaces that threads share stay `std::sync::Arc<dyn Fn ..>`.

use crate::locked::alone;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering, fence};

/// More references than this abort the process, as `std::sync::Arc` does: counting on would wrap.
const MAX_REFS: u32 = i32::MAX as u32;

// The header comes first in every handle's allocation, whatever `T` is, so that `clone_counted`
// finds the count and the last handle's drop finds the function that frees the allocation
// without knowing `T`: the drop of a handle is then the same code for every class, and the
// arms of an enum's drop over the classes that threads share are all alike.
#[repr(C)]
struct Inner<T> {
    count: AtomicU32,
    /// The address of `free::<T>`, less that of `anchor`: the code is far smaller than 2 GB, so
    /// it fits in the half of the header's word that the count leaves.
    free: i32,
    value: T,
}

/// A counted reference to a `T` that threads can share. See the module comment.
pub struct Arc<T> {
    ptr: NonNull<Inner<T>>,
    owns: PhantomData<Inner<T>>,
}

// As `std::sync::Arc`: a handle moves to or is shared with another thread only if the value may be
// shared and dropped there.
unsafe impl<T: Sync + Send> Send for Arc<T> {}
unsafe impl<T: Sync + Send> Sync for Arc<T> {}

impl<T> Arc<T> {
    #[inline]
    pub fn new(value: T) -> Arc<T> {
        let free = (free::<T> as unsafe fn(NonNull<()>) as usize).wrapping_sub(anchor as fn() as usize);
        let free = i32::try_from(free as isize).expect("the code is smaller than 2 GB");
        let inner = Box::new(Inner { count: AtomicU32::new(1), free, value });
        Arc { ptr: NonNull::from(Box::leak(inner)), owns: PhantomData }
    }
}

impl<T> Arc<T> {
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
        this.inner().count.load(Ordering::Acquire) as usize
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
}

impl<T> Clone for Arc<T> {
    #[inline]
    fn clone(&self) -> Arc<T> {
        count_up(&self.inner().count);
        Arc { ptr: self.ptr, owns: PhantomData }
    }
}

impl<T> Drop for Arc<T> {
    #[inline]
    fn drop(&mut self) {
        if count_down(&self.inner().count) {
            free_last(self.ptr.cast());
        }
    }
}

/// The address that `Inner::free` is relative to.
#[inline(never)]
fn anchor() {}

/// Frees the allocation of `Inner<T>` at `inner`, whose last handle was dropped.
unsafe fn free<T>(inner: NonNull<()>) {
    // The last handle: nothing else can reach the allocation.
    drop(unsafe { Box::from_raw(inner.cast::<Inner<T>>().as_ptr()) });
}

/// Frees the allocation whose last handle was dropped by the function its header names, so that
/// the drop of a handle is not generic.
#[cold]
#[inline(never)]
fn free_last(inner: NonNull<Inner<()>>) {
    // The header is the same for every `T` (`Inner` is `repr(C)`), and its offset was made from
    // `free::<T>` of the allocation's own `T`.
    unsafe {
        let offset = (*inner.as_ptr()).free as isize;
        let address = (anchor as fn() as usize).wrapping_add(offset as usize);
        let free: unsafe fn(NonNull<()>) = std::mem::transmute(address);
        free(inner.cast());
    }
}

// The count changes are not generic, so each is compiled once rather than once per class, and only
// the plain path is inlined where a handle is copied or dropped: there are hundreds of thousands of
// such places in a large program.

/// One more handle. Inlined even into the largest functions, the copies of `Object` that match
/// on every class, where a call would cost more than the change it makes.
#[inline(always)]
fn count_up(count: &AtomicU32) {
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
fn count_up_atomic(count: &AtomicU32) {
    if count.fetch_add(1, Ordering::Relaxed) > MAX_REFS {
        std::process::abort();
    }
}

/// One handle fewer: whether it was the last one, whose value is to be freed.
#[inline(always)]
fn count_down(count: &AtomicU32) -> bool {
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
fn count_down_atomic(count: &AtomicU32) -> bool {
    if count.fetch_sub(1, Ordering::Release) != 1 {
        return false;
    }
    // Every other handle's changes to the value happen before it is freed.
    fence(Ordering::Acquire);
    true
}

/// An enum of handles whose first `COUNTED` variants each hold one `Arc` (of any `T`): the enum
/// of a class or interface of the translation, with the variants of its classes first. A copy of
/// one of those variants is a copy of the handle, whichever class it is, so `clone_counted`
/// makes it with one count change and no `match` over the classes.
///
/// # Safety
///
/// The enum is `#[repr(u16)]`, so each variant is laid out as a `#[repr(C)]` struct of its
/// discriminant and its field, and its discriminants are those of the declaration order (no
/// explicit ones). Each of its first `COUNTED` variants has exactly one field, an `Arc` of a sized
/// type.
pub unsafe trait CountedHandles: Sized {
    const COUNTED: u16;
}

/// `counted_handles!(Enum, n)`: the enum's first `n` variants hold one `Arc` each, as
/// `CountedHandles` requires. The generated code declares it with the enum.
#[macro_export]
macro_rules! counted_handles {
    ($enum:ty, $counted:expr) => {
        // The translator writes the enum `#[repr(u16)]` with the handles' variants first.
        unsafe impl $crate::CountedHandles for $enum {
            const COUNTED: u16 = $counted;
        }
    };
}

/// A copy of `e` when it is one of the variants that hold an `Arc`: one more count on the
/// handle, and the enum's bits copied. `None` for any other variant, which the enum's `Clone`
/// copies by its `match`.
#[inline(always)]
pub fn clone_counted<E: CountedHandles>(e: &E) -> Option<E> {
    let at = e as *const E as *const u8;
    // A `#[repr(u16)]` enum starts with its discriminant.
    let tag = unsafe { *(at as *const u16) };
    if tag >= E::COUNTED {
        return None;
    }
    // The variant's one field, a handle, follows the discriminant at the handle's alignment,
    // and the count is the first field of the allocation it points to (`Inner` is `repr(C)`).
    let count = unsafe { *(at.add(std::mem::align_of::<NonNull<()>>()) as *const *const AtomicU32) };
    count_up(unsafe { &*count });
    // The handle now counts one more owner: the copy.
    Some(unsafe { std::ptr::read(e) })
}

impl<T> Deref for Arc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.inner().value
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Arc<T> {
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

    #[repr(u16)]
    enum Value {
        Small(Arc<u8>),
        Text(Arc<String>),
        Plain(u8),
    }

    crate::counted_handles!(Value, 2);

    #[test]
    fn a_counted_variant_is_copied_with_one_more_count() {
        let text = Arc::new("t".to_string());
        let value = Value::Text(text.clone());
        let Some(Value::Text(copy)) = clone_counted(&value) else { panic!("a copy of the handle") };
        assert!(Arc::ptr_eq(&copy, &text));
        assert_eq!(Arc::strong_count(&text), 3);
        drop(value);
        drop(copy);
        assert_eq!(Arc::strong_count(&text), 1);
        let small = Value::Small(Arc::new(7));
        let Some(Value::Small(copy)) = clone_counted(&small) else { panic!("a copy of the handle") };
        assert_eq!(*copy, 7);
        let plain = Value::Plain(1);
        assert!(clone_counted(&plain).is_none());
        let Value::Plain(n) = plain else { panic!("the same variant") };
        assert_eq!(n, 1);
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
