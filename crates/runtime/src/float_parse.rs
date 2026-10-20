//! `Double.parseDouble` and `Float.parseFloat`, which read Java's grammar: surrounding white space
//! is trimmed, `NaN` and `Infinity` are accepted, a `d` or `f` suffix is allowed, and so are
//! hexadecimal floats. Decimal text goes to Rust's parser, which rounds correctly to the target
//! type (a `Float` is parsed straight to `f32`, not through `f64`). Rust does not read hexadecimal
//! floats, so those are rounded here, half to even, with the subnormal and overflow cases.

use crate::throw;
use std::ops::Neg;
use std::str::FromStr;

/// The two float types that Java parses: `f64` for `Double`, `f32` for `Float`.
trait Float: FromStr + Copy + Neg<Output = Self> {
    /// The significand bits, including the implicit leading one.
    const PRECISION: u32;
    /// The exponent of the unit in the last place of the subnormals: the least subnormal's.
    const MIN_ULP: i64;
    /// The nearest value of this type to `v`, for a `v` already on its grid, or NaN or infinity.
    fn from_f64(v: f64) -> Self;
}

impl Float for f64 {
    const PRECISION: u32 = 53;
    const MIN_ULP: i64 = -1074;
    fn from_f64(v: f64) -> Self {
        v
    }
}

impl Float for f32 {
    const PRECISION: u32 = 24;
    const MIN_ULP: i64 = -149;
    fn from_f64(v: f64) -> Self {
        v as f32
    }
}

/// Java's `Double.parseDouble`, which throws `NumberFormatException` for what Java rejects.
pub fn parse_double(s: &str) -> f64 {
    parse(s)
}

/// Java's `Float.parseFloat`, which throws `NumberFormatException` for what Java rejects.
pub fn parse_float(s: &str) -> f32 {
    parse(s)
}

fn parse<T: Float>(s: &str) -> T {
    let trimmed = s.trim_matches(|c: char| c <= ' ');
    if trimmed.is_empty() {
        throw("java.lang.NumberFormatException", "empty String");
    }
    read::<T>(trimmed).unwrap_or_else(|| {
        throw("java.lang.NumberFormatException", &format!("For input string: \"{trimmed}\""))
    })
}

/// A trimmed, non-empty string in Java's floating-point grammar, or `None`.
fn read<T: Float>(s: &str) -> Option<T> {
    let (negative, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let magnitude = if body == "NaN" {
        T::from_f64(f64::NAN)
    } else if body == "Infinity" {
        T::from_f64(f64::INFINITY)
    } else if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        T::from_f64(hex_float(hex, T::PRECISION, T::MIN_ULP)?)
    } else {
        decimal_float::<T>(body)?
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// A decimal floating-point literal without its sign, such as `1.5e3d`.
fn decimal_float<T: Float>(s: &str) -> Option<T> {
    let s = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = digits(int) && digits(frac) && !(int.is_empty() && frac.is_empty());
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    if mantissa_ok && exponent_ok { s.parse().ok() } else { None }
}

/// A hexadecimal floating-point literal without its `0x`, such as `1.8p1f`, rounded to a float
/// with `precision` bits whose least subnormal has exponent `min_ulp`. The binary exponent after
/// `p` is required. The result is exact for every float type, so `f32` conversion is exact too.
fn hex_float(s: &str, precision: u32, min_ulp: i64) -> Option<f64> {
    let s = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    let (mantissa, exponent) = s.split_once(['p', 'P'])?;
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    let exponent = binary_exponent(exponent)?;

    // The significand as an integer `mant` of up to 60 bits, the digits beyond it reduced to a
    // sticky flag, and `dropped` binary places between `mant` and the digits.
    let (mut mant, mut dropped, mut sticky) = (0u64, 0i64, false);
    for c in int.chars().chain(frac.chars()) {
        let d = u64::from(c.to_digit(16)?);
        if mant < 1 << 56 {
            mant = mant * 16 + d;
        } else {
            sticky |= d != 0;
            dropped += 4;
        }
    }
    if mant == 0 {
        return Some(0.0);
    }
    // The value is `mant * 2^k`, plus a little more when `sticky`.
    let k = dropped + exponent - 4 * frac.len() as i64;
    let bits = 64 - i64::from(mant.leading_zeros());
    // The exponent of the last bit kept: the precision's, or the subnormals' when that is lower.
    let ulp = (bits - 1 + k - (i64::from(precision) - 1)).max(min_ulp);
    if ulp > 1100 {
        return Some(f64::INFINITY);
    }
    let shift = ulp - k;
    if shift > 120 {
        return Some(0.0);
    }
    let mant = u128::from(mant);
    let kept = if shift > 0 {
        // `sticky` implies at least 57 bits in `mant`, so `shift` is positive whenever it is set.
        let shift = shift as u32;
        let kept = mant >> shift;
        let rest = mant & ((1 << shift) - 1);
        let half = 1 << (shift - 1);
        kept + u128::from(rest > half || (rest == half && (sticky || kept & 1 == 1)))
    } else {
        mant << -shift
    };
    Some(scale(kept as f64, ulp))
}

/// The binary exponent of a hexadecimal float: an optional sign, then decimal digits, saturated
/// far beyond any float's range so that huge exponents read as overflow or underflow.
fn binary_exponent(s: &str) -> Option<i64> {
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude = digits.bytes().fold(0i64, |v, b| (v * 10 + i64::from(b - b'0')).min(1 << 40));
    Some(if negative { -magnitude } else { magnitude })
}

/// `v * 2^e` for an integer `v` and `e` from the subnormal range up to 1100. Each factor is a
/// normal power of two, so the only rounding is the one that the result itself needs.
fn scale(v: f64, e: i64) -> f64 {
    let half = e / 2;
    v * pow2(half) * pow2(e - half)
}

/// `2^n` for `n` in the normal exponent range of `f64`.
fn pow2(n: i64) -> f64 {
    f64::from_bits(((n + 1023) as u64) << 52)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_hexadecimal_floats_half_to_even() {
        assert_eq!(hex_float("1.00000000000008p0", 53, -1074), Some(1.0));
        assert_eq!(hex_float("1.00000000000018p0", 53, -1074), Some(1.0 + 2f64.powi(-51)));
        assert_eq!(hex_float("1.000000000000080000000000000001p0", 53, -1074), Some(1.0 + 2f64.powi(-52)));
        assert_eq!(hex_float("1.000001p0", 24, -149), Some(1.0));
        assert_eq!(hex_float("1.000003p0", 24, -149), Some(1.0 + 2f64.powi(-22)));
        assert_eq!(hex_float("1p-150", 24, -149), Some(0.0));
        assert_eq!(hex_float("1.8p-150", 24, -149), Some(2f64.powi(-149)));
        assert_eq!(hex_float("0.0000000000001p-1022", 53, -1074), Some(f64::from_bits(1)));
    }

    #[test]
    fn reads_huge_and_tiny_exponents() {
        assert_eq!(hex_float("1p99999999999999999999", 53, -1074), Some(f64::INFINITY));
        assert_eq!(hex_float("1p-99999999999999999999", 53, -1074), Some(0.0));
        assert_eq!(hex_float("0p99999999999999999", 53, -1074), Some(0.0));
        assert_eq!(hex_float("1p1024", 53, -1074), Some(f64::INFINITY));
        assert_eq!(hex_float("1.fffffffffffff8p1023", 53, -1074), Some(f64::INFINITY));
        assert_eq!(hex_float("1.fffffffffffff7p1023", 53, -1074), Some(f64::MAX));
    }

    #[test]
    fn rejects_what_java_rejects() {
        for bad in ["", "0x", "0x1", "0x1.8", "0x1p", "0x.p1", "0xgp0", "1e", "1e+", ".", "-", "1.5ff"] {
            assert_eq!(read::<f64>(bad), None, "{bad}");
        }
        assert_eq!(read::<f64>("0x1.8p3F"), Some(12.0));
        assert_eq!(read::<f32>("1.5f"), Some(1.5));
        assert_eq!(read::<f32>("-.5e1d"), Some(-5.0));
        assert_eq!(read::<f32>("3.4028235677973366e38"), Some(f32::MAX));
        assert_eq!(read::<f32>("3.4028235677973367e38"), Some(f32::INFINITY));
        assert_eq!(read::<f32>("3.4028235e38"), Some(f32::MAX));
        assert_eq!(read::<f32>("16777217"), Some(16777216.0));
    }
}
