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
//! `tag` reads the class number with one shift, and every test of a class (`matches!(o.0.tag(),
//! 2..=5)`, the `match` of a dispatcher, a cast) compares the number with 32-bit immediates:
//! `mov; shr $48; cmp $n; jne`, or a subtraction first for a range. The shift is written in
//! assembly that reads no memory, which the optimizer still merges with another shift of the same
//! bits and moves out of a loop, so a test that an earlier one decides is still dropped, but
//! cannot see into: written as Rust, LLVM turned the shift and compare into a mask of the whole
//! word and a compare with the class number shifted up, 64-bit constants that each take a 10-byte
//! load, and after an earlier test narrowed the mask so far that the code generator no longer
//! turned it back into a shift (on the Minecraft server, 80,000 tests as `and` with a mask, then
//! a constant load and a compare, and 100,000 range tests that add a 64-bit constant). The costs:
//! a test of a value that LLVM sees made with a known class is no longer folded (the translation
//! makes such tests rarely, as its own facts already name the class there), and the inliner
//! counts the shift as a call, so a function stops being inlined with somewhat fewer tests in it
//! (in a probe, at 12 tests rather than 16).
//! `tag_or_null` gives 0 for `None`, so that a test of a value that may be null is the same one
//! compare.
//!
//! `to_pointer` gives back the handle with the class number cleared from its top bits, so a
//! `java::Arc` is always the plain address and reading through one is a plain load: the
//! translation clears the bits once, where it takes the handle out of the `Thin` at a class's
//! arm, rather than at each read of the object, which on the Minecraft server had been one
//! instruction in fourteen. A value in the low bytes is lent where it lies, as x86-64 keeps the
//! low bytes of a word first. The collector ignores the top bits when it scans memory for handles
//! (`gc.rs`), as a field of an interface type holds a `Thin`.

use crate::Arc;
use std::num::NonZeroU64;

/// The bits of a handle's pointer that are the address: the 48 of x86-64's user addresses.
pub const ADDRESS: usize = (1 << 48) - 1;

/// See the module comment.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Thin(NonZeroU64);

/// A type that is one pointer: a handle, a `#[repr(transparent)]` struct around one (a builder),
/// or a string with an identity (`java::JStr`).
///
/// # Safety
///
/// The type is a `java::Arc<T>`, `#[repr(transparent)]` over a type that is, or one pointer whose
/// bits are an address of at most 48 bits, and it has no `Drop`.
pub unsafe trait Pointer: Sized {}

unsafe impl<T> Pointer for Arc<T> {}

impl Thin {
    /// The number of the value's class.
    #[inline(always)]
    pub fn tag(self) -> u16 {
        class_number(self.0.get())
    }

    /// The number of the class of `o`, or 0 for `None`, which no class has: a test of the number
    /// is then also the test for null, with no branch of its own.
    #[inline(always)]
    pub fn tag_or_null(o: Option<Thin>) -> u16 {
        // `None` is 0, so this is the value's bits.
        class_number(o.map_or(0, |t| t.0.get()))
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

    /// The handle that the value holds, with the class number cleared.
    ///
    /// # Safety
    ///
    /// The value was made by `pointer` with a handle of type `P`.
    #[inline(always)]
    pub unsafe fn to_pointer<P: Pointer>(self) -> P {
        // A `Pointer` is one pointer, and these bits are the one `pointer` was given.
        unsafe { std::mem::transmute_copy::<usize, P>(&(self.0.get() as usize & ADDRESS)) }
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
        // The handle's pointer is these bits without the tag, and the `T` lies where the `O`
        // does, borrowed as `Deref` would lend the `O` for as long as the handle is.
        unsafe { &*((self.0.get() as usize & ADDRESS) as *const T) }
    }

    /// The value `v` of class `tag`: in the low bytes when it fits in six, else in a box.
    #[inline(always)]
    pub fn value<T>(tag: u16, v: T) -> Thin {
        if const { Thin::fits::<T>() } {
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
        if const { Thin::fits::<T>() } {
            // The value lies in the low bytes, which come first.
            unsafe { &*(self as *const Thin as *const T) }
        } else {
            // The box's handle without the tag, read as `Arc<T>`'s `Deref` reads it.
            unsafe { self.as_prefix::<T>() }
        }
    }

    /// The ordinal of the enum constant that the value holds (`value` of a Rust enum without
    /// fields, as the translation writes a Java enum): such an enum's bits are its discriminant,
    /// which counts the constants from 0 in their order, and the bytes above it are zero.
    #[inline(always)]
    pub fn ordinal(self) -> i32 {
        (self.0.get() & ADDRESS as u64) as i32
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

/// The top 16 bits of `bits`, by one shift that the optimizer cannot see into (see the module
/// comment).
#[inline(always)]
fn class_number(bits: u64) -> u16 {
    #[cfg(target_arch = "x86_64")]
    {
        let mut n = bits;
        // A shift, which reads and writes no memory and leaves the flags.
        unsafe {
            std::arch::asm!("shr {n}, 48", n = inout(reg) n, options(pure, nomem, nostack, preserves_flags));
        }
        // Facts of the instruction, which the optimizer cannot see: what it leaves is below 2^16,
        // so a compare reads the 32 bits that hold it with no zero-extension first; and it is at
        // most the bits it shifted, so where the number is a class's (not 0), the bits are not
        // 0 either, and the value is known not null.
        unsafe { std::hint::assert_unchecked(n >> 16 == 0) };
        unsafe { std::hint::assert_unchecked(n <= bits) };
        n as u16
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        (bits >> 48) as u16
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
    fn a_handle_comes_back_without_its_class_number() {
        let text = Arc::new("t".to_string());
        let thin = Thin::pointer(7, text.clone());
        assert_eq!(thin.tag(), 7);
        let back: Arc<String> = unsafe { thin.to_pointer() };
        assert!(Arc::ptr_eq(&back, &text));
        assert_eq!(back.as_str(), "t");
    }

    #[test]
    fn the_class_number_is_the_top_sixteen_bits_and_null_is_zero() {
        for tag in [1, 2, 0x7b60, 0x8000, u16::MAX] {
            let thin = Thin::value(tag, u32::MAX);
            assert_eq!(thin.tag(), tag);
            assert_eq!(Thin::tag_or_null(Some(thin)), tag);
            assert_eq!(thin.ordinal(), -1);
        }
        assert_eq!(Thin::tag_or_null(None), 0);
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
    fn an_enum_constant_is_its_ordinal_and_its_bits_say_which() {
        #[derive(Clone, Copy, PartialEq)]
        enum Small {
            A,
            B,
            C,
        }
        let few = [Small::A, Small::B, Small::C];
        for (ordinal, &constant) in few.iter().enumerate() {
            assert_eq!(Thin::value(11, constant).ordinal(), ordinal as i32);
        }
        assert!(Thin::value(11, Small::B) == Thin::value(11, Small::B));
        assert!(Thin::value(11, Small::B) != Thin::value(11, Small::C));
        // An enum of more than 256 constants is two bytes.
        macro_rules! large {
            ($($name:ident)*) => {
                #[derive(Clone, Copy)]
                #[allow(dead_code)]
                enum Large { $($name,)* Last }
            };
        }
        large!(
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 C0 C1 C2 C3 C4 C5 C6 C7 C8 C9
            D0 D1 D2 D3 D4 D5 D6 D7 D8 D9 E0 E1 E2 E3 E4 E5 E6 E7 E8 E9 F0 F1 F2 F3 F4 F5 F6 F7 F8 F9
            G0 G1 G2 G3 G4 G5 G6 G7 G8 G9 H0 H1 H2 H3 H4 H5 H6 H7 H8 H9 I0 I1 I2 I3 I4 I5 I6 I7 I8 I9
            J0 J1 J2 J3 J4 J5 J6 J7 J8 J9 K0 K1 K2 K3 K4 K5 K6 K7 K8 K9 L0 L1 L2 L3 L4 L5 L6 L7 L8 L9
            M0 M1 M2 M3 M4 M5 M6 M7 M8 M9 N0 N1 N2 N3 N4 N5 N6 N7 N8 N9 O0 O1 O2 O3 O4 O5 O6 O7 O8 O9
            P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 Q0 Q1 Q2 Q3 Q4 Q5 Q6 Q7 Q8 Q9 R0 R1 R2 R3 R4 R5 R6 R7 R8 R9
            S0 S1 S2 S3 S4 S5 S6 S7 S8 S9 T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 U0 U1 U2 U3 U4 U5 U6 U7 U8 U9
            V0 V1 V2 V3 V4 V5 V6 V7 V8 V9 W0 W1 W2 W3 W4 W5 W6 W7 W8 W9 X0 X1 X2 X3 X4 X5 X6 X7 X8 X9
            Y0 Y1 Y2 Y3 Y4 Y5 Y6 Y7 Y8 Y9 Z0 Z1 Z2 Z3 Z4 Z5 Z6 Z7 Z8 Z9
        );
        assert_eq!(std::mem::size_of::<Large>(), 2);
        assert_eq!(Thin::value(12, Large::Last).ordinal(), Large::Last as i32);
        assert_eq!(Large::Last as i32, 260);
    }

    #[test]
    fn none_takes_no_room() {
        assert_eq!(std::mem::size_of::<Option<Thin>>(), 8);
    }
}
