//! Boxed values (`Integer`, `Double` and the rest) and the methods of their classes: the shared
//! objects that `==` compares, the `valueOf` cache, Java's parsing rules and hash codes.

use crate::throw;
use std::ops::Deref;
use std::rc::Rc;

/// A boxed value. Two boxes are `==` only when they are the same object, which `same` tests.
/// Dereferences to the value it holds.
#[derive(Clone, Debug)]
pub struct Boxed<T>(Rc<T>);

impl<T> Deref for Boxed<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

/// Java's `a == b` on two boxed values: the same object.
pub fn same<T>(a: &Boxed<T>, b: &Boxed<T>) -> bool {
    Rc::ptr_eq(&a.0, &b.0)
}

/// Java's `a == b` on two boxed values that may be null: both null, or the same object.
pub fn same_optional<T>(a: Option<&Boxed<T>>, b: Option<&Boxed<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// A box that is not cached: `Double.valueOf` and the like make a new object every time.
pub fn box_new<T>(v: T) -> Boxed<T> {
    Boxed(Rc::new(v))
}

/// Defines `$name`, which boxes a value in `$low..=$high` as a shared object from a cache, as
/// `valueOf` does, and any other value as a new object.
macro_rules! cached {
    ($name:ident, $cache:ident, $t:ty, $low:expr, $high:expr) => {
        thread_local! {
            static $cache: Vec<Boxed<$t>> = ($low..=$high).map(|v| Boxed(Rc::new(v))).collect();
        }

        pub fn $name(v: $t) -> Boxed<$t> {
            if ($low..=$high).contains(&v) {
                $cache.with(|cache| cache[(v as i64 - $low as i64) as usize].clone())
            } else {
                box_new(v)
            }
        }
    };
}

cached!(box_int, INTS, i32, -128, 127);
cached!(box_long, LONGS, i64, -128, 127);
cached!(box_short, SHORTS, i16, -128, 127);
cached!(box_byte, BYTES, i8, -128, 127);
cached!(box_char, CHARS, u16, 0, 127);

thread_local! {
    static BOOLEANS: [Boxed<bool>; 2] = [Boxed(Rc::new(false)), Boxed(Rc::new(true))];
}

/// `Boolean.valueOf`: `TRUE` and `FALSE` are always the same two objects.
pub fn box_bool(v: bool) -> Boxed<bool> {
    BOOLEANS.with(|cache| cache[v as usize].clone())
}

/// `Integer.parseInt`, `Long.parseLong` and the like: Rust's integer parsing reads Java's
/// syntax for radix 10 (an optional sign, then digits and nothing else).
pub fn parse_number<T: std::str::FromStr>(s: &str) -> T {
    s.parse()
        .unwrap_or_else(|_| throw("java.lang.NumberFormatException", &format!("For input string: \"{s}\"")))
}

/// `Double.parseDouble`, which reads Java's grammar: surrounding white space is trimmed, `NaN`
/// and `Infinity` are accepted, a `d` or `f` suffix is allowed, and so are hexadecimal floats.
pub fn parse_double(s: &str) -> f64 {
    let trimmed = s.trim_matches(|c: char| c <= ' ');
    if trimmed.is_empty() {
        throw("java.lang.NumberFormatException", "empty String");
    }
    java_double(trimmed).unwrap_or_else(|| {
        throw("java.lang.NumberFormatException", &format!("For input string: \"{trimmed}\""))
    })
}

fn java_double(s: &str) -> Option<f64> {
    let (negative, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let magnitude = if body == "NaN" {
        f64::NAN
    } else if body == "Infinity" {
        f64::INFINITY
    } else if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        hex_float(hex)?
    } else {
        decimal_float(body)?
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// A decimal floating-point literal without its sign, such as `1.5e3d`.
fn decimal_float(s: &str) -> Option<f64> {
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

/// A hexadecimal floating-point literal without its `0x`, such as `1.8p1f`: the binary exponent
/// after `p` is required.
fn hex_float(s: &str) -> Option<f64> {
    let s = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    let (mantissa, exponent) = s.split_once(['p', 'P'])?;
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    let mut value = 0.0;
    for d in int.chars() {
        value = value * 16.0 + d.to_digit(16)? as f64;
    }
    let mut scale = 1.0 / 16.0;
    for d in frac.chars() {
        value += d.to_digit(16)? as f64 * scale;
        scale /= 16.0;
    }
    let exponent: i32 = exponent.parse().ok()?;
    Some(value * 2f64.powi(exponent))
}

/// `Long.hashCode`, which is `Integer.hashCode` of the value's high and low halves mixed.
pub fn long_hash(v: i64) -> i32 {
    (v ^ ((v as u64) >> 32) as i64) as i32
}

/// `Double.hashCode`: the hash of the bits, with every NaN counted as the canonical one.
pub fn double_hash(v: f64) -> i32 {
    let bits = if v.is_nan() { 0x7ff8_0000_0000_0000 } else { v.to_bits() as i64 };
    long_hash(bits)
}

/// `Float.hashCode`: the bits, with every NaN counted as the canonical one.
pub fn float_hash(v: f32) -> i32 {
    if v.is_nan() { 0x7fc0_0000 } else { v.to_bits() as i32 }
}

/// `Boolean.hashCode`.
pub fn bool_hash(v: bool) -> i32 {
    if v { 1231 } else { 1237 }
}

/// `Double.compare`: like `<` and `==`, but NaN is equal to itself and larger than everything,
/// and `-0.0` is less than `0.0`.
pub fn double_compare(a: f64, b: f64) -> i32 {
    if a < b {
        return -1;
    }
    if a > b {
        return 1;
    }
    let (x, y) = (double_hash_bits(a), double_hash_bits(b));
    match x.cmp(&y) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn double_hash_bits(v: f64) -> i64 {
    if v.is_nan() { 0x7ff8_0000_0000_0000 } else { v.to_bits() as i64 }
}

/// `Boolean.compare`: `false` is less than `true`.
pub fn bool_compare(a: bool, b: bool) -> i32 {
    (a as i32) - (b as i32)
}

/// `Character.getNumericValue` for ASCII: the value of a digit or a letter in base 36, and -1 for
/// anything else.
pub fn char_numeric_value(c: u16) -> i32 {
    crate::character::numeric_value(c)
}

/// `Boolean.parseBoolean`: true for `true` in any case, and false for anything else, null included.
pub fn parse_boolean(s: &str) -> bool {
    s.eq_ignore_ascii_case("true")
}

/// `Double.equals`, which compares bits, so every NaN equals every NaN and `-0.0` differs from
/// `0.0`. Unlike `==`, it is not the comparison of the values.
pub fn double_equals(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valueof_caches_small_values() {
        assert!(same(&box_int(100), &box_int(100)));
        assert!(!same(&box_int(1000), &box_int(1000)));
        assert!(same(&box_int(-128), &box_int(-128)));
        assert!(!same(&box_int(128), &box_int(128)));
        assert!(same(&box_char(127), &box_char(127)));
        assert!(!same(&box_char(128), &box_char(128)));
        assert!(same(&box_bool(true), &box_bool(true)));
        assert!(!same(&box_new(1.0), &box_new(1.0)));
        assert!(same(&box_byte(-128), &box_byte(-128)));
        assert!(!same(&box_short(128), &box_short(128)));
        assert_eq!(*box_long(-5), -5);
    }

    #[test]
    fn same_optional_compares_nulls() {
        let a = box_int(1000);
        assert!(same_optional::<i32>(None, None));
        assert!(!same_optional(Some(&a), None));
        assert!(same_optional(Some(&a), Some(&a.clone())));
    }

    #[test]
    fn parses_doubles_like_java() {
        // Each expected value is what `Double.parseDouble` returns for the same string.
        let cases = [
            ("1d", 1.0),
            (" 2 ", 2.0),
            ("0x1p3", 8.0),
            ("0x1.8p1f", 3.0),
            ("-.5", -0.5),
            ("1.", 1.0),
            ("1e3", 1000.0),
            ("+7.25F", 7.25),
            ("0X1P-2", 0.25),
            ("-Infinity", f64::NEG_INFINITY),
        ];
        for (text, java) in cases {
            assert_eq!(parse_double(text), java, "{text:?}");
        }
        assert!(parse_double("NaN").is_nan());
    }

    #[test]
    fn rejects_what_java_rejects() {
        for text in ["1_0", "0x10", "1e", "Infinity1", "infinity", "nan", "1.2.3", "."] {
            assert!(java_double(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn parses_integers_like_java() {
        assert_eq!(parse_number::<i32>("-12"), -12);
        assert_eq!(parse_number::<i32>("+7"), 7);
        assert_eq!(parse_number::<i64>("123456789012"), 123456789012);
        assert_eq!(parse_number::<i32>("2147483647"), i32::MAX);
        assert!("2147483648".parse::<i32>().is_err());
        assert!(" 1".parse::<i64>().is_err());
    }

    #[test]
    fn hashes_like_java() {
        assert_eq!(long_hash(0), 0);
        assert_eq!(long_hash(-1), 0);
        assert_eq!(long_hash(1 << 40), 256);
        assert_eq!(long_hash(i64::MIN), -2147483648);
        assert_eq!(long_hash(123456789012), -1097262584);
        assert_eq!(double_hash(0.0), 0);
        assert_eq!(double_hash(-1.0), -1074790400);
        assert_eq!(double_hash(1.5), 1073217536);
        assert_eq!(double_hash(-0.0), -2147483648);
        assert_eq!(double_hash(f64::NAN), 2146959360);
        assert_eq!(bool_hash(true), 1231);
        assert_eq!(bool_hash(false), 1237);
    }

    #[test]
    fn compares_like_java() {
        assert_eq!(double_compare(0.0, -0.0), 1);
        assert_eq!(double_compare(-0.0, 0.0), -1);
        assert_eq!(double_compare(f64::NAN, f64::INFINITY), 1);
        assert_eq!(double_compare(2.0, 2.0), 0);
        assert_eq!(bool_compare(false, true), -1);
        assert_eq!(bool_compare(true, true), 0);
    }

    #[test]
    fn numeric_values_of_characters() {
        assert_eq!(char_numeric_value(b'z' as u16), 35);
        assert_eq!(char_numeric_value(b'7' as u16), 7);
        assert_eq!(char_numeric_value(b'-' as u16), -1);
    }

    #[test]
    fn parses_booleans_like_java() {
        // `Boolean.parseBoolean` results for each string, null included as "not true".
        for (text, java) in
            [("true", true), ("TRUE", true), ("tRuE", true), ("yes", false), ("", false), (" true", false)]
        {
            assert_eq!(parse_boolean(text), java, "{text:?}");
        }
    }

    #[test]
    fn double_equality_compares_bits() {
        // `Double.equals` results: NaN equals NaN, and 0.0 and -0.0 are different.
        assert!(double_equals(f64::NAN, f64::NAN));
        assert!(!double_equals(0.0, -0.0));
        assert!(double_equals(1.0, 1.0));
    }
}
