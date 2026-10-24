//! The `Math` methods that throw: the exact integer arithmetic (`addExact` and the rest), which
//! throws `ArithmeticException` on overflow, and `clamp`, which throws `IllegalArgumentException`
//! for bounds out of order.

use crate::{double_to_string, float_to_string, max_f32, min_f32, throw};
use std::fmt::Display;

/// The integer types of the exact methods, with the messages of their overflows.
pub trait Exact: Copy + Ord + Display {
    const OVERFLOW: &'static str;
    const ABS_OVERFLOW: &'static str;

    fn try_add(self, b: Self) -> Option<Self>;
    fn try_sub(self, b: Self) -> Option<Self>;
    fn try_mul(self, b: Self) -> Option<Self>;
    fn try_neg(self) -> Option<Self>;
    fn try_abs(self) -> Option<Self>;
    fn try_pow(self, n: u32) -> Option<Self>;
}

macro_rules! exact {
    ($t:ty, $overflow:literal, $abs_overflow:literal) => {
        impl Exact for $t {
            const OVERFLOW: &'static str = $overflow;
            const ABS_OVERFLOW: &'static str = $abs_overflow;

            fn try_add(self, b: Self) -> Option<Self> {
                self.checked_add(b)
            }

            fn try_sub(self, b: Self) -> Option<Self> {
                self.checked_sub(b)
            }

            fn try_mul(self, b: Self) -> Option<Self> {
                self.checked_mul(b)
            }

            fn try_neg(self) -> Option<Self> {
                self.checked_neg()
            }

            fn try_abs(self) -> Option<Self> {
                self.checked_abs()
            }

            fn try_pow(self, n: u32) -> Option<Self> {
                self.checked_pow(n)
            }
        }
    };
}

exact!(i32, "integer overflow", "Overflow to represent absolute value of Integer.MIN_VALUE");
exact!(i64, "long overflow", "Overflow to represent absolute value of Long.MIN_VALUE");

fn overflow(message: &str) -> ! {
    throw("java.lang.ArithmeticException", message)
}

/// `Math.addExact`. `incrementExact` is this with a literal 1.
pub fn add_exact<T: Exact>(a: T, b: T) -> T {
    a.try_add(b).unwrap_or_else(|| overflow(T::OVERFLOW))
}

/// `Math.subtractExact`. `decrementExact` is this with a literal 1.
pub fn sub_exact<T: Exact>(a: T, b: T) -> T {
    a.try_sub(b).unwrap_or_else(|| overflow(T::OVERFLOW))
}

/// `Math.multiplyExact`.
pub fn mul_exact<T: Exact>(a: T, b: T) -> T {
    a.try_mul(b).unwrap_or_else(|| overflow(T::OVERFLOW))
}

/// `Math.negateExact`.
pub fn neg_exact<T: Exact>(a: T) -> T {
    a.try_neg().unwrap_or_else(|| overflow(T::OVERFLOW))
}

/// `Math.absExact`, whose overflow has its own message.
pub fn abs_exact<T: Exact>(a: T) -> T {
    a.try_abs().unwrap_or_else(|| overflow(T::ABS_OVERFLOW))
}

/// `Math.powExact` on `int` and `long`: `x` to the power `n`, which throws for a negative `n` and
/// where the power overflows. Java squares and multiplies as Rust's `checked_pow` does, so a
/// power that fits never overflows on the way.
pub fn pow_exact<T: Exact>(x: T, n: i32) -> T {
    let Ok(n) = u32::try_from(n) else { overflow("negative exponent") };
    x.try_pow(n).unwrap_or_else(|| overflow(T::OVERFLOW))
}

/// `Math.toIntExact`.
pub fn to_int_exact(a: i64) -> i32 {
    i32::try_from(a).unwrap_or_else(|_| overflow("integer overflow"))
}

/// `Math.clamp` on integers, for `int` and `long` bounds alike.
pub fn clamp<T: Ord + Display>(value: T, min: T, max: T) -> T {
    if min > max {
        throw("java.lang.IllegalArgumentException", &format!("{min} > {max}"));
    }
    value.max(min).min(max)
}

/// `Math.clamp(double, double, double)`: NaN bounds are rejected, and `Math.min` and `Math.max`
/// order the result as they do (so a NaN value gives NaN, and -0.0 is below 0.0).
pub fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    if min.is_nan() {
        throw("java.lang.IllegalArgumentException", "min is NaN");
    }
    if max.is_nan() {
        throw("java.lang.IllegalArgumentException", "max is NaN");
    }
    if min > max {
        let message = format!("{} > {}", double_to_string(min), double_to_string(max));
        throw("java.lang.IllegalArgumentException", &message);
    }
    crate::min(max, crate::max(value, min))
}

/// `Math.clamp(float, float, float)`, as `clamp_f64`.
pub fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    if min.is_nan() {
        throw("java.lang.IllegalArgumentException", "min is NaN");
    }
    if max.is_nan() {
        throw("java.lang.IllegalArgumentException", "max is NaN");
    }
    if min > max {
        let message = format!("{} > {}", float_to_string(min), float_to_string(max));
        throw("java.lang.IllegalArgumentException", &message);
    }
    min_f32(max, max_f32(value, min))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_results_within_range() {
        assert_eq!(add_exact(2, 3), 5);
        assert_eq!(mul_exact(-3i64, 4), -12);
        assert_eq!(abs_exact(-5), 5);
        assert_eq!(to_int_exact(-7), -7);
        assert_eq!((pow_exact(-2, 31), pow_exact(3i64, 0), pow_exact(-1, 7)), (i32::MIN, 1, -1));
        assert_eq!(pow_exact(10i64, 18), 1_000_000_000_000_000_000);
        assert_eq!(clamp(5i64, 1, 3), 3);
        assert_eq!(clamp_f64(-0.0, 0.0, 1.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(clamp_f64(-0.0, -1.0, -0.0).to_bits(), (-0.0f64).to_bits());
        assert!(clamp_f64(f64::NAN, 0.0, 1.0).is_nan());
    }

    #[test]
    fn overflows_are_detected() {
        assert!(i32::MAX.try_add(1).is_none());
        assert!(i64::MIN.try_sub(1).is_none());
        assert!(i64::MAX.try_mul(2).is_none());
        assert!(i32::MIN.try_neg().is_none());
        assert!(i32::MIN.try_abs().is_none());
        assert!(i64::MIN.try_abs().is_none());
        assert!(i32::try_from(1i64 << 31).is_err());
    }
}
