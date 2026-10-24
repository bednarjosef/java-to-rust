//! `java::Arc<T>`: the handle of an object in the collected heap (`gc.rs`).
//!
//! A handle is a pointer to the value and nothing more: copying it copies the pointer and dropping
//! it does nothing. The collector frees the value once no handle, field or array element reaches
//! it, so there is no count to change and no code that frees a value when its last handle goes,
//! which made up about a third of the time of a program like javac and two thirds of its code.
//! The value is never dropped, as a Java object is never destroyed: memory it owned that Rust
//! would have freed in its drop is found unreachable by the collector too.
//!
//! There are no weak references, and the value is sized: a handle of a `dyn` type could not be
//! made from one of a sized type anyway (that needs the unstable `CoerceUnsized`), so closures of
//! functional interfaces that threads share stay `std::sync::Arc<dyn Fn ..>`.

use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

/// A reference to a `T` in the collected heap. See the module comment.
pub struct Arc<T> {
    ptr: NonNull<T>,
    owns: PhantomData<T>,
}

// As `std::sync::Arc`: a handle moves to or is shared with another thread only if the value may be
// shared there.
unsafe impl<T: Sync + Send> Send for Arc<T> {}
unsafe impl<T: Sync + Send> Sync for Arc<T> {}

impl<T> Arc<T> {
    #[inline]
    pub fn new(value: T) -> Arc<T> {
        if std::mem::size_of::<T>() == 0 {
            return Arc::new_zero_sized(value);
        }
        Arc { ptr: NonNull::from(Box::leak(Box::new(value))), owns: PhantomData }
    }

    /// A handle of a value of no size (an object without fields), which still needs an address
    /// of its own: Java's `==` tells two such objects apart.
    #[cold]
    fn new_zero_sized(value: T) -> Arc<T> {
        let layout =
            std::alloc::Layout::from_size_align(1, std::mem::align_of::<T>()).expect("a valid layout");
        let p = unsafe { std::alloc::alloc(layout) } as *mut T;
        let Some(ptr) = NonNull::new(p) else { std::alloc::handle_alloc_error(layout) };
        // Writing a value of no size writes nothing, but moves it into place.
        unsafe { ptr.as_ptr().write(value) };
        Arc { ptr, owns: PhantomData }
    }

    /// Whether `a` and `b` are handles of the same object (Java's `==`).
    #[inline]
    pub fn ptr_eq(a: &Arc<T>, b: &Arc<T>) -> bool {
        std::ptr::addr_eq(a.ptr.as_ptr(), b.ptr.as_ptr())
    }

    /// The address of the value, which identifies the object while it lives.
    #[inline]
    pub fn as_ptr(this: &Arc<T>) -> *const T {
        this.ptr.as_ptr()
    }

    /// How many handles there may be of the object: handles are not counted, so always more than
    /// one, which keeps every caller on the path for a shared object.
    #[inline]
    pub fn strong_count(_: &Arc<T>) -> usize {
        2
    }

    /// Always 0: there are no weak references (kept for the shape of `std::sync::Arc`'s).
    #[inline]
    pub fn weak_count(_: &Arc<T>) -> usize {
        0
    }

    /// Always `None`: with handles not counted, no handle is known to be the only one.
    #[inline]
    pub fn get_mut(_: &mut Arc<T>) -> Option<&mut T> {
        None
    }
}

impl<T> Clone for Arc<T> {
    #[inline(always)]
    fn clone(&self) -> Arc<T> {
        Arc { ptr: self.ptr, owns: PhantomData }
    }
}

/// An enum of handles whose first `HANDLES` variants each hold one `Arc` (of any `T`): the enum
/// of a class or interface of the translation, with the variants of its classes first. A copy of
/// one of those variants is a copy of the handle, whichever class it is, so `copy_handle_variant`
/// makes it with no `match` over the classes.
///
/// # Safety
///
/// The enum is `#[repr(u16)]`, so each variant is laid out as a `#[repr(C)]` struct of its
/// discriminant and its field, and its discriminants are those of the declaration order (no
/// explicit ones). Each of its first `HANDLES` variants has exactly one field, an `Arc` of a sized
/// type.
pub unsafe trait HandleVariants: Sized {
    const HANDLES: u16;
}

/// `handle_variants!(Enum, n)`: the enum's first `n` variants hold one `Arc` each, as
/// `HandleVariants` requires. The generated code declares it with the enum.
#[macro_export]
macro_rules! handle_variants {
    ($enum:ty, $handles:expr) => {
        // The translator writes the enum `#[repr(u16)]` with the handles' variants first.
        unsafe impl $crate::HandleVariants for $enum {
            const HANDLES: u16 = $handles;
        }
    };
}

/// A copy of `e` when it is one of the variants that hold an `Arc`: the enum's bits copied, which
/// copies the handle. `None` for any other variant, which the enum's `Clone` copies by its `match`.
#[inline(always)]
pub fn copy_handle_variant<E: HandleVariants>(e: &E) -> Option<E> {
    let at = e as *const E as *const u8;
    // A `#[repr(u16)]` enum starts with its discriminant.
    let tag = unsafe { *(at as *const u16) };
    if tag >= E::HANDLES {
        return None;
    }
    // The variant holds one handle, which is a pointer with no count.
    Some(unsafe { std::ptr::read(e) })
}

impl<T> Deref for Arc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // The collector keeps the value while a handle of it is reachable.
        unsafe { self.ptr.as_ref() }
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

    #[repr(u16)]
    enum Value {
        Small(Arc<u8>),
        Text(Arc<String>),
        Plain(u8),
    }

    crate::handle_variants!(Value, 2);

    #[test]
    fn a_handle_variant_is_copied_as_its_handle() {
        let text = Arc::new("t".to_string());
        let value = Value::Text(text.clone());
        let Some(Value::Text(copy)) = copy_handle_variant(&value) else { panic!("a copy of the handle") };
        assert!(Arc::ptr_eq(&copy, &text));
        let small = Value::Small(Arc::new(7));
        let Some(Value::Small(copy)) = copy_handle_variant(&small) else { panic!("a copy of the handle") };
        assert_eq!(*copy, 7);
        let plain = Value::Plain(1);
        assert!(copy_handle_variant(&plain).is_none());
        let Value::Plain(n) = plain else { panic!("the same variant") };
        assert_eq!(n, 1);
    }

    #[test]
    fn objects_of_no_size_have_addresses_of_their_own() {
        let (a, b) = (Arc::new(()), Arc::new(()));
        assert!(!Arc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&a, &a.clone()));
    }

    #[test]
    fn no_handle_is_known_to_be_the_only_one() {
        let mut a = Arc::new(1);
        assert!(Arc::get_mut(&mut a).is_none());
        assert!(Arc::strong_count(&a) > 1);
    }
}
