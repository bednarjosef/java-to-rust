//! Strings whose identity the program observes: a `String` that `==` compares by reference, as
//! Java does. Every other string of a translation is a plain Rust `String`, a value; one that
//! reaches such a comparison is a `JStr`, a shared text whose clones are the same object.
//!
//! Java interns string literals and constant expressions, and `intern()` returns the one object
//! it keeps for a text: `interned` keeps them in a table for the life of the program, so two
//! interned strings are the same object exactly when their texts are equal. Every other string
//! that Java makes is a new object (`JStr::new`), and the `String` methods that return the string
//! itself under a condition (`substring` of all of it, `trim` with nothing to trim) return the
//! same `JStr` there (`kept`, `kept_replace`, `kept_repeat`).

use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock};

/// A string with an identity: clones are the same object, `same_str` compares objects.
#[derive(Clone)]
pub struct JStr(Arc<str>);

impl JStr {
    /// A new string object with the text `text`.
    pub fn new(text: impl AsRef<str>) -> JStr {
        JStr(Arc::from(text.as_ref()))
    }

    /// The text, as a new Rust `String`.
    pub fn text(&self) -> String {
        self.0.to_string()
    }
}

impl JStr {
    /// The text, borrowed.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A new empty string.
impl Default for JStr {
    fn default() -> JStr {
        JStr::new("")
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

impl From<JStr> for String {
    fn from(s: JStr) -> String {
        s.text()
    }
}

// Equality, order and hashing are of the text, as `equals`, `compareTo` and `hashCode` are; `==`
// on objects is `same_str`.
impl PartialEq for JStr {
    fn eq(&self, other: &JStr) -> bool {
        *self.0 == *other.0
    }
}

impl Eq for JStr {}

impl PartialEq<str> for JStr {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}

impl PartialEq<&str> for JStr {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}

impl PartialEq<String> for JStr {
    fn eq(&self, other: &String) -> bool {
        *self.0 == **other
    }
}

impl PartialEq<JStr> for str {
    fn eq(&self, other: &JStr) -> bool {
        self == &*other.0
    }
}

impl PartialEq<JStr> for &str {
    fn eq(&self, other: &JStr) -> bool {
        *self == &*other.0
    }
}

impl PartialEq<JStr> for String {
    fn eq(&self, other: &JStr) -> bool {
        **self == *other.0
    }
}

impl PartialOrd for JStr {
    fn partial_cmp(&self, other: &JStr) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JStr {
    fn cmp(&self, other: &JStr) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl std::hash::Hash for JStr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl std::borrow::Borrow<str> for JStr {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Deref for JStr {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for JStr {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for JStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl crate::Handle for JStr {
    fn address(&self) -> *const u8 {
        Arc::as_ptr(&self.0) as *const u8
    }
}

fn table() -> &'static Mutex<HashSet<Arc<str>>> {
    static TABLE: OnceLock<Mutex<HashSet<Arc<str>>>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

/// The one string object that Java keeps for the text `text`: a literal's, a constant's, and
/// what `intern()` returns.
pub fn interned(text: &str) -> JStr {
    let mut table = table().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(found) = table.get(text) {
        return JStr(found.clone());
    }
    let shared: Arc<str> = Arc::from(text);
    table.insert(shared.clone());
    JStr(shared)
}

/// The interned string of a literal, looked up once per place in the code (`cell`).
pub fn interned_literal(cell: &'static OnceLock<JStr>, text: &'static str) -> JStr {
    cell.get_or_init(|| interned(text)).clone()
}

/// `a == b` of two strings: whether they are the same object.
pub fn same_str(a: &JStr, b: &JStr) -> bool {
    Arc::ptr_eq(&a.0, &b.0)
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
pub fn kept(s: &JStr, result: impl AsRef<str>) -> JStr {
    let result = result.as_ref();
    if result == &**s { s.clone() } else { new_or_empty(result) }
}

/// The string that `result` is where the JDK makes it from a builder or by `substring` of
/// another (`StringBuilder.toString`, `String.format`): the interned `""` when it is empty, a new
/// string otherwise.
pub fn new_or_empty(result: impl AsRef<str>) -> JStr {
    let result = result.as_ref();
    if result.is_empty() { interned("") } else { JStr::new(result) }
}

/// A string as the translation makes it: a `JStr` is the object itself, a `String` the text of a
/// new one (`Object.toString`'s `Name@hash`, a box's text).
pub trait IntoShared {
    fn into_shared(self) -> JStr;
}

impl IntoShared for JStr {
    fn into_shared(self) -> JStr {
        self
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
pub fn interned_text(text: impl AsRef<str>) -> JStr {
    interned(text.as_ref())
}

/// `s.split(regex, limit)` of a shared string: its parts are new strings, the interned `""`, or
/// `s` itself where the JDK takes all of it (`substring` of all of a string is the string).
pub fn split_kept(s: &JStr, regex: &str, limit: i32) -> Vec<JStr> {
    crate::regex::split(s, regex, limit).into_iter().map(|part| kept(s, part)).collect()
}

/// What `Arrays.toString` returns: the literal `"null"` or `"[]"`, a new string otherwise.
pub fn array_text(text: impl AsRef<str>) -> JStr {
    match text.as_ref() {
        text @ ("null" | "[]") => interned(text),
        text => JStr::new(text),
    }
}

/// The one string that Java keeps for `key` (a system property, a class's simple name) while
/// its text is `text`: the same object on every call.
pub fn per_key(key: &str, text: impl AsRef<str>) -> JStr {
    static KEPT: OnceLock<Mutex<std::collections::HashMap<(String, String), JStr>>> = OnceLock::new();
    let text = text.as_ref();
    let mut kept = KEPT.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    kept.entry((key.to_string(), text.to_string())).or_insert_with(|| JStr::new(text)).clone()
}

/// `s.replace(target, replacement)` of two strings, whose text is `result`: `s` itself when the
/// target does not occur (or when both are the same single `char`), as the JDK's
/// `String.replace` returns it, else a new string.
pub fn kept_replace(s: &JStr, result: impl AsRef<str>, target: &str, replacement: &str) -> JStr {
    let units = |t: &str| t.encode_utf16().count();
    let unchanged = match (units(target), units(replacement)) {
        (0, _) => false,
        (1, 1) => target == replacement || !s.contains(target),
        _ => !s.contains(target),
    };
    if unchanged { s.clone() } else { JStr::new(result) }
}

/// `s.repeat(count)`, whose text is `result`: `s` itself for one, the interned `""` for none or
/// an empty `s`, a new string otherwise.
pub fn kept_repeat(s: &JStr, result: impl AsRef<str>, count: i32) -> JStr {
    match count {
        1 => s.clone(),
        _ => new_or_empty(result),
    }
}

/// `s.replaceAll(regex, replacement)` and `replaceFirst`, whose text is `result`: `s` itself when
/// the expression matches nowhere, as `Matcher.replaceAll` returns its input then.
pub fn kept_regex(s: &JStr, result: impl AsRef<str>, regex: &str) -> JStr {
    if crate::regex::finds(s, regex) { new_or_empty(result) } else { s.clone() }
}

/// `s.stripIndent()` and `s.translateEscapes()`, whose text is `result`: the interned `""` for an
/// empty `s`, a new string otherwise.
pub fn empty_or_new(s: &JStr, result: impl AsRef<str>) -> JStr {
    if s.is_empty() { interned("") } else { JStr::new(result) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interned_strings_are_one_object_per_text() {
        let a = interned("abc");
        let b = interned(&(String::from("ab") + "c"));
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
        assert!(same_str(&kept(&s, String::new()), &interned("")));
        assert!(same_str(&kept_replace(&s, "abc", "x", "y"), &s));
        assert!(same_str(&kept_replace(&s, "abc", "a", "a"), &s));
        assert!(!same_str(&kept_replace(&s, "abc", "ab", "ab"), &s));
        assert!(same_str(&kept_repeat(&s, "abc", 1), &s));
        assert!(same_str(&kept_repeat(&JStr::new(""), "", 3), &interned("")));
    }
}
