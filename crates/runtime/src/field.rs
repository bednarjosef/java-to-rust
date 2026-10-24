//! `java::Field<T>`: a field of an object that the program changes after building it, read and
//! written by value through the object's address, as Java does (docs/object-model.md, 1.2).
//!
//! An object whose fields change is shared by every reference to it, so a field is a cell: a
//! read copies its value out (`get`), a write puts a new one in (`set`). No reference into the
//! cell is ever handed out, so two accesses cannot conflict the way two borrows of a `RefCell`
//! can, and there is no borrow flag to keep. The values are handles, primitives and strings,
//! whose clones touch nothing but their own bits or bytes, so a read never runs code that could
//! write the field it reads.
//!
//! Only the objects of one thread hold `Field`s (it is not `Sync`); those that threads share keep
//! their `java::Locked`.

use std::cell::UnsafeCell;
use std::fmt;

#[repr(transparent)]
pub struct Field<T>(UnsafeCell<T>);

impl<T> Field<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Field<T> {
        Field(UnsafeCell::new(value))
    }

    /// A copy of the value.
    #[inline(always)]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        // No reference into the cell outlives a call of `get` or `set`, and cloning the value
        // does not reach the cell (see the module comment).
        unsafe { (*self.0.get()).clone() }
    }

    /// Replaces the value.
    #[inline(always)]
    pub fn set(&self, value: T) {
        // As in `get`: nothing else reads or writes the cell meanwhile.
        let old = unsafe { std::ptr::replace(self.0.get(), value) };
        drop(old);
    }

    /// Replaces the value, returning the old one.
    #[inline(always)]
    pub fn replace(&self, value: T) -> T {
        // As in `get`.
        unsafe { std::ptr::replace(self.0.get(), value) }
    }

    /// The value of a cell that nothing else reaches.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: Default> Default for Field<T> {
    fn default() -> Field<T> {
        Field::new(T::default())
    }
}

/// A copy of the object copies each field's value (`Object.clone()`).
impl<T: Clone> Clone for Field<T> {
    fn clone(&self) -> Field<T> {
        Field::new(self.get())
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for Field<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_is_read_and_written_by_value() {
        let f = Field::new(3);
        f.set(f.get() + 4);
        assert_eq!(f.get(), 7);
        assert_eq!(f.replace(1), 7);
        let s = Field::new(String::from("a"));
        s.set(s.get() + "b");
        assert_eq!(s.get(), "ab");
        assert_eq!(s.clone().into_inner(), "ab");
    }
}
