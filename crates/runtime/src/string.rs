//! Java's `String` methods on Rust strings. Java indexes a string by UTF-16 code unit, so every
//! index here counts code units: on an ASCII string they are its bytes, and other strings are
//! walked or converted to their code units. A result that would cut a surrogate pair in half
//! holds a lone surrogate in Java, which no Rust string can; it becomes `?`, as Java prints it.
//!
//! Out-of-range indices throw `StringIndexOutOfBoundsException` with Java's message.

use crate::{is_java_whitespace, throw};

/// The UTF-16 code units of `s`.
pub fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// A string from UTF-16 code units; a lone surrogate becomes `?`, as Java prints it.
pub fn from_units(units: &[u16]) -> String {
    char::decode_utf16(units.iter().copied()).map(|c| c.unwrap_or('?')).collect()
}

/// Java's `s.length()`: the number of UTF-16 code units.
fn length(s: &str) -> usize {
    if s.is_ascii() { s.len() } else { s.encode_utf16().count() }
}

/// The byte offset in `s` of the code unit at `index` (which may be the length), or `None` when
/// it is the second unit of a surrogate pair.
fn byte_offset(s: &str, index: usize) -> Option<usize> {
    let mut units = 0;
    for (offset, c) in s.char_indices() {
        if units == index {
            return Some(offset);
        }
        units += c.len_utf16();
        if units > index {
            return None;
        }
    }
    (units == index).then_some(s.len())
}

/// The byte offset of the first character that starts at or after code unit `index`.
fn byte_offset_at_or_after(s: &str, index: usize) -> usize {
    let mut units = 0;
    for (offset, c) in s.char_indices() {
        if units >= index {
            return offset;
        }
        units += c.len_utf16();
    }
    s.len()
}

/// The code unit index of the byte offset `offset`, which is on a character boundary.
fn unit_index(s: &str, offset: usize) -> i32 {
    if s.is_ascii() { offset as i32 } else { s[..offset].encode_utf16().count() as i32 }
}

/// Java's check of a range `[begin, end)` of a string of `length` units.
fn check_begin_end(begin: i32, end: i32, length: usize) -> Result<(), crate::Exception> {
    if begin < 0 || begin > end || end as i64 > length as i64 {
        let message = format!("Range [{begin}, {end}) out of bounds for length {length}");
        return Err(crate::Exception::new("java.lang.StringIndexOutOfBoundsException", message));
    }
    Ok(())
}

/// `s.substring(begin)`.
pub fn substring_from(s: &str, begin: i32) -> String {
    try_substring_from(s, begin).unwrap_or_else(crate::uncaught)
}

/// `s.substring(begin, end)`.
pub fn substring(s: &str, begin: i32, end: i32) -> String {
    try_substring(s, begin, end).unwrap_or_else(crate::uncaught)
}

/// `substring_from` where the program observes the `StringIndexOutOfBoundsException`.
pub fn try_substring_from(s: &str, begin: i32) -> Result<String, crate::Exception> {
    let len = length(s);
    try_substring(s, begin, len as i32)
}

/// `substring` where the program observes the `StringIndexOutOfBoundsException`.
pub fn try_substring(s: &str, begin: i32, end: i32) -> Result<String, crate::Exception> {
    if s.is_ascii() {
        check_begin_end(begin, end, s.len())?;
        return Ok(s[begin as usize..end as usize].to_string());
    }
    check_begin_end(begin, end, length(s))?;
    Ok(match (byte_offset(s, begin as usize), byte_offset(s, end as usize)) {
        (Some(a), Some(b)) => s[a..b].to_string(),
        _ => from_units(&units(s)[begin as usize..end as usize]),
    })
}

/// `s.indexOf(ch, from)`, where `ch` is a code point (a `char` widened to `int`).
pub fn index_of_char(s: &str, ch: i32, from: i32) -> i32 {
    let from = from.max(0) as usize;
    if s.is_ascii() {
        if !(0..0x80).contains(&ch) || from >= s.len() {
            return -1;
        }
        return s.as_bytes()[from..].iter().position(|&b| b as i32 == ch).map_or(-1, |i| (i + from) as i32);
    }
    let units = units(s);
    let mut buf = [0u16; 2];
    let target: &[u16] = match u32::try_from(ch) {
        Ok(c) if c < 0x10000 => {
            buf[0] = c as u16;
            &buf[..1]
        }
        Ok(c) => match char::from_u32(c) {
            Some(c) => c.encode_utf16(&mut buf),
            None => return -1,
        },
        Err(_) => return -1,
    };
    let last = units.len().saturating_sub(target.len() - 1);
    (from..last).find(|&i| units[i..].starts_with(target)).map_or(-1, |i| i as i32)
}

/// `s.lastIndexOf(ch, from)`, where `ch` is a code point.
pub fn last_index_of_char(s: &str, ch: i32, from: i32) -> i32 {
    if from < 0 {
        return -1;
    }
    if s.is_ascii() {
        if !(0..0x80).contains(&ch) || s.is_empty() {
            return -1;
        }
        let end = (from as usize).min(s.len() - 1);
        return s.as_bytes()[..=end].iter().rposition(|&b| b as i32 == ch).map_or(-1, |i| i as i32);
    }
    let units = units(s);
    let mut buf = [0u16; 2];
    let target: &[u16] = match u32::try_from(ch) {
        Ok(c) if c < 0x10000 => {
            buf[0] = c as u16;
            &buf[..1]
        }
        Ok(c) => match char::from_u32(c) {
            Some(c) => c.encode_utf16(&mut buf),
            None => return -1,
        },
        Err(_) => return -1,
    };
    if units.len() < target.len() {
        return -1;
    }
    let end = (from as usize).min(units.len() - target.len());
    (0..=end).rev().find(|&i| units[i..].starts_with(target)).map_or(-1, |i| i as i32)
}

/// `s.indexOf(t, from)`.
pub fn index_of(s: &str, t: &str, from: i32) -> i32 {
    if s.is_ascii() {
        let from = from.clamp(0, s.len() as i32) as usize;
        return s[from..].find(t).map_or(-1, |i| (i + from) as i32);
    }
    let len = length(s);
    let from = from.clamp(0, len as i32) as usize;
    if t.is_empty() {
        return from as i32;
    }
    // A match starts on a character: `t` cannot start with the second half of a pair.
    let start = byte_offset_at_or_after(s, from);
    s[start..].find(t).map_or(-1, |i| unit_index(s, start + i))
}

/// `s.lastIndexOf(t, from)`.
pub fn last_index_of(s: &str, t: &str, from: i32) -> i32 {
    let (len, t_len) = (length(s) as i64, length(t) as i64);
    let from = (from as i64).min(len - t_len);
    if from < 0 {
        return -1;
    }
    if t.is_empty() {
        return from as i32;
    }
    if s.is_ascii() {
        let end = (from as usize + t.len()).min(s.len());
        return s[..end].rfind(t).map_or(-1, |i| i as i32);
    }
    // The match starts at a character that starts at or before unit `from`.
    let mut starts = Vec::new();
    let mut units = 0;
    for (offset, c) in s.char_indices() {
        if units > from as usize {
            break;
        }
        starts.push((offset, units));
        units += c.len_utf16();
    }
    starts.into_iter().rev().find(|&(offset, _)| s[offset..].starts_with(t)).map_or(-1, |(_, u)| u as i32)
}

/// `s.startsWith(prefix, offset)`.
pub fn starts_with_at(s: &str, prefix: &str, offset: i32) -> bool {
    let (len, p_len) = (length(s) as i64, length(prefix) as i64);
    if offset < 0 || offset as i64 > len - p_len {
        return false;
    }
    if prefix.is_empty() {
        return true;
    }
    match byte_offset(s, offset as usize) {
        Some(at) => s[at..].starts_with(prefix),
        None => false,
    }
}

/// Java's `s.isBlank()`: empty or only whitespace, as `Character.isWhitespace` defines it.
pub fn is_blank(s: &str) -> bool {
    s.chars().all(|c| is_java_whitespace(c as u32))
}

/// `s.trim()`: without the code units up to `' '` at both ends.
pub fn trim(s: &str) -> String {
    s.trim_matches(|c: char| c <= ' ').to_string()
}

/// `s.strip()`: without whitespace (`Character.isWhitespace`) at both ends.
pub fn strip(s: &str) -> String {
    s.trim_matches(|c: char| is_java_whitespace(c as u32)).to_string()
}

/// `s.stripLeading()`.
pub fn strip_leading(s: &str) -> String {
    s.trim_start_matches(|c: char| is_java_whitespace(c as u32)).to_string()
}

/// `s.stripTrailing()`.
pub fn strip_trailing(s: &str) -> String {
    s.trim_end_matches(|c: char| is_java_whitespace(c as u32)).to_string()
}

/// `s.toUpperCase()` in the root locale: Unicode's full case mapping, so `ß` is `SS`.
pub fn uppercase(s: &str) -> String {
    if s.is_ascii() { s.to_ascii_uppercase() } else { s.to_uppercase() }
}

/// `s.toLowerCase()` in the root locale: Unicode's full case mapping, with a final `Σ` as `ς`.
pub fn lowercase(s: &str) -> String {
    if s.is_ascii() { s.to_ascii_lowercase() } else { s.to_lowercase() }
}

/// `s.replace(old, new)` on `char`s, which replaces code units, even halves of a pair.
pub fn replace_char(s: &str, old: u16, new: u16) -> String {
    let surrogate = |c: u16| (0xD800..0xE000).contains(&c);
    if !surrogate(old) && !surrogate(new) {
        let (old, new) = (char::from_u32(old as u32).unwrap(), char::from_u32(new as u32).unwrap());
        return s.chars().map(|c| if c == old { new } else { c }).collect();
    }
    let replaced: Vec<u16> = s.encode_utf16().map(|c| if c == old { new } else { c }).collect();
    from_units(&replaced)
}

/// `s.replace(target, replacement)` on strings. An empty target matches before every code unit
/// and at the end.
pub fn replace(s: &str, target: &str, replacement: &str) -> String {
    if !target.is_empty() {
        return s.replace(target, replacement);
    }
    if s.is_ascii() {
        return s.replace("", replacement);
    }
    let r = units(replacement);
    let mut out = r.clone();
    for unit in s.encode_utf16() {
        out.push(unit);
        out.extend_from_slice(&r);
    }
    from_units(&out)
}

/// `s.repeat(count)`.
pub fn repeat(s: &str, count: i32) -> String {
    if count < 0 {
        throw("java.lang.IllegalArgumentException", &format!("count is negative: {count}"));
    }
    if (length(s) as u64) * (count as u64) > i32::MAX as u64 {
        throw("java.lang.OutOfMemoryError", "Required length exceeds implementation limit");
    }
    s.repeat(count as usize)
}

/// `a.compareTo(b)`: the difference of the first code units that differ, or of the lengths.
pub fn compare_to(a: &str, b: &str) -> i32 {
    if a.is_ascii() && b.is_ascii() {
        let (x, y) = (a.as_bytes(), b.as_bytes());
        return match x.iter().zip(y).find(|(p, q)| p != q) {
            Some((p, q)) => *p as i32 - *q as i32,
            None => x.len() as i32 - y.len() as i32,
        };
    }
    let mut x = a.encode_utf16();
    let mut y = b.encode_utf16();
    loop {
        match (x.next(), y.next()) {
            (Some(p), Some(q)) if p == q => {}
            (Some(p), Some(q)) => return p as i32 - q as i32,
            (Some(_), None) => return 1 + x.count() as i32,
            (None, Some(_)) => return -1 - y.count() as i32,
            (None, None) => return 0,
        }
    }
}

/// `Character.toUpperCase(int)`: the simple case mapping of a code point.
fn upper_code_point(cp: u32) -> u32 {
    match char::from_u32(cp) {
        Some(c) => {
            let mut mapped = c.to_uppercase();
            match (mapped.next(), mapped.next()) {
                (Some(u), None) => u as u32,
                _ => cp,
            }
        }
        None => cp,
    }
}

/// `Character.toLowerCase(int)`: the simple case mapping of a code point.
fn lower_code_point(cp: u32) -> u32 {
    if cp == 0x130 {
        return 'i' as u32;
    }
    match char::from_u32(cp) {
        Some(c) => {
            let mut mapped = c.to_lowercase();
            match (mapped.next(), mapped.next()) {
                (Some(l), None) => l as u32,
                _ => cp,
            }
        }
        None => cp,
    }
}

/// Java's case-insensitive comparison of two code points: through upper case, then lower case.
fn compare_code_point_ci(a: u32, b: u32) -> i32 {
    let (a, b) = (upper_code_point(a), upper_code_point(b));
    if a == b {
        return 0;
    }
    let (a, b) = (lower_code_point(a), lower_code_point(b));
    a as i32 - b as i32
}

/// The code point that the code unit at `k` belongs to, as Java's `codePointIncluding` finds it:
/// a low surrogate joins the high one before it, a high surrogate the low one after it (then the
/// flag says that one more unit was used).
fn code_point_including(units: &[u16], k: usize) -> (u32, bool) {
    let c = units[k];
    let high = |c: u16| (0xD800..0xDC00).contains(&c);
    let low = |c: u16| (0xDC00..0xE000).contains(&c);
    let pair = |h: u16, l: u16| 0x10000 + ((h as u32 - 0xD800) << 10) + (l as u32 - 0xDC00);
    if low(c) && k > 0 && high(units[k - 1]) {
        (pair(units[k - 1], c), false)
    } else if high(c) && k + 1 < units.len() && low(units[k + 1]) {
        (pair(c, units[k + 1]), true)
    } else {
        (c as u32, false)
    }
}

/// `a.compareToIgnoreCase(b)`.
pub fn compare_to_ignore_case(a: &str, b: &str) -> i32 {
    if a.is_ascii() && b.is_ascii() {
        let (x, y) = (a.as_bytes(), b.as_bytes());
        for (p, q) in x.iter().zip(y) {
            if p != q {
                let (p, q) = (p.to_ascii_lowercase(), q.to_ascii_lowercase());
                if p != q {
                    return p as i32 - q as i32;
                }
            }
        }
        return x.len() as i32 - y.len() as i32;
    }
    let (x, y) = (units(a), units(b));
    let (mut k1, mut k2) = (0, 0);
    while k1 < x.len() && k2 < y.len() {
        let (c1, c2) = (x[k1] as u32, y[k2] as u32);
        if c1 != c2 && compare_code_point_ci(c1, c2) != 0 {
            let (cp1, skip1) = code_point_including(&x, k1);
            let (cp2, skip2) = code_point_including(&y, k2);
            k1 += skip1 as usize;
            k2 += skip2 as usize;
            let diff = compare_code_point_ci(cp1, cp2);
            if diff != 0 {
                return diff;
            }
        }
        k1 += 1;
        k2 += 1;
    }
    x.len() as i32 - y.len() as i32
}

/// `a.equalsIgnoreCase(b)`.
pub fn equals_ignore_case(a: &str, b: &str) -> bool {
    if a.len() == b.len() && a.eq_ignore_ascii_case(b) {
        return true;
    }
    length(a) == length(b) && compare_to_ignore_case(a, b) == 0
}

/// `s.hashCode()`: `s[0]*31^(n-1) + ... + s[n-1]` over the code units, wrapping.
pub fn string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// `new String(chars, offset, count)` and `String.valueOf(chars, offset, count)`.
pub fn from_chars_range(chars: &[u16], offset: i32, count: i32) -> String {
    try_from_chars_range(chars, offset, count).unwrap_or_else(crate::uncaught)
}

/// `from_chars_range` where the program observes the `StringIndexOutOfBoundsException`.
pub fn try_from_chars_range(chars: &[u16], offset: i32, count: i32) -> Result<String, crate::Exception> {
    let len = chars.len();
    if offset < 0 || count < 0 || offset as i64 + count as i64 > len as i64 {
        let message = format!("offset {offset}, count {count}, length {len}");
        return Err(crate::Exception::new("java.lang.StringIndexOutOfBoundsException", message));
    }
    Ok(from_units(&chars[offset as usize..(offset + count) as usize]))
}

/// The text of each element of a `String.join`: a string, or `null`.
pub trait JoinItem {
    fn text(&self) -> &str;
}

impl JoinItem for String {
    fn text(&self) -> &str {
        self
    }
}

impl JoinItem for &str {
    fn text(&self) -> &str {
        self
    }
}

impl<T: JoinItem> JoinItem for Option<T> {
    fn text(&self) -> &str {
        self.as_ref().map_or("null", JoinItem::text)
    }
}

/// `String.join(delimiter, elements)`.
pub fn join<T: JoinItem>(delimiter: &str, elements: &[T]) -> String {
    let mut out = String::new();
    for (i, e) in elements.iter().enumerate() {
        if i > 0 {
            out.push_str(delimiter);
        }
        out.push_str(e.text());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substrings_count_code_units() {
        assert_eq!(substring_from("hello", 2), "llo");
        assert_eq!(substring("héllo", 1, 3), "él");
        assert_eq!(substring("a😀b", 1, 3), "😀");
        assert_eq!(substring("a😀b", 1, 2), "?");
        assert_eq!(substring_from("abc", 3), "");
    }

    #[test]
    fn searches_count_code_units() {
        assert_eq!(index_of_char("a😀b", 'b' as i32, 0), 3);
        assert_eq!(index_of_char("a😀b", 0x1F600, 0), 1);
        assert_eq!(index_of_char("a😀b", 0xDE00, 0), 2);
        assert_eq!(last_index_of_char("abcabc", 'b' as i32, 3), 1);
        assert_eq!(last_index_of_char("abcabc", 'b' as i32, 100), 4);
        assert_eq!(index_of("a😀bcb", "b", 0), 3);
        assert_eq!(index_of("a😀bcb", "b", 4), 5);
        assert_eq!(index_of("abc", "", 5), 3);
        assert_eq!(last_index_of("a😀bcb", "b", 4), 3);
        assert_eq!(last_index_of("abcab", "ab", 10), 3);
        assert_eq!(last_index_of("abc", "", 10), 3);
        assert_eq!(last_index_of("éaé", "é", 1), 0);
        assert_eq!(last_index_of("éaé", "é", 2), 2);
    }

    #[test]
    fn comparisons_follow_java() {
        assert_eq!(compare_to("apple", "banana"), -1);
        assert_eq!(compare_to("ab", "abcd"), -2);
        assert_eq!(compare_to("\u{FFFF}", "😀"), 0xFFFF - 0xD83D);
        assert_eq!(compare_to_ignore_case("Hello", "hello"), 0);
        assert_eq!(compare_to_ignore_case("a", "B"), -1);
        assert!(equals_ignore_case("STRASSE", "strasse"));
        assert!(!equals_ignore_case("ß", "SS"));
        assert_eq!(string_hash("hello"), 99162322);
        assert_eq!(string_hash(""), 0);
    }

    #[test]
    fn replacing_and_trimming() {
        assert_eq!(replace("abc", "", "-"), "-a-b-c-");
        assert_eq!(replace("aaa", "aa", "b"), "ba");
        assert_eq!(replace_char("a.b.c", '.' as u16, '/' as u16), "a/b/c");
        assert_eq!(trim(" \t x y \n"), "x y");
        assert_eq!(strip("\u{2003}x\u{A0}"), "x\u{A0}");
        assert!(is_blank(" \u{2028}\u{1F}"));
        assert_eq!(uppercase("straße"), "STRASSE");
        assert_eq!(join(", ", &[Some("a"), None]), "a, null");
    }
}
