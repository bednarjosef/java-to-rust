//! `Math.ceilDiv`, `Math.ceilMod` and `Math.ceilDivExact`: the quotient rounded toward positive
//! infinity, its remainder, and the exact quotient, which throws where it overflows; and the other
//! exact quotients, `Math.divideExact` and `Math.floorDivExact`. A zero divisor throws as in a
//! division (`crate::div`).

use crate::exact::Exact;
use crate::{Exception, Integer, div, rem, throw};
use std::ops::{Add, BitAnd, Sub};

/// Whether the exact quotient is rounded up: the operands have the same sign and the division
/// leaves a remainder `r`.
fn rounds_up<T: Integer + PartialOrd + Default>(a: T, b: T, r: T) -> bool {
    let zero = T::default();
    (a < zero) == (b < zero) && r != zero
}

/// Whether `a / b` overflows, which only `MIN_VALUE / -1` does. `Math` finds it from the sign bits
/// of the operands and of the quotient, and so does this.
fn overflows<T: Integer + PartialOrd + Default + BitAnd<Output = T>>(a: T, b: T) -> bool {
    let zero = T::default();
    (a & b & div(a, b)) < zero
}

/// `Math.ceilDiv` on `int` and `long`: the smallest integer not below the quotient; `MIN_VALUE /
/// -1` wraps.
pub fn ceil_div<T: Integer + PartialOrd + Default + Add<Output = T> + From<i8>>(a: T, b: T) -> T {
    let q = div(a, b);
    if rounds_up(a, b, rem(a, b)) { q + T::from(1) } else { q }
}

/// `Math.ceilMod` on `int` and `long`: `a - ceilDiv(a, b) * b`, which has the sign opposite to
/// `b`'s, or is zero.
pub fn ceil_mod<T: Integer + PartialOrd + Default + Sub<Output = T>>(a: T, b: T) -> T {
    let r = rem(a, b);
    if rounds_up(a, b, r) { r - b } else { r }
}

/// `Math.ceilDivExact` on `int` and `long`: `ceilDiv`, but throws `ArithmeticException` where the
/// quotient overflows.
pub fn ceil_div_exact<T>(a: T, b: T) -> T
where
    T: Integer + Exact + PartialOrd + Default + Add<Output = T> + BitAnd<Output = T> + From<i8>,
{
    if overflows(a, b) {
        throw("java.lang.ArithmeticException", T::OVERFLOW);
    }
    ceil_div(a, b)
}

/// `Math.divideExact` on `int` and `long`: the quotient, but throws `ArithmeticException` where it
/// overflows.
pub fn divide_exact<T>(a: T, b: T) -> T
where
    T: Integer + Exact + PartialOrd + Default + BitAnd<Output = T>,
{
    if overflows(a, b) {
        throw("java.lang.ArithmeticException", T::OVERFLOW);
    }
    div(a, b)
}

/// `Math.floorDivExact` on `int` and `long`: `floorDiv`, but throws `ArithmeticException` where
/// the quotient overflows.
pub fn floor_div_exact<T>(a: T, b: T) -> T
where
    T: Integer + Exact + PartialOrd + Default + Sub<Output = T> + BitAnd<Output = T> + From<i8>,
{
    if overflows(a, b) {
        throw("java.lang.ArithmeticException", T::OVERFLOW);
    }
    crate::floor_div(a, b)
}

/// `Math.ceilDiv` where the program observes the `ArithmeticException` of a zero divisor.
pub fn try_ceil_div<T: Integer + PartialOrd + Default + Add<Output = T> + From<i8>>(
    a: T,
    b: T,
) -> Result<T, Exception> {
    crate::try_div(a, b)?;
    Ok(ceil_div(a, b))
}

/// `Math.ceilMod` where the program observes the `ArithmeticException` of a zero divisor.
pub fn try_ceil_mod<T: Integer + PartialOrd + Default + Sub<Output = T>>(a: T, b: T) -> Result<T, Exception> {
    crate::try_rem(a, b)?;
    Ok(ceil_mod(a, b))
}

/// `Math.ceilDivExact` where the program observes its `ArithmeticException`s: a zero divisor, and
/// the overflow.
pub fn try_ceil_div_exact<T>(a: T, b: T) -> Result<T, Exception>
where
    T: Integer + Exact + PartialOrd + Default + Add<Output = T> + BitAnd<Output = T> + From<i8>,
{
    crate::try_div(a, b)?;
    if overflows(a, b) {
        return Err(Exception::new("java.lang.ArithmeticException", T::OVERFLOW));
    }
    Ok(ceil_div(a, b))
}

/// `Math.divideExact` where the program observes its `ArithmeticException`s: a zero divisor, and
/// the overflow.
pub fn try_divide_exact<T>(a: T, b: T) -> Result<T, Exception>
where
    T: Integer + Exact + PartialOrd + Default + BitAnd<Output = T>,
{
    crate::try_div(a, b)?;
    if overflows(a, b) {
        return Err(Exception::new("java.lang.ArithmeticException", T::OVERFLOW));
    }
    Ok(div(a, b))
}

/// `Math.floorDivExact` where the program observes its `ArithmeticException`s: a zero divisor,
/// and the overflow.
pub fn try_floor_div_exact<T>(a: T, b: T) -> Result<T, Exception>
where
    T: Integer + Exact + PartialOrd + Default + Sub<Output = T> + BitAnd<Output = T> + From<i8>,
{
    crate::try_div(a, b)?;
    if overflows(a, b) {
        return Err(Exception::new("java.lang.ArithmeticException", T::OVERFLOW));
    }
    Ok(crate::floor_div(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceil_div_and_mod_match_java() {
        // Expected values from Java's Math.ceilDiv and Math.ceilMod.
        assert_eq!((ceil_div(7, 2), ceil_div(-7, 2), ceil_div(7, -2), ceil_div(-7, -2)), (4, -3, -3, 4));
        assert_eq!((ceil_mod(7, 3), ceil_mod(-7, 3), ceil_mod(7, -3), ceil_mod(-7, -3)), (-2, -1, 1, 2));
        assert_eq!(ceil_div(i32::MIN, -1), i32::MIN);
        assert_eq!(ceil_mod(i64::MIN, -1), 0);
        assert_eq!(ceil_div(-6i64, 3), -2);
    }

    #[test]
    fn ceil_div_exact_throws_on_overflow_only() {
        // Expected values from Java's Math.ceilDivExact.
        assert_eq!(ceil_div_exact(7, -2), -3);
        assert_eq!(ceil_div_exact(-6i64, 3), -2);
        assert!(
            matches!(try_ceil_div_exact(i32::MIN, -1), Err(e) if e.message.as_deref() == Some("integer overflow"))
        );
        assert!(
            matches!(try_ceil_div_exact(i64::MIN, -1), Err(e) if e.message.as_deref() == Some("long overflow"))
        );
        assert!(matches!(try_ceil_div_exact(1i64, 0), Err(e) if e.message.as_deref() == Some("/ by zero")));
    }

    #[test]
    fn divide_and_floor_div_exact_throw_on_overflow_only() {
        // Expected values from Java's Math.divideExact and Math.floorDivExact.
        assert_eq!((divide_exact(7, -2), floor_div_exact(7, -2)), (-3, -4));
        assert_eq!((divide_exact(-6i64, 3), floor_div_exact(-7i64, 2)), (-2, -4));
        assert_eq!(floor_div_exact(i32::MIN, 1), i32::MIN);
        assert!(
            matches!(try_floor_div_exact(i32::MIN, -1), Err(e) if e.message.as_deref() == Some("integer overflow"))
        );
        assert!(
            matches!(try_divide_exact(i64::MIN, -1), Err(e) if e.message.as_deref() == Some("long overflow"))
        );
        assert!(matches!(try_floor_div_exact(1, 0), Err(e) if e.message.as_deref() == Some("/ by zero")));
    }
}
