//! `java::Thin`: a value of a type that objects of several classes can be (a class with
//! subclasses, an interface, `Object`), in eight bytes. The translation writes such a type as a
//! struct around a `Thin`, with a constructor and a `view()` per class (docs/object-model.md).
//!
//! The top 16 bits are the number of the value's class, which the translation gives every class
//! once for the whole program, so a value keeps its bits when it becomes a value of a supertype.
//! The 48 bits below are the address of the object, which is all a `java::Arc` holds, or a value
//! of six bytes or less (an `int`, a `char`, a constant of an enum), or else the address of a box
//! that holds the value. A class number is never 0, so a `Thin` is never 0, and `Option` of it
//! uses 0 for `None`.
//!
//! A `java::Arc` ignores the top 16 bits of its pointer (`ADDRESS`), so the `Thin` of a handle can
//! be read as the handle where it lies: `as_pointer` lends the handle for as long as the `Thin`
//! is borrowed, and the code of each class's arm uses it as it used the handle of an enum's
//! variant. A value in the low bytes is lent the same way, as x86-64 keeps the low bytes of a word
//! first. The collector ignores the top bits too when it scans memory for handles (`gc.rs`).

use crate::Arc;
use std::num::NonZeroU64;

/// The bits of a handle's pointer that are the address: the 48 of x86-64's user addresses.
pub const ADDRESS: usize = (1 << 48) - 1;

/// See the module comment.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Thin(NonZeroU64);

/// A type that is one pointer that ignores the top 16 bits: a handle, a `#[repr(transparent)]`
/// struct around one (a builder), or a string with an identity (`java::JStr`).
///
/// # Safety
///
/// The type is a `java::Arc<T>`, `#[repr(transparent)]` over a type that is, or one pointer that
/// masks its top 16 bits with `ADDRESS` wherever it reads through it, and it has no `Drop`.
pub unsafe trait Pointer: Sized {}

unsafe impl<T> Pointer for Arc<T> {}

impl Thin {
    /// The number of the value's class.
    #[inline(always)]
    pub fn tag(self) -> u16 {
        (self.0.get() >> 48) as u16
    }

    /// The address that the value holds: for a handle (`pointer`), its object's, which is
    /// `java::Arc::as_ptr` of the handle.
    #[inline(always)]
    pub fn address(self) -> *const u8 {
        (self.0.get() as usize & ADDRESS) as *const u8
    }

    /// The handle `p` of an object of class `tag`.
    #[inline(always)]
    pub fn pointer<P: Pointer>(tag: u16, p: P) -> Thin {
        // A `Pointer` is one pointer, and dropping it does nothing.
        let address = unsafe { std::mem::transmute_copy::<P, usize>(&p) } & ADDRESS;
        Thin::of(tag, address as u64)
    }

    /// The handle that the value holds, lent while the value is borrowed.
    ///
    /// # Safety
    ///
    /// The value was made by `pointer` with a handle of type `P`.
    #[inline(always)]
    pub unsafe fn as_pointer<P: Pointer>(&self) -> &P {
        // The handle's pointer is these bits, whose tag it ignores.
        unsafe { &*(self as *const Thin as *const P) }
    }

    /// The start of the object whose handle the value holds, read as a `T` and lent while the
    /// value is borrowed: the fields struct of a class tree, which every class of the tree keeps
    /// at offset 0 of its object, so an enum's `base()` reads it with no `match` on the class.
    ///
    /// # Safety
    ///
    /// The value was made by `pointer` with a `java::Arc<O>`, whose `O` holds a `T` at offset 0
    /// that `&O` lends as `&T` (a `#[repr(C)]` struct whose first field is the `T` or holds it at
    /// offset 0, or a `java::Monitored` of one).
    #[inline(always)]
    pub unsafe fn as_prefix<T>(&self) -> &T {
        // The handle's pointer is these bits without the tag, as `Arc`'s `Deref` reads it, and
        // the `T` lies where the `O` does, borrowed as `Deref` would lend the `O` for as long as
        // the handle is.
        unsafe { &*((self.0.get() as usize & ADDRESS) as *const T) }
    }

    /// The value `v` of class `tag`: in the low bytes when it fits in six, else in a box.
    #[inline(always)]
    pub fn value<T>(tag: u16, v: T) -> Thin {
        if Thin::fits::<T>() {
            let mut bits = 0u64;
            // It fits, and the bits are aligned for it.
            unsafe { std::ptr::write(&mut bits as *mut u64 as *mut T, v) };
            Thin::of(tag, bits)
        } else {
            Thin::pointer(tag, Arc::new(v))
        }
    }

    /// The value that the value holds, lent while it is borrowed.
    ///
    /// # Safety
    ///
    /// The value was made by `value` with a value of type `T`.
    #[inline(always)]
    pub unsafe fn as_value<T>(&self) -> &T {
        if Thin::fits::<T>() {
            // The value lies in the low bytes, which come first.
            unsafe { &*(self as *const Thin as *const T) }
        } else {
            unsafe { self.as_pointer::<Arc<T>>() }
        }
    }

    /// Whether a `T` is held in the low six bytes.
    const fn fits<T>() -> bool {
        std::mem::size_of::<T>() <= 6 && std::mem::align_of::<T>() <= 8
    }

    #[inline(always)]
    fn of(tag: u16, low: u64) -> Thin {
        debug_assert!(tag != 0 && low >> 48 == 0);
        // The tag is not 0.
        Thin(unsafe { NonZeroU64::new_unchecked((tag as u64) << 48 | low) })
    }
}

/// A value whose class number belongs to no class of its type: memory that the translation does
/// not write was read as one.
#[cold]
#[inline(never)]
pub fn bad_tag(tag: u16) -> ! {
    panic!("a reference with the class number {tag}, of no class of its type")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_is_lent_where_it_lies() {
        let text = Arc::new("t".to_string());
        let thin = Thin::pointer(7, text.clone());
        assert_eq!(thin.tag(), 7);
        let lent: &Arc<String> = unsafe { thin.as_pointer() };
        assert!(Arc::ptr_eq(lent, &text));
        assert_eq!(lent.as_str(), "t");
    }

    #[test]
    fn the_start_of_an_object_is_read_where_it_lies() {
        struct Root {
            name: &'static str,
        }
        #[repr(C)]
        struct Middle {
            base: Root,
            flag: bool,
        }
        #[repr(C)]
        struct Leaf {
            base: Middle,
            count: u64,
        }
        let leaf = Arc::new(Leaf { base: Middle { base: Root { name: "leaf" }, flag: true }, count: 3 });
        let thin = Thin::pointer(9, leaf.clone());
        let root: &Root = unsafe { thin.as_prefix() };
        assert_eq!(root.name, "leaf");
        assert!(std::ptr::eq(root, &leaf.base.base));
        let monitored =
            Arc::new(crate::Monitored::new(Middle { base: Root { name: "middle" }, flag: false }));
        let thin = Thin::pointer(10, monitored.clone());
        let root: &Root = unsafe { thin.as_prefix() };
        assert!(std::ptr::eq(root, &monitored.base));
        assert!(!monitored.flag && leaf.base.flag && leaf.count == 3);
    }

    #[test]
    fn small_values_are_held_and_large_ones_boxed() {
        let n = Thin::value(3, -5i32);
        assert_eq!(unsafe { *n.as_value::<i32>() }, -5);
        let b = Thin::value(4, false);
        assert!(!unsafe { *b.as_value::<bool>() });
        let d = Thin::value(5, -0.0f64);
        assert_eq!(n.tag(), 3);
        assert!(unsafe { d.as_value::<f64>() }.is_sign_negative());
        let s = Thin::value(6, String::from("long text"));
        assert_eq!(unsafe { s.as_value::<String>() }, "long text");
    }

    #[test]
    fn none_takes_no_room() {
        assert_eq!(std::mem::size_of::<Option<Thin>>(), 8);
    }
}
