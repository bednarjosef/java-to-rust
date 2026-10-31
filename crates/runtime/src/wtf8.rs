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
use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};

/// The text of a string literal.
pub const fn wtf8(s: &str) -> &Wtf8 {
    Wtf8::new(s)
}

/// The text of a string literal that holds a lone surrogate, given as its WTF-8 bytes. Written in
/// a `const` block, it checks the bytes when the program is compiled.
///
/// # Panics
/// If the bytes are not WTF-8, or hold a high surrogate's 3-byte form directly before a low one's.
pub const fn wtf8_bytes(bytes: &[u8]) -> &Wtf8 {
    assert!(well_formed(bytes), "a string literal that is not WTF-8");
    // SAFETY: the bytes were just checked.
    unsafe { Wtf8::from_bytes_unchecked(bytes) }
}

/// The case of a `switch` on a string with many labels whose label is `text`, given the labels'
/// WTF-8 bytes, sorted, each with its case: `usize::MAX` when no label is. The generator writes
/// such a switch as a `match` on the case rather than on the bytes, whose thousands of
/// comparisons in one function are slow for LLVM to compile.
pub fn string_case(text: &[u8], cases: &[(&[u8], usize)]) -> usize {
    cases.binary_search_by(|(label, _)| (*label).cmp(text)).map_or(usize::MAX, |i| cases[i].1)
}

/// Whether `bytes` are WTF-8 in which no high surrogate's 3-byte form comes directly before a low
/// one's.
const fn well_formed(bytes: &[u8]) -> bool {
    let mut i = 0;
    let mut after_high = false;
    while i < bytes.len() {
        let b = bytes[i];
        let (len, low, high) = match b {
            0x00..=0x7F => (1, 0x80, 0xBF),
            0xC2..=0xDF => (2, 0x80, 0xBF),
            0xE0 => (3, 0xA0, 0xBF),
            0xE1..=0xEF => (3, 0x80, 0xBF),
            0xF0 => (4, 0x90, 0xBF),
            0xF1..=0xF3 => (4, 0x80, 0xBF),
            0xF4 => (4, 0x80, 0x8F),
            _ => return false,
        };
        if i + len > bytes.len() {
            return false;
        }
        if len > 1 && (bytes[i + 1] < low || bytes[i + 1] > high) {
            return false;
        }
        let mut j = 2;
        while j < len {
            if bytes[i + j] & 0xC0 != 0x80 {
                return false;
            }
            j += 1;
        }
        let surrogate = b == 0xED && bytes[i + 1] >= 0xA0;
        if after_high && surrogate && bytes[i + 1] >= 0xB0 {
            return false;
        }
        after_high = surrogate && bytes[i + 1] < 0xB0;
        i += len;
    }
    true
}

/// A string slice of WTF-8 bytes.
#[derive(Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Wtf8([u8]);

/// Whether `a` and `b` are the same bytes: inline for texts of up to 32 bytes, which most keys
/// and names are, a word at a time where the slice comparison calls `memcmp`, whose call costs
/// more than comparing so few bytes.
#[inline]
pub(crate) fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    let n = a.len();
    if n != b.len() {
        return false;
    }
    let word = |s: &[u8], i: usize| u64::from_ne_bytes(s[i..i + 8].try_into().unwrap());
    let half = |s: &[u8], i: usize| u32::from_ne_bytes(s[i..i + 4].try_into().unwrap());
    match n {
        // The last word may overlap the one before it, which compares those bytes twice.
        8..=32 => {
            let mut i = 0;
            while i + 8 < n {
                if word(a, i) != word(b, i) {
                    return false;
                }
                i += 8;
            }
            word(a, n - 8) == word(b, n - 8)
        }
        4..8 => half(a, 0) == half(b, 0) && half(a, n - 4) == half(b, n - 4),
        0..4 => a.iter().zip(b).all(|(x, y)| x == y),
        _ => a == b,
    }
}

impl PartialEq for Wtf8 {
    #[inline]
    fn eq(&self, other: &Wtf8) -> bool {
        same_bytes(&self.0, &other.0)
    }
}

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
        // The 3-byte form of a surrogate is the only WTF-8 that is not UTF-8.
        std::str::from_utf8(&self.0).ok()
    }

    /// The text as a Rust string, ending the program where it holds a lone surrogate, which
    /// `what` does not take.
    pub fn expect_str(&self, what: &str) -> &str {
        self.to_str().unwrap_or_else(|| {
            crate::throw_uncaught(format!(
                "{what} of a string with a lone surrogate is not supported by the translation"
            ))
        })
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

    /// `s.contains(t)`: whether `t` occurs in it, as code units.
    pub fn contains(&self, t: &Wtf8) -> bool {
        crate::index_of(self, t, 0) >= 0
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

    /// The byte offset of the first occurrence of `t` from byte `from`, which must be a boundary.
    /// Bytes compare as units unless `t` starts with a low surrogate or ends with a high one
    /// (`has_half_edges`), which can match half of a pair.
    pub fn find_from(&self, t: &Wtf8, from: usize) -> Option<usize> {
        find_bytes(&self.0[from..], &t.0).map(|i| i + from)
    }

    /// The byte offset of the last occurrence of `t` that ends by byte `end`.
    pub fn rfind_before(&self, t: &Wtf8, end: usize) -> Option<usize> {
        rfind_bytes(&self.0[..end], &t.0)
    }

    /// Whether it starts with a low surrogate or ends with a high one: then, as a pattern, it can
    /// match half of a pair, which a comparison of bytes does not see.
    pub fn has_half_edges(&self) -> bool {
        starts_low(&self.0) || ends_high(&self.0)
    }

    /// The code points with the byte offset each starts at.
    pub fn code_point_indices(&self) -> impl Iterator<Item = (usize, u32)> + '_ {
        let mut offset = 0;
        self.code_points().map(move |c| {
            let at = offset;
            offset += code_point_len(c);
            (at, c)
        })
    }

    /// Without the code points at its start (when `start`) and at its end (when `end`) for which
    /// `trimmed` holds.
    pub fn trim_by(&self, start: bool, end: bool, trimmed: impl Fn(u32) -> bool) -> &Wtf8 {
        let mut from = 0;
        if start {
            from = self.code_point_indices().find(|&(_, c)| !trimmed(c)).map_or(self.0.len(), |(i, _)| i);
        }
        let mut to = self.0.len();
        if end {
            to = from;
            for (i, c) in self.code_point_indices().skip_while(|&(i, _)| i < from) {
                if !trimmed(c) {
                    to = i + code_point_len(c);
                }
            }
        }
        self.slice(from, to)
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

/// Whether WTF-8 `bytes` end with a high surrogate.
fn ends_high(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[bytes.len() - 3] == 0xED && (0xA0..0xB0).contains(&bytes[bytes.len() - 2])
}

/// How many first-byte candidates `find_bytes` and `rfind_bytes` try before they search the rest
/// with the standard library's linear-time search.
const CANDIDATES: usize = 64;

/// The first occurrence of `n` in `h`: a scan for its first byte, comparing the rest there, so
/// that a match near the start costs only the bytes up to it (a search of the whole text as a
/// `str` checked all of it to be UTF-8 first, which made a loop of `indexOf(t, from)` over a long
/// string quadratic). After `CANDIDATES` misses the rest goes to the standard library's search,
/// linear in the worst case, where it is UTF-8. A pattern's first byte is never inside a code
/// point, so a candidate is a code point boundary.
fn find_bytes(h: &[u8], n: &[u8]) -> Option<usize> {
    let Some((&first, rest)) = n.split_first() else { return Some(0) };
    let mut at = 0;
    for _ in 0..CANDIDATES {
        let p = at + h[at..].iter().position(|&b| b == first)?;
        if h.len() - p < n.len() {
            return None;
        }
        if &h[p + 1..p + n.len()] == rest {
            return Some(p);
        }
        at = p + 1;
    }
    let h = &h[at..];
    let found = match (std::str::from_utf8(h), std::str::from_utf8(n)) {
        (Ok(h), Ok(n)) => h.find(n),
        _ => h.windows(n.len()).position(|w| w == n),
    };
    found.map(|i| i + at)
}

/// The last occurrence of `n` in `h`, as `find_bytes` from the end.
fn rfind_bytes(h: &[u8], n: &[u8]) -> Option<usize> {
    let Some((&first, rest)) = n.split_first() else { return Some(h.len()) };
    let mut end = h.len().checked_sub(n.len() - 1)?;
    for _ in 0..CANDIDATES {
        let p = h[..end].iter().rposition(|&b| b == first)?;
        if &h[p + 1..p + n.len()] == rest {
            return Some(p);
        }
        end = p;
    }
    // A match starts before `end`, so ends before `end + n.len() - 1`.
    let h = &h[..end + n.len() - 1];
    match (std::str::from_utf8(h), std::str::from_utf8(n)) {
        (Ok(h), Ok(n)) => h.rfind(n),
        _ => h.windows(n.len()).rposition(|w| w == n),
    }
}

/// Whether WTF-8 `bytes` start with a low surrogate.
fn starts_low(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0xED && (0xB0..0xC0).contains(&bytes[1])
}

/// The surrogate of the 3 bytes `b`.
fn surrogate_at(b: &[u8]) -> u16 {
    0xD000 | (u16::from(b[1] & 0x3F) << 6) | u16::from(b[2] & 0x3F)
}

/// The length in bytes of code point `c` in WTF-8.
fn code_point_len(c: u32) -> usize {
    match c {
        0..0x80 => 1,
        0x80..0x800 => 2,
        0x800..0x10000 => 3,
        _ => 4,
    }
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
        JString::of(self.0.to_vec())
    }
}

/// An owned string of WTF-8 bytes, and whether they are all ASCII once something has asked
/// (`is_ascii_text`), as Java's `String` keeps its coder: on an ASCII string a code unit index
/// is a byte offset, so the `String` methods that take an index need no walk of the text
/// (`java::Text`). It also keeps its `hashCode()` once computed (0 until then), as Java's
/// `String` does, in what would otherwise be padding. Any change to the text forgets both.
pub struct JString(Vec<u8>, AtomicU8, AtomicI32);

/// What a `JString` knows of its bytes: nothing yet, all ASCII, or not.
const UNKNOWN: u8 = 0;
const ASCII: u8 = 1;
const NOT_ASCII: u8 = 2;

impl JString {
    pub const fn new() -> JString {
        JString(Vec::new(), AtomicU8::new(UNKNOWN), AtomicI32::new(0))
    }

    const fn of(bytes: Vec<u8>) -> JString {
        JString(bytes, AtomicU8::new(UNKNOWN), AtomicI32::new(0))
    }

    /// The text `bytes`, which the caller knows are ASCII.
    pub(crate) fn from_ascii(bytes: Vec<u8>) -> JString {
        debug_assert!(bytes.is_ascii());
        JString(bytes, AtomicU8::new(ASCII), AtomicI32::new(0))
    }

    pub fn with_capacity(capacity: usize) -> JString {
        JString::of(Vec::with_capacity(capacity))
    }

    /// Whether every byte is ASCII, so that each is a code unit: found by a scan the first time,
    /// then kept until the text changes.
    #[inline]
    pub fn is_ascii_text(&self) -> bool {
        match self.1.load(Ordering::Relaxed) {
            ASCII => true,
            NOT_ASCII => false,
            _ => {
                let ascii = self.0.is_ascii();
                self.1.store(if ascii { ASCII } else { NOT_ASCII }, Ordering::Relaxed);
                ascii
            }
        }
    }

    /// Records whether the text is ASCII, which the caller knows.
    pub(crate) fn know_ascii(&self, ascii: bool) {
        debug_assert_eq!(ascii, self.0.is_ascii());
        self.1.store(if ascii { ASCII } else { NOT_ASCII }, Ordering::Relaxed);
    }

    /// `hashCode()`: computed the first time, then kept until the text changes. Threads that
    /// compute it at once store the same value.
    #[inline]
    pub fn hash_code(&self) -> i32 {
        let hash = self.2.load(Ordering::Relaxed);
        if hash != 0 {
            return hash;
        }
        let hash = crate::string::text_hash(self);
        self.2.store(hash, Ordering::Relaxed);
        hash
    }

    /// Forgets whether the text is ASCII, and its hash, before it changes.
    fn changing(&mut self) {
        *self.1.get_mut() = UNKNOWN;
        *self.2.get_mut() = 0;
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

    /// The string itself, as the `java::Text` that a `String` method taking an index reads,
    /// which keeps whether it is ASCII; `as_wtf8` gives the text alone.
    pub fn as_text(&self) -> &JString {
        self
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
        self.changing();
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
        self.changing();
        self.0.extend_from_slice(text.as_bytes());
    }

    pub fn push(&mut self, c: char) {
        let mut buffer = [0; 4];
        self.push_str(c.encode_utf8(&mut buffer));
    }

    /// Appends one UTF-16 code unit, which may be half of a pair.
    pub fn push_unit(&mut self, unit: u16) {
        self.changing();
        if (0xDC00..0xE000).contains(&unit)
            && let Some(high) = self.last_high()
        {
            self.0.truncate(self.0.len() - 3);
            let c = 0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(unit) - 0xDC00);
            encode(c, &mut self.0);
        } else {
            encode(u32::from(unit), &mut self.0);
        }
    }

    /// Appends code point `c`, which may be a lone surrogate.
    pub fn push_code_point(&mut self, c: u32) {
        self.changing();
        if c < 0x10000 {
            self.push_unit(c as u16);
        } else {
            encode(c, &mut self.0);
        }
    }

    pub fn clear(&mut self) {
        self.changing();
        self.0.clear();
    }
}

impl Clone for JString {
    fn clone(&self) -> JString {
        JString(
            self.0.clone(),
            AtomicU8::new(self.1.load(Ordering::Relaxed)),
            AtomicI32::new(self.2.load(Ordering::Relaxed)),
        )
    }
}

impl Default for JString {
    fn default() -> JString {
        JString::new()
    }
}

impl PartialEq for JString {
    /// Two strings whose hashes are both known and differ are not equal, which a lookup that
    /// compares keys of the same bucket finds without reading their text.
    #[inline]
    fn eq(&self, other: &JString) -> bool {
        let (a, b) = (self.2.load(Ordering::Relaxed), other.2.load(Ordering::Relaxed));
        if a != b && a != 0 && b != 0 {
            return false;
        }
        same_bytes(&self.0, &other.0)
    }
}

impl Eq for JString {}

impl PartialOrd for JString {
    fn partial_cmp(&self, other: &JString) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JString {
    fn cmp(&self, other: &JString) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Hash for JString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
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
        JString::of(s.as_bytes().to_vec())
    }
}

impl From<String> for JString {
    fn from(s: String) -> JString {
        JString::of(s.into_bytes())
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

impl PartialEq<&str> for Wtf8 {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other.as_bytes()
    }
}

impl PartialEq<JString> for Wtf8 {
    fn eq(&self, other: &JString) -> bool {
        same_bytes(&self.0, &other.0)
    }
}

impl PartialEq<Wtf8> for JString {
    fn eq(&self, other: &Wtf8) -> bool {
        same_bytes(&self.0, &other.0)
    }
}

impl PartialEq<&Wtf8> for JString {
    fn eq(&self, other: &&Wtf8) -> bool {
        same_bytes(&self.0, &other.0)
    }
}

impl PartialEq<JString> for &Wtf8 {
    fn eq(&self, other: &JString) -> bool {
        same_bytes(&self.0, &other.0)
    }
}

impl PartialEq<JString> for str {
    fn eq(&self, other: &JString) -> bool {
        *self.as_bytes() == *other.0
    }
}

impl PartialEq<JString> for &str {
    fn eq(&self, other: &JString) -> bool {
        *self.as_bytes() == *other.0
    }
}

/// A piece of a string concatenation (`java::concat!`), appended to the text being made: a
/// string as it is, so that the halves of a pair that meet join, and anything else as Java shows
/// it.
pub trait Piece {
    fn push_to(&self, out: &mut JString);
}

impl Piece for str {
    fn push_to(&self, out: &mut JString) {
        out.push_str(self);
    }
}

impl Piece for String {
    fn push_to(&self, out: &mut JString) {
        out.push_str(self);
    }
}

impl Piece for Wtf8 {
    fn push_to(&self, out: &mut JString) {
        out.push_wtf8(self);
    }
}

impl Piece for JString {
    fn push_to(&self, out: &mut JString) {
        out.push_wtf8(self);
    }
}

impl Piece for crate::JStr {
    fn push_to(&self, out: &mut JString) {
        out.push_wtf8(self);
    }
}

/// A `char`, which may be half of a pair.
impl Piece for crate::Char {
    fn push_to(&self, out: &mut JString) {
        out.push_unit(self.0);
    }
}

/// Formatted text, such as numbers and objects shown through `Display`.
impl Piece for fmt::Arguments<'_> {
    fn push_to(&self, out: &mut JString) {
        let _ = fmt::Write::write_fmt(out, *self);
    }
}

impl<T: Piece + ?Sized> Piece for &T {
    fn push_to(&self, out: &mut JString) {
        (**self).push_to(out);
    }
}

/// A string concatenation: the text of its pieces (`Piece`) one after another.
#[macro_export]
macro_rules! concat {
    ($($piece:expr),* $(,)?) => {{
        let mut text = $crate::JString::new();
        $( $crate::Piece::push_to(&$piece, &mut text); )*
        text
    }};
}

/// `s += a + b`: the pieces appended to the string `s` (a `JString`), one after another.
#[macro_export]
macro_rules! append {
    ($target:expr, $($piece:expr),* $(,)?) => {{
        let target: &mut $crate::JString = &mut $target;
        $( $crate::Piece::push_to(&$piece, target); )*
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &Wtf8) -> Vec<u16> {
        s.units().collect()
    }

    #[test]
    fn a_string_keeps_its_hash_until_it_changes() {
        let mut s = JString::from("minecraft");
        assert_eq!(std::mem::size_of::<JString>(), 32);
        assert_eq!(s.hash_code(), crate::string::text_hash(&s));
        assert_eq!(s.2.load(Ordering::Relaxed), s.hash_code());
        assert_eq!(s.clone().2.load(Ordering::Relaxed), s.hash_code());
        s.push_str(":stone");
        assert_eq!(s.2.load(Ordering::Relaxed), 0);
        assert_eq!(s.hash_code(), crate::string::text_hash(Wtf8::new("minecraft:stone")));
        // Java's hash of "f5a5a608" is 0, which is computed each time.
        assert_eq!(JString::from("f5a5a608").hash_code(), 0);
    }

    #[test]
    fn literal_bytes_are_checked() {
        const LONE: &Wtf8 = wtf8_bytes(b"a\xed\xa0\x80b");
        assert_eq!(units(LONE), [0x61, 0xD800, 0x62]);
        assert!(well_formed("aé😀".as_bytes()));
        assert!(well_formed(b"\xed\xb0\x80\xed\xa0\x80"));
        // The halves of a pair, a truncated character, an overlong form and a stray byte.
        assert!(!well_formed(b"\xed\xa0\xbd\xed\xb8\x80"));
        assert!(!well_formed(b"\xe2\x82"));
        assert!(!well_formed(b"\xc0\xaf"));
        assert!(!well_formed(b"\x80"));
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
    fn concatenation_joins_halves_and_formats_the_rest() {
        let smiley: Vec<u16> = "😀".encode_utf16().collect();
        let high = JString::from_units(&smiley[..1]);
        let text = crate::concat!("n=", format_args!("{}", 5), high, crate::Char(smiley[1]), Wtf8::new("!"));
        assert_eq!(text, *"n=5😀!");
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

    #[test]
    fn same_bytes_finds_a_difference_anywhere() {
        for n in 0..48 {
            let a: Vec<u8> = (0..n as u8).collect();
            assert!(same_bytes(&a, &a.clone()));
            for i in 0..n {
                let mut b = a.clone();
                b[i] ^= 0x80;
                assert!(!same_bytes(&a, &b), "{n} bytes, differing at {i}");
            }
            assert!(!same_bytes(&a, &a[..n.saturating_sub(1)]) || n == 0);
        }
    }

    #[test]
    fn strings_with_different_known_hashes_differ() {
        let (a, b) = (JString::from("Aa"), JString::from("BB"));
        // Java's well-known pair of equal hashes, which must still be compared by their text.
        assert_eq!(a.hash_code(), b.hash_code());
        assert_ne!(a, b);
        let c = JString::from("Ab");
        c.hash_code();
        assert_ne!(a, c);
        assert_eq!(a, JString::from("Aa"));
    }
}
