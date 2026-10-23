//! Fields that never change once their object is built, read without borrowing the object.
//!
//! An object that several places reach is a `Locked` (or a `RefCell` in a program with one
//! thread), and a read of one of its fields borrows the whole object: `this.borrow().syms`
//! takes the lock, which the end of the statement gives back, and a read whose borrow must end
//! sooner copies the field's value out. A field that only the constructors of its object write,
//! through `this` (Java's `final` fields, and others written the same way), holds the same value
//! from the end of the constructor on, so a read of it needs neither: `java::fixed!(this, syms)`
//! is the field itself, borrowed for as long as the handle `this` is, as `&*this.borrow()` would be
//! if the borrow did not hold the lock.
//!
//! That is sound because nothing writes the field while such a borrow lives. A constructor writes
//! it before any code reads it through a borrow that could still be alive: Java forbids reading a
//! `final` field before the constructor assigns it, and a method that the constructor calls and
//! that reads the field (and sees null) returns before the constructor goes on. Between threads,
//! the object reaches another thread only through a field or an element of an object or a static,
//! each written and read under a lock, or as an argument of `Thread.start`; either orders the
//! constructor's writes before the other thread's reads, as Java's `final` field semantics do.
//!
//! The borrow is of the field alone (`&raw const (*data).syms`), not of the whole data, so the
//! borrow of another of the object's fields to change it does not alias it.

use crate::Locked;
use std::cell::RefCell;
use std::rc::Rc;

/// The data of an object behind its cell, which a fixed field's read reaches without a borrow.
pub trait FixedFields {
    type Data: ?Sized;

    /// The address of the object's data.
    fn data(&self) -> *const Self::Data;

    /// The field that `field` projects from the data's address, borrowed as long as the cell is.
    /// Called only by `fixed!`, with a field that its object's constructors alone write.
    #[inline(always)]
    fn fixed_field<U: ?Sized>(&self, field: impl FnOnce(*const Self::Data) -> *const U) -> &U {
        // The field lives as long as the cell, and nothing writes it once its object is built
        // (see the module comment).
        unsafe { &*field(self.data()) }
    }
}

impl<T: ?Sized> FixedFields for Locked<T> {
    type Data = T;

    #[inline(always)]
    fn data(&self) -> *const T {
        self.as_ptr()
    }
}

impl<T: ?Sized> FixedFields for RefCell<T> {
    type Data = T;

    #[inline(always)]
    fn data(&self) -> *const T {
        self.as_ptr()
    }
}

/// A handle of an object whose data a fixed field's read reaches: `Rc`, `java::Arc`, or a
/// reference to one, as the receiver of `.borrow()` is.
pub trait FixedHandle {
    type Cell: FixedFields + ?Sized;

    /// The object's cell.
    fn cell(&self) -> &Self::Cell;
}

impl<C: FixedFields> FixedHandle for crate::Arc<C> {
    type Cell = C;

    #[inline(always)]
    fn cell(&self) -> &C {
        self
    }
}

impl<C: FixedFields + ?Sized> FixedHandle for Rc<C> {
    type Cell = C;

    #[inline(always)]
    fn cell(&self) -> &C {
        self
    }
}

impl<H: FixedHandle + ?Sized> FixedHandle for &H {
    type Cell = H::Cell;

    #[inline(always)]
    fn cell(&self) -> &H::Cell {
        (**self).cell()
    }
}

/// `fixed!(object, field)` or `fixed!(object, base.field)`: the field of the object's data that
/// its constructors alone write, read without borrowing the object (see the module comment).
/// `object` is a handle of the object or a reference to one; a temporary one lives to the end of
/// the statement, as the `Ref` of `object.borrow().field` would.
#[macro_export]
macro_rules! fixed {
    ($object:expr, $($field:ident).+) => {
        (*$crate::FixedFields::fixed_field(
            $crate::FixedHandle::cell(&$object),
            |data| unsafe { &raw const (*data).$($field).+ },
        ))
    };
}

#[cfg(test)]
mod tests {
    use crate::Locked;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Base {
        name: String,
    }

    struct Node {
        base: Base,
        count: i32,
    }

    #[test]
    fn a_fixed_field_is_read_while_another_field_changes() {
        let node = Rc::new(Locked::new(Node { base: Base { name: "a".to_string() }, count: 0 }));
        let name = &crate::fixed!(node, base.name);
        node.borrow_mut().count += 1;
        assert_eq!(name, "a");
        assert_eq!(node.borrow().count, 1);
    }

    #[test]
    fn a_fixed_field_of_a_ref_cell_is_read_while_the_cell_is_borrowed_to_change() {
        let node = Rc::new(RefCell::new(Node { base: Base { name: "b".to_string() }, count: 0 }));
        let held = node.borrow_mut();
        assert_eq!(crate::fixed!(&node, base.name).len(), 1);
        drop(held);
    }

    #[test]
    fn a_fixed_field_of_a_temporary_handle_lives_to_the_end_of_the_statement() {
        let make = || crate::Arc::new(Locked::new(Node { base: Base { name: "c".to_string() }, count: 0 }));
        assert_eq!(crate::fixed!(make(), base.name).as_str(), "c");
    }
}
