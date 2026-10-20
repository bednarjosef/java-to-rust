//! `Integer.parseInt(s, radix)`, `Long.parseLong(s, radix)` and `Integer.valueOf(s, radix)`, which
//! read Java's syntax: an optional sign, then digits that `Character.digit` accepts in the radix.
//! Java reads the string one UTF-16 unit at a time, so a digit outside the BMP is two units and is
//! rejected, while every BMP decimal digit counts. Rust's `from_str_radix` reads only ASCII digits.

use crate::character_classes::digit;
use crate::{Exception, uncaught};

const MIN_RADIX: i32 = 2;
const MAX_RADIX: i32 = 36;

/// Past this bound a value is out of range for every integer type, so it is held here instead of
/// growing; the bound is far enough below `i128::MAX` that one more digit cannot overflow.
const CAP: i128 = 1 << 100;

/// Java's `parseInt(s, radix)` for the integer type `T`, which throws `NumberFormatException` for
/// what Java rejects. `None` is a null string.
pub fn parse_radix<T: TryFrom<i128>>(s: Option<&str>, radix: i32) -> T {
    try_parse_radix(s, radix).unwrap_or_else(uncaught)
}

/// `parse_radix` where the program observes the `NumberFormatException`.
pub fn try_parse_radix<T: TryFrom<i128>>(s: Option<&str>, radix: i32) -> Result<T, Exception> {
    let fail = |message: String| Exception::new("java.lang.NumberFormatException", message);
    let Some(s) = s else { return Err(fail("Cannot parse null string: null".to_string())) };
    let value = read(s, radix).map_err(fail)?;
    T::try_from(value).map_err(|_| fail(for_input(s, radix)))
}

/// The value of a string in `radix`, or Java's message for why it is rejected. Only the range of
/// the target type can reject a number that is read, and `parse_radix` checks that.
fn read(s: &str, radix: i32) -> Result<i128, String> {
    check_radix(radix)?;
    let units: Vec<u16> = s.encode_utf16().collect();
    let (negative, digits) = match units.first() {
        Some(&c) if c == u16::from(b'-') => (true, &units[1..]),
        Some(&c) if c == u16::from(b'+') => (false, &units[1..]),
        _ => (false, &units[..]),
    };
    if digits.is_empty() {
        return Err(for_input(s, radix));
    }
    let mut value = 0i128;
    for &c in digits {
        let d = digit(c, radix);
        if d < 0 {
            return Err(for_input(s, radix));
        }
        value = (value * i128::from(radix) + i128::from(d)).min(CAP);
    }
    Ok(if negative { -value } else { value })
}

/// Java's check of the radix of the parse methods, with the message it throws.
pub(crate) fn check_radix(radix: i32) -> Result<(), String> {
    if radix < MIN_RADIX {
        return Err(format!("radix {radix} less than Character.MIN_RADIX"));
    }
    if radix > MAX_RADIX {
        return Err(format!("radix {radix} greater than Character.MAX_RADIX"));
    }
    Ok(())
}

/// Java's `NumberFormatException.forInputString`, which names the radix unless it is 10.
pub(crate) fn for_input(s: &str, radix: i32) -> String {
    let radix = if radix == 10 { String::new() } else { format!(" under radix {radix}") };
    format!("For input string: \"{s}\"{radix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_signs_and_every_radix() {
        assert_eq!(read("-ff", 16), Ok(-255));
        assert_eq!(read("+7", 10), Ok(7));
        assert_eq!(read("zz", 36), Ok(1295));
        assert_eq!(read("1010", 2), Ok(10));
        assert_eq!(read("-0", 8), Ok(0));
        assert_eq!(parse_radix::<i32>(Some("-80000000"), 16), i32::MIN);
        assert_eq!(parse_radix::<i64>(Some("7fffffffffffffff"), 16), i64::MAX);
    }

    #[test]
    fn reads_decimal_digits_of_every_script() {
        // U+0661 ARABIC-INDIC DIGIT ONE, U+FF12 FULLWIDTH DIGIT TWO, U+0966 DEVANAGARI DIGIT ZERO.
        assert_eq!(read("\u{661}\u{ff12}\u{966}", 10), Ok(120));
        assert_eq!(read("\u{ff21}\u{ff41}", 16), Ok(0xaa));
    }

    #[test]
    fn rejects_what_java_rejects() {
        for (bad, radix) in [
            ("", 10),
            ("-", 10),
            ("+", 10),
            ("--1", 10),
            ("+-1", 10),
            (" 1", 10),
            ("1 ", 10),
            ("1_0", 10),
            ("2", 2),
            ("g", 16),
            ("1\u{1f600}", 10),
            ("\u{1d7d9}", 10),
        ] {
            assert!(read(bad, radix).is_err(), "{bad:?} in {radix}");
        }
        assert_eq!(read("g", 16).unwrap_err(), "For input string: \"g\" under radix 16");
        assert_eq!(read("", 10).unwrap_err(), "For input string: \"\"");
        assert_eq!(read("1", 1).unwrap_err(), "radix 1 less than Character.MIN_RADIX");
        assert_eq!(read("1", 37).unwrap_err(), "radix 37 greater than Character.MAX_RADIX");
    }

    #[test]
    fn range_of_the_target_type_rejects_large_values() {
        assert!(i32::try_from(read("2147483648", 10).unwrap()).is_err());
        assert!(i32::try_from(read("-2147483649", 10).unwrap()).is_err());
        assert!(i64::try_from(read("9223372036854775808", 10).unwrap()).is_err());
        assert!(i64::try_from(read(&"9".repeat(200), 10).unwrap()).is_err());
        assert_eq!(i32::try_from(read("-2147483648", 10).unwrap()), Ok(i32::MIN));
    }
}
