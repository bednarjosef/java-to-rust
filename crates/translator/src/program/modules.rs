//! Source and library roots that are Java modules, with a `module-info.java` at the root.
//!
//! The translation treats a program as one closed world, so the modules' boundaries (what each
//! exports and requires) do not change what it does: javac compiles every root as part of one
//! module, `java.base` when the JDK's own sources are a library and the unnamed module otherwise,
//! so that each class is read from its source on the source path and not from the JDK's class
//! files (`JavaTree`'s `oneModule`). What the modules declare still serves one purpose: when the
//! program uses a module whose sources are not given, the error says which one to add.

use super::ProgramError;
use std::path::{Path, PathBuf};

/// A root with a `module-info.java`.
#[derive(Debug)]
pub struct Module {
    /// The module's name, such as `jdk.compiler`.
    pub name: String,
    /// Its `module-info.java`.
    pub file: PathBuf,
    /// The modules it requires, `transitive` and `static` ones too.
    pub requires: Vec<String>,
}

/// The modules among `roots`, in their order; a root without a `module-info.java`, or with one
/// that declares no module, is not one.
pub fn modules<'a>(roots: impl IntoIterator<Item = &'a PathBuf>) -> Vec<Module> {
    roots
        .into_iter()
        .filter_map(|root| {
            let file = root.join("module-info.java");
            let source = std::fs::read_to_string(&file).ok()?;
            let (name, requires) = declaration(&source)?;
            Some(Module { name, file, requires })
        })
        .collect()
}

/// The `module-info.java` that javac may see: `java.base`'s, whose packages javac compiles only
/// inside that module. Every other one is hidden from it.
pub fn kept(modules: &[Module]) -> Option<&Path> {
    modules.iter().find(|m| m.name == "java.base").map(|m| m.file.as_path())
}

/// The name of the module that `source` declares and the modules it requires.
fn declaration(source: &str) -> Option<(String, Vec<String>)> {
    let source = without_comments(source);
    let tokens = tokens(&source);
    let at = tokens.iter().position(|t| *t == "module")?;
    let name = tokens.get(at + 1).filter(|n| n.chars().all(|c| c.is_alphanumeric() || "._$".contains(c)))?;
    let mut requires = Vec::new();
    let mut rest = tokens[at + 2..].iter();
    while let Some(token) = rest.next() {
        if *token != "requires" {
            continue;
        }
        // `requires [transitive] [static] name;`, where `transitive` may also be a module's name.
        let directive: Vec<&str> = rest.by_ref().take_while(|t| **t != ";").copied().collect();
        if let Some(name) = directive.last() {
            requires.push(name.to_string());
        }
    }
    Some((name.to_string(), requires))
}

/// The words and punctuation of `source`, a word being a qualified name.
fn tokens(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    for (at, c) in source.char_indices() {
        let word = c.is_alphanumeric() || "._$".contains(c);
        match (word, start) {
            (true, None) => start = Some(at),
            (false, Some(from)) => {
                out.push(&source[from..at]);
                start = None;
            }
            _ => {}
        }
        if !word && !c.is_whitespace() {
            out.push(&source[at..at + c.len_utf8()]);
        }
    }
    if let Some(from) = start {
        out.push(&source[from..]);
    }
    out
}

/// `source` with its comments replaced by spaces.
fn without_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('/', Some('/')) => {
                chars.by_ref().take_while(|&c| c != '\n').for_each(drop);
                out.push('\n');
            }
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for c in chars.by_ref() {
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

/// javac's error, with a hint at the module to add when the program uses a package that javac
/// cannot find and one of its modules requires a module whose sources are not given (and that
/// the JDK does not have either, so its classes cannot come from there).
pub fn explain(mut error: ProgramError, modules: &[Module]) -> ProgramError {
    if !error.error.message.ends_with("does not exist") {
        return error;
    }
    let given: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
    let mut missing: Vec<(&str, &str)> = Vec::new();
    for module in modules {
        for required in &module.requires {
            if !given.contains(&required.as_str()) && !missing.iter().any(|(m, _)| m == required) {
                missing.push((required, &module.name));
            }
        }
    }
    if missing.is_empty() {
        return error;
    }
    let system = system_modules();
    missing.retain(|(m, _)| !system.iter().any(|s| s == m));
    let hint = match &missing[..] {
        [] => return error,
        [(module, by)] => format!(
            "module {by} requires module {module}, whose sources are not given: pass the root of its sources as --library"
        ),
        _ => format!(
            "modules whose sources are not given are required ({}): pass the root of each one's sources as --library",
            missing.iter().map(|(m, by)| format!("{m} by {by}")).collect::<Vec<_>>().join(", ")
        ),
    };
    error.error.message = format!("{}; {hint}", error.error.message);
    error
}

/// The modules of the JDK that runs javac, by name.
fn system_modules() -> Vec<String> {
    let Ok(output) = std::process::Command::new("java").arg("--list-modules").output() else {
        return Vec::new();
    };
    let listing = String::from_utf8_lossy(&output.stdout);
    listing.lines().filter_map(|l| l.split('@').next()).map(|m| m.trim().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::declaration;

    #[test]
    fn a_declaration_names_its_module_and_what_it_requires() {
        let source = "/* header with module word */\n@Deprecated\nopen module com.example.app {\n    // requires commented.out;\n    requires transitive java.compiler;\n    requires static lib.extra;\n    requires transitive;\n    exports com.example.app;\n    uses com.example.Spi;\n}\n";
        let (name, requires) = declaration(source).expect("a module");
        assert_eq!(name, "com.example.app");
        assert_eq!(requires, ["java.compiler", "lib.extra", "transitive"]);
    }
}
