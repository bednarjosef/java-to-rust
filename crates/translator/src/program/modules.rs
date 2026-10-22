//! Source and library roots that are Java modules, with a `module-info.java` at the root.
//!
//! The translation treats a program as one closed world, so the modules' boundaries (what each
//! exports and requires) do not change what compiles: javac compiles every root as part of one
//! module, `java.base` when the JDK's own sources are a library and the unnamed module otherwise,
//! so that each class is read from its source on the source path and not from the JDK's class
//! files (`JavaTree`'s `oneModule`). What the modules declare serves at run time instead: the
//! translation's `java.lang.Module` answers from it as the JVM's boot layer does (`contents`),
//! `ServiceLoader` finds the providers that modules declare (`service_providers`), and when the
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

/// A service provider that the sources declare: the service and the provider, by qualified name
/// (`a.B.C` for the binary name `a.B$C`), and whether a module declares it, so that its public
/// static `provider()` makes it if it has one, rather than its constructor.
pub type ServiceProvider = (String, String, bool);

/// The service providers that the roots declare, in the order that `ServiceLoader` finds them:
/// those of the modules' `provides S with P1, P2;`, then those of the files
/// `META-INF/services/S`, one provider per line, `#` starting a comment, root by root (see
/// `java/patches/java/util/ServiceLoader.java`).
pub fn service_providers<'a>(roots: impl IntoIterator<Item = &'a PathBuf> + Clone) -> Vec<ServiceProvider> {
    let mut out: Vec<ServiceProvider> = Vec::new();
    let mut add = |service: &str, provider: &str, modular: bool| {
        let (service, provider) = (service.replace('$', "."), provider.replace('$', "."));
        if !out.iter().any(|(s, p, _)| *s == service && *p == provider) {
            out.push((service, provider, modular));
        }
    };
    for root in roots.clone() {
        let Ok(source) = std::fs::read_to_string(root.join("module-info.java")) else { continue };
        let source = without_comments(&source);
        let tokens = tokens(&source);
        let mut rest = tokens.iter();
        while let Some(token) = rest.next() {
            if *token != "provides" {
                continue;
            }
            let directive: Vec<&str> = rest.by_ref().take_while(|t| **t != ";").copied().collect();
            if let [service, "with", providers @ ..] = &directive[..] {
                providers.iter().filter(|p| **p != ",").for_each(|p| add(service, p, true));
            }
        }
    }
    for root in roots {
        let Ok(files) = std::fs::read_dir(root.join("META-INF").join("services")) else { continue };
        let mut files: Vec<PathBuf> = files.flatten().map(|f| f.path()).collect();
        files.sort();
        for file in files {
            let (Some(service), Ok(text)) =
                (file.file_name().and_then(|n| n.to_str()), std::fs::read_to_string(&file))
            else {
                continue;
            };
            for line in text.lines() {
                let provider = line.split('#').next().unwrap_or_default().trim();
                if !provider.is_empty() {
                    add(service, provider, false);
                }
            }
        }
    }
    out
}

/// What a module declares and what its root holds, which the translation's `java.lang.Module`
/// answers with (`java/patches/java/lang/Module.java`, `lower/modules.rs`). The unnamed module,
/// whose name is empty, holds the roots without a `module-info.java`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contents {
    pub name: String,
    /// Whether it is an `open module`.
    pub open: bool,
    /// The modules it requires, each with whether `transitive`.
    pub requires: Vec<(String, bool)>,
    /// The packages it exports and opens, each with the modules it does so to (none: to all).
    pub exports: Vec<(String, Vec<String>)>,
    pub opens: Vec<(String, Vec<String>)>,
    /// The services it uses, by qualified name.
    pub uses: Vec<String>,
    /// Its packages: the directories of its root with Java sources, as packages.
    pub packages: Vec<String>,
    /// The files of its root that are not Java sources, by their path from the root with `/`.
    pub resources: Vec<String>,
}

/// The contents of the modules among `roots`, in their order, then of the unnamed module if a
/// root has no `module-info.java`. The packages of the translator's own classes (`overlay`) are
/// `java.base`'s, as the classes they replace or add to are.
pub fn contents<'a>(roots: impl IntoIterator<Item = &'a PathBuf>, overlay: Option<&Path>) -> Vec<Contents> {
    let mut named: Vec<Contents> = Vec::new();
    let mut unnamed = Contents::default();
    let mut any_unnamed = false;
    for root in roots {
        let source = std::fs::read_to_string(root.join("module-info.java")).ok();
        let declared = source.as_deref().and_then(full_declaration);
        let (packages, resources) = root_contents(root);
        match declared {
            Some(mut module) => {
                module.packages = packages;
                module.resources = resources;
                named.push(module);
            }
            None => {
                any_unnamed = true;
                unnamed.packages.extend(packages);
                unnamed.resources.extend(resources);
            }
        }
    }
    if let (Some(overlay), Some(base)) = (overlay, named.iter_mut().find(|m| m.name == "java.base")) {
        base.packages.extend(root_contents(overlay).0);
        base.packages.sort();
        base.packages.dedup();
    }
    if any_unnamed {
        unnamed.packages.sort();
        unnamed.packages.dedup();
        named.push(unnamed);
    }
    named
}

/// The packages of the root `root` (each directory with a Java source, the root itself being the
/// unnamed package `""`) and its other files, sorted.
fn root_contents(root: &Path) -> (Vec<String>, Vec<String>) {
    let (mut packages, mut resources) = (Vec::new(), Vec::new());
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut has_source = false;
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let relative = relative.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
            if path.is_dir() {
                dirs.push(path);
            } else if relative.ends_with(".java") {
                has_source |= relative != "module-info.java";
            } else {
                resources.push(relative);
            }
        }
        if has_source {
            let package = dir.strip_prefix(root).unwrap_or(&dir).to_string_lossy();
            packages.push(package.replace(std::path::MAIN_SEPARATOR, "."));
        }
    }
    packages.sort();
    resources.sort();
    (packages, resources)
}

/// What `source` declares: the module's name, whether it is open, and its `requires`,
/// `exports`, `opens` and `uses`.
fn full_declaration(source: &str) -> Option<Contents> {
    let source = without_comments(source);
    let tokens = tokens(&source);
    let at = tokens.iter().position(|t| *t == "module")?;
    let name = tokens.get(at + 1).filter(|n| n.chars().all(|c| c.is_alphanumeric() || "._$".contains(c)))?;
    let mut module =
        Contents { name: name.to_string(), open: tokens[..at].contains(&"open"), ..Contents::default() };
    let body = tokens[at + 2..].iter().position(|t| *t == "{").map_or(at + 2, |b| at + 3 + b);
    let mut rest = tokens[body.min(tokens.len())..].iter();
    while let Some(&keyword) = rest.next() {
        let directive: Vec<&str> =
            rest.by_ref().take_while(|t| **t != ";").copied().filter(|t| *t != ",").collect();
        match (keyword, &directive[..]) {
            // `requires [transitive] [static] name;`, where `transitive` may also be a module's name.
            ("requires", [.., last]) => {
                let transitive = directive[..directive.len() - 1].contains(&"transitive");
                module.requires.push((last.to_string(), transitive));
            }
            ("exports" | "opens", [package, rest @ ..]) => {
                let to: Vec<String> = match rest {
                    ["to", targets @ ..] => targets.iter().map(|t| t.to_string()).collect(),
                    _ => Vec::new(),
                };
                let list = if keyword == "exports" { &mut module.exports } else { &mut module.opens };
                list.push((package.to_string(), to));
            }
            ("uses", [service]) => module.uses.push(service.replace('$', ".")),
            _ => {}
        }
    }
    Some(module)
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
    use super::{contents, declaration, full_declaration, service_providers};

    #[test]
    fn providers_come_from_provides_then_service_files() {
        let root = std::env::temp_dir().join(format!("service-providers-{}", std::process::id()));
        std::fs::create_dir_all(root.join("META-INF/services")).unwrap();
        std::fs::write(
            root.join("module-info.java"),
            "module m {\n    // provides commented.Out with x.Y;\n    provides a.Spi with a.One, a.Outer.Two;\n}\n",
        )
        .unwrap();
        std::fs::write(root.join("META-INF/services/a.Spi"), "# providers\na.Three # the third\n\na.One\n")
            .unwrap();
        std::fs::write(root.join("META-INF/services/b.Other$Spi"), "b.Impl$Nested\n").unwrap();
        let providers = service_providers(std::slice::from_ref(&root));
        std::fs::remove_dir_all(&root).unwrap();
        let found: Vec<(&str, &str, bool)> =
            providers.iter().map(|(s, p, m)| (s.as_str(), p.as_str(), *m)).collect();
        assert_eq!(
            found,
            [
                ("a.Spi", "a.One", true),
                ("a.Spi", "a.Outer.Two", true),
                ("a.Spi", "a.Three", false),
                ("b.Other.Spi", "b.Impl.Nested", false)
            ]
        );
    }

    #[test]
    fn a_full_declaration_has_what_the_module_exports_opens_and_uses() {
        let source = "open module m.n { requires transitive a.b; requires static c; exports p.q; exports p.r to x, y; opens p.s to z; uses p.q.Spi; uses p.q.Outer$Inner; provides p.q.Spi with p.r.Impl; }";
        let module = full_declaration(source).expect("a module");
        assert_eq!(module.name, "m.n");
        assert!(module.open);
        assert_eq!(module.requires, [("a.b".to_string(), true), ("c".to_string(), false)]);
        let strings = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            module.exports,
            [("p.q".to_string(), Vec::new()), ("p.r".to_string(), strings(&["x", "y"]))]
        );
        assert_eq!(module.opens, [("p.s".to_string(), strings(&["z"]))]);
        assert_eq!(module.uses, strings(&["p.q.Spi", "p.q.Outer.Inner"]));
        assert!(!full_declaration("module plain { }").expect("a module").open);
    }

    #[test]
    fn a_root_holds_packages_and_resources() {
        let root = std::env::temp_dir().join(format!("module-contents-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::create_dir_all(root.join("a/empty")).unwrap();
        std::fs::write(root.join("module-info.java"), "module m { exports a.b; }").unwrap();
        std::fs::write(root.join("a/b/C.java"), "package a.b; class C {}").unwrap();
        std::fs::write(root.join("a/b/data.txt"), "x").unwrap();
        let found = contents(std::slice::from_ref(&root), None);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].packages, ["a.b"]);
        assert_eq!(found[0].resources, ["a/b/data.txt"]);
    }

    #[test]
    fn a_declaration_names_its_module_and_what_it_requires() {
        let source = "/* header with module word */\n@Deprecated\nopen module com.example.app {\n    // requires commented.out;\n    requires transitive java.compiler;\n    requires static lib.extra;\n    requires transitive;\n    exports com.example.app;\n    uses com.example.Spi;\n}\n";
        let (name, requires) = declaration(source).expect("a module");
        assert_eq!(name, "com.example.app");
        assert_eq!(requires, ["java.compiler", "lib.extra", "transitive"]);
    }
}
