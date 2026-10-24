//! Boxed values (`Integer`, `Double` and the rest) and the methods of their classes: the shared
//! objects that `==` compares, the `valueOf` cache, Java's parsing rules and hash codes.

use crate::Arc;
use std::ops::Deref;

/// A boxed value. Two boxes are `==` only when they are the same object, which `same` tests.
/// Dereferences to the value it holds.
#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Boxed<T>(Arc<T>);

// A box is its handle.
unsafe impl<T> crate::Pointer for Boxed<T> {}

impl<T> Deref for Boxed<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

/// Java's `a == b` on two boxed values: the same object.
pub fn same<T>(a: &Boxed<T>, b: &Boxed<T>) -> bool {
    Arc::ptr_eq(&a.0, &b.0)
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
    Boxed(Arc::new(v))
}

/// Defines `$name`, which boxes a value in `$low..=$high` as a shared object from a cache, as
/// `valueOf` does, and any other value as a new object.
macro_rules! cached {
    ($name:ident, $cache:ident, $t:ty, $low:expr, $high:expr) => {
        thread_local! {
            static $cache: Vec<Boxed<$t>> = ($low..=$high).map(|v| Boxed(Arc::new(v))).collect();
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
    static BOOLEANS: [Boxed<bool>; 2] = [Boxed(Arc::new(false)), Boxed(Arc::new(true))];
}

/// `Boolean.valueOf`: `TRUE` and `FALSE` are always the same two objects.
pub fn box_bool(v: bool) -> Boxed<bool> {
    BOOLEANS.with(|cache| cache[v as usize].clone())
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
pub fn parse_boolean(s: &crate::Wtf8) -> bool {
    let s = &*s.to_string_lossy();
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
    use crate::float_parse::parse_double;

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
            assert_eq!(parse_double(crate::Wtf8::new(text)), java, "{text:?}");
        }
        assert!(parse_double(crate::Wtf8::new("NaN")).is_nan());
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
            assert_eq!(parse_boolean(crate::Wtf8::new(text)), java, "{text:?}");
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
