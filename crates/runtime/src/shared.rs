//! `java::Arc<T>`: the handle of an object in the collected heap (`gc.rs`).
//!
//! A handle is a pointer to the value and nothing more: copying it copies the pointer and dropping
//! it does nothing. The collector frees the value once no handle, field or array element reaches
//! it, so there is no count to change and no code that frees a value when its last handle goes,
//! which made up about a third of the time of a program like javac and two thirds of its code.
//! The value is never dropped, as a Java object is never destroyed: memory it owned that Rust
//! would have freed in its drop is found unreachable by the collector too.
//!
//! An object that never leaves the method that creates it may live in a `Slot` on that method's
//! stack instead (`Slot::put`): its handle is the same `Arc`, so the code that uses it does not
//! change.
//!
//! A handle is always strong (a Java weak reference holds its referent in a `java::WeakField`
//! instead, `weak.rs`), and the value is sized: a handle of a `dyn` type could not be
//! made from one of a sized type anyway (that needs the unstable `CoerceUnsized`), so closures of
//! functional interfaces that threads share stay `std::sync::Arc<dyn Fn ..>`.

use crate::thin::ADDRESS;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

/// A reference to a `T` in the collected heap. See the module comment.
#[repr(transparent)]
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
        // In a constant, so that a build without optimizations has no call of `new_zero_sized` for
        // a type with a size.
        if const { std::mem::size_of::<T>() == 0 } {
            return Arc::new_zero_sized(value);
        }
        // A slot of the thread's free list or span where there is one, taken inline (`take_slot`);
        // otherwise the global allocator's path, which refills them.
        let slot = crate::gc::take_slot::<T>() as *mut T;
        let Some(ptr) = NonNull::new(slot) else {
            let ptr = NonNull::from(Box::leak(Box::new(value)));
            crate::gc::note_type::<T>(ptr.as_ptr());
            return Arc { ptr, owns: PhantomData };
        };
        unsafe { ptr.as_ptr().write(value) };
        crate::gc::note_type::<T>(ptr.as_ptr());
        Arc { ptr, owns: PhantomData }
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
        crate::gc::note_type::<T>(ptr.as_ptr());
        Arc { ptr, owns: PhantomData }
    }

    /// The handle of a value that the caller made in a slot of the collected heap (an array,
    /// `array.rs`).
    ///
    /// # Safety
    ///
    /// `ptr` is a `T` in a slot of the collected heap.
    #[inline(always)]
    pub(crate) unsafe fn from_heap(ptr: *mut T) -> Arc<T> {
        Arc { ptr: unsafe { NonNull::new_unchecked(ptr) }, owns: PhantomData }
    }

    /// The handle of the object at `address`, which a 4-byte reference named (`compact.rs`).
    ///
    /// # Safety
    ///
    /// `address` is that of a live `T` that a handle was made for.
    #[inline(always)]
    pub(crate) unsafe fn from_address(address: usize) -> Arc<T> {
        Arc { ptr: unsafe { NonNull::new_unchecked(address as *mut T) }, owns: PhantomData }
    }

    /// Whether `a` and `b` are handles of the same object (Java's `==`).
    #[inline]
    pub fn ptr_eq(a: &Arc<T>, b: &Arc<T>) -> bool {
        std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
    }

    /// The address of the value, which identifies the object while it lives: the pointer without
    /// the class number that a handle lent by a `java::Thin` carries in its top bits.
    #[inline(always)]
    pub fn as_ptr(this: &Arc<T>) -> *const T {
        (this.ptr.as_ptr() as usize & ADDRESS) as *const T
    }

    /// How many handles there may be of the object: handles are not counted, so always more than
    /// one, which keeps every caller on the path for a shared object.
    #[inline]
    pub fn strong_count(_: &Arc<T>) -> usize {
        2
    }

    /// Always 0: a handle is never weak (kept for the shape of `std::sync::Arc`'s).
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

/// A place on the stack of a method for one object that never leaves the method (docs/collector.md,
/// "Objects on the stack"). The translation declares one at the start of the method for each
/// `new`, or call of a method that returns a new object, whose object stays in it, and `put`
/// makes the object there (the called method is given the slot to do so).
///
/// The collector scans the stack word by word, so the objects in the heap that the object's
/// fields name stay alive while the slot does. The collector itself ignores the address, which is
/// outside its heap, and nothing in the heap can name the object, because the analysis that
/// chose the slot proved that no reference to it is stored anywhere but in the method's
/// variables, nor kept past the method's end.
///
/// The slot has at least one byte, so that two slots of objects without fields have addresses of
/// their own, which Java's `==` tells apart.
#[repr(C)]
pub struct Slot<T> {
    value: std::cell::UnsafeCell<std::mem::MaybeUninit<T>>,
    _byte: u8,
}

impl<T> Slot<T> {
    #[inline(always)]
    pub const fn new() -> Slot<T> {
        Slot { value: std::cell::UnsafeCell::new(std::mem::MaybeUninit::uninit()), _byte: 0 }
    }

    /// Makes `value` the object in the slot, and returns its handle. An object put there before
    /// is overwritten without being dropped, as a Java object is never destroyed: the
    /// translation puts a new object in a slot again (in a loop) only when the one before can no
    /// longer be reached.
    #[inline(always)]
    pub fn put(&self, value: T) -> Arc<T> {
        let place = self.value.get();
        // The slot is borrowed shared, and no reference to its value is lent out (`Arc` reads
        // through a raw pointer), so writing through the cell's pointer aliases nothing.
        unsafe { (*place).write(value) };
        let ptr = unsafe { NonNull::new_unchecked(place as *mut T) };
        Arc { ptr, owns: PhantomData }
    }
}

impl<T> Default for Slot<T> {
    fn default() -> Slot<T> {
        Slot::new()
    }
}

impl<T> Clone for Arc<T> {
    #[inline(always)]
    fn clone(&self) -> Arc<T> {
        Arc { ptr: self.ptr, owns: PhantomData }
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // The collector keeps the value while a handle of it is reachable.
        unsafe { &*Arc::as_ptr(self) }
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

    #[test]
    fn objects_of_no_size_have_addresses_of_their_own() {
        let (a, b) = (Arc::new(()), Arc::new(()));
        assert!(!Arc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&a, &a.clone()));
    }

    #[test]
    fn objects_in_slots_are_reached_through_their_handles() {
        let slot = Slot::new();
        let a = slot.put(std::cell::Cell::new(3));
        a.set(4);
        assert_eq!(a.get(), 4);
        let b = slot.put(std::cell::Cell::new(5));
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(b.get(), 5);
        let (x, y) = (Slot::new(), Slot::new());
        assert!(!Arc::ptr_eq(&x.put(()), &y.put(())));
    }

    #[test]
    fn no_handle_is_known_to_be_the_only_one() {
        let mut a = Arc::new(1);
        assert!(Arc::get_mut(&mut a).is_none());
        assert!(Arc::strong_count(&a) > 1);
    }
}
