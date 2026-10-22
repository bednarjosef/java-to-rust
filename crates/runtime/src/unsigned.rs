//! The unsigned methods of `Integer` and `Long`, which read the bits of an `int` or `long` as an
//! unsigned value. Generated code passes the value as a `u64`, and `bits` (32 or 64) says which
//! width the parse checks its range against.

use crate::character_classes::digit;
use crate::integer_parse::{check_radix, for_input};
use crate::{Exception, throw, uncaught};

/// Java's `Integer.divideUnsigned` and `Long.divideUnsigned`.
pub fn divide_unsigned(a: u64, b: u64) -> u64 {
    if b == 0 {
        throw("java.lang.ArithmeticException", "/ by zero");
    }
    a / b
}

/// Java's `Integer.remainderUnsigned` and `Long.remainderUnsigned`.
pub fn remainder_unsigned(a: u64, b: u64) -> u64 {
    if b == 0 {
        throw("java.lang.ArithmeticException", "/ by zero");
    }
    a % b
}

/// Java's `compareUnsigned`: -1, 0 or 1, as `Integer.compare` gives.
pub fn compare_unsigned(a: u64, b: u64) -> i32 {
    a.cmp(&b) as i32
}

/// Java's `toUnsignedString(value, radix)`. A radix outside 2 to 36 means 10, as it does for
/// `Integer.toString`.
pub fn to_unsigned_string(value: u64, radix: i32) -> String {
    let radix = if (2..=36).contains(&radix) { radix as u32 } else { 10 };
    let mut digits = Vec::new();
    let mut rest = value;
    loop {
        digits.push(char::from_digit((rest % u64::from(radix)) as u32, radix).expect("a digit of the radix"));
        rest /= u64::from(radix);
        if rest == 0 {
            break;
        }
    }
    digits.iter().rev().collect()
}

/// Java's `parseUnsignedInt(s, radix)` (`bits` 32) and `parseUnsignedLong(s, radix)` (`bits` 64),
/// which throw `NumberFormatException` for what Java rejects. `None` is a null string.
pub fn parse_unsigned(s: Option<&crate::Wtf8>, radix: i32, bits: u32) -> u64 {
    try_parse_unsigned(s, radix, bits).unwrap_or_else(uncaught)
}

/// The checked form of `parse_unsigned`.
pub fn try_parse_unsigned(s: Option<&crate::Wtf8>, radix: i32, bits: u32) -> Result<u64, Exception> {
    let s = s.map(|s| s.to_string_lossy());
    let s = s.as_deref();
    let Some(s) = s else {
        return Err(Exception::new("java.lang.NumberFormatException", "Cannot parse null string"));
    };
    read(s, radix, bits).map_err(|message| Exception::new("java.lang.NumberFormatException", message))
}

/// The value of an unsigned string: a leading `+` is allowed, a leading `-` is not, and each digit
/// is checked as it is read, so an invalid digit and an overflow are reported in the order they
/// come in the string, as Java reports them.
fn read(s: &str, radix: i32, bits: u32) -> Result<u64, String> {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.is_empty() {
        return Err(for_input(s, radix));
    }
    if units[0] == u16::from(b'-') {
        return Err(format!("Illegal leading minus sign on unsigned string {s}."));
    }
    check_radix(radix)?;
    let digits = if units[0] == u16::from(b'+') { &units[1..] } else { &units[..] };
    if digits.is_empty() {
        return Err(for_input(s, radix));
    }
    let (max, kind) = if bits == 32 { (u64::from(u32::MAX), "int") } else { (u64::MAX, "long") };
    let mut value = 0u128;
    for &c in digits {
        let d = digit(c, radix);
        if d < 0 {
            return Err(for_input(s, radix));
        }
        // `value` stays at most `max` before a step, so this cannot overflow `u128`.
        value = value * radix as u128 + d as u128;
        if value > u128::from(max) {
            return Err(format!("String value {s} exceeds range of unsigned {kind}."));
        }
    }
    Ok(value as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divides_and_compares_as_unsigned() {
        assert_eq!(divide_unsigned(u64::from(u32::MAX), 7), 613_566_756);
        assert_eq!(remainder_unsigned(u64::MAX, 10), 5);
        assert_eq!(compare_unsigned(u64::MAX, 1), 1);
        assert_eq!(compare_unsigned(1, u64::MAX), -1);
        assert_eq!(compare_unsigned(3, 3), 0);
    }

    #[test]
    fn prints_in_the_radix_or_in_decimal() {
        assert_eq!(to_unsigned_string(u64::MAX, 2).len(), 64);
        assert_eq!(to_unsigned_string(35, 36), "z");
        assert_eq!(to_unsigned_string(u64::MAX, 36), "3w5e11264sgsf");
        assert_eq!(to_unsigned_string(0, 16), "0");
        assert_eq!(to_unsigned_string(5, 99), "5");
        assert_eq!(to_unsigned_string(u64::from(u32::MAX), 1), "4294967295");
    }

    #[test]
    fn reads_what_java_reads() {
        assert_eq!(read("4294967295", 10, 32), Ok(4_294_967_295));
        assert_eq!(read("+7", 10, 32), Ok(7));
        assert_eq!(read("ffffffff", 16, 32), Ok(4_294_967_295));
        assert_eq!(read("000001", 10, 32), Ok(1));
        assert_eq!(read("18446744073709551615", 10, 64), Ok(u64::MAX));
        assert_eq!(read("+18446744073709551615", 10, 64), Ok(u64::MAX));
        assert_eq!(read("7fffffffffffffff", 16, 64), Ok(i64::MAX as u64));
        assert_eq!(read("\u{661}\u{662}", 10, 32), Ok(12));
    }

    #[test]
    fn rejects_what_java_rejects_with_java_s_message() {
        let cases = [
            ("-1", 10, 32, "Illegal leading minus sign on unsigned string -1."),
            ("-0", 10, 64, "Illegal leading minus sign on unsigned string -0."),
            ("4294967296", 10, 32, "String value 4294967296 exceeds range of unsigned int."),
            ("99999999999x", 10, 32, "String value 99999999999x exceeds range of unsigned int."),
            ("42949672x6", 10, 32, "For input string: \"42949672x6\""),
            (
                "18446744073709551616",
                10,
                64,
                "String value 18446744073709551616 exceeds range of unsigned long.",
            ),
            (
                "46116860184273879040",
                10,
                64,
                "String value 46116860184273879040 exceeds range of unsigned long.",
            ),
            ("1234567890123x", 10, 64, "For input string: \"1234567890123x\""),
            ("fffffffffffffffg", 16, 64, "For input string: \"fffffffffffffffg\" under radix 16"),
            ("", 10, 32, "For input string: \"\""),
            ("", 16, 64, "For input string: \"\" under radix 16"),
            ("+", 10, 32, "For input string: \"+\""),
            ("1", 1, 32, "radix 1 less than Character.MIN_RADIX"),
        ];
        for (text, radix, bits, message) in cases {
            assert_eq!(read(text, radix, bits).unwrap_err(), message, "{text:?} in {radix}");
        }
    }
}
