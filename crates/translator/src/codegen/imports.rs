//! The `use` lines of a package's module for the items of the crate root that it names. The
//! crate root holds the items that every module uses (`Object`, `Throwable`, the standard
//! exceptions), which the generator names from other modules by their paths (`crate::Object`).
//! A module that names one of them gets `use crate::Object;` and names it `Object`, unless that
//! name is already something else in the module.
//!
//! This works on the module's text rather than in the name resolver, because the generator makes
//! names from the resolver's paths and its own suffixes (`crate::IllegalArgumentExceptionFields`
//! from `crate::` and a class's name): only the written text has the whole name.

use std::collections::BTreeSet;

/// One identifier of Rust text outside literals and comments: where it starts, and whether it is
/// named through `crate::` (`Some(start of crate::)`), named in scope (`None`), or neither
/// (after another `::` or a `.`, which needs no name in scope).
struct Word {
    start: usize,
    end: usize,
    through_crate: Option<usize>,
    in_scope: bool,
}

/// The identifiers of `text`, outside string and character literals and comments.
fn words(text: &str) -> Vec<Word> {
    let bytes = text.as_bytes();
    let mut words = Vec::new();
    let mut i = 0;
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            // A character literal such as 'a', '\n' or '\u{1F600}'; a lifetime or a label ('a,
            // 'outer:) has no closing quote, and its name is not a name in scope.
            b'\'' => {
                let close = if bytes.get(i + 1) == Some(&b'\\') {
                    text[i + 2..].find('\'').map(|at| i + 2 + at)
                } else {
                    let c = text[i + 1..].chars().next().map_or(1, char::len_utf8);
                    (bytes.get(i + 1 + c) == Some(&b'\'')).then_some(i + 1 + c)
                };
                match close {
                    Some(close) => i = close + 1,
                    None => {
                        i += 1;
                        while i < bytes.len() && ident(bytes[i]) {
                            i += 1;
                        }
                    }
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i = text[i..].find('\n').map_or(bytes.len(), |at| i + at);
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = text[i + 2..].find("*/").map_or(bytes.len(), |at| i + 2 + at + 2);
            }
            b if ident(b) && !b.is_ascii_digit() => {
                let start = i;
                while i < bytes.len() && ident(bytes[i]) {
                    i += 1;
                }
                // A raw identifier (`r#type`) is one word.
                if &text[start..i] == "r" && bytes.get(i) == Some(&b'#') {
                    i += 1;
                    while i < bytes.len() && ident(bytes[i]) {
                        i += 1;
                    }
                }
                // A byte string (`b"..."`) is a literal.
                if &text[start..i] == "b" && bytes.get(i) == Some(&b'"') {
                    continue;
                }
                let before = text[..start].trim_end_matches([' ', '\n']);
                let through_crate = before
                    .strip_suffix("::")
                    .and_then(|path| path.trim_end().strip_suffix("crate"))
                    .filter(|rest| !rest.ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == ':'))
                    .map(|rest| rest.len());
                let qualified = before.ends_with("::") || before.ends_with('.');
                words.push(Word { start, end: i, through_crate, in_scope: !qualified });
            }
            b if b.is_ascii_digit() => {
                // A number, with its suffix (`1i64`, `0x1F`).
                while i < bytes.len() && ident(bytes[i]) {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    words
}

/// Names the crate root's items in `texts`, the items of one package's module, by `use` lines:
/// rewrites each `crate::Name` that names an item of the root to `Name`, where `Name` is no
/// other name in scope in the module, and returns the `use` lines. `modules` are the root's
/// modules (named through `crate::` too, and left as they are), and `taken` the names that the
/// module's other `use` lines bring in.
pub(super) fn import_root_items(
    texts: &mut [String],
    modules: &BTreeSet<String>,
    taken: &BTreeSet<String>,
) -> String {
    let all: Vec<Vec<Word>> = texts.iter().map(|text| words(text)).collect();
    let named =
        || texts.iter().zip(&all).flat_map(|(text, words)| words.iter().map(|w| (&text[w.start..w.end], w)));
    // The names wanted through `crate::`, a few, and of those, the ones in scope: the module's
    // other names, every word of its text, are only looked up.
    let mut wanted: BTreeSet<&str> =
        named().filter(|(_, w)| w.through_crate.is_some()).map(|(name, _)| name).collect();
    for (name, w) in named() {
        if w.through_crate.is_none() && w.in_scope {
            wanted.remove(name);
        }
    }
    let imported: BTreeSet<String> = wanted
        .into_iter()
        .filter(|name| !modules.contains(*name) && !taken.contains(*name))
        .map(str::to_string)
        .collect();
    if imported.is_empty() {
        return String::new();
    }
    for (text, words) in texts.iter_mut().zip(&all) {
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for word in words {
            let Some(path) = word.through_crate else { continue };
            if imported.contains(&text[word.start..word.end]) {
                out.push_str(&text[at..path]);
                at = word.start;
            }
        }
        out.push_str(&text[at..]);
        *text = out;
    }
    imported.iter().map(|name| format!("use crate::{name};\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn import(text: &str) -> (String, String) {
        let mut texts = vec![text.to_string()];
        let modules = BTreeSet::from(["com".to_string()]);
        let uses = import_root_items(&mut texts, &modules, &BTreeSet::from(["Rc".to_string()]));
        (texts.remove(0), uses)
    }

    #[test]
    fn root_items_are_named_through_use_lines() {
        let (text, uses) = import(
            "let o: crate::Object = crate::Object::Integer(1);\nlet e = crate::IllegalArgumentExceptionFields::new();\n",
        );
        assert_eq!(
            text,
            "let o: Object = Object::Integer(1);\nlet e = IllegalArgumentExceptionFields::new();\n"
        );
        assert_eq!(uses, "use crate::IllegalArgumentExceptionFields;\nuse crate::Object;\n");
    }

    #[test]
    fn names_in_scope_modules_and_literals_stay() {
        // `Point` is a struct of the module too, `com` a module, `Rc` another `use` line's.
        let (text, uses) = import(
            "struct Point;\nlet a = crate::Point::new();\nlet b = crate::com::x::f();\nlet c: crate::Rc = \"crate::Object\".len();\nlet d = '\"'; // crate::Object\nlet e = crate::Throwable::X;\n",
        );
        assert_eq!(
            text,
            "struct Point;\nlet a = crate::Point::new();\nlet b = crate::com::x::f();\nlet c: crate::Rc = \"crate::Object\".len();\nlet d = '\"'; // crate::Object\nlet e = Throwable::X;\n"
        );
        assert_eq!(uses, "use crate::Throwable;\n");
    }

    #[test]
    fn lifetimes_and_qualified_names_are_not_names_in_scope() {
        let (text, uses) =
            import("fn f<'a>(x: &'a crate::Object) -> java::Object { x.Object; crate::Object::A }\n");
        assert_eq!(text, "fn f<'a>(x: &'a Object) -> java::Object { x.Object; Object::A }\n");
        assert_eq!(uses, "use crate::Object;\n");
    }
}
