//! Strings as WTF-8, which hold exactly Java's UTF-16 code units: UTF-8 that also encodes a
//! lone surrogate in UTF-8's 3-byte form (`ED A0..BF ..`). A surrogate pair is always its 4-byte
//! character, never two 3-byte halves, so each Java string has one encoding: equality, order of
//! the bytes and hashing are of the text. Text without lone surrogates is byte for byte its
//! UTF-8, so a `&str` is a `&Wtf8` as it is.
//!
//! `Wtf8` is the string slice and `JString` the owned string. Appending text that starts with a
//! low surrogate to text that ends with a high one joins the two into their character, as Java
//! shows them as one pair (`JString::push_wtf8`).

use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/// A string slice of WTF-8 bytes.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Wtf8([u8]);

impl Wtf8 {
    /// The text of a Rust string, which is its own WTF-8.
    pub const fn new(s: &str) -> &Wtf8 {
        // SAFETY: `Wtf8` is a transparent wrapper of `[u8]`, and UTF-8 is WTF-8.
        unsafe { &*(s.as_bytes() as *const [u8] as *const Wtf8) }
    }

    /// The slice of `bytes`, which must be WTF-8 without a 3-byte high surrogate directly before
    /// a 3-byte low one.
    ///
    /// # Safety
    /// The bytes must be well-formed as described.
    pub const unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Wtf8 {
        // SAFETY: `Wtf8` is a transparent wrapper of `[u8]`.
        unsafe { &*(bytes as *const [u8] as *const Wtf8) }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_ascii(&self) -> bool {
        self.0.is_ascii()
    }

    /// The text as a Rust string, or `None` where it holds a lone surrogate.
    pub fn to_str(&self) -> Option<&str> {
        if has_surrogate(&self.0) {
            return None;
        }
        // SAFETY: WTF-8 without surrogates is UTF-8.
        Some(unsafe { std::str::from_utf8_unchecked(&self.0) })
    }

    /// The text as Java prints it in UTF-8: a lone surrogate as `?`.
    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        match self.to_str() {
            Some(s) => std::borrow::Cow::Borrowed(s),
            None => std::borrow::Cow::Owned(
                self.code_points().map(|c| char::from_u32(c).unwrap_or('?')).collect(),
            ),
        }
    }

    /// The code points, a lone surrogate as its own value.
    pub fn code_points(&self) -> CodePoints<'_> {
        CodePoints { bytes: &self.0 }
    }

    /// The UTF-16 code units, as Java holds them.
    pub fn units(&self) -> Units<'_> {
        Units { points: self.code_points(), low: None }
    }

    /// Java's `length()`: the number of UTF-16 code units.
    pub fn unit_len(&self) -> usize {
        if self.is_ascii() {
            return self.0.len();
        }
        // Each lead byte starts one code point; a 4-byte one is two units.
        self.0.iter().map(|&b| usize::from(b & 0xC0 != 0x80) + usize::from(b >= 0xF0)).sum()
    }

    /// The text from byte `start` to byte `end`, which must be the boundaries of code points.
    pub fn slice(&self, start: usize, end: usize) -> &Wtf8 {
        assert!(
            self.is_boundary(start) && self.is_boundary(end) && start <= end,
            "a slice inside a code point"
        );
        // SAFETY: a range between code point boundaries of WTF-8 is WTF-8, and it has no seam
        // of a pair that the whole did not have.
        unsafe { Wtf8::from_bytes_unchecked(&self.0[start..end]) }
    }

    /// Whether byte `index` is the boundary of a code point (or the end).
    pub fn is_boundary(&self, index: usize) -> bool {
        index == self.0.len() || self.0.get(index).is_some_and(|&b| b & 0xC0 != 0x80)
    }

    /// Whether `prefix` is at the start: a byte comparison, which is the comparison of units
    /// except where `prefix` ends with a high surrogate that this text has as half of a pair.
    pub fn starts_with(&self, prefix: &Wtf8) -> bool {
        self.0.starts_with(&prefix.0)
            || (ends_high(&prefix.0) && self.units().take(prefix.unit_len()).eq(prefix.units()))
    }

    /// Whether `suffix` is at the end, as `starts_with`.
    pub fn ends_with(&self, suffix: &Wtf8) -> bool {
        if self.0.ends_with(&suffix.0) {
            return true;
        }
        if !starts_low(&suffix.0) {
            return false;
        }
        let (n, m) = (self.unit_len(), suffix.unit_len());
        n >= m && self.units().skip(n - m).eq(suffix.units())
    }

    /// The high surrogate at the end, if it ends with one.
    fn last_high(&self) -> Option<u16> {
        ends_high(&self.0).then(|| surrogate_at(&self.0[self.0.len() - 3..]))
    }

    /// The low surrogate that it starts with, if any.
    fn first_low(&self) -> Option<u16> {
        starts_low(&self.0).then(|| surrogate_at(&self.0[..3]))
    }
}

/// Whether WTF-8 `bytes` hold a surrogate's 3-byte form.
fn has_surrogate(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|w| w[0] == 0xED && w[1] >= 0xA0)
}

/// Whether WTF-8 `bytes` end with a high surrogate.
fn ends_high(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[bytes.len() - 3] == 0xED && (0xA0..0xB0).contains(&bytes[bytes.len() - 2])
}

/// Whether WTF-8 `bytes` start with a low surrogate.
fn starts_low(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0xED && (0xB0..0xC0).contains(&bytes[1])
}

/// The surrogate of the 3 bytes `b`.
fn surrogate_at(b: &[u8]) -> u16 {
    0xD000 | (u16::from(b[1] & 0x3F) << 6) | u16::from(b[2] & 0x3F)
}

/// The WTF-8 bytes of code point `c`, which may be a surrogate, appended to `out`.
fn encode(c: u32, out: &mut Vec<u8>) {
    match c {
        0..0x80 => out.push(c as u8),
        0x80..0x800 => out.extend([0xC0 | (c >> 6) as u8, 0x80 | (c & 0x3F) as u8]),
        0x800..0x10000 => {
            out.extend([0xE0 | (c >> 12) as u8, 0x80 | ((c >> 6) & 0x3F) as u8, 0x80 | (c & 0x3F) as u8])
        }
        _ => out.extend([
            0xF0 | (c >> 18) as u8,
            0x80 | ((c >> 12) & 0x3F) as u8,
            0x80 | ((c >> 6) & 0x3F) as u8,
            0x80 | (c & 0x3F) as u8,
        ]),
    }
}

/// The code points of WTF-8 text.
#[derive(Clone)]
pub struct CodePoints<'a> {
    bytes: &'a [u8],
}

impl Iterator for CodePoints<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        let (&b, rest) = self.bytes.split_first()?;
        let (c, n) = match b {
            0..0x80 => (u32::from(b), 1),
            0xC0..0xE0 => ((u32::from(b & 0x1F) << 6) | u32::from(rest[0] & 0x3F), 2),
            0xE0..0xF0 => (
                (u32::from(b & 0x0F) << 12) | (u32::from(rest[0] & 0x3F) << 6) | u32::from(rest[1] & 0x3F),
                3,
            ),
            _ => (
                (u32::from(b & 0x07) << 18)
                    | (u32::from(rest[0] & 0x3F) << 12)
                    | (u32::from(rest[1] & 0x3F) << 6)
                    | u32::from(rest[2] & 0x3F),
                4,
            ),
        };
        self.bytes = &self.bytes[n..];
        Some(c)
    }
}

/// The UTF-16 code units of WTF-8 text.
#[derive(Clone)]
pub struct Units<'a> {
    points: CodePoints<'a>,
    low: Option<u16>,
}

impl Iterator for Units<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        if let Some(low) = self.low.take() {
            return Some(low);
        }
        let c = self.points.next()?;
        if c < 0x10000 {
            return Some(c as u16);
        }
        let c = c - 0x10000;
        self.low = Some(0xDC00 | (c & 0x3FF) as u16);
        Some(0xD800 | (c >> 10) as u16)
    }
}

impl Hash for Wtf8 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// As Java prints it in UTF-8: a lone surrogate as `?`.
impl fmt::Display for Wtf8 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.to_str() {
            Some(s) => f.pad(s),
            None => f.pad(&self.to_string_lossy()),
        }
    }
}

impl fmt::Debug for Wtf8 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.to_str() {
            Some(s) => fmt::Debug::fmt(s, f),
            None => {
                let shown: String = self
                    .code_points()
                    .map(|c| char::from_u32(c).map_or_else(|| format!("\\u{{{c:x}}}"), |c| c.to_string()))
                    .collect();
                write!(f, "\"{shown}\"")
            }
        }
    }
}

impl PartialEq<str> for Wtf8 {
    fn eq(&self, other: &str) -> bool {
        self.0 == *other.as_bytes()
    }
}

impl AsRef<Wtf8> for str {
    fn as_ref(&self) -> &Wtf8 {
        Wtf8::new(self)
    }
}

impl AsRef<Wtf8> for Wtf8 {
    fn as_ref(&self) -> &Wtf8 {
        self
    }
}

impl AsRef<Wtf8> for String {
    fn as_ref(&self) -> &Wtf8 {
        Wtf8::new(self)
    }
}

impl<'a> From<&'a str> for &'a Wtf8 {
    fn from(s: &'a str) -> &'a Wtf8 {
        Wtf8::new(s)
    }
}

impl ToOwned for Wtf8 {
    type Owned = JString;

    fn to_owned(&self) -> JString {
        JString(self.0.to_vec())
    }
}

/// An owned string of WTF-8 bytes.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JString(Vec<u8>);

impl JString {
    pub const fn new() -> JString {
        JString(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> JString {
        JString(Vec::with_capacity(capacity))
    }

    /// The string of UTF-16 code units `units`, exactly: a pair is its character, a lone
    /// surrogate its 3-byte form.
    pub fn from_units(units: &[u16]) -> JString {
        let mut out = JString::with_capacity(units.len());
        for &u in units {
            out.push_unit(u);
        }
        out
    }

    pub fn as_wtf8(&self) -> &Wtf8 {
        // SAFETY: a `JString` holds well-formed WTF-8.
        unsafe { Wtf8::from_bytes_unchecked(&self.0) }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Appends `text`; a low surrogate at its start joins a high one at the end of this string.
    pub fn push_wtf8(&mut self, text: &Wtf8) {
        if let (Some(high), Some(low)) = (self.last_high(), text.first_low()) {
            self.0.truncate(self.0.len() - 3);
            let c = 0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
            encode(c, &mut self.0);
            self.0.extend_from_slice(&text.0[3..]);
        } else {
            self.0.extend_from_slice(&text.0);
        }
    }

    /// Appends a Rust string, which never starts with a lone surrogate.
    pub fn push_str(&mut self, text: &str) {
        self.0.extend_from_slice(text.as_bytes());
    }

    pub fn push(&mut self, c: char) {
        let mut buffer = [0; 4];
        self.push_str(c.encode_utf8(&mut buffer));
    }

    /// Appends one UTF-16 code unit, which may be half of a pair.
    pub fn push_unit(&mut self, unit: u16) {
        let mut bytes = Vec::with_capacity(3);
        encode(u32::from(unit), &mut bytes);
        // SAFETY: one encoded code point is well-formed WTF-8.
        self.push_wtf8(unsafe { Wtf8::from_bytes_unchecked(&bytes) });
    }

    /// Appends code point `c`, which may be a lone surrogate.
    pub fn push_code_point(&mut self, c: u32) {
        if c < 0x10000 {
            self.push_unit(c as u16);
        } else {
            encode(c, &mut self.0);
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl Deref for JString {
    type Target = Wtf8;

    fn deref(&self) -> &Wtf8 {
        self.as_wtf8()
    }
}

impl Borrow<Wtf8> for JString {
    fn borrow(&self) -> &Wtf8 {
        self.as_wtf8()
    }
}

impl AsRef<Wtf8> for JString {
    fn as_ref(&self) -> &Wtf8 {
        self.as_wtf8()
    }
}

impl From<&str> for JString {
    fn from(s: &str) -> JString {
        JString(s.as_bytes().to_vec())
    }
}

impl From<String> for JString {
    fn from(s: String) -> JString {
        JString(s.into_bytes())
    }
}

impl From<&Wtf8> for JString {
    fn from(s: &Wtf8) -> JString {
        s.to_owned()
    }
}

impl PartialEq<str> for JString {
    fn eq(&self, other: &str) -> bool {
        self.0 == other.as_bytes()
    }
}

impl PartialEq<&str> for JString {
    fn eq(&self, other: &&str) -> bool {
        self.0 == other.as_bytes()
    }
}

impl fmt::Display for JString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_wtf8(), f)
    }
}

impl fmt::Debug for JString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_wtf8(), f)
    }
}

/// Formatted text appends as it is (it is UTF-8).
impl fmt::Write for JString {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &Wtf8) -> Vec<u16> {
        s.units().collect()
    }

    #[test]
    fn plain_text_is_its_utf8() {
        let s = JString::from("aé😀");
        assert_eq!(s.as_bytes(), "aé😀".as_bytes());
        assert_eq!(s.to_str(), Some("aé😀"));
        assert_eq!(units(&s), "aé😀".encode_utf16().collect::<Vec<_>>());
        assert_eq!(s.unit_len(), 4);
        assert_eq!(s.to_string(), "aé😀");
    }

    #[test]
    fn a_lone_surrogate_is_kept_and_prints_as_a_question_mark() {
        let s = JString::from_units(&[0x61, 0xDE00, 0x62]);
        assert_eq!(s.as_bytes(), [0x61, 0xED, 0xB8, 0x80, 0x62]);
        assert_eq!(s.to_str(), None);
        assert_eq!(units(&s), [0x61, 0xDE00, 0x62]);
        assert_eq!(s.unit_len(), 3);
        assert_eq!(s.to_string(), "a?b");
        assert_eq!(format!("{s:?}"), "\"a\\u{de00}b\"");
    }

    #[test]
    fn halves_that_meet_join_into_their_character() {
        let smiley: Vec<u16> = "😀".encode_utf16().collect();
        let mut s = JString::from_units(&smiley[..1]);
        assert_eq!(s.unit_len(), 1);
        s.push_wtf8(&JString::from_units(&smiley[1..]));
        assert_eq!(s, *"😀");
        assert_eq!(JString::from_units(&smiley), *"😀");
        let mut t = JString::from("x");
        t.push_unit(smiley[0]);
        t.push_unit(smiley[1]);
        assert_eq!(t, *"x😀");
    }

    #[test]
    fn halves_in_the_wrong_order_stay_apart() {
        let s = JString::from_units(&[0xDC00, 0xD800]);
        assert_eq!(units(&s), [0xDC00, 0xD800]);
        assert_eq!(s.len(), 6);
    }

    #[test]
    fn prefixes_and_suffixes_compare_units() {
        let s = JString::from("😀x");
        let smiley: Vec<u16> = "😀".encode_utf16().collect();
        assert!(s.starts_with(Wtf8::new("😀")));
        assert!(s.starts_with(&JString::from_units(&smiley[..1])));
        assert!(!s.starts_with(&JString::from_units(&smiley[1..])));
        let t = JString::from("x😀");
        assert!(t.ends_with(&JString::from_units(&smiley[1..])));
        assert!(!t.ends_with(&JString::from_units(&smiley[..1])));
    }

    #[test]
    fn slices_are_between_code_points() {
        let s = JString::from("aé");
        assert_eq!(s.slice(1, 3), Wtf8::new("é"));
        assert!(!s.is_boundary(2));
    }

    #[test]
    fn code_points_include_lone_surrogates() {
        let s = JString::from_units(&[0xD800, 0x41]);
        assert_eq!(s.code_points().collect::<Vec<_>>(), [0xD800, 0x41]);
        let mut t = JString::new();
        t.push_code_point(0x1F600);
        t.push_code_point(0xDFFF);
        assert_eq!(units(&t), [0xD83D, 0xDE00, 0xDFFF]);
    }
}
