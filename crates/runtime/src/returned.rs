//! `Ret`: how a function of a split program's body crate returns a `Result` of a small value.
//!
//! Rust returns a `Result<i32, Throwable>` through memory: the `i32` of `Ok` and the pointer of
//! `Err` are at different offsets, so the value is no pair of registers, and the caller passes the
//! address of a slot that the function writes and the caller reads back. A `Result<u64, E>` of a
//! pointer-sized `E` has both at the same offset and comes back in two registers. A moved
//! function that returns a `Result` of an `i32`, a `bool`, a `float` or a narrower integer
//! returns its value widened to a `u64` (`Ret`), and its declaration in the header narrows it
//! back where it is called (`split.rs`). Measured on a call in a loop: 17 instructions where it
//! was 19.

use std::marker::PhantomData;

/// A `Result<T, E>` whose `Ok` value is widened to a `u64`, with the same layout as a
/// `Result<u64, E>`.
#[repr(transparent)]
pub struct Ret<T, E>(Result<u64, E>, PhantomData<T>);

/// A value that `Ret` widens to a `u64` and back.
pub trait Widened: Copy {
    fn widen(self) -> u64;
    fn narrow(wide: u64) -> Self;
}

macro_rules! widened_integers {
    ($($t:ty => $unsigned:ty),*) => {$(
        impl Widened for $t {
            #[inline(always)]
            fn widen(self) -> u64 {
                self as $unsigned as u64
            }
            #[inline(always)]
            fn narrow(wide: u64) -> Self {
                wide as $unsigned as $t
            }
        }
    )*};
}

widened_integers!(i32 => u32, i16 => u16, u16 => u16, i8 => u8, u8 => u8);

impl Widened for bool {
    #[inline(always)]
    fn widen(self) -> u64 {
        self as u64
    }
    #[inline(always)]
    fn narrow(wide: u64) -> Self {
        wide != 0
    }
}

impl Widened for f32 {
    #[inline(always)]
    fn widen(self) -> u64 {
        self.to_bits() as u64
    }
    #[inline(always)]
    fn narrow(wide: u64) -> Self {
        f32::from_bits(wide as u32)
    }
}

impl<T: Widened, E> From<Result<T, E>> for Ret<T, E> {
    #[inline(always)]
    fn from(result: Result<T, E>) -> Self {
        Ret(result.map(T::widen), PhantomData)
    }
}

impl<T: Widened, E> Ret<T, E> {
    /// The `Result` as the function's code made it.
    #[inline(always)]
    pub fn result(self) -> Result<T, E> {
        self.0.map(T::narrow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_values_come_back_as_they_went() {
        let back = |r: Result<i32, u8>| Ret::from(r).result();
        assert_eq!(back(Ok(-7)), Ok(-7));
        assert_eq!(back(Ok(i32::MIN)), Ok(i32::MIN));
        assert_eq!(back(Err(3)), Err(3));
        assert_eq!(Ret::<bool, ()>::from(Ok(true)).result(), Ok(true));
        assert_eq!(Ret::<i16, ()>::from(Ok(-2)).result(), Ok(-2));
        assert_eq!(Ret::<u16, ()>::from(Ok(0xffff)).result(), Ok(0xffff));
        assert_eq!(Ret::<i8, ()>::from(Ok(-128)).result(), Ok(-128));
        let nan = f32::from_bits(0x7fc0_1234);
        assert_eq!(Ret::<f32, ()>::from(Ok(nan)).result().map(f32::to_bits), Ok(0x7fc0_1234));
        assert_eq!(Ret::<f32, ()>::from(Ok(-0.0)).result().map(f32::to_bits), Ok((-0.0f32).to_bits()));
    }
}
