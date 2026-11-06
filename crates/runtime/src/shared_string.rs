//! Strings whose identity the program observes: a `String` that `==` compares by reference, as
//! Java does. Every other string of a translation is a plain `JString`, a value; one that
//! reaches such a comparison is a `JStr`, a shared text whose clones are the same object.
//!
//! Java interns string literals and constant expressions, and `intern()` returns the one object
//! it keeps for a text: `interned` keeps them in a table for the life of the program, so two
//! interned strings are the same object exactly when their texts are equal. Every other string
//! that Java makes is a new object (`JStr::new`), and the `String` methods that return the string
//! itself under a condition (`substring` of all of it, `trim` with nothing to trim) return the
//! same `JStr` there (`kept`, `kept_replace`, `kept_repeat`).

use crate::thin::ADDRESS;
use crate::{JString, Wtf8};
use std::alloc::{Layout, alloc, handle_alloc_error};
use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::{Mutex, OnceLock};

/// A string with an identity: copies are the same object, `same_str` compares objects. Its text
/// is WTF-8, as a `JString`'s.
///
/// The object is the text's length in four bytes, whose top bit tells whether the text is all
/// ASCII (Java's coder, see `JString::is_ascii_text`), then its hash in four (0 until `hash_code`
/// computes it, as Java's `String.hash`), then the text, in the collected heap, and a `JStr` is
/// its address: one word, copied without a count. Its alignment of four puts it
/// in a block that the collector does not scan for pointers. The top 16 bits of the address are
/// ignored, as a `java::Arc`'s are, so a `java::Thin` holds a `JStr` where it lies.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JStr(NonNull<u32>);

/// Where the text starts in a string object: after its length and its hash.
const TEXT: usize = 8;

/// The bit of the length word set when the text is all ASCII.
const ASCII_BIT: u32 = 1 << 31;

// The text never changes once made.
unsafe impl Send for JStr {}
unsafe impl Sync for JStr {}

// A `JStr` is one pointer, whose tag bits it ignores, and has no `Drop`.
unsafe impl crate::Pointer for JStr {}

impl JStr {
    /// A new string object with the text `text`.
    pub fn new(text: impl AsRef<Wtf8>) -> JStr {
        let bytes = text.as_ref().as_bytes();
        let len = u32::try_from(bytes.len())
            .ok()
            .filter(|&len| len & ASCII_BIT == 0)
            .expect("a string of less than 2 GB of text");
        let len = if bytes.is_ascii() { len | ASCII_BIT } else { len };
        let layout = Layout::from_size_align(TEXT + bytes.len(), 4).expect("a string's size fits");
        // The allocation has room for the length, the hash and the text, aligned for the length.
        unsafe {
            let p = alloc(layout);
            if p.is_null() {
                handle_alloc_error(layout);
            }
            (p as *mut u32).write(len);
            (p as *mut u32).add(1).write(0);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.add(TEXT), bytes.len());
            crate::gc::note_name(p, "java.lang.String");
            JStr(NonNull::new_unchecked(p as *mut u32))
        }
    }

    /// The address of the object, which a 4-byte reference holds divided by 8 (`compact.rs`).
    #[inline(always)]
    pub(crate) fn address(&self) -> usize {
        self.header() as usize
    }

    /// The string whose object is at `address`.
    ///
    /// # Safety
    ///
    /// `address` is that of a live string object (`address`).
    #[inline(always)]
    pub(crate) unsafe fn from_address(address: usize) -> JStr {
        JStr(unsafe { NonNull::new_unchecked(address as *mut u32) })
    }

    /// The address of the object. A string taken out of a `java::Thin` has its class number
    /// cleared (`Thin::to_pointer`).
    fn header(&self) -> *const u32 {
        debug_assert!(
            self.0.as_ptr() as usize & !ADDRESS == 0,
            "a string with a class number in its top bits"
        );
        self.0.as_ptr()
    }

    /// The text's bytes.
    fn bytes(&self) -> &[u8] {
        let header = self.header();
        // The object is the length and then that many bytes, alive while it is reachable.
        unsafe {
            std::slice::from_raw_parts((header as *const u8).add(TEXT), (*header & !ASCII_BIT) as usize)
        }
    }

    /// Whether the text is all ASCII, so that its code units are its bytes.
    #[inline]
    pub fn is_ascii_text(&self) -> bool {
        // The object's first word is its length and this bit, alive while it is reachable.
        unsafe { *self.header() & ASCII_BIT != 0 }
    }

    /// `hashCode()`: computed on first use and kept in the object, as Java keeps it. Threads that
    /// compute it at once store the same value.
    #[inline]
    pub fn hash_code(&self) -> i32 {
        let hash = self.known_hash();
        if hash != 0 {
            return hash;
        }
        let hash = crate::string::text_hash(self.as_wtf8());
        self.hash_cell().store(hash, std::sync::atomic::Ordering::Relaxed);
        hash
    }

    /// The hash kept in the object, or 0 before it is computed.
    #[inline]
    fn known_hash(&self) -> i32 {
        self.hash_cell().load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The object's second word, its hash, alive while the object is reachable.
    #[inline]
    fn hash_cell(&self) -> &std::sync::atomic::AtomicI32 {
        unsafe { std::sync::atomic::AtomicI32::from_ptr(self.header().add(1) as *mut i32) }
    }

    /// The text, as a new plain string.
    pub fn text(&self) -> JString {
        self.as_wtf8().to_owned()
    }

    /// The text, borrowed.
    /// The string itself, as the `java::Text` that a `String` method taking an index reads,
    /// which keeps whether it is ASCII; `as_wtf8` gives the text alone.
    pub fn as_text(&self) -> &JStr {
        self
    }

    pub fn as_wtf8(&self) -> &Wtf8 {
        // SAFETY: a `JStr` holds the bytes of a `Wtf8`.
        unsafe { Wtf8::from_bytes_unchecked(self.bytes()) }
    }
}

/// A new empty string.
impl Default for JStr {
    fn default() -> JStr {
        JStr::new("")
    }
}

impl From<JString> for JStr {
    fn from(text: JString) -> JStr {
        JStr::new(text)
    }
}

impl From<String> for JStr {
    fn from(text: String) -> JStr {
        JStr::new(text)
    }
}

impl From<&str> for JStr {
    fn from(text: &str) -> JStr {
        JStr::new(text)
    }
}

impl From<JStr> for JString {
    fn from(s: JStr) -> JString {
        s.text()
    }
}

// Equality, order and hashing are of the text, as `equals`, `compareTo` and `hashCode` are; `==`
// on objects is `same_str`.
impl PartialEq for JStr {
    /// The same object is equal, as Java's `equals` first asks; otherwise, as `JString`'s, hashes
    /// both known and different tell the strings apart without their text.
    #[inline]
    fn eq(&self, other: &JStr) -> bool {
        if self.header() == other.header() {
            return true;
        }
        let (a, b) = (self.known_hash(), other.known_hash());
        if a != b && a != 0 && b != 0 {
            return false;
        }
        crate::wtf8::same_bytes(self.bytes(), other.bytes())
    }
}

impl Eq for JStr {}

impl PartialEq<str> for JStr {
    fn eq(&self, other: &str) -> bool {
        crate::wtf8::same_bytes(self.bytes(), other.as_bytes())
    }
}

impl PartialEq<&str> for JStr {
    fn eq(&self, other: &&str) -> bool {
        crate::wtf8::same_bytes(self.bytes(), other.as_bytes())
    }
}

impl PartialEq<Wtf8> for JStr {
    fn eq(&self, other: &Wtf8) -> bool {
        crate::wtf8::same_bytes(self.bytes(), other.as_bytes())
    }
}

impl PartialEq<&Wtf8> for JStr {
    fn eq(&self, other: &&Wtf8) -> bool {
        crate::wtf8::same_bytes(self.bytes(), other.as_bytes())
    }
}

impl PartialEq<JStr> for Wtf8 {
    fn eq(&self, other: &JStr) -> bool {
        crate::wtf8::same_bytes(self.as_bytes(), other.bytes())
    }
}

impl PartialEq<JStr> for &Wtf8 {
    fn eq(&self, other: &JStr) -> bool {
        crate::wtf8::same_bytes(self.as_bytes(), other.bytes())
    }
}

impl PartialEq<JString> for JStr {
    fn eq(&self, other: &JString) -> bool {
        crate::wtf8::same_bytes(self.bytes(), other.as_bytes())
    }
}

impl PartialEq<JStr> for JString {
    fn eq(&self, other: &JStr) -> bool {
        crate::wtf8::same_bytes(self.as_bytes(), other.bytes())
    }
}

impl PartialOrd for JStr {
    fn partial_cmp(&self, other: &JStr) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JStr {
    fn cmp(&self, other: &JStr) -> std::cmp::Ordering {
        self.bytes().cmp(other.bytes())
    }
}

impl std::hash::Hash for JStr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_wtf8().hash(state)
    }
}

impl std::borrow::Borrow<Wtf8> for JStr {
    fn borrow(&self) -> &Wtf8 {
        self.as_wtf8()
    }
}

impl Deref for JStr {
    type Target = Wtf8;

    fn deref(&self) -> &Wtf8 {
        self.as_wtf8()
    }
}

impl AsRef<Wtf8> for JStr {
    fn as_ref(&self) -> &Wtf8 {
        self.as_wtf8()
    }
}

impl fmt::Display for JStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_wtf8(), f)
    }
}

impl fmt::Debug for JStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_wtf8(), f)
    }
}

impl crate::Handle for JStr {
    fn address(&self) -> *const u8 {
        self.header() as *const u8
    }
}

fn table() -> &'static Mutex<HashSet<JStr>> {
    static TABLE: OnceLock<Mutex<HashSet<JStr>>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

/// The one string object that Java keeps for the text `text`: a literal's, a constant's, and
/// what `intern()` returns.
pub fn interned(text: impl AsRef<Wtf8>) -> JStr {
    let text = text.as_ref();
    let mut table = table().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(found) = table.get(text) {
        return *found;
    }
    let shared = JStr::new(text);
    table.insert(shared);
    shared
}

/// The interned string of a literal, looked up once per place in the code (`cell`).
pub fn interned_literal(cell: &'static OnceLock<JStr>, text: &'static Wtf8) -> JStr {
    *cell.get_or_init(|| interned(text))
}

/// `a == b` of two strings: whether they are the same object.
pub fn same_str(a: &JStr, b: &JStr) -> bool {
    a.header() == b.header()
}

/// `a == b` of two strings that may be null.
pub fn same_nullable_str(a: Option<&JStr>, b: Option<&JStr>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same_str(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// The string that a method of `s` returns whose text is `result`, where the JDK returns `s`
/// itself when the text is unchanged (`substring` of all of it, `trim`, `strip`, the case
/// mappings, `replace` of a `char`, `concat` of `""`), the interned `""` when it is empty, and a
/// new string otherwise.
pub fn kept(s: &JStr, result: impl AsRef<Wtf8>) -> JStr {
    let result = result.as_ref();
    if *result == **s { *s } else { new_or_empty(result) }
}

/// The string that `result` is where the JDK makes it from a builder or by `substring` of
/// another (`StringBuilder.toString`, `String.format`): the interned `""` when it is empty, a new
/// string otherwise.
pub fn new_or_empty(result: impl AsRef<Wtf8>) -> JStr {
    let result = result.as_ref();
    if result.is_empty() { interned("") } else { JStr::new(result) }
}

/// A string as the translation makes it: a `JStr` is the object itself, a plain string the text
/// of a new one (`Object.toString`'s `Name@hash`, a box's text).
pub trait IntoShared {
    fn into_shared(self) -> JStr;
}

impl IntoShared for JStr {
    fn into_shared(self) -> JStr {
        self
    }
}

impl IntoShared for JString {
    fn into_shared(self) -> JStr {
        JStr::new(self)
    }
}

impl IntoShared for String {
    fn into_shared(self) -> JStr {
        JStr::new(self)
    }
}

/// The string that `Object.toString` returns, `text` as the translation makes it (see
/// `IntoShared`).
pub fn shared(text: impl IntoShared) -> JStr {
    text.into_shared()
}

/// The interned string of the text `text`, which a method makes that returns one: a class's
/// name, an enum constant's.
pub fn interned_text(text: impl AsRef<Wtf8>) -> JStr {
    interned(text)
}

/// `s.split(regex, limit)` of a shared string: its parts are new strings, the interned `""`, or
/// `s` itself where the JDK takes all of it (`substring` of all of a string is the string).
pub fn split_kept(s: &JStr, regex: &Wtf8, limit: i32) -> Vec<JStr> {
    crate::regex::split(s, regex, limit).into_iter().map(|part| kept(s, part)).collect()
}

/// What `Arrays.toString` returns: the literal `"null"` or `"[]"`, a new string otherwise.
pub fn array_text(text: impl AsRef<Wtf8>) -> JStr {
    let text = text.as_ref();
    if *text == *"null" || *text == *"[]" { interned(text) } else { JStr::new(text) }
}

/// The one string that Java keeps for `key` (a system property, a class's simple name) while
/// its text is `text`: the same object on every call.
pub fn per_key(key: &str, text: impl AsRef<Wtf8>) -> JStr {
    static KEPT: OnceLock<Mutex<std::collections::HashMap<(String, JString), JStr>>> = OnceLock::new();
    let text = text.as_ref();
    let mut kept = KEPT.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *kept.entry((key.to_owned(), text.to_owned())).or_insert_with(|| JStr::new(text))
}

/// `s.replace(target, replacement)` of two strings, whose text is `result`: `s` itself when the
/// target does not occur (or when both are the same single `char`), as the JDK's
/// `String.replace` returns it, else a new string.
pub fn kept_replace(s: &JStr, result: impl AsRef<Wtf8>, target: &Wtf8, replacement: &Wtf8) -> JStr {
    let absent = || crate::index_of(s, target, 0) < 0;
    let unchanged = match (target.unit_len(), replacement.unit_len()) {
        (0, _) => false,
        (1, 1) => target == replacement || absent(),
        _ => absent(),
    };
    if unchanged { *s } else { JStr::new(result) }
}

/// `s.repeat(count)`, whose text is `result`: `s` itself for one, the interned `""` for none or
/// an empty `s`, a new string otherwise.
pub fn kept_repeat(s: &JStr, result: impl AsRef<Wtf8>, count: i32) -> JStr {
    match count {
        1 => *s,
        _ => new_or_empty(result),
    }
}

/// `s.replaceAll(regex, replacement)` and `replaceFirst`, whose text is `result`: `s` itself when
/// the expression matches nowhere, as `Matcher.replaceAll` returns its input then.
pub fn kept_regex(s: &JStr, result: impl AsRef<Wtf8>, regex: &Wtf8) -> JStr {
    if crate::regex::finds(s, regex) { new_or_empty(result) } else { *s }
}

/// `s.stripIndent()` and `s.translateEscapes()`, whose text is `result`: the interned `""` for an
/// empty `s`, a new string otherwise.
pub fn empty_or_new(s: &JStr, result: impl AsRef<Wtf8>) -> JStr {
    if s.is_empty() { interned("") } else { JStr::new(result) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interned_strings_are_one_object_per_text() {
        let a = interned("abc");
        let b = interned(String::from("ab") + "c");
        assert!(same_str(&a, &b));
        assert!(!same_str(&a, &JStr::new("abc")));
        assert!(!same_str(&JStr::new("abc"), &JStr::new("abc")));
        let c = JStr::new("x");
        assert!(same_str(&c, &c.clone()));
    }

    #[test]
    fn kept_returns_the_receiver_when_unchanged() {
        let s = JStr::new("abc");
        assert!(same_str(&kept(&s, "abc"), &s));
        assert!(!same_str(&kept(&s, "ab"), &s));
        assert!(same_str(&kept(&s, JString::new()), &interned("")));
        assert!(same_str(&kept_replace(&s, "abc", Wtf8::new("x"), Wtf8::new("y")), &s));
        assert!(same_str(&kept_replace(&s, "abc", Wtf8::new("a"), Wtf8::new("a")), &s));
        assert!(!same_str(&kept_replace(&s, "abc", Wtf8::new("ab"), Wtf8::new("ab")), &s));
        assert!(same_str(&kept_repeat(&s, "abc", 1), &s));
        assert!(same_str(&kept_repeat(&JStr::new(""), "", 3), &interned("")));
    }
}
