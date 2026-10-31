//! The warning of `sun.misc.Unsafe`'s methods that reach memory (`beforeMemoryAccess()`): the
//! first call of one of them in a run prints, on standard error, which method was called and by
//! which class, and where that class was loaded from. A translated program runs as `java Main`
//! does from the folder of its classes, whose class path is `.`: the classes come from the
//! working directory.

use crate::Class;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the warning has been printed.
static WARNED: AtomicBool = AtomicBool::new(false);

/// `beforeMemoryAccess()` of the method `method` of `sun.misc.Unsafe`, called by class `caller`,
/// with `--sun-misc-unsafe-memory-access=warn`, the default: the warning, once per run.
pub fn memory_access_warning(caller: &Class, method: &str) {
    if WARNED.swap(true, Ordering::SeqCst) {
        return;
    }
    let name = caller.get_name();
    let who = match class_path_location() {
        Some(location) => format!("{name} ({location})"),
        None => name.to_string(),
    };
    let text = format!(
        "WARNING: A terminally deprecated method in sun.misc.Unsafe has been called\n\
         WARNING: sun.misc.Unsafe::{method} has been called by {who}\n\
         WARNING: Please consider reporting this to the maintainers of {caller}\n\
         WARNING: sun.misc.Unsafe::{method} will be removed in a future release\n"
    );
    let _ = crate::out::err().write_all(text.as_bytes());
}

/// The location of the class path `.`, as the JVM's class loader makes it
/// (`URLClassPath.toFileURL`): the working directory's canonical path, encoded as
/// `ParseUtil.fileToEncodedURL` encodes it, as a `file:` URL that ends with `/`. `None` when the
/// path has no canonical form, which the JVM leaves out of the class path.
fn class_path_location() -> Option<String> {
    let dir = std::fs::canonicalize(".").ok()?;
    let mut path = encode_path(&dir.to_string_lossy());
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    if !path.ends_with('/') {
        path.push('/');
    }
    Some(format!("file:{path}"))
}

/// `ParseUtil.encodePath(path)`: each UTF-16 unit of `path` that is not `/`, a letter, a digit
/// or a character that a URL's path may hold, as `%` and two lowercase hex digits of each byte
/// of its UTF-8 form (a surrogate on its own, as Java encodes it).
fn encode_path(path: &str) -> String {
    // The characters below 128 that are encoded (`ParseUtil.L_ENCODED` and `H_ENCODED`): the
    // controls, space, `"`, `#`, `%`, `;`, `<`, `=`, `>`, `?`, `[`, `\`, `]`, `^`, `` ` ``, `{`,
    // `|`, `}` and DEL.
    const LOW: u64 = 0xF800802DFFFFFFFF;
    const HIGH: u64 = 0xB800000178000000;
    let encoded = |c: u32| if c < 64 { LOW >> c & 1 == 1 } else { HIGH >> (c - 64) & 1 == 1 };
    let escape = |out: &mut String, b: u32| out.push_str(&format!("%{:x}{:x}", b >> 4 & 0xF, b & 0xF));
    let mut out = String::new();
    for c in path.encode_utf16().map(u32::from) {
        if c == u32::from(b'/') || c < 0x80 && !encoded(c) {
            out.push(char::from(c as u8));
        } else if c < 0x80 {
            escape(&mut out, c);
        } else if c > 0x7FF {
            escape(&mut out, 0xE0 | (c >> 12 & 0x0F));
            escape(&mut out, 0x80 | (c >> 6 & 0x3F));
            escape(&mut out, 0x80 | (c & 0x3F));
        } else {
            escape(&mut out, 0xC0 | (c >> 6 & 0x1F));
            escape(&mut out, 0x80 | (c & 0x3F));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode_path;

    #[test]
    fn paths_are_encoded_as_the_class_loader_encodes_them() {
        assert_eq!(encode_path("/work/classes"), "/work/classes");
        assert_eq!(encode_path("/a b/c#d%e"), "/a%20b/c%23d%25e");
        assert_eq!(encode_path("/x/é/€/😀"), "/x/%c3%a9/%e2%82%ac/%ed%a0%bd%ed%b8%80");
        assert_eq!(encode_path("/(ok)~!*'-_.:@&+$,"), "/(ok)~!*'-_.:@&+$,");
        assert_eq!(encode_path("/a;b=c?d"), "/a%3bb%3dc%3fd");
    }
}
