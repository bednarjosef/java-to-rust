//! The decimal digits of the Basic Multilingual Plane, the characters of Unicode general category
//! Nd that `Character.digit` accepts. A run is ten code units: its zero, then the nine after it
//! as 1 to 9. Supplementary digits are surrogate pairs in a `String`, so Java's `char`-based
//! parsing never sees them.

/// The zero of each run of decimal digits in the Basic Multilingual Plane, in ascending order.
/// Generated from `Character.getType` and `Character.digit` of Java 26.
const ZEROS: [u16; 37] = [
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6, 0x0D66,
    0x0DE6, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50,
    0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
];

/// The value of a decimal digit in the Basic Multilingual Plane, `None` for any other code unit.
pub fn decimal_digit(c: u16) -> Option<u32> {
    let run = ZEROS.partition_point(|&zero| zero <= c).checked_sub(1)?;
    let value = c - ZEROS[run];
    (value < 10).then_some(u32::from(value))
}

#[cfg(test)]
mod tests {
    use super::decimal_digit;

    #[test]
    fn reads_decimal_digits_of_every_script() {
        assert_eq!(decimal_digit(u16::from(b'7')), Some(7));
        assert_eq!(decimal_digit(0x0669), Some(9));
        assert_eq!(decimal_digit(0xFF10), Some(0));
        assert_eq!(decimal_digit(0x0E59), Some(9));
        assert_eq!(decimal_digit(0x0660 + 10), None);
        assert_eq!(decimal_digit(u16::from(b'a')), None);
        assert_eq!(decimal_digit(0x00B2), None);
    }
}
