//! The charsets of the JDK: their names and aliases, and the tables of the single-byte ones
//! (windows-1252, the ISO-8859 ones, KOI8-R, the IBM code pages, ...), which `java/Charsets.java`
//! takes from the JDK's own coders. `build.rs` runs it with the JDK that builds the translator
//! (`charsets.json`); where the translator runs with another JDK, it runs it again with that one,
//! so the tables are always those of the JDK whose `javac` compiles the program.
//!
//! A program that looks a charset up by name carries the tables (`codegen`'s `main` gives them to
//! the runtime, `java::register_charsets`), so that `Charset.forName`, `isSupported`, `getBytes`
//! and `new String` of one behave as on that JDK. The other charsets of the JDK (`missing`) are
//! rejected where a constant names one (`lower/charsets.rs`); a program that computes a name
//! carries their names too, and throws `UnsupportedOperationException` for one, which the census
//! lists.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Data {
    jdk: String,
    charsets: Vec<JdkCharset>,
}

/// One charset of the JDK.
#[derive(Deserialize)]
pub struct JdkCharset {
    pub name: String,
    /// The name that `getEncoding` of a reader gives (`Cp1252`).
    pub historical: String,
    pub aliases: Vec<String>,
    /// For a single-byte charset, the char of each byte (U+FFFD where a byte has none).
    pub decode: Option<Vec<u16>>,
    /// The chars whose byte is not the first byte that decodes to them, and that byte (-1 where
    /// the charset cannot encode the char).
    #[serde(default)]
    pub overrides: Vec<(u16, i16)>,
    /// The byte that replaces what the charset cannot encode.
    #[serde(default)]
    pub replacement: u8,
}

impl JdkCharset {
    /// Whether `name` is this charset's name or one of its aliases, which Java matches whatever
    /// the case.
    pub fn named(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name) || self.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    }

    /// Whether the runtime has this charset itself (UTF-8 and the other five).
    pub fn in_runtime(&self) -> bool {
        java_runtime::Charset::known(java_runtime::Wtf8::new(&self.name)).is_some()
    }
}

/// The JDK's charsets, read once.
pub fn charsets() -> &'static [JdkCharset] {
    static CHARSETS: OnceLock<Vec<JdkCharset>> = OnceLock::new();
    CHARSETS.get_or_init(|| {
        let built: Data = serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/charsets.json")))
            .expect("build.rs writes the charsets of the JDK");
        let running = crate::lower::system::jdk_properties()
            .and_then(|p| Some(format!("{} {}", p.get("java.home")?, p.get("java.runtime.version")?)));
        match running {
            Some(running) if running != built.jdk => asked_now().unwrap_or(built.charsets),
            _ => built.charsets,
        }
    })
}

/// The charsets of the JDK on the path, which `java/Charsets.java` prints; `None` when it cannot
/// be run.
fn asked_now() -> Option<Vec<JdkCharset>> {
    let output =
        std::process::Command::new("java").args(["-cp", env!("OUT_DIR"), "Charsets"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let data: Data = serde_json::from_slice(&output.stdout).ok()?;
    Some(data.charsets)
}

/// The charset of the JDK that `name` names, if any.
pub fn find(name: &str) -> Option<&'static JdkCharset> {
    charsets().iter().find(|c| c.named(name))
}

/// The charset of the JDK that `name` names where the translation has it neither in the runtime
/// nor as a table: one of the multi-byte charsets, such as Shift_JIS, GBK or UTF-32.
pub fn missing(name: &str) -> Option<&'static JdkCharset> {
    find(name).filter(|c| c.decode.is_none() && !c.in_runtime())
}

/// The names and aliases of the charsets that are `missing`.
pub fn missing_names() -> Vec<&'static str> {
    charsets()
        .iter()
        .filter(|c| c.decode.is_none() && !c.in_runtime())
        .flat_map(|c| std::iter::once(&c.name).chain(&c.aliases))
        .map(String::as_str)
        .collect()
}

/// A Rust string literal of the chars `units`, none of which is a surrogate.
pub fn decode_literal(units: &[u16]) -> String {
    let mut text = String::from("\"");
    for &unit in units {
        match char::from_u32(u32::from(unit)) {
            Some(c) if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' => text.push(c),
            _ => text.push_str(&format!("\\u{{{unit:x}}}")),
        }
    }
    text.push('"');
    text
}
