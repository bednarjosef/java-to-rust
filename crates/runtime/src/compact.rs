//! References of 4 bytes in the fields of objects (docs/native-speed.md 1.1).
//!
//! The heap lies from 4 GB to 32 GB of the address space (`gc.rs`), and every object in it starts
//! at a multiple of 8 bytes, so an object's address divided by 8 fits in 4 bytes, and that is the
//! reference a field holds: reading it is a shift, which an x86-64 address folds in (`[r*8]`), and
//! there is no base to add. 0 is null, which no object's address is. A `java::Arc` stays the
//! form of a reference in locals, parameters and registers: 8 bytes, used with no decoding; a
//! field converts as it is read and written.
//!
//! `java::RefField` and `java::SharedRefField` are the cells of such fields, with the methods of
//! `java::Field` and `java::SharedField`, for the values that `Compress` names: a handle of one
//! class (`java::Arc`), a string with an identity (`java::JStr`), and `Option`s of them.
//!
//! The collector finds these references by reading every 4 bytes of a slot that, times 8, is the
//! start of a taken slot as one, in a program whose allocator is `java::CompactAllocator`
//! (`gc::compact_references`), and only in objects aligned to 8 bytes: it does not scan the blocks
//! of less aligned allocations (strings, buffers of numbers), so a struct with such a field is
//! `#[repr(align(8))]`. An object outside the heap (one in a `java::Slot` on a method's stack) is
//! never stored in such a field: the translation keeps those objects out of fields altogether.

use crate::{Arc, Field, JStr, SharedField};
use std::fmt;
use std::marker::PhantomData;

/// A value that a field holds in 4 bytes: a reference to an object in the heap, or null.
///
/// # Safety
///
/// `unpack(pack(v))` is `v`, `pack` of a value is never 0 unless the value is `None`, and the
/// value has no `Drop`.
pub unsafe trait Compress: Sized {
    /// The 4 bytes of the value.
    fn pack(&self) -> u32;

    /// The value of 4 bytes that `pack` made.
    ///
    /// # Safety
    ///
    /// `bits` came from `pack` of a value of this type.
    unsafe fn unpack(bits: u32) -> Self;
}

/// The 4 bytes of the reference to the object at `address`: the address divided by 8.
#[inline(always)]
fn bits_of(address: usize) -> u32 {
    // A handle that a `java::Thin` lends keeps a class number in its top 16 bits.
    let address = address & crate::thin::ADDRESS;
    // An address past the heap (an object on a method's stack) has no 4-byte reference: a check
    // that costs a shift and a branch never taken.
    if address >= crate::gc::COMPACT_END {
        not_in_heap(address);
    }
    (address >> 3) as u32
}

#[cold]
#[inline(never)]
fn not_in_heap(address: usize) -> ! {
    panic!("a 4-byte reference to an object outside the heap, at {address:#x}")
}

unsafe impl<T> Compress for Arc<T> {
    #[inline(always)]
    fn pack(&self) -> u32 {
        bits_of(Arc::as_ptr(self) as usize)
    }

    #[inline(always)]
    unsafe fn unpack(bits: u32) -> Arc<T> {
        unsafe { Arc::from_address((bits as usize) << 3) }
    }
}

unsafe impl Compress for JStr {
    #[inline(always)]
    fn pack(&self) -> u32 {
        bits_of(self.address())
    }

    #[inline(always)]
    unsafe fn unpack(bits: u32) -> JStr {
        unsafe { JStr::from_address((bits as usize) << 3) }
    }
}

/// `None` is 0.
unsafe impl<T: Compress> Compress for Option<T> {
    #[inline(always)]
    fn pack(&self) -> u32 {
        self.as_ref().map_or(0, T::pack)
    }

    #[inline(always)]
    unsafe fn unpack(bits: u32) -> Option<T> {
        (bits != 0).then(|| unsafe { T::unpack(bits) })
    }
}

/// A field of an object that one thread uses whose value is a reference held in 4 bytes: a
/// `java::Field` of the 4 bytes.
#[repr(transparent)]
pub struct RefField<T: Compress>(Field<u32>, PhantomData<T>);

impl<T: Compress> RefField<T> {
    #[inline(always)]
    pub fn new(value: T) -> RefField<T> {
        RefField(Field::new(value.pack()), PhantomData)
    }

    /// A copy of the value.
    #[inline(always)]
    pub fn get(&self) -> T {
        unsafe { T::unpack(self.0.get()) }
    }

    /// Replaces the value.
    #[inline(always)]
    pub fn set(&self, value: T) {
        self.0.set(value.pack())
    }

    /// Replaces the value, returning the old one.
    #[inline(always)]
    pub fn replace(&self, value: T) -> T {
        unsafe { T::unpack(self.0.replace(value.pack())) }
    }

    pub fn into_inner(self) -> T {
        unsafe { T::unpack(self.0.into_inner()) }
    }
}

/// Null.
impl<T: Compress> Default for RefField<Option<T>> {
    fn default() -> RefField<Option<T>> {
        RefField(Field::new(0), PhantomData)
    }
}

/// A copy of the object copies each field's value (`Object.clone()`).
impl<T: Compress> Clone for RefField<T> {
    fn clone(&self) -> RefField<T> {
        RefField(Field::new(self.0.get()), PhantomData)
    }
}

impl<T: Compress + fmt::Debug> fmt::Debug for RefField<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

/// A field of an object that threads share whose value is a reference held in 4 bytes: a
/// `java::SharedField` of the 4 bytes, an atomic word, loaded with acquire and stored with release
/// (sequentially consistent for a `volatile` field).
#[repr(transparent)]
pub struct SharedRefField<T: Compress>(SharedField<u32>, PhantomData<T>);

// As `java::SharedField`: every access is one atomic action on the 4 bytes.
unsafe impl<T: Compress + Send> Sync for SharedRefField<T> {}

impl<T: Compress> SharedRefField<T> {
    #[inline(always)]
    pub fn new(value: T) -> SharedRefField<T> {
        SharedRefField(SharedField::new(value.pack()), PhantomData)
    }

    /// A copy of the value.
    #[inline(always)]
    pub fn get(&self) -> T {
        unsafe { T::unpack(self.0.get()) }
    }

    /// Replaces the value.
    #[inline(always)]
    pub fn set(&self, value: T) {
        self.0.set(value.pack())
    }

    /// Replaces the value, returning the old one.
    pub fn replace(&self, value: T) -> T {
        unsafe { T::unpack(self.0.replace(value.pack())) }
    }

    /// `compareAndExchange` through `Unsafe`: the old value, and the field set to `value` if the
    /// old value is `same` as `expected`, in one atomic action.
    pub fn compare_exchange_by(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> T {
        let new = value.pack();
        loop {
            let bits = self.0.get();
            let old = unsafe { T::unpack(bits) };
            if !same(&old, &expected) || self.0.compare_exchange_by(bits, new, |a, b| a == b) == bits {
                return old;
            }
        }
    }

    /// `compareAndSet` through `Unsafe`: whether the old value was `same` as `expected`, and the
    /// field then set to `value`, in one atomic action.
    pub fn compare_and_set_by(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> bool {
        let expected_bits = expected.pack();
        let old = self.compare_exchange_by(expected, value, &same);
        same(&old, &unsafe { T::unpack(expected_bits) })
    }

    /// The old value, and the field set to `f` of it, in one atomic action.
    pub fn get_and_update(&self, f: impl Fn(&T) -> T) -> T {
        loop {
            let bits = self.0.get();
            let old = unsafe { T::unpack(bits) };
            let new = f(&old).pack();
            if self.0.compare_exchange_by(bits, new, |a, b| a == b) == bits {
                return old;
            }
        }
    }

    /// A copy of the value of a `volatile` field.
    #[inline(always)]
    pub fn get_volatile(&self) -> T {
        unsafe { T::unpack(self.0.get_volatile()) }
    }

    /// Replaces the value of a `volatile` field.
    #[inline(always)]
    pub fn set_volatile(&self, value: T) {
        self.0.set_volatile(value.pack())
    }

    /// A copy of the value of a field of an object that one thread alone uses: a plain read
    /// (`SharedField::get_confined`).
    #[inline(always)]
    pub fn get_confined(&self) -> T {
        unsafe { T::unpack(self.0.get_confined()) }
    }

    /// Replaces the value of a field of an object that one thread alone uses.
    #[inline(always)]
    pub fn set_confined(&self, value: T) {
        self.0.set_confined(value.pack())
    }

    /// Replaces the value of a field of an object that one thread alone uses, returning the old
    /// one.
    #[inline(always)]
    pub fn replace_confined(&self, value: T) -> T {
        unsafe { T::unpack(self.0.replace_confined(value.pack())) }
    }

    /// `compare_exchange_by` on a field of an object that one thread alone uses.
    #[inline(always)]
    pub fn compare_exchange_by_confined(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> T {
        let old = self.get_confined();
        if same(&old, &expected) {
            self.set_confined(value);
        }
        old
    }

    /// `compare_and_set_by` on a field of an object that one thread alone uses.
    #[inline(always)]
    pub fn compare_and_set_by_confined(&self, expected: T, value: T, same: impl Fn(&T, &T) -> bool) -> bool {
        let old = self.get_confined();
        let swapped = same(&old, &expected);
        if swapped {
            self.set_confined(value);
        }
        swapped
    }

    /// `get_and_update` on a field of an object that one thread alone uses.
    #[inline(always)]
    pub fn get_and_update_confined(&self, f: impl Fn(&T) -> T) -> T {
        let old = self.get_confined();
        self.set_confined(f(&old));
        old
    }

    pub fn into_inner(self) -> T {
        unsafe { T::unpack(self.0.into_inner()) }
    }
}

/// Null.
impl<T: Compress> Default for SharedRefField<Option<T>> {
    fn default() -> SharedRefField<Option<T>> {
        SharedRefField(SharedField::new(0), PhantomData)
    }
}

/// A copy of the object copies each field's value (`Object.clone()`).
impl<T: Compress> Clone for SharedRefField<T> {
    fn clone(&self) -> SharedRefField<T> {
        SharedRefField(SharedField::new(self.0.get()), PhantomData)
    }
}

impl<T: Compress + fmt::Debug> fmt::Debug for SharedRefField<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}
