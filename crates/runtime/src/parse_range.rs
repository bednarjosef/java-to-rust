//! The `CharSequence` forms of the parse methods, `parseInt(s, begin, end, radix)` and the like,
//! which read the UTF-16 units from `begin` up to `end`. Java checks the arguments in this order:
//! the string, the range, the radix, then the digits. Each error index is relative to `begin`, and
//! the message quotes the characters read, except the unsigned range messages, which quote the
//! whole string.

use crate::character_classes::digit;
use crate::integer_parse::{check_radix, for_input};
use crate::{Exception, uncaught};

const NUMBER_FORMAT: &str = "java.lang.NumberFormatException";

/// `Integer.parseInt(s, begin, end, radix)` (`bits` 32) and `Long.parseLong` (`bits` 64). `None` is
/// a null string, a `NullPointerException`.
pub fn parse_range<T: TryFrom<i128>>(
    s: Option<&crate::Wtf8>,
    begin: i32,
    end: i32,
    radix: i32,
    bits: u32,
) -> T {
    try_parse_range(s, begin, end, radix, bits).unwrap_or_else(uncaught)
}

/// The checked form of `parse_range`.
pub fn try_parse_range<T: TryFrom<i128>>(
    s: Option<&crate::Wtf8>,
    begin: i32,
    end: i32,
    radix: i32,
    bits: u32,
) -> Result<T, Exception> {
    let s = s.map(|s| s.to_string_lossy());
    let s = s.as_deref();
    let units = check_range(s, begin, end)?;
    let value = read_signed(&units, begin as usize, end as usize, radix, bits)?;
    // The loop never reads a value outside the range of `bits`.
    Ok(T::try_from(value).unwrap_or_else(|_| unreachable!("the digits fit in {bits} bits")))
}

/// `Integer.parseUnsignedInt(s, begin, end, radix)` (`bits` 32) and `Long.parseUnsignedLong` (`bits`
/// 64), as the `u64` of the value; the caller casts it to the Java type. `None` is a null string.
pub fn parse_unsigned_range(s: Option<&crate::Wtf8>, begin: i32, end: i32, radix: i32, bits: u32) -> u64 {
    try_parse_unsigned_range(s, begin, end, radix, bits).unwrap_or_else(uncaught)
}

/// The checked form of `parse_unsigned_range`. Java hands short strings to `parseInt` and
/// `parseLong`, and longer ones to a loop that checks the range of the unsigned type.
pub fn try_parse_unsigned_range(
    s: Option<&crate::Wtf8>,
    begin: i32,
    end: i32,
    radix: i32,
    bits: u32,
) -> Result<u64, Exception> {
    let s = s.map(|s| s.to_string_lossy());
    let s = s.as_deref();
    let units = check_range(s, begin, end)?;
    let (begin, end) = (begin as usize, end as usize);
    if begin == end {
        return Err(Exception::new(NUMBER_FORMAT, for_input("", radix)));
    }
    if units[begin] == u16::from(b'-') {
        let text = String::from_utf16_lossy(&units);
        return Err(Exception::new(
            NUMBER_FORMAT,
            format!("Illegal leading minus sign on unsigned string {text}."),
        ));
    }
    let len = end - begin;
    let short = if bits == 32 {
        len <= 5 || (radix == 10 && len <= 9)
    } else {
        len <= 12 || (radix == 10 && len <= 18)
    };
    if short {
        return Ok(read_signed(&units, begin, end, radix, bits)? as u64);
    }
    if bits == 32 {
        // Read as a `long` first, so a value beyond `long` is the `long` error.
        let value = read_signed(&units, begin, end, radix, 64)? as u64;
        return if value >> 32 == 0 { Ok(value) } else { Err(exceeds(&units, "int")) };
    }
    check_radix(radix).map_err(|message| Exception::new(NUMBER_FORMAT, message))?;
    let mut value = 0u128;
    let mut i = begin;
    if units[i] == u16::from(b'+') {
        i += 1;
        if i == end {
            return Err(error_at(&units, begin, end, i));
        }
    }
    while i < end {
        let d = digit(units[i], radix);
        if d < 0 {
            return Err(error_at(&units, begin, end, i));
        }
        // `value` is at most `u64::MAX` before a step, so this cannot overflow `u128`.
        value = value * radix as u128 + d as u128;
        if value > u128::from(u64::MAX) {
            return Err(exceeds(&units, "long"));
        }
        i += 1;
    }
    Ok(value as u64)
}

/// The string as UTF-16 units, once Java's `requireNonNull` and `checkFromToIndex` have passed.
fn check_range(s: Option<&str>, begin: i32, end: i32) -> Result<Vec<u16>, Exception> {
    let Some(s) = s else { return Err(Exception::bare("java.lang.NullPointerException")) };
    let units: Vec<u16> = s.encode_utf16().collect();
    if begin < 0 || begin > end || end as usize > units.len() {
        let message = format!("Range [{begin}, {end}) out of bounds for length {}", units.len());
        return Err(Exception::new("java.lang.IndexOutOfBoundsException", message));
    }
    Ok(units)
}

/// Java's loop for `parseInt` (`bits` 32) and `parseLong` (`bits` 64) over `units[begin..end]`.
/// It accumulates negatively, so the one extra value of the negative range is read, and it checks
/// each digit as it is read.
fn read_signed(units: &[u16], begin: usize, end: usize, radix: i32, bits: u32) -> Result<i128, Exception> {
    check_radix(radix).map_err(|message| Exception::new(NUMBER_FORMAT, message))?;
    if begin == end {
        return Err(Exception::new(NUMBER_FORMAT, for_input("", radix)));
    }
    let bound = 1i128 << (bits - 1);
    let mut i = begin;
    let mut negative = false;
    let mut limit = 1 - bound;
    if units[i] < u16::from(b'0') {
        if units[i] == u16::from(b'-') {
            negative = true;
            limit = -bound;
        } else if units[i] != u16::from(b'+') {
            return Err(error_at(units, begin, end, i));
        }
        i += 1;
        if i == end {
            return Err(error_at(units, begin, end, i));
        }
    }
    let radix = i128::from(radix);
    let multmin = limit / radix;
    let mut result = 0i128;
    while i < end {
        let d = i128::from(digit(units[i], radix as i32));
        if d < 0 || result < multmin {
            return Err(error_at(units, begin, end, i));
        }
        result *= radix;
        if result < limit + d {
            return Err(error_at(units, begin, end, i));
        }
        i += 1;
        result -= d;
    }
    Ok(if negative { result } else { -result })
}

/// Java's `NumberFormatException.forCharSequence`: the index is relative to `begin`.
fn error_at(units: &[u16], begin: usize, end: usize, index: usize) -> Exception {
    let text = String::from_utf16_lossy(&units[begin..end]);
    Exception::new(NUMBER_FORMAT, format!("Error at index {} in: \"{text}\"", index - begin))
}

/// The unsigned overflow message, which quotes the whole string.
fn exceeds(units: &[u16], kind: &str) -> Exception {
    let text = String::from_utf16_lossy(units);
    Exception::new(NUMBER_FORMAT, format!("String value {text} exceeds range of unsigned {kind}."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message<T: std::fmt::Debug>(r: Result<T, Exception>) -> String {
        r.unwrap_err().to_string()
    }

    #[test]
    fn range_reads_the_indices_it_is_given() {
        assert_eq!(try_parse_range::<i32>(Some(crate::Wtf8::new("x-12y")), 1, 4, 10, 32), Ok(-12));
        assert_eq!(
            try_parse_range::<i64>(Some(crate::Wtf8::new("9223372036854775807")), 0, 19, 10, 64),
            Ok(i64::MAX)
        );
        assert_eq!(
            try_parse_range::<i32>(Some(crate::Wtf8::new("-2147483648")), 0, 11, 10, 32),
            Ok(i32::MIN)
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("abc")), 2, 1, 10, 32)),
            "java.lang.IndexOutOfBoundsException: Range [2, 1) out of bounds for length 3"
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("abc")), 0, 4, 10, 32)),
            "java.lang.IndexOutOfBoundsException: Range [0, 4) out of bounds for length 3"
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("a12")), 0, 3, 10, 32)),
            "java.lang.NumberFormatException: Error at index 0 in: \"a12\""
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("12x")), 0, 3, 10, 32)),
            "java.lang.NumberFormatException: Error at index 2 in: \"12x\""
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("2147483648")), 0, 10, 10, 32)),
            "java.lang.NumberFormatException: Error at index 9 in: \"2147483648\""
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("-")), 0, 1, 10, 32)),
            "java.lang.NumberFormatException: Error at index 1 in: \"-\""
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("7")), 0, 0, 10, 32)),
            "java.lang.NumberFormatException: For input string: \"\""
        );
        assert_eq!(
            message(try_parse_range::<i32>(Some(crate::Wtf8::new("7")), 0, 1, 1, 32)),
            "java.lang.NumberFormatException: radix 1 less than Character.MIN_RADIX"
        );
    }
}
