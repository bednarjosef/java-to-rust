//! Java's `String` methods on WTF-8 strings (`Wtf8`, `JString`), which hold Java's UTF-16 code
//! units exactly. Java indexes a string by code unit, so every index here counts code units: on
//! an ASCII string they are its bytes, and other strings are walked or converted to their code
//! units. A result that cuts a surrogate pair in half holds the lone surrogate, as Java's does.
//!
//! Out-of-range indices throw `StringIndexOutOfBoundsException` with Java's message.

use crate::{JString, Wtf8, is_java_whitespace, throw, throw_exception};

/// The UTF-16 code units of `s`.
pub fn units(s: &Wtf8) -> Vec<u16> {
    s.units().collect()
}

/// A string from UTF-16 code units, exactly.
pub fn from_units(units: &[u16]) -> JString {
    JString::from_units(units)
}

/// Java's `s.length()`: the number of UTF-16 code units.
fn length(s: &Wtf8) -> usize {
    s.unit_len()
}

/// The units a code point takes.
fn unit_count(c: u32) -> usize {
    if c >= 0x10000 { 2 } else { 1 }
}

/// The byte offset in `s` of the code unit at `index` (which may be the length), or `None` when
/// it is the second unit of a surrogate pair.
fn byte_offset(s: &Wtf8, index: usize) -> Option<usize> {
    let mut units = 0;
    for (offset, c) in s.code_point_indices() {
        if units == index {
            return Some(offset);
        }
        units += unit_count(c);
        if units > index {
            return None;
        }
    }
    (units == index).then_some(s.len())
}

/// The byte offset of the first code point that starts at or after code unit `index`.
fn byte_offset_at_or_after(s: &Wtf8, index: usize) -> usize {
    let mut units = 0;
    for (offset, c) in s.code_point_indices() {
        if units >= index {
            return offset;
        }
        units += unit_count(c);
    }
    s.len()
}

/// The code unit index of the byte offset `offset`, which is on a code point boundary.
fn unit_index(s: &Wtf8, offset: usize) -> i32 {
    if s.is_ascii() { offset as i32 } else { s.slice(0, offset).unit_len() as i32 }
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
pub fn substring_from<S: Text + ?Sized>(s: &S, begin: i32) -> JString {
    try_substring_from(s, begin).unwrap_or_else(crate::uncaught)
}

/// A string that a `String` method taking a code unit index reads: its text, and whether that
/// is all ASCII, in which case a unit index is a byte offset. A `JString` and a `JStr` keep that
/// (Java's coder; `JString::is_ascii_text`), so a loop over a long string that is not known to be
/// ASCII from its translation does not scan all of it at each call; a borrowed `Wtf8` is scanned.
pub trait Text {
    fn wtf8_text(&self) -> &Wtf8;
    fn ascii_text(&self) -> bool;
}

impl Text for Wtf8 {
    #[inline]
    fn wtf8_text(&self) -> &Wtf8 {
        self
    }
    #[inline]
    fn ascii_text(&self) -> bool {
        self.is_ascii()
    }
}

impl Text for JString {
    #[inline]
    fn wtf8_text(&self) -> &Wtf8 {
        self.as_wtf8()
    }
    #[inline]
    fn ascii_text(&self) -> bool {
        self.is_ascii_text()
    }
}

impl Text for crate::JStr {
    #[inline]
    fn wtf8_text(&self) -> &Wtf8 {
        self.as_wtf8()
    }
    #[inline]
    fn ascii_text(&self) -> bool {
        self.is_ascii_text()
    }
}

macro_rules! text_through {
    ($($t:ty),*) => {$(
        impl<T: Text + ?Sized> Text for $t {
            #[inline]
            fn wtf8_text(&self) -> &Wtf8 {
                (**self).wtf8_text()
            }
            #[inline]
            fn ascii_text(&self) -> bool {
                (**self).ascii_text()
            }
        }
    )*};
}

text_through!(&T, &mut T, Box<T>, crate::locked::Ref<'_, T>, crate::locked::RefMut<'_, T>);

/// `s.substring(begin, end)`.
pub fn substring<S: Text + ?Sized>(s: &S, begin: i32, end: i32) -> JString {
    try_substring(s, begin, end).unwrap_or_else(crate::uncaught)
}

/// `s.substring(begin, end)` of a string that the translation proved ASCII, whose code units are
/// its bytes: no scan of the string.
#[inline]
pub fn ascii_substring(s: &Wtf8, begin: i32, end: i32) -> JString {
    try_ascii_substring(s, begin, end).unwrap_or_else(crate::uncaught)
}

/// `s.substring(begin)` of an ASCII string.
#[inline]
pub fn ascii_substring_from(s: &Wtf8, begin: i32) -> JString {
    try_ascii_substring_from(s, begin).unwrap_or_else(crate::uncaught)
}

/// `ascii_substring` where the program observes the `StringIndexOutOfBoundsException`.
#[inline]
pub fn try_ascii_substring(s: &Wtf8, begin: i32, end: i32) -> Result<JString, crate::Exception> {
    check_begin_end(begin, end, s.len())?;
    Ok(JString::from_ascii(s.as_bytes()[begin as usize..end as usize].to_vec()))
}

/// `ascii_substring_from` where the program observes the `StringIndexOutOfBoundsException`.
#[inline]
pub fn try_ascii_substring_from(s: &Wtf8, begin: i32) -> Result<JString, crate::Exception> {
    try_ascii_substring(s, begin, s.len() as i32)
}

/// `substring_from` where the program observes the `StringIndexOutOfBoundsException`.
#[inline]
pub fn try_substring_from<S: Text + ?Sized>(s: &S, begin: i32) -> Result<JString, crate::Exception> {
    let (ascii, s) = (s.ascii_text(), s.wtf8_text());
    let len = if ascii { s.len() } else { length(s) };
    substring_in(s, ascii, begin, len as i32)
}

/// `substring` where the program observes the `StringIndexOutOfBoundsException`.
#[inline]
pub fn try_substring<S: Text + ?Sized>(s: &S, begin: i32, end: i32) -> Result<JString, crate::Exception> {
    substring_in(s.wtf8_text(), s.ascii_text(), begin, end)
}

/// `try_substring` of the text `s`, `ascii` when it is all ASCII.
fn substring_in(s: &Wtf8, ascii: bool, begin: i32, end: i32) -> Result<JString, crate::Exception> {
    if ascii {
        return try_ascii_substring(s, begin, end);
    }
    check_begin_end(begin, end, length(s))?;
    Ok(match (byte_offset(s, begin as usize), byte_offset(s, end as usize)) {
        (Some(a), Some(b)) => s.slice(a, b).to_owned(),
        _ => from_units(&units(s)[begin as usize..end as usize]),
    })
}

/// The code units of code point `ch` (a `char` widened to `int`, or a supplementary one), or
/// `None` for a value that is no code point.
fn code_point_units(ch: i32, buf: &mut [u16; 2]) -> Option<&[u16]> {
    match u32::try_from(ch) {
        Ok(c) if c < 0x10000 => {
            buf[0] = c as u16;
            Some(&buf[..1])
        }
        Ok(c) => char::from_u32(c).map(|c| &*c.encode_utf16(buf)),
        Err(_) => None,
    }
}

/// `s.indexOf(ch, from)`, where `ch` is a code point (a `char` widened to `int`).
#[inline]
pub fn index_of_char<S: Text + ?Sized>(s: &S, ch: i32, from: i32) -> i32 {
    index_of_char_in(s.wtf8_text(), s.ascii_text(), ch, from)
}

fn index_of_char_in(s: &Wtf8, ascii: bool, ch: i32, from: i32) -> i32 {
    let from = from.max(0) as usize;
    if ascii {
        if !(0..0x80).contains(&ch) || from >= s.len() {
            return -1;
        }
        return s.as_bytes()[from..].iter().position(|&b| b as i32 == ch).map_or(-1, |i| (i + from) as i32);
    }
    let units = units(s);
    let mut buf = [0u16; 2];
    let Some(target) = code_point_units(ch, &mut buf) else { return -1 };
    let last = units.len().saturating_sub(target.len() - 1);
    (from..last).find(|&i| units[i..].starts_with(target)).map_or(-1, |i| i as i32)
}

/// `s.lastIndexOf(ch, from)`, where `ch` is a code point.
#[inline]
pub fn last_index_of_char<S: Text + ?Sized>(s: &S, ch: i32, from: i32) -> i32 {
    last_index_of_char_in(s.wtf8_text(), s.ascii_text(), ch, from)
}

fn last_index_of_char_in(s: &Wtf8, ascii: bool, ch: i32, from: i32) -> i32 {
    if from < 0 {
        return -1;
    }
    if ascii {
        if !(0..0x80).contains(&ch) || s.is_empty() {
            return -1;
        }
        let end = (from as usize).min(s.len() - 1);
        return s.as_bytes()[..=end].iter().rposition(|&b| b as i32 == ch).map_or(-1, |i| i as i32);
    }
    let units = units(s);
    let mut buf = [0u16; 2];
    let Some(target) = code_point_units(ch, &mut buf) else { return -1 };
    if units.len() < target.len() {
        return -1;
    }
    let end = (from as usize).min(units.len() - target.len());
    (0..=end).rev().find(|&i| units[i..].starts_with(target)).map_or(-1, |i| i as i32)
}

/// `indexOf` over code units, for a pattern that can match half of a pair.
fn index_of_units(s: &Wtf8, t: &Wtf8, from: usize) -> i32 {
    let (s, t) = (units(s), units(t));
    if t.len() > s.len() {
        return -1;
    }
    (from..=s.len() - t.len()).find(|&i| s[i..].starts_with(&t)).map_or(-1, |i| i as i32)
}

/// `s.indexOf(t, from)`.
#[inline]
pub fn index_of<S: Text + ?Sized>(s: &S, t: &Wtf8, from: i32) -> i32 {
    index_of_in(s.wtf8_text(), s.ascii_text(), t, from)
}

fn index_of_in(s: &Wtf8, ascii: bool, t: &Wtf8, from: i32) -> i32 {
    if ascii {
        let from = from.clamp(0, s.len() as i32) as usize;
        return s.find_from(t, from).map_or(-1, |i| i as i32);
    }
    let len = length(s);
    let from = from.clamp(0, len as i32) as usize;
    if t.is_empty() {
        return from as i32;
    }
    if t.has_half_edges() {
        return index_of_units(s, t, from);
    }
    // A match starts on a code point: `t` does not start with the second half of a pair.
    let start = byte_offset_at_or_after(s, from);
    s.find_from(t, start).map_or(-1, |i| unit_index(s, i))
}

/// `s.lastIndexOf(t, from)`.
#[inline]
pub fn last_index_of<S: Text + ?Sized>(s: &S, t: &Wtf8, from: i32) -> i32 {
    last_index_of_in(s.wtf8_text(), s.ascii_text(), t, from)
}

fn last_index_of_in(s: &Wtf8, ascii: bool, t: &Wtf8, from: i32) -> i32 {
    let len = if ascii { s.len() } else { length(s) };
    let (len, t_len) = (len as i64, length(t) as i64);
    let from = (from as i64).min(len - t_len);
    if from < 0 {
        return -1;
    }
    if t.is_empty() {
        return from as i32;
    }
    if ascii {
        let end = (from as usize + t.len()).min(s.len());
        return s.rfind_before(t, end).map_or(-1, |i| i as i32);
    }
    if t.has_half_edges() {
        let (s, t) = (units(s), units(t));
        return (0..=from as usize).rev().find(|&i| s[i..].starts_with(&t)).map_or(-1, |i| i as i32);
    }
    // The match starts at a code point that starts at or before unit `from`.
    let mut starts = Vec::new();
    let mut units = 0;
    for (offset, c) in s.code_point_indices() {
        if units > from as usize {
            break;
        }
        starts.push((offset, units));
        units += unit_count(c);
    }
    let bytes = s.as_bytes();
    starts
        .into_iter()
        .rev()
        .find(|&(offset, _)| bytes[offset..].starts_with(t.as_bytes()))
        .map_or(-1, |(_, u)| u as i32)
}

/// `s.startsWith(prefix, offset)`.
pub fn starts_with_at(s: &Wtf8, prefix: &Wtf8, offset: i32) -> bool {
    let (len, p_len) = (length(s) as i64, length(prefix) as i64);
    if offset < 0 || offset as i64 > len - p_len {
        return false;
    }
    if prefix.is_empty() {
        return true;
    }
    match byte_offset(s, offset as usize) {
        Some(at) if !prefix.has_half_edges() => s.as_bytes()[at..].starts_with(prefix.as_bytes()),
        _ => units(s)[offset as usize..].starts_with(&units(prefix)),
    }
}

/// `s.lines()`: the lines of `s`, each ended by `\n`, `\r` or `\r\n` or by the end of the
/// string, without an empty line after the last line end.
pub fn lines(s: &Wtf8) -> Vec<JString> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let end = bytes[at..].iter().position(|&b| b == b'\n' || b == b'\r').map_or(bytes.len(), |i| at + i);
        out.push(s.slice(at, end).to_owned());
        at = match bytes[end..] {
            [b'\r', b'\n', ..] => end + 2,
            [] => end,
            _ => end + 1,
        };
    }
    out
}

/// Java's `s.isBlank()`: empty or only whitespace, as `Character.isWhitespace` defines it.
pub fn is_blank(s: &Wtf8) -> bool {
    s.code_points().all(is_java_whitespace)
}

/// `s.trim()`: without the code units up to `' '` at both ends.
pub fn trim(s: &Wtf8) -> JString {
    s.trim_by(true, true, |c| c <= ' ' as u32).to_owned()
}

/// `s.strip()`: without whitespace (`Character.isWhitespace`) at both ends.
pub fn strip(s: &Wtf8) -> JString {
    s.trim_by(true, true, is_java_whitespace).to_owned()
}

/// `s.stripLeading()`.
pub fn strip_leading(s: &Wtf8) -> JString {
    s.trim_by(true, false, is_java_whitespace).to_owned()
}

/// `s.stripTrailing()`.
pub fn strip_trailing(s: &Wtf8) -> JString {
    s.trim_by(false, true, is_java_whitespace).to_owned()
}

/// `map` applied to the text of `s` between its lone surrogates, which stay as they are.
fn map_text(s: &Wtf8, map: impl Fn(&str) -> String) -> JString {
    if let Some(text) = s.to_str() {
        return map(text).into();
    }
    let mut out = JString::new();
    let mut run = String::new();
    for c in s.code_points() {
        match char::from_u32(c) {
            Some(c) => run.push(c),
            None => {
                out.push_str(&map(&run));
                run.clear();
                out.push_unit(c as u16);
            }
        }
    }
    out.push_str(&map(&run));
    out
}

/// `s.toUpperCase()` in the root locale: Unicode's full case mapping, so `ß` is `SS`.
pub fn uppercase(s: &Wtf8) -> JString {
    if s.is_ascii() {
        s.expect_str("toUpperCase").to_ascii_uppercase().into()
    } else {
        map_text(s, str::to_uppercase)
    }
}

/// `s.toLowerCase()` in the root locale: Unicode's full case mapping, with a final `Σ` as `ς`.
pub fn lowercase(s: &Wtf8) -> JString {
    if s.is_ascii() {
        s.expect_str("toLowerCase").to_ascii_lowercase().into()
    } else {
        map_text(s, str::to_lowercase)
    }
}

/// `s.toUpperCase(locale)`, given the locale's language: the root locale's mapping, which is
/// every language's but Turkish, Azeri and Lithuanian (`special_casing`).
pub fn uppercase_in(s: &Wtf8, language: &Wtf8) -> JString {
    special_casing(language);
    uppercase(s)
}

/// `s.toLowerCase(locale)`, given the locale's language (see `uppercase_in`).
pub fn lowercase_in(s: &Wtf8, language: &Wtf8) -> JString {
    special_casing(language);
    lowercase(s)
}

/// Ends the program for a language whose case mapping is not the root locale's: Java maps the
/// dotted and dotless `i` of Turkish and Azeri, and the dot above of Lithuanian, its own way,
/// which the runtime does not have.
fn special_casing(language: &Wtf8) {
    if matches!(language.as_bytes(), b"tr" | b"az" | b"lt") {
        crate::throw_uncaught(format!(
            "the case mapping of the language \"{language}\" is not supported by the translation"
        ));
    }
}

/// `s.replace(old, new)` on `char`s, which replaces code units, even halves of a pair.
pub fn replace_char(s: &Wtf8, old: u16, new: u16) -> JString {
    let surrogate = |c: u16| (0xD800..0xE000).contains(&c);
    if let (false, false, Some(text)) = (surrogate(old), surrogate(new), s.to_str()) {
        let (old, new) = (char::from_u32(old as u32).unwrap(), char::from_u32(new as u32).unwrap());
        return text.chars().map(|c| if c == old { new } else { c }).collect::<String>().into();
    }
    let replaced: Vec<u16> = s.units().map(|c| if c == old { new } else { c }).collect();
    from_units(&replaced)
}

/// `s.replace(target, replacement)` on strings. An empty target matches before every code unit
/// and at the end.
pub fn replace(s: &Wtf8, target: &Wtf8, replacement: &Wtf8) -> JString {
    if let (Some(s), Some(t), Some(r)) = (s.to_str(), target.to_str(), replacement.to_str())
        && (!t.is_empty() || s.is_ascii())
    {
        return s.replace(t, r).into();
    }
    let (s, t, r) = (units(s), units(target), units(replacement));
    let mut out = Vec::with_capacity(s.len());
    if t.is_empty() {
        out.extend_from_slice(&r);
        for &unit in &s {
            out.push(unit);
            out.extend_from_slice(&r);
        }
        return from_units(&out);
    }
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with(&t) {
            out.extend_from_slice(&r);
            i += t.len();
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    from_units(&out)
}

/// `s.repeat(count)`.
pub fn repeat(s: &Wtf8, count: i32) -> JString {
    if count < 0 {
        throw("java.lang.IllegalArgumentException", &format!("count is negative: {count}"));
    }
    if (length(s) as u64) * (count as u64) > i32::MAX as u64 {
        throw("java.lang.OutOfMemoryError", "Required length exceeds implementation limit");
    }
    let mut out = JString::with_capacity(s.len() * count as usize);
    for _ in 0..count {
        out.push_wtf8(s);
    }
    out
}

/// `a.compareTo(b)`: the difference of the first code units that differ, or of the lengths.
pub fn compare_to(a: &Wtf8, b: &Wtf8) -> i32 {
    if a.is_ascii() && b.is_ascii() {
        let (x, y) = (a.as_bytes(), b.as_bytes());
        return match x.iter().zip(y).find(|(p, q)| p != q) {
            Some((p, q)) => *p as i32 - *q as i32,
            None => x.len() as i32 - y.len() as i32,
        };
    }
    let mut x = a.units();
    let mut y = b.units();
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
pub fn compare_to_ignore_case(a: &Wtf8, b: &Wtf8) -> i32 {
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
    compare_units_ci(&units(a), &units(b))
}

/// `compareToIgnoreCase` of two runs of code units: units that differ are compared through
/// upper and then lower case, as the code points they belong to.
fn compare_units_ci(x: &[u16], y: &[u16]) -> i32 {
    let (mut k1, mut k2) = (0, 0);
    while k1 < x.len() && k2 < y.len() {
        let (c1, c2) = (x[k1] as u32, y[k2] as u32);
        if c1 != c2 && compare_code_point_ci(c1, c2) != 0 {
            let (cp1, skip1) = code_point_including(x, k1);
            let (cp2, skip2) = code_point_including(y, k2);
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

/// `s.contentEquals(text)`: whether the texts are equal.
pub fn content_equals(a: &Wtf8, b: &Wtf8) -> bool {
    a == b
}

/// `a.equalsIgnoreCase(b)`.
pub fn equals_ignore_case(a: &Wtf8, b: &Wtf8) -> bool {
    if a.len() == b.len() && a.as_bytes().eq_ignore_ascii_case(b.as_bytes()) {
        return true;
    }
    length(a) == length(b) && compare_to_ignore_case(a, b) == 0
}

/// `s.hashCode()`: `s[0]*31^(n-1) + ... + s[n-1]` over the code units, wrapping. A string object
/// (`JStr`) keeps its hash once computed.
#[inline]
pub fn string_hash<S: StringHash + ?Sized>(s: &S) -> i32 {
    s.string_hash()
}

/// A string `string_hash` takes: the text of any string type, or a string object, which keeps
/// its hash.
pub trait StringHash {
    fn string_hash(&self) -> i32;
}

impl StringHash for Wtf8 {
    fn string_hash(&self) -> i32 {
        text_hash(self)
    }
}

impl StringHash for JString {
    fn string_hash(&self) -> i32 {
        text_hash(self)
    }
}

impl StringHash for crate::JStr {
    fn string_hash(&self) -> i32 {
        self.hash_code()
    }
}

impl<S: StringHash + ?Sized> StringHash for &S {
    fn string_hash(&self) -> i32 {
        (**self).string_hash()
    }
}

/// The hash of `s`'s text, computed.
pub(crate) fn text_hash(s: &Wtf8) -> i32 {
    // The ASCII bytes at the start are units of their own, read without decoding.
    let bytes = s.as_bytes();
    let ascii = bytes.iter().position(|&b| !b.is_ascii()).unwrap_or(bytes.len());
    let step = |h: i32, c: i32| h.wrapping_mul(31).wrapping_add(c);
    let h = ascii_hash(&bytes[..ascii]);
    s.slice(ascii, bytes.len()).units().fold(h, |h, c| step(h, c as i32))
}

/// `31 * h + b` over `bytes`, eight at a time: `h * 31^8 + b0 * 31^7 + .. + b7`, the same sum
/// modulo 2^32, whose eight products are independent where the one-at-a-time form waits for each
/// multiplication before the next.
fn ascii_hash(bytes: &[u8]) -> i32 {
    const POWERS: [i32; 9] = {
        let mut p = [1i32; 9];
        let mut i = 1;
        while i < 9 {
            p[i] = p[i - 1].wrapping_mul(31);
            i += 1;
        }
        p
    };
    let (chunks, rest) = bytes.as_chunks::<8>();
    let mut h = 0i32;
    for c in chunks {
        let mut sum = h.wrapping_mul(POWERS[8]);
        for (i, &b) in c.iter().enumerate() {
            sum = sum.wrapping_add((b as i32).wrapping_mul(POWERS[7 - i]));
        }
        h = sum;
    }
    rest.iter().fold(h, |h, &b| h.wrapping_mul(31).wrapping_add(b as i32))
}

/// `new String(chars, offset, count)` and `String.valueOf(chars, offset, count)`.
pub fn from_chars_range(chars: &(impl crate::Source<u16> + ?Sized), offset: i32, count: i32) -> JString {
    try_from_chars_range(chars, offset, count).unwrap_or_else(crate::uncaught)
}

/// `from_chars_range` where the program observes the `StringIndexOutOfBoundsException`.
pub fn try_from_chars_range(
    chars: &(impl crate::Source<u16> + ?Sized),
    offset: i32,
    count: i32,
) -> Result<JString, crate::Exception> {
    let len = chars.length();
    if offset < 0 || count < 0 || offset as i64 + count as i64 > len as i64 {
        let message = format!("offset {offset}, count {count}, length {len}");
        return Err(crate::Exception::new("java.lang.StringIndexOutOfBoundsException", message));
    }
    Ok(from_units(&chars.range(offset as usize..(offset + count) as usize)))
}

/// `s.getChars(begin, end, dst, dst_begin)`: the UTF-16 units `begin..end` of `s` into `dst` from
/// `dst_begin`.
pub fn get_chars(
    s: &Wtf8,
    begin: i32,
    end: i32,
    dst: &mut (impl crate::Target<u16> + ?Sized),
    dst_begin: i32,
) {
    try_get_chars(s, begin, end, dst, dst_begin).unwrap_or_else(crate::uncaught)
}

/// `get_chars` where the program observes the `StringIndexOutOfBoundsException`: a range outside
/// the string, or a destination too short, with the JDK's messages.
pub fn try_get_chars(
    s: &Wtf8,
    begin: i32,
    end: i32,
    dst: &mut (impl crate::Target<u16> + ?Sized),
    dst_begin: i32,
) -> Result<(), crate::Exception> {
    const OUT: &str = "java.lang.StringIndexOutOfBoundsException";
    let units = units(s);
    check_begin_end(begin, end, units.len())?;
    let count = end - begin;
    if dst_begin < 0 || dst_begin as i64 + count as i64 > dst.length() as i64 {
        let message =
            format!("Range [{dst_begin}, {dst_begin} + {count}) out of bounds for length {}", dst.length());
        return Err(crate::Exception::new(OUT, message));
    }
    dst.store(dst_begin as usize, &units[begin as usize..end as usize]);
    Ok(())
}

/// `s.getChars(begin, end, dst, dst_begin)` on a string held as a `CharSequence`, whose `dst`
/// may be null: Java checks the range on the string before it reads `dst.length`. A failure is
/// thrown (`crate::throw`), for a `java::catching` around the call.
pub fn sequence_get_chars(
    s: &Wtf8,
    begin: i32,
    end: i32,
    dst: Option<&impl crate::Elements<u16>>,
    dst_begin: i32,
) {
    let checked = match dst {
        Some(dst) => try_get_chars(s, begin, end, &mut crate::lent::Handled(dst), dst_begin),
        None => check_begin_end(begin, end, crate::string_length(s) as usize)
            .and_then(|()| Err(null_destination())),
    };
    checked.unwrap_or_else(|e| crate::throw_exception(e))
}

/// The `NullPointerException` of the JDK's `getChars` given a null `dst`, which reads its length.
pub(crate) fn null_destination() -> crate::Exception {
    let message = "Cannot read the array length because \"dst\" is null";
    crate::Exception::new("java.lang.NullPointerException", message)
}

/// `s.getBytes(begin, end, dst, dst_begin)`, the deprecated form that keeps the low byte of each
/// code unit.
pub fn get_low_bytes(
    s: &Wtf8,
    begin: i32,
    end: i32,
    dst: &mut (impl crate::Target<i8> + ?Sized),
    dst_begin: i32,
) {
    let units = units(s);
    check_begin_end(begin, end, units.len()).unwrap_or_else(|e| throw_exception(e));
    let count = end - begin;
    if dst_begin < 0 || dst_begin as i64 + count as i64 > dst.length() as i64 {
        let message =
            format!("Range [{dst_begin}, {dst_begin} + {count}) out of bounds for length {}", dst.length());
        throw("java.lang.StringIndexOutOfBoundsException", &message);
    }
    let bytes: Vec<i8> = units[begin as usize..end as usize].iter().map(|&u| u as i8).collect();
    dst.store(dst_begin as usize, &bytes);
}

fn is_high(c: u16) -> bool {
    (0xD800..0xDC00).contains(&c)
}

fn is_low(c: u16) -> bool {
    (0xDC00..0xE000).contains(&c)
}

fn pair(high: u16, low: u16) -> i32 {
    0x10000 + ((high as i32 - 0xD800) << 10) + (low as i32 - 0xDC00)
}

/// The `StringIndexOutOfBoundsException` of an index outside `0..length`.
fn check_index(index: i32, length: usize) -> usize {
    if index < 0 || index as i64 >= length as i64 {
        let message = format!("Index {index} out of bounds for length {length}");
        throw("java.lang.StringIndexOutOfBoundsException", &message);
    }
    index as usize
}

/// `codePointAt(index)` of the code units `units`: a high surrogate followed by a low one is
/// their pair; any other unit is its own code point.
pub(crate) fn code_point_at_units(units: &[u16], index: i32) -> i32 {
    let i = check_index(index, units.len());
    match units.get(i + 1) {
        Some(&low) if is_high(units[i]) && is_low(low) => pair(units[i], low),
        _ => units[i] as i32,
    }
}

/// `codePointBefore(index)` of the code units `units`: the code point that ends before `index`.
pub(crate) fn code_point_before_units(units: &[u16], index: i32) -> i32 {
    let i = check_index(index.wrapping_sub(1), units.len());
    match i.checked_sub(1).map(|h| units[h]) {
        Some(high) if is_low(units[i]) && is_high(high) => pair(high, units[i]),
        _ => units[i] as i32,
    }
}

/// `codePointCount(begin, end)` of the code units `units`: a surrogate pair counts once, a lone
/// surrogate once too.
pub(crate) fn code_point_count_units(units: &[u16], begin: i32, end: i32) -> i32 {
    if begin < 0 || begin > end || end as i64 > units.len() as i64 {
        let message = format!("Range [{begin}, {end}) out of bounds for length {}", units.len());
        throw("java.lang.IndexOutOfBoundsException", &message);
    }
    let range = &units[begin as usize..end as usize];
    let pairs = range.windows(2).filter(|w| is_high(w[0]) && is_low(w[1])).count();
    (range.len() - pairs) as i32
}

/// `offsetByCodePoints(index, offset)` of the code units `units`, as
/// `Character.offsetByCodePoints` walks them; out of range is an `IndexOutOfBoundsException`
/// without a message.
pub(crate) fn offset_by_code_points_units(units: &[u16], index: i32, offset: i32) -> i32 {
    let out = || -> ! { throw_exception(crate::Exception::bare("java.lang.IndexOutOfBoundsException")) };
    let length = units.len();
    if index < 0 || index as usize > length {
        out();
    }
    let mut x = index as usize;
    if offset >= 0 {
        for _ in 0..offset {
            if x >= length {
                out();
            }
            x += 1;
            if is_high(units[x - 1]) && x < length && is_low(units[x]) {
                x += 1;
            }
        }
    } else {
        for _ in offset..0 {
            if x == 0 {
                out();
            }
            x -= 1;
            if is_low(units[x]) && x > 0 && is_high(units[x - 1]) {
                x -= 1;
            }
        }
    }
    x as i32
}

/// `s.codePointAt(index)`.
pub fn code_point_at(s: &Wtf8, index: i32) -> i32 {
    code_point_at_units(&units(s), index)
}

/// `s.codePointBefore(index)`.
pub fn code_point_before(s: &Wtf8, index: i32) -> i32 {
    code_point_before_units(&units(s), index)
}

/// `s.codePointCount(begin, end)`.
pub fn code_point_count(s: &Wtf8, begin: i32, end: i32) -> i32 {
    code_point_count_units(&units(s), begin, end)
}

/// `s.offsetByCodePoints(index, offset)`.
pub fn offset_by_code_points(s: &Wtf8, index: i32, offset: i32) -> i32 {
    offset_by_code_points_units(&units(s), index, offset)
}

/// `s.regionMatches(ignore_case, toffset, other, ooffset, len)`: whether the `len` code units of
/// `s` from `toffset` equal those of `other` from `ooffset`, ignoring case as `compareToIgnoreCase`
/// does when `ignore_case`.
pub fn region_matches(
    s: &Wtf8,
    ignore_case: bool,
    toffset: i32,
    other: &Wtf8,
    ooffset: i32,
    len: i32,
) -> bool {
    let (a, b) = (units(s), units(other));
    if ooffset < 0
        || toffset < 0
        || toffset as i64 > a.len() as i64 - len as i64
        || ooffset as i64 > b.len() as i64 - len as i64
    {
        return false;
    }
    if len <= 0 {
        return true;
    }
    let (a, b) = (&a[toffset as usize..][..len as usize], &b[ooffset as usize..][..len as usize]);
    if !ignore_case {
        return a == b;
    }
    compare_units_ci(a, b) == 0
}

/// `s.stripIndent()`: the lines of `s` without the white space that all its non-blank lines (and
/// a blank last line) begin with, nor any at their ends, joined by `\n`. A string that ends with
/// a line terminator keeps every line's indentation, and ends with `\n`.
pub fn strip_indent(s: &Wtf8) -> JString {
    if s.is_empty() {
        return JString::new();
    }
    let opt_out = matches!(s.as_bytes().last(), Some(b'\n' | b'\r'));
    let lines = lines(s);
    let white = |c: u32| is_java_whitespace(c);
    // White space is one code unit per code point, so counting code points counts units.
    let leading = |line: &Wtf8| line.code_points().take_while(|&c| white(c)).count();
    let outdent = if opt_out {
        0
    } else {
        let mut outdent = lines
            .iter()
            .filter(|line| leading(line) != line.code_points().count())
            .map(|line| leading(line))
            .min()
            .unwrap_or(usize::MAX);
        let last = lines.last().expect("a string that is not empty has a line");
        if last.code_points().all(white) {
            outdent = outdent.min(last.code_points().count());
        }
        outdent
    };
    let mut text = JString::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            text.push_str("\n");
        }
        let trimmed = line.trim_by(false, true, white);
        if !trimmed.code_points().all(white) {
            let skip = outdent.min(leading(line));
            let start = trimmed.code_point_indices().nth(skip).map_or(trimmed.len(), |(i, _)| i);
            text.push_wtf8(trimmed.slice(start, trimmed.len()));
        }
    }
    if opt_out {
        text.push_str("\n");
    }
    text
}

/// `new String(codePoints, offset, count)`, which throws for a range outside the array and for
/// an invalid code point.
pub fn from_code_points(points: &(impl crate::Source<i32> + ?Sized), offset: i32, count: i32) -> JString {
    let range = range_of(points.length(), offset, count).unwrap_or_else(|e| crate::throw_exception(e));
    let mut s = JString::new();
    for &point in points.range(range).iter() {
        if !(0..=0x10FFFF).contains(&point) {
            let message = format!("Not a valid Unicode code point: 0x{point:X}");
            crate::throw("java.lang.IllegalArgumentException", &message);
        }
        s.push_code_point(point as u32);
    }
    s
}

/// `new String(ascii, hibyte, offset, count)`: each `char` is `hibyte`'s low byte over a byte.
pub fn from_high_byte(
    bytes: &(impl crate::Source<i8> + ?Sized),
    hibyte: i32,
    offset: i32,
    count: i32,
) -> JString {
    let range = range_of(bytes.length(), offset, count).unwrap_or_else(|e| crate::throw_exception(e));
    let high = ((hibyte & 0xff) as u16) << 8;
    let units: Vec<u16> = bytes.range(range).iter().map(|&b| high | u16::from(b as u8)).collect();
    from_units(&units)
}

/// The elements `offset .. offset + count` of an array of length `len`, or the
/// `StringIndexOutOfBoundsException` that a constructor of `String` throws for them.
fn range_of(len: usize, offset: i32, count: i32) -> Result<std::ops::Range<usize>, crate::Exception> {
    if offset < 0 || count < 0 || offset as i64 + count as i64 > len as i64 {
        let message = format!("offset {offset}, count {count}, length {len}");
        return Err(crate::Exception::new("java.lang.StringIndexOutOfBoundsException", message));
    }
    Ok(offset as usize..(offset + count) as usize)
}

/// `String.join(prefix, suffix, delimiter, elements, size)` of the JDK: the first `size` of
/// `elements` joined by `delimiter`, between `prefix` and `suffix`.
pub fn join_range<T: JoinItem + Clone>(
    prefix: &Wtf8,
    suffix: &Wtf8,
    delimiter: &Wtf8,
    elements: &(impl crate::Source<T> + ?Sized),
    size: i32,
) -> JString {
    let mut out = prefix.to_owned();
    out.push_wtf8(&join(delimiter, &elements.range(0..size as usize)));
    out.push_wtf8(suffix);
    out
}

/// The text of each element of a `String.join`: a string, or `null`.
pub trait JoinItem {
    fn text(&self) -> &Wtf8;
}

impl JoinItem for JString {
    fn text(&self) -> &Wtf8 {
        self
    }
}

impl JoinItem for crate::JStr {
    fn text(&self) -> &Wtf8 {
        self
    }
}

impl JoinItem for &Wtf8 {
    fn text(&self) -> &Wtf8 {
        self
    }
}

impl JoinItem for &str {
    fn text(&self) -> &Wtf8 {
        Wtf8::new(self)
    }
}

impl<T: JoinItem> JoinItem for Option<T> {
    fn text(&self) -> &Wtf8 {
        self.as_ref().map_or(Wtf8::new("null"), JoinItem::text)
    }
}

/// `String.join(delimiter, elements)`.
pub fn join<T: JoinItem>(delimiter: &Wtf8, elements: &[T]) -> JString {
    let mut out = JString::new();
    for (i, e) in elements.iter().enumerate() {
        if i > 0 {
            out.push_wtf8(delimiter);
        }
        out.push_wtf8(e.text());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> &Wtf8 {
        Wtf8::new(s)
    }

    #[test]
    fn case_mapping_in_a_locale_is_the_root_locales_for_most_languages() {
        assert_eq!(uppercase_in(w("straße"), w("en")), "STRASSE");
        assert_eq!(uppercase_in(w("title"), w("")), "TITLE");
        assert_eq!(lowercase_in(w("\u{39f}\u{394}\u{39f}\u{3a3}"), w("de")), "\u{3bf}\u{3b4}\u{3bf}\u{3c2}");
        assert_eq!(lowercase_in(w("ISTANBUL"), w("fr")), "istanbul");
    }

    #[test]
    fn a_string_keeps_whether_it_is_ascii_until_it_changes() {
        let mut s = JString::from("plain");
        assert!(s.is_ascii_text());
        s.push_str("é");
        assert!(!s.is_ascii_text());
        s.clear();
        s.push_unit(b'x' as u16);
        assert!(s.is_ascii_text());
        s.push_unit(0xD800);
        assert!(!s.is_ascii_text());
        assert!(s.clone() == s && !s.clone().is_ascii_text());
        assert!(crate::JStr::new("plain").is_ascii_text());
        assert!(!crate::JStr::new("caf\u{e9}").is_ascii_text());
        assert_eq!(crate::JStr::new("caf\u{e9}").as_wtf8(), w("caf\u{e9}"));
        assert!(substring(&JString::from("abcdef"), 1, 3).is_ascii_text());
    }

    #[test]
    fn index_methods_on_a_kept_string_match_those_on_its_text() {
        let mut lone = JString::from("a b");
        lone.push_unit(0xD800);
        lone.push_str(" x😀y é ab ");
        let texts = [JString::from("ab cd ab x"), JString::from("caf\u{e9} x😀y ab "), lone, JString::new()];
        for owned in &texts {
            let shared = crate::JStr::new(owned.as_wtf8());
            let text = owned.as_wtf8();
            let len = crate::string_length(text);
            assert_eq!(crate::string_length(owned), len);
            assert_eq!(crate::string_length(&shared), len);
            for i in -1..=len + 1 {
                assert_eq!(index_of_char(owned, ' ' as i32, i), index_of_char(text, ' ' as i32, i));
                assert_eq!(index_of_char(&shared, 'x' as i32, i), index_of_char(text, 'x' as i32, i));
                assert_eq!(last_index_of_char(owned, 'a' as i32, i), last_index_of_char(text, 'a' as i32, i));
                assert_eq!(index_of(owned, w("ab"), i), index_of(text, w("ab"), i));
                assert_eq!(last_index_of(&shared, w("ab"), i), last_index_of(text, w("ab"), i));
                assert_eq!(crate::try_char_at(owned, i).ok(), crate::try_char_at(text, i).ok());
                assert_eq!(crate::try_char_at(&shared, i).ok(), crate::try_char_at(text, i).ok());
                for end in i..=len + 1 {
                    let a = try_substring(owned, i, end).ok();
                    assert_eq!(a, try_substring(text, i, end).ok());
                    assert_eq!(try_substring(&shared, i, end).ok(), a);
                }
                assert_eq!(try_substring_from(&shared, i).ok(), try_substring_from(text, i).ok());
            }
        }
    }

    #[test]
    fn a_search_past_many_near_misses_finds_what_a_search_of_units_finds() {
        let naive = |s: &Wtf8, t: &Wtf8, from: i32| index_of_units(s, t, from.max(0) as usize);
        let naive_last = |s: &Wtf8, t: &Wtf8, from: i32| {
            let (s, t) = (units(s), units(t));
            let end = (from as i64).min(s.len() as i64 - t.len() as i64);
            (0..=end).rev().find(|&i| s[i as usize..].starts_with(&t)).map_or(-1, |i| i as i32)
        };
        let mut lone = JString::from(&*"a".repeat(150));
        lone.push_unit(0xD800);
        lone.push_str(&"ab".repeat(80));
        lone.push_str("aab😀");
        let texts = [JString::from(&*format!("{}ab{}", "a".repeat(200), "a".repeat(100))), lone];
        for s in &texts {
            for t in ["ab", "aab", "aab😀", "ba", "abc"] {
                for from in [0, 1, 70, 149, 150, 151, 200, 400] {
                    assert_eq!(index_of(s, w(t), from), naive(s, w(t), from), "{t} from {from}");
                    assert_eq!(last_index_of(s, w(t), from), naive_last(s, w(t), from), "{t} before {from}");
                }
            }
        }
    }

    #[test]
    fn a_hash_is_javas_for_every_length_and_kind_of_text() {
        let one_at_a_time = |s: &Wtf8| s.units().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32));
        let texts = [
            "",
            "a",
            "minecraft:stone",
            "the quick brown fox jumps over the lazy dog",
            "héllo wörld, a longer text",
            "😀 and \u{7f}\u{80}",
        ];
        for text in texts {
            for end in 0..=text.len() {
                if let Some(prefix) = text.get(..end) {
                    assert_eq!(text_hash(w(prefix)), one_at_a_time(w(prefix)), "{prefix:?}");
                }
            }
        }
        assert_eq!(text_hash(w("hello")), 99162322);
    }

    #[test]
    fn substrings_count_code_units() {
        assert_eq!(substring_from(w("hello"), 2), "llo");
        assert_eq!(substring(w("héllo"), 1, 3), "él");
        assert_eq!(substring(w("a😀b"), 1, 3), "😀");
        assert_eq!(units(&substring(w("a😀b"), 1, 2)), [0xD83D]);
        let tail = substring_from(w("😀x"), 1);
        assert_eq!(units(&tail), [0xDE00, 'x' as u16]);
        assert_eq!(compare_to(w("<"), &tail), '<' as i32 - 0xDE00);
        assert_eq!(compare_to(w("日"), &tail), 0x65E5 - 0xDE00);
        let mut joined = substring(w("a😀b"), 0, 2);
        joined.push_wtf8(&tail);
        assert_eq!(joined, *"a😀x");
        assert_eq!(substring_from(w("abc"), 3), "");
    }

    #[test]
    fn searches_count_code_units() {
        assert_eq!(index_of_char(w("a😀b"), 'b' as i32, 0), 3);
        assert_eq!(index_of_char(w("a😀b"), 0x1F600, 0), 1);
        assert_eq!(index_of_char(w("a😀b"), 0xDE00, 0), 2);
        assert_eq!(last_index_of_char(w("abcabc"), 'b' as i32, 3), 1);
        assert_eq!(last_index_of_char(w("abcabc"), 'b' as i32, 100), 4);
        assert_eq!(index_of(w("a😀bcb"), w("b"), 0), 3);
        assert_eq!(index_of(w("a😀bcb"), w("b"), 4), 5);
        assert_eq!(index_of(w("abc"), w(""), 5), 3);
        assert_eq!(last_index_of(w("a😀bcb"), w("b"), 4), 3);
        assert_eq!(last_index_of(w("abcab"), w("ab"), 10), 3);
        assert_eq!(last_index_of(w("abc"), w(""), 10), 3);
        assert_eq!(last_index_of(w("éaé"), w("é"), 1), 0);
        assert_eq!(last_index_of(w("éaé"), w("é"), 2), 2);
    }

    #[test]
    fn comparisons_follow_java() {
        assert_eq!(compare_to(w("apple"), w("banana")), -1);
        assert_eq!(compare_to(w("ab"), w("abcd")), -2);
        assert_eq!(compare_to(w("\u{FFFF}"), w("😀")), 0xFFFF - 0xD83D);
        assert_eq!(compare_to_ignore_case(w("Hello"), w("hello")), 0);
        assert_eq!(compare_to_ignore_case(w("a"), w("B")), -1);
        assert!(equals_ignore_case(w("STRASSE"), w("strasse")));
        assert!(!equals_ignore_case(w("ß"), w("SS")));
        assert_eq!(string_hash(w("hello")), 99162322);
        assert_eq!(string_hash(w("")), 0);
        assert_eq!(string_hash(w("héllo")), 103094734);
        assert_eq!(string_hash(w("a😀")), 1866116);
        assert_eq!(string_hash(w("ab😀c")), 147461023);
    }

    #[test]
    fn replacing_and_trimming() {
        assert_eq!(replace(w("abc"), w(""), w("-")), "-a-b-c-");
        assert_eq!(replace(w("aaa"), w("aa"), w("b")), "ba");
        assert_eq!(replace_char(w("a.b.c"), '.' as u16, '/' as u16), "a/b/c");
        assert_eq!(trim(w(" \t x y \n")), "x y");
        assert_eq!(strip(w("\u{2003}x\u{A0}")), "x\u{A0}");
        assert!(is_blank(w(" \u{2028}\u{1F}")));
        assert_eq!(uppercase(w("straße")), "STRASSE");
        assert_eq!(join(w(", "), &[Some("a"), None]), "a, null");
    }

    #[test]
    fn joined_ranges_have_a_prefix_and_suffix() {
        assert_eq!(join_range(w("["), w("]"), w(", "), &[Some("a"), None, Some("c")], 2), "[a, null]");
        assert_eq!(join_range(w("<"), w(">"), w("-"), &[] as &[&str], 0), "<>");
    }

    #[test]
    fn lines_end_at_each_line_terminator() {
        assert_eq!(lines(w("a\nb\r\nc\rd\n\ne\n")), ["a", "b", "c", "d", "", "e"]);
        assert!(lines(w("")).is_empty());
        assert_eq!(lines(w("\n")), [""]);
        assert_eq!(lines(w("\r\r\n")), ["", ""]);
        assert_eq!(lines(w("x")), ["x"]);
    }
}
