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
            let ptr = crate::gc::alloc_object(std::alloc::Layout::new::<T>()) as *mut T;
            unsafe { ptr.write(value) };
            let ptr = unsafe { NonNull::new_unchecked(ptr) };
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

    /// The address of the value, which identifies the object while it lives. A handle taken out
    /// of a `java::Thin` has its class number cleared (`Thin::to_pointer`), so this is the
    /// pointer as it is.
    #[inline(always)]
    pub fn as_ptr(this: &Arc<T>) -> *const T {
        debug_assert!(
            this.ptr.as_ptr() as usize & !ADDRESS == 0,
            "a handle with a class number in its top bits"
        );
        this.ptr.as_ptr()
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

    /// Moves the object in the slot to the collected heap when `handle` names it, and makes
    /// `handle` name the moved object, keeping the class number in its top bits; does nothing
    /// when `handle` names anything else, or nothing. The translation calls it before a statement
    /// where an object that it made in the slot may leave its method (docs/collector.md,
    /// "Objects on the stack"): nothing else names the object then, so the moved object is the
    /// object from then on. The old one is left in the slot as it was, not dropped.
    ///
    /// # Safety
    ///
    /// `H` is a handle of the slot's objects or of a supertype's: a `java::Arc<T>`, a struct
    /// around a `java::Thin`, or an `Option` of one, and no other handle of the object in the slot
    /// is used after the call.
    ///
    /// The handle's new word comes back from the out-of-line move (`move_out`), which never sees
    /// where the handle is: had it been given the handle's address, the local holding the handle
    /// would live in memory for the whole method, as a place that an unknown function may keep,
    /// and each use of it after any atomic load (a field's acquire read) would load it again and
    /// clear its class number again, where the local in a register is read once.
    #[inline(always)]
    pub unsafe fn to_heap<H>(&self, handle: &mut H) {
        const { assert!(std::mem::size_of::<H>() == std::mem::size_of::<usize>()) };
        // A handle is one word: the address, with a class number in the top bits in a `Thin`, or
        // 0 for `None`.
        let word = unsafe { std::ptr::read(handle as *const H as *const usize) };
        if word & ADDRESS == self.value.get() as usize {
            let moved = unsafe { self.move_out(word) };
            unsafe { std::ptr::write(handle as *mut H as *mut usize, moved) };
        }
    }

    /// The word of the handle `word` of the slot's object once the object is moved to the heap:
    /// the moved object's address, with `word`'s class number.
    #[cold]
    #[inline(never)]
    unsafe fn move_out(&self, word: usize) -> usize {
        let moved = Arc::new(unsafe { std::ptr::read((*self.value.get()).as_ptr()) });
        (word & !ADDRESS) | Arc::as_ptr(&moved) as usize
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
    fn objects_move_from_their_slots_once() {
        let slot = Slot::new();
        let mut a = slot.put(std::cell::Cell::new(3));
        a.set(4);
        let on_stack = a.clone();
        unsafe { slot.to_heap(&mut a) };
        assert!(!Arc::ptr_eq(&a, &on_stack));
        assert_eq!(a.get(), 4);
        let moved = a.clone();
        unsafe { slot.to_heap(&mut a) };
        assert!(Arc::ptr_eq(&a, &moved));
        let mut none: Option<Arc<std::cell::Cell<i32>>> = None;
        unsafe { slot.to_heap(&mut none) };
        assert!(none.is_none());
        let mut held = Some(slot.put(std::cell::Cell::new(5)));
        unsafe { slot.to_heap(&mut held) };
        let held = held.expect("still a handle");
        assert!(!std::ptr::addr_eq(Arc::as_ptr(&held), Arc::as_ptr(&on_stack)));
        assert_eq!(held.get(), 5);
        // A handle with its class number in the top bits keeps it, and names the moved copy.
        let mut tagged = crate::Thin::pointer(7, slot.put(std::cell::Cell::new(6)));
        unsafe { slot.to_heap(&mut tagged) };
        assert_eq!(tagged.tag(), 7);
        let tagged: Arc<std::cell::Cell<i32>> = unsafe { tagged.to_pointer() };
        assert!(!std::ptr::addr_eq(Arc::as_ptr(&tagged), Arc::as_ptr(&on_stack)));
        assert_eq!(tagged.get(), 6);
    }

    #[test]
    fn no_handle_is_known_to_be_the_only_one() {
        let mut a = Arc::new(1);
        assert!(Arc::get_mut(&mut a).is_none());
        assert!(Arc::strong_count(&a) > 1);
    }
}
