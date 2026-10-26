//! Java's standard charsets, for `String.getBytes` and `new String(bytes, ..)`. Encoding and
//! decoding follow the JDK (`sun.nio.cs` and `String.decodeUTF8_UTF16`), including how each
//! charset replaces what it cannot encode or decode, so that the output matches Java's bytes.
//!
//! Besides the six that the runtime has, a program may carry the JDK's single-byte charsets
//! (windows-1252, the ISO-8859 ones, KOI8-R, the IBM code pages, ...) as tables
//! (`ByteCharset`), which the translator took from the JDK it was built with and which the
//! program gives the runtime as it starts (`register_charsets`).

use crate::string::string_hash;
use crate::{Array, from_vec, throw};
use std::fmt;
use std::sync::OnceLock;

const REPLACEMENT: char = '\u{FFFD}';

/// One of the charsets that the translation supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    Utf8,
    UsAscii,
    Latin1,
    /// `UTF-16`: a byte order mark and big-endian when encoding; decoding honours a mark.
    Utf16,
    Utf16Be,
    Utf16Le,
    /// A single-byte charset of the JDK, which the program carries.
    Bytes(&'static ByteCharset),
}

/// A single-byte charset of the JDK, as a table that the translator took from the JDK's own
/// coders: what each byte decodes to, and what each char encodes to.
pub struct ByteCharset {
    name: &'static str,
    aliases: &'static [&'static str],
    /// The 256 chars of the bytes, in order; U+FFFD where a byte has none.
    decode: &'static str,
    /// The chars whose byte is not the first byte that decodes to them: another byte, or -1 for
    /// a char that the charset cannot encode.
    overrides: &'static [(u16, i16)],
    /// The byte that replaces what cannot be encoded.
    replacement: u8,
    coding: OnceLock<Coding>,
}

/// The tables of a `ByteCharset` as the coders read them, made on first use.
struct Coding {
    decode: [char; 256],
    /// Each char that encodes, with its byte, by char.
    encode: Vec<(u16, u8)>,
}

impl ByteCharset {
    pub const fn new(
        name: &'static str,
        aliases: &'static [&'static str],
        decode: &'static str,
        overrides: &'static [(u16, i16)],
        replacement: u8,
    ) -> ByteCharset {
        ByteCharset { name, aliases, decode, overrides, replacement, coding: OnceLock::new() }
    }

    fn coding(&self) -> &Coding {
        self.coding.get_or_init(|| {
            let mut decode = [REPLACEMENT; 256];
            for (b, c) in self.decode.chars().enumerate().take(256) {
                decode[b] = c;
            }
            let mut encode: Vec<(u16, u8)> = Vec::with_capacity(256);
            for (b, &c) in decode.iter().enumerate() {
                if c != REPLACEMENT && !encode.iter().any(|&(e, _)| u32::from(e) == c as u32) {
                    encode.push((c as u32 as u16, b as u8));
                }
            }
            encode.retain(|(c, _)| !self.overrides.iter().any(|(o, _)| o == c));
            encode.extend(self.overrides.iter().filter(|(_, b)| *b >= 0).map(|&(c, b)| (c, b as u8)));
            encode.sort_unstable();
            Coding { decode, encode }
        })
    }

    fn named(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name) || self.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    }

    /// One byte per char: the table's, or the replacement for a char that it cannot encode, a
    /// surrogate pair and a lone surrogate, as the JDK's `SingleByte` encoder replaces them.
    fn encode(&self, s: &crate::Wtf8) -> Vec<u8> {
        let encode = &self.coding().encode;
        let byte = |c: u32| {
            let c = u16::try_from(c).ok().filter(|c| !(0xD800..0xE000).contains(c))?;
            encode.binary_search_by_key(&c, |&(e, _)| e).ok().map(|at| encode[at].1)
        };
        s.code_points().map(|c| byte(c).unwrap_or(self.replacement)).collect()
    }

    fn decode(&self, bytes: &[i8]) -> String {
        let decode = &self.coding().decode;
        bytes.iter().map(|&b| decode[b as u8 as usize]).collect()
    }
}

/// Two tables are the same charset only if they are the same table, as Java has one object for
/// each charset.
impl PartialEq for ByteCharset {
    fn eq(&self, other: &ByteCharset) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for ByteCharset {}

impl fmt::Debug for ByteCharset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// The single-byte charsets that the program carries, which `register_charsets` sets as it starts.
static PROGRAM_CHARSETS: OnceLock<&'static [ByteCharset]> = OnceLock::new();

/// Gives the runtime the charsets that the program carries, before anything looks one up.
pub fn register_charsets(charsets: &'static [ByteCharset]) {
    let _ = PROGRAM_CHARSETS.set(charsets);
}

/// `toString()` is the canonical name, so a charset prints as Java prints it.
impl fmt::Display for Charset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The canonical names and aliases of each charset, which Java matches case-insensitively.
const NAMES: &[(&[&str], Charset)] = &[
    (&["UTF-8", "UTF8", "unicode-1-1-utf-8"], Charset::Utf8),
    (
        &[
            "US-ASCII",
            "ASCII",
            "us-ascii",
            "ascii7",
            "646",
            "ANSI_X3.4-1968",
            "ANSI_X3.4-1986",
            "IBM367",
            "ISO646-US",
            "ISO_646.irv:1991",
            "cp367",
            "csASCII",
            "iso-ir-6",
            "iso_646.irv:1983",
            "us",
        ],
        Charset::UsAscii,
    ),
    (
        &[
            "ISO-8859-1",
            "latin1",
            "l1",
            "819",
            "8859_1",
            "IBM819",
            "IBM-819",
            "ISO8859-1",
            "ISO8859_1",
            "ISO_8859-1",
            "ISO_8859-1:1987",
            "ISO_8859_1",
            "cp819",
            "csISOLatin1",
            "iso-ir-100",
        ],
        Charset::Latin1,
    ),
    (&["UTF-16", "UTF_16", "utf16", "unicode", "UnicodeBig"], Charset::Utf16),
    (&["UTF-16BE", "UTF_16BE", "UnicodeBigUnmarked", "X-UTF-16BE", "ISO-10646-UCS-2"], Charset::Utf16Be),
    (&["UTF-16LE", "UTF_16LE", "UnicodeLittleUnmarked", "X-UTF-16LE"], Charset::Utf16Le),
];

impl Charset {
    /// The charset of `name`: one of the runtime's, or one that the program carries.
    fn lookup(name: &str) -> Option<Charset> {
        Charset::builtin(name).or_else(|| {
            let program = PROGRAM_CHARSETS.get()?;
            program.iter().find(|c| c.named(name)).map(Charset::Bytes)
        })
    }

    fn builtin(name: &str) -> Option<Charset> {
        NAMES
            .iter()
            .find(|(names, _)| names.iter().any(|n| n.eq_ignore_ascii_case(name)))
            .map(|&(_, charset)| charset)
    }

    /// The charset of the runtime's six that `forName(name)` finds, without failing: `None` where
    /// the name is not one of theirs. The translator finds the charset of a constant name with it.
    pub fn known(name: &crate::Wtf8) -> Option<Charset> {
        let name = &*name.to_string_lossy();
        Charset::builtin(name)
    }

    /// `name()`: the canonical name, which `toString()` gives too.
    pub fn name(self) -> &'static str {
        match self {
            Charset::Utf8 => "UTF-8",
            Charset::UsAscii => "US-ASCII",
            Charset::Latin1 => "ISO-8859-1",
            Charset::Utf16 => "UTF-16",
            Charset::Utf16Be => "UTF-16BE",
            Charset::Utf16Le => "UTF-16LE",
            Charset::Bytes(table) => table.name,
        }
    }

    /// `displayName()` in English, which these charsets' display names are: their canonical names.
    pub fn display_name(self) -> &'static str {
        self.name()
    }

    /// `hashCode()`, which Java defines as the hash code of the name.
    pub fn hash_code(self) -> i32 {
        string_hash(crate::Wtf8::new(self.name()))
    }

    /// `Charset.defaultCharset()`: UTF-8, as Java 18 and later have it (JEP 400).
    pub fn default_charset() -> Charset {
        Charset::Utf8
    }

    /// `Charset.forName(name)`: a null name is an `IllegalArgumentException`, a malformed name an
    /// `IllegalCharsetNameException` and an unknown one an `UnsupportedCharsetException`, each
    /// with the name as its message.
    pub fn for_name(name: Option<&crate::Wtf8>) -> Charset {
        let name = name.map(|name| name.to_string_lossy());
        let name = name.as_deref();
        let name = Charset::legal_name(name);
        Charset::lookup(name).unwrap_or_else(|| throw("java.nio.charset.UnsupportedCharsetException", name))
    }

    /// `Charset.isSupported(name)`, which checks the name as `for_name` does.
    pub fn is_supported(name: Option<&crate::Wtf8>) -> bool {
        let name = name.map(|name| name.to_string_lossy());
        let name = name.as_deref();
        Charset::lookup(Charset::legal_name(name)).is_some()
    }

    /// The name, checked as Java checks it: letters and digits, and `-+:_.` after the first
    /// character.
    fn legal_name(name: Option<&str>) -> &str {
        let name = name.unwrap_or_else(|| throw("java.lang.IllegalArgumentException", "Null charset name"));
        let legal = !name.is_empty()
            && name.char_indices().all(|(i, c)| c.is_ascii_alphanumeric() || (i > 0 && "-+:_.".contains(c)));
        if !legal {
            throw("java.nio.charset.IllegalCharsetNameException", name);
        }
        name
    }

    /// A charset given by name to `getBytes` or `new String`, which throws
    /// `UnsupportedEncodingException` for an unknown name.
    pub fn for_encoding_name(name: &crate::Wtf8) -> Charset {
        let name = &*name.to_string_lossy();
        Charset::lookup(name).unwrap_or_else(|| throw("java.io.UnsupportedEncodingException", name))
    }

    /// The bytes of `s` in this charset. What it cannot encode becomes `?` (one per character,
    /// so an emoji is one `?`), as Java replaces it.
    pub fn encode(self, s: &crate::Wtf8) -> Vec<u8> {
        if let Charset::Bytes(table) = self {
            return table.encode(s);
        }
        let s = &*s.to_string_lossy();
        match self {
            Charset::Utf8 => s.as_bytes().to_vec(),
            Charset::UsAscii => s.chars().map(|c| if c < '\u{80}' { c as u8 } else { b'?' }).collect(),
            Charset::Latin1 => s.chars().map(|c| if c < '\u{100}' { c as u8 } else { b'?' }).collect(),
            // The JDK writes the mark with the first character, so an empty string has no bytes.
            Charset::Utf16 if s.is_empty() => Vec::new(),
            Charset::Utf16 => {
                [0xFE, 0xFF].into_iter().chain(Charset::Utf16Be.encode(crate::Wtf8::new(s))).collect()
            }
            Charset::Utf16Be => s.encode_utf16().flat_map(u16::to_be_bytes).collect(),
            Charset::Utf16Le => s.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            Charset::Bytes(_) => unreachable!("a table encodes above"),
        }
    }

    /// The text of `bytes` in this charset, with each malformed sequence replaced by U+FFFD as
    /// Java replaces it.
    pub fn decode(self, bytes: &[i8]) -> String {
        if let Charset::Bytes(table) = self {
            return table.decode(bytes);
        }
        let bytes: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
        match self {
            Charset::Utf8 => decode_utf8(&bytes),
            Charset::UsAscii => {
                bytes.iter().map(|&b| if b < 0x80 { b as char } else { REPLACEMENT }).collect()
            }
            Charset::Latin1 => bytes.iter().map(|&b| b as char).collect(),
            Charset::Utf16 => decode_utf16(&bytes, true, true),
            Charset::Utf16Be => decode_utf16(&bytes, true, false),
            Charset::Utf16Le => decode_utf16(&bytes, false, false),
            Charset::Bytes(_) => unreachable!("a table decodes above"),
        }
    }
}

/// `s.getBytes(charset)`, a new array of bytes.
pub fn get_bytes(s: &crate::Wtf8, charset: Charset) -> Array<i8> {
    from_vec(charset.encode(s).into_iter().map(|b| b as i8).collect())
}

/// `new String(bytes, charset)`.
pub fn new_string(bytes: &[i8], charset: Charset) -> crate::JString {
    charset.decode(bytes).into()
}

/// `new String(bytes, offset, length, charset)`, which throws for a range outside the array.
pub fn new_string_range(
    bytes: &(impl crate::Source<i8> + ?Sized),
    offset: i32,
    length: i32,
    charset: Charset,
) -> crate::JString {
    try_new_string_range(bytes, offset, length, charset).unwrap_or_else(crate::uncaught)
}

/// `new_string_range` where the program observes the `StringIndexOutOfBoundsException` of a range
/// outside the array.
pub fn try_new_string_range(
    bytes: &(impl crate::Source<i8> + ?Sized),
    offset: i32,
    length: i32,
    charset: Charset,
) -> Result<crate::JString, crate::Exception> {
    let len = bytes.length() as i64;
    if offset < 0 || length < 0 || i64::from(offset) > len - i64::from(length) {
        let message = format!("Range [{offset}, {offset} + {length}) out of bounds for length {len}");
        return Err(crate::Exception::new("java.lang.StringIndexOutOfBoundsException", message));
    }
    Ok(charset.decode(&bytes.range(offset as usize..(offset + length) as usize)).into())
}

/// A code point as a character; the callers only pass valid scalar values.
fn scalar(code: u32) -> char {
    char::from_u32(code).unwrap_or(REPLACEMENT)
}

fn not_continuation(b: u8) -> bool {
    b & 0xC0 != 0x80
}

/// The UTF-8 decoder of `String`, which replaces each maximal malformed subsequence with one
/// U+FFFD, except that an encoded surrogate (`ED A0 80`) gives one U+FFFD for its three bytes.
fn decode_utf8(src: &[u8]) -> String {
    let len = src.len();
    let mut out = String::with_capacity(len);
    let mut sp = 0;
    while sp < len {
        let b1 = src[sp];
        sp += 1;
        if b1 < 0x80 {
            out.push(b1 as char);
        } else if (0xC2..=0xDF).contains(&b1) {
            if sp < len {
                let b2 = src[sp];
                sp += 1;
                if not_continuation(b2) {
                    out.push(REPLACEMENT);
                    sp -= 1;
                } else {
                    out.push(scalar((u32::from(b1 & 0x1F) << 6) | u32::from(b2 & 0x3F)));
                }
                continue;
            }
            out.push(REPLACEMENT);
            break;
        } else if (0xE0..=0xEF).contains(&b1) {
            if sp + 1 < len {
                let (b2, b3) = (src[sp], src[sp + 1]);
                sp += 2;
                if malformed3(b1, b2, b3) {
                    out.push(REPLACEMENT);
                    sp -= 3;
                    sp += malformed3_length(src, sp);
                } else {
                    let code =
                        (u32::from(b1 & 0x0F) << 12) | (u32::from(b2 & 0x3F) << 6) | u32::from(b3 & 0x3F);
                    out.push(if (0xD800..=0xDFFF).contains(&code) { REPLACEMENT } else { scalar(code) });
                }
                continue;
            }
            if sp < len && malformed3_2(b1, src[sp]) {
                out.push(REPLACEMENT);
                continue;
            }
            out.push(REPLACEMENT);
            break;
        } else if (0xF0..=0xF7).contains(&b1) {
            if sp + 2 < len {
                let (b2, b3, b4) = (src[sp], src[sp + 1], src[sp + 2]);
                sp += 3;
                let code = (u32::from(b1 & 0x07) << 18)
                    | (u32::from(b2 & 0x3F) << 12)
                    | (u32::from(b3 & 0x3F) << 6)
                    | u32::from(b4 & 0x3F);
                if malformed4(b2, b3, b4) || !(0x10000..=0x10FFFF).contains(&code) {
                    out.push(REPLACEMENT);
                    sp -= 4;
                    sp += malformed4_length(src, sp);
                } else {
                    out.push(scalar(code));
                }
                continue;
            }
            if b1 > 0xF4 || (sp < len && malformed4_2(b1, src[sp])) {
                out.push(REPLACEMENT);
                continue;
            }
            sp += 1;
            out.push(REPLACEMENT);
            if sp < len && not_continuation(src[sp]) {
                continue;
            }
            break;
        } else {
            out.push(REPLACEMENT);
        }
    }
    out
}

/// Whether the three bytes of a three-byte sequence are malformed (an overlong encoding, a
/// missing continuation byte).
fn malformed3(b1: u8, b2: u8, b3: u8) -> bool {
    (b1 == 0xE0 && b2 & 0xE0 == 0x80) || not_continuation(b2) || not_continuation(b3)
}

fn malformed3_2(b1: u8, b2: u8) -> bool {
    (b1 == 0xE0 && b2 & 0xE0 == 0x80) || not_continuation(b2)
}

/// How many bytes of the malformed sequence at `at` are consumed: one or two.
fn malformed3_length(src: &[u8], at: usize) -> usize {
    let (b1, b2) = (src[at], src[at + 1]);
    if (b1 == 0xE0 && b2 & 0xE0 == 0x80) || not_continuation(b2) { 1 } else { 2 }
}

fn malformed4(b2: u8, b3: u8, b4: u8) -> bool {
    not_continuation(b2) || not_continuation(b3) || not_continuation(b4)
}

fn malformed4_2(b1: u8, b2: u8) -> bool {
    (b1 == 0xF0 && !(0x90..=0xBF).contains(&b2)) || (b1 == 0xF4 && b2 & 0xF0 != 0x80) || not_continuation(b2)
}

/// How many bytes of the malformed four-byte sequence at `at` are consumed: one to three.
fn malformed4_length(src: &[u8], at: usize) -> usize {
    let (b1, b2) = (src[at], src[at + 1]);
    if b1 > 0xF4
        || (b1 == 0xF0 && !(0x90..=0xBF).contains(&b2))
        || (b1 == 0xF4 && b2 & 0xF0 != 0x80)
        || not_continuation(b2)
    {
        1
    } else if not_continuation(src[at + 2]) {
        2
    } else {
        3
    }
}

/// The UTF-16 decoder of `sun.nio.cs.UnicodeDecoder`. A high surrogate followed by a unit that
/// is not a low surrogate gives one U+FFFD for both units; an unpaired low surrogate gives one
/// for its unit; a trailing odd byte, or a high surrogate cut short, gives one for the rest.
fn decode_utf16(src: &[u8], mut big_endian: bool, detect_mark: bool) -> String {
    let unit = |at: usize, big: bool| {
        let pair = [src[at], src[at + 1]];
        if big { u16::from_be_bytes(pair) } else { u16::from_le_bytes(pair) }
    };
    let mut out = String::with_capacity(src.len() / 2);
    let mut at = 0;
    if detect_mark && src.len() > 1 {
        match unit(0, true) {
            0xFEFF => at = 2,
            0xFFFE => {
                big_endian = false;
                at = 2;
            }
            _ => {}
        }
    }
    while src.len() - at > 1 {
        let c = unit(at, big_endian);
        match c {
            0xD800..=0xDBFF if src.len() - at < 4 => break,
            0xD800..=0xDBFF => {
                let c2 = unit(at + 2, big_endian);
                if (0xDC00..=0xDFFF).contains(&c2) {
                    let code = 0x10000 + ((u32::from(c) - 0xD800) << 10) + (u32::from(c2) - 0xDC00);
                    out.push(scalar(code));
                } else {
                    out.push(REPLACEMENT);
                }
                at += 4;
            }
            0xDC00..=0xDFFF => {
                out.push(REPLACEMENT);
                at += 2;
            }
            _ => {
                out.push(scalar(u32::from(c)));
                at += 2;
            }
        }
    }
    if at < src.len() {
        out.push(REPLACEMENT);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed(bytes: &[u8]) -> Vec<i8> {
        bytes.iter().map(|&b| b as i8).collect()
    }

    #[test]
    fn utf8_malformed_input_is_replaced_as_the_jdk_does() {
        let cases: &[(&[u8], &str)] = &[
            (&[0xE2, 0x82], "\u{FFFD}"),
            (&[0xE2, 0x41], "\u{FFFD}A"),
            (&[0xED, 0xA0, 0x80], "\u{FFFD}"),
            (&[0xF0, 0x9F, 0x98], "\u{FFFD}"),
            (&[0xF0, 0x9F, 0x41], "\u{FFFD}A"),
            (&[0xC0, 0xAF], "\u{FFFD}\u{FFFD}"),
            (&[0x80, 0xF8], "\u{FFFD}\u{FFFD}"),
            (&[0xF5, 0x80, 0x80, 0x80], "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"),
            (&[0xF0, 0x80, 0x80, 0x80], "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"),
            ("a\u{e9}\u{20ac}\u{1F600}".as_bytes(), "a\u{e9}\u{20ac}\u{1F600}"),
        ];
        for (bytes, text) in cases {
            assert_eq!(Charset::Utf8.decode(&signed(bytes)), *text, "{bytes:02X?}");
        }
    }

    #[test]
    fn utf16_decodes_pairs_and_replaces_lone_surrogates() {
        let cases: &[(&[u8], &str)] = &[
            (&[0xFE, 0xFF, 0x00, 0x41], "A"),
            (&[0xFF, 0xFE, 0x41, 0x00], "A"),
            (&[0xD8, 0x3D, 0xDE, 0x00], "\u{1F600}"),
            (&[0xD8, 0x3D, 0x00, 0x41], "\u{FFFD}"),
            (&[0xDE, 0x00], "\u{FFFD}"),
            (&[0x00, 0x41, 0x00], "A\u{FFFD}"),
            (&[0xD8, 0x3D], "\u{FFFD}"),
            (&[0xFE, 0xFF], ""),
        ];
        for (bytes, text) in cases {
            assert_eq!(Charset::Utf16.decode(&signed(bytes)), *text, "{bytes:02X?}");
        }
    }
}
