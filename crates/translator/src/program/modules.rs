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

/// The modules of the JDK that the JVM's boot loader defines (Java 26). The translation's
/// `java.lang.Module` has the same list (`java/patches/java/lang/Module.java`).
const BOOT_MODULES: &[&str] = &[
    "java.base",
    "java.datatransfer",
    "java.desktop",
    "java.instrument",
    "java.logging",
    "java.management",
    "java.management.rmi",
    "java.naming",
    "java.prefs",
    "java.rmi",
    "java.security.sasl",
    "java.xml",
    "jdk.jfr",
    "jdk.management",
    "jdk.management.agent",
    "jdk.management.jfr",
    "jdk.naming.rmi",
    "jdk.net",
    "jdk.nio.mapmode",
    "jdk.sctp",
    "jdk.unsupported",
];

/// The modules of the JDK that the JVM's platform loader defines (Java 26), as in `Module.java`.
const PLATFORM_MODULES: &[&str] = &[
    "java.compiler",
    "java.net.http",
    "java.scripting",
    "java.security.jgss",
    "java.smartcardio",
    "java.sql",
    "java.sql.rowset",
    "java.transaction.xa",
    "java.xml.crypto",
    "jdk.accessibility",
    "jdk.charsets",
    "jdk.crypto.cryptoki",
    "jdk.dynalink",
    "jdk.httpserver",
    "jdk.localedata",
    "jdk.naming.dns",
    "jdk.security.auth",
    "jdk.security.jgss",
    "jdk.xml.dom",
    "jdk.zipfs",
];

/// The name HotSpot gives the class loader of the named module `module`: `bootstrap` and
/// `platform` for the JDK's modules of those loaders, `app` (the system class loader) for any
/// other.
pub fn loader_name(module: &str) -> &'static str {
    if BOOT_MODULES.contains(&module) {
        "bootstrap"
    } else if PLATFORM_MODULES.contains(&module) {
        "platform"
    } else {
        "app"
    }
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
    /// The class files that Java would have of its sources: `a/b/C.class` for `a/b/C.java`.
    pub classes: Vec<String>,
    /// Its roots, as absolute paths: the directory of a named module, and those of the class
    /// path for the unnamed module, in their order. Its resources are read from them at run
    /// time, as Java reads the class path's directories (`ClassLoader.classPath0`).
    pub roots: Vec<PathBuf>,
    /// For the unnamed module, the packages whose root has a manifest (`META-INF/MANIFEST.MF`,
    /// that of the jar the root's classes come from), each with the attributes that its
    /// `Package` gives (`package_attributes`), a `Name: value` line each.
    pub manifests: Vec<(String, String)>,
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
        let (packages, resources, classes) = root_contents(root);
        let absolute = std::path::absolute(root).unwrap_or_else(|_| root.clone());
        match declared {
            Some(mut module) => {
                module.packages = packages;
                module.resources = resources;
                module.classes = classes;
                module.roots = vec![absolute];
                named.push(module);
            }
            None => {
                any_unnamed = true;
                // A package in two roots is the first's, as the class path would have it.
                if let Ok(manifest) = std::fs::read_to_string(root.join("META-INF/MANIFEST.MF")) {
                    let sections = manifest_sections(&manifest);
                    for package in &packages {
                        if !unnamed.packages.contains(package) {
                            let lines = package_attributes(&sections, package);
                            unnamed.manifests.push((package.clone(), lines));
                        }
                    }
                }
                unnamed.packages.extend(packages);
                unnamed.resources.extend(resources);
                unnamed.classes.extend(classes);
                unnamed.roots.push(absolute);
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
        unnamed.manifests.sort();
        named.push(unnamed);
    }
    named
}

/// The files that `ClassLoader.getResource` of the class loader `loader` (`bootstrap`,
/// `platform` or `app`, see `loader_name`) finds, each with the module that holds it: `module m`,
/// or `the class path` for the unnamed module, which only `app` has. As in Java's
/// `BuiltinClassLoader`, a file in a package of one of the loader's named modules is found only
/// in that module, and only if it is a class file or the module opens the package to all, and
/// one in a package of another loader's module is not found in a named module; any other file
/// of a named module is found in the first one that holds it. The class path comes after the
/// named modules.
pub fn loader_resources(modules: &[Contents], loader: &str) -> Vec<(String, String)> {
    let named: Vec<&Contents> = modules.iter().filter(|m| !m.name.is_empty()).collect();
    // The named module, of any loader, with the package of the file `name`.
    let package_module = |name: &str| {
        let package = name.rsplit_once('/').map_or(String::new(), |(dir, _)| dir.replace('/', "."));
        named.iter().position(|m| m.packages.contains(&package)).map(|i| (i, package))
    };
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();
    for (index, m) in named.iter().enumerate().filter(|(_, m)| loader_name(&m.name) == loader) {
        for name in m.classes.iter().chain(&m.resources) {
            let visible = match package_module(name) {
                Some((owner, package)) => {
                    owner == index
                        && (name.ends_with(".class")
                            || m.open
                            || m.opens.iter().any(|(p, to)| *p == package && to.is_empty()))
                }
                None => true,
            };
            if visible && seen.insert(name.clone()) {
                found.push((name.clone(), format!("module {}", m.name)));
            }
        }
    }
    if loader == "app" {
        for m in modules.iter().filter(|m| m.name.is_empty()) {
            for name in m.classes.iter().chain(&m.resources) {
                if seen.insert(name.clone()) {
                    found.push((name.clone(), "the class path".to_string()));
                }
            }
        }
    }
    found
}

/// The packages of the root `root` (each directory with a Java source, the root itself being the
/// unnamed package `""`), its other files, and the class files of its sources, sorted.
fn root_contents(root: &Path) -> (Vec<String>, Vec<String>, Vec<String>) {
    let (mut packages, mut resources, mut classes) = (Vec::new(), Vec::new(), Vec::new());
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
            } else if let Some(stem) = relative.strip_suffix(".java") {
                has_source |= relative != "module-info.java";
                if !stem.ends_with("module-info") && !stem.ends_with("package-info") {
                    classes.push(format!("{stem}.class"));
                }
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
    classes.sort();
    (packages, resources, classes)
}

/// The sections of a jar's manifest: the main attributes first, under the name `""`, then each
/// entry's, under its `Name`; each attribute with its name in lower case, as names are compared
/// without case. A line that starts with a space continues the one before it.
fn manifest_sections(manifest: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut sections = vec![(String::new(), Vec::new())];
    let mut lines: Vec<String> = Vec::new();
    for line in manifest.lines() {
        match (line.strip_prefix(' '), lines.last_mut()) {
            (Some(more), Some(last)) if !last.is_empty() => last.push_str(more),
            _ => lines.push(line.to_string()),
        }
    }
    let mut fresh = false;
    for line in lines {
        if line.is_empty() {
            fresh = true;
            continue;
        }
        let Some((name, value)) = line.split_once(": ") else { continue };
        let name = name.to_ascii_lowercase();
        if fresh && name == "name" {
            sections.push((value.to_string(), Vec::new()));
        } else if let Some((_, attributes)) = sections.last_mut() {
            attributes.push((name, value.to_string()));
        }
        fresh = false;
    }
    sections
}

/// The versioning attributes of package `package` from the manifest's `sections`, as Java's
/// `ClassLoader.definePackage(name, manifest, url)` takes them: each from the section of the
/// package's directory (`a/b/`), else from the main attributes. A `Name: value` line each, in
/// the order of `Package`'s constructor; one the manifest does not give is left out.
fn package_attributes(sections: &[(String, Vec<(String, String)>)], package: &str) -> String {
    const NAMES: [&str; 6] = [
        "Specification-Title",
        "Specification-Version",
        "Specification-Vendor",
        "Implementation-Title",
        "Implementation-Version",
        "Implementation-Vendor",
    ];
    let path = format!("{}/", package.replace('.', "/"));
    let get = |section: &str, name: &str| {
        let (_, attributes) = sections.iter().find(|(n, _)| n == section)?;
        attributes.iter().find(|(n, _)| *n == name.to_ascii_lowercase()).map(|(_, v)| v.as_str())
    };
    let mut lines = Vec::new();
    for name in NAMES {
        if let Some(value) = get(&path, name).or_else(|| get("", name)) {
            lines.push(format!("{name}: {value}"));
        }
    }
    lines.join("\n")
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
    use super::{
        Contents, contents, declaration, full_declaration, loader_resources, manifest_sections,
        package_attributes, service_providers,
    };

    #[test]
    fn a_package_takes_its_section_s_attributes_then_the_main_ones() {
        let manifest = "Manifest-Version: 1.0\r\nSpecification-Title: Apache Log4j Co\r\n re\r\n\
                        specification-version: 2.26.0\r\nImplementation-Vendor: The Apache Software Foundation\r\n\r\n\
                        Name: a/b/\r\nSpecification-Title: Inner\r\nSealed: true\r\n\r\n";
        let sections = manifest_sections(manifest);
        assert_eq!(
            package_attributes(&sections, "a.b"),
            "Specification-Title: Inner\nSpecification-Version: 2.26.0\n\
             Implementation-Vendor: The Apache Software Foundation"
        );
        assert_eq!(
            package_attributes(&sections, "a"),
            "Specification-Title: Apache Log4j Core\nSpecification-Version: 2.26.0\n\
             Implementation-Vendor: The Apache Software Foundation"
        );
    }

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
        assert_eq!(found[0].classes, ["a/b/C.class"]);
    }

    #[test]
    fn a_loader_finds_the_files_of_its_modules_that_are_not_encapsulated() {
        let strings = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let module = |name: &str, packages: &[&str], resources: &[&str], classes: &[&str]| Contents {
            name: name.into(),
            packages: strings(packages),
            resources: strings(resources),
            classes: strings(classes),
            ..Contents::default()
        };
        let mut app = module(
            "app",
            &["a", "b"],
            &["a/x.txt", "b/y.txt", "META-INF/z", "java/lang/q.txt"],
            &["a/A.class"],
        );
        app.opens = vec![("b".into(), vec![]), ("a".into(), vec!["other".into()])];
        let base = module("java.base", &["java.lang"], &["java/lang/d.dat"], &["java/lang/Object.class"]);
        let path = module("", &["c"], &["a/x.txt", "c/w.txt"], &["c/C.class"]);
        let modules = [base, app, path];
        let held = |loader| -> Vec<String> {
            loader_resources(&modules, loader).into_iter().map(|(n, m)| format!("{n} in {m}")).collect()
        };
        assert_eq!(held("bootstrap"), ["java/lang/Object.class in module java.base"]);
        assert_eq!(held("platform"), Vec::<String>::new());
        assert_eq!(
            held("app"),
            [
                "a/A.class in module app",
                "b/y.txt in module app",
                "META-INF/z in module app",
                "c/C.class in the class path",
                "a/x.txt in the class path",
                "c/w.txt in the class path"
            ]
        );
    }

    #[test]
    fn a_declaration_names_its_module_and_what_it_requires() {
        let source = "/* header with module word */\n@Deprecated\nopen module com.example.app {\n    // requires commented.out;\n    requires transitive java.compiler;\n    requires static lib.extra;\n    requires transitive;\n    exports com.example.app;\n    uses com.example.Spi;\n}\n";
        let (name, requires) = declaration(source).expect("a module");
        assert_eq!(name, "com.example.app");
        assert_eq!(requires, ["java.compiler", "lib.extra", "transitive"]);
    }
}
