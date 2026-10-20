//! The code point methods of `Character`. A code point is an `int`, which may be a supplementary
//! character that one `char` (a UTF-16 unit) cannot hold, so it is one or two `u16` units.

use crate::{Array, from_vec, throw};

const MIN_SUPPLEMENTARY: i32 = 0x1_0000;
const MAX_CODE_POINT: i32 = 0x10_FFFF;
const MIN_HIGH_SURROGATE: u32 = 0xD800;
const MIN_LOW_SURROGATE: u32 = 0xDC00;

/// Java's `Character.isValidCodePoint`.
pub fn is_valid_code_point(cp: i32) -> bool {
    (0..=MAX_CODE_POINT).contains(&cp)
}

/// Java's `Character.isBmpCodePoint`: the code point fits in one `char`.
pub fn is_bmp_code_point(cp: i32) -> bool {
    (cp as u32) >> 16 == 0
}

/// Java's `Character.isSupplementaryCodePoint`.
pub fn is_supplementary_code_point(cp: i32) -> bool {
    (MIN_SUPPLEMENTARY..=MAX_CODE_POINT).contains(&cp)
}

/// Java's `Character.charCount`: 2 for a supplementary code point, else 1.
pub fn char_count(cp: i32) -> i32 {
    if cp >= MIN_SUPPLEMENTARY { 2 } else { 1 }
}

/// Java's `Character.toCodePoint(high, low)`, which does not check that the units are surrogates.
pub fn to_code_point(high: u16, low: u16) -> i32 {
    let offset = MIN_SUPPLEMENTARY - ((MIN_HIGH_SURROGATE as i32) << 10) - MIN_LOW_SURROGATE as i32;
    ((i32::from(high) << 10) + i32::from(low)).wrapping_add(offset)
}

/// Java's `Character.highSurrogate`: the leading unit of a supplementary code point.
pub fn high_surrogate(cp: i32) -> u16 {
    (((cp as u32) >> 10).wrapping_add(MIN_HIGH_SURROGATE - (MIN_SUPPLEMENTARY as u32 >> 10))) as u16
}

/// Java's `Character.lowSurrogate`: the trailing unit of a supplementary code point.
pub fn low_surrogate(cp: i32) -> u16 {
    ((cp & 0x3FF) as u32 + MIN_LOW_SURROGATE) as u16
}

/// Java's `Character.toChars(cp)`, which throws `IllegalArgumentException` for a code point out
/// of range.
pub fn to_chars(cp: i32) -> Array<u16> {
    from_vec(units(cp).unwrap_or_else(|message| throw("java.lang.IllegalArgumentException", &message)))
}

/// Java's `Character.toString(cp)`. A lone surrogate, which a Rust string cannot hold, prints as
/// `?`, as Java's UTF-8 output does.
pub fn code_point_string(cp: i32) -> String {
    let units = units(cp).unwrap_or_else(|message| throw("java.lang.IllegalArgumentException", &message));
    char::decode_utf16(units).map(|unit| unit.unwrap_or('?')).collect()
}

/// The units of a code point, or Java's message for one that is not valid.
fn units(cp: i32) -> Result<Vec<u16>, String> {
    if is_bmp_code_point(cp) {
        Ok(vec![cp as u16])
    } else if is_valid_code_point(cp) {
        Ok(vec![high_surrogate(cp), low_surrogate(cp)])
    } else {
        Err(format!("Not a valid Unicode code point: 0x{:X}", cp as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_joins_supplementary_code_points() {
        // U+1F600 is 0xD83D 0xDE00 in UTF-16.
        assert_eq!(high_surrogate(0x1_F600), 0xD83D);
        assert_eq!(low_surrogate(0x1_F600), 0xDE00);
        assert_eq!(to_code_point(0xD83D, 0xDE00), 0x1_F600);
        assert_eq!(to_code_point(0xDBFF, 0xDFFF), 0x10_FFFF);
        assert_eq!(high_surrogate(0x1_0000), 0xD800);
        assert_eq!(low_surrogate(0x10_FFFF), 0xDFFF);
    }

    #[test]
    fn classifies_code_points_as_java_does() {
        assert_eq!(char_count(0xFFFF), 1);
        assert_eq!(char_count(0x1_0000), 2);
        assert!(is_bmp_code_point(0xFFFF) && !is_bmp_code_point(0x1_0000));
        assert!(is_supplementary_code_point(0x10_FFFF) && !is_supplementary_code_point(0x11_0000));
        assert!(is_valid_code_point(0) && is_valid_code_point(0x10_FFFF));
        assert!(!is_valid_code_point(0x11_0000) && !is_valid_code_point(-1));
        assert!(!is_bmp_code_point(-1) && !is_supplementary_code_point(-1));
    }

    #[test]
    fn units_of_a_code_point() {
        assert_eq!(units(0x41), Ok(vec![0x41]));
        assert_eq!(units(0x1_F600), Ok(vec![0xD83D, 0xDE00]));
        assert_eq!(code_point_string(0x1_F600), "\u{1F600}");
        assert_eq!(code_point_string(0xD800), "?");
    }

    #[test]
    fn rejects_what_java_rejects_with_java_s_message() {
        assert_eq!(units(-1), Err("Not a valid Unicode code point: 0xFFFFFFFF".to_string()));
        assert_eq!(units(0x11_0000), Err("Not a valid Unicode code point: 0x110000".to_string()));
    }
}
