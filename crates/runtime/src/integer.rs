//! The bit and radix methods of `Integer` and `Long` that Rust has no one method for.

/// `Integer.highestOneBit` and `Long.highestOneBit` of a value of `bits` bits: the highest set
/// bit alone, or 0.
pub fn highest_one_bit(v: i64, bits: u32) -> i64 {
    let unsigned = if bits == 64 { v as u64 } else { (v as u64) & ((1 << bits) - 1) };
    if unsigned == 0 { 0 } else { (1u64 << (63 - unsigned.leading_zeros())) as i64 }
}

/// `Integer.toString(v, radix)` and `Long.toString(v, radix)`: a radix outside 2 to 36 is 10,
/// and a negative value has a `-` sign.
pub fn to_string_radix(v: i64, radix: i32) -> String {
    let radix = if (2..=36).contains(&radix) { radix as u64 } else { 10 };
    let mut magnitude = v.unsigned_abs();
    if magnitude == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while magnitude > 0 {
        digits.push(std::char::from_digit((magnitude % radix) as u32, radix as u32).expect("a digit"));
        magnitude /= radix;
    }
    if v < 0 {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radix_conversions_follow_java() {
        assert_eq!(to_string_radix(255, 16), "ff");
        assert_eq!(to_string_radix(-255, 2), "-11111111");
        assert_eq!(to_string_radix(i64::MIN, 10), "-9223372036854775808");
        assert_eq!(to_string_radix(7, 99), "7");
        assert_eq!(crate::parse_radix::<i32>(Some("-ff"), 16), -255);
        assert_eq!(crate::parse_radix::<i32>(Some("-80000000"), 16), i32::MIN);
        assert_eq!(highest_one_bit(-1, 32), 1 << 31);
        assert_eq!(highest_one_bit(10, 32), 8);
        assert_eq!(highest_one_bit(0, 64), 0);
    }
}
