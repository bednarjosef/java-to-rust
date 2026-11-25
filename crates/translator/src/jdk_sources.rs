//! `--jdk`: the JDK's own sources as a program's libraries, found without being named. They come
//! from the `lib/src.zip` of the JDK that `java` on `PATH` runs (one directory per module, as
//! `java.base/java/lang/Object.java`), unpacked once into the user's cache for that JDK's version.
//!
//! Only the modules the program uses become libraries, as `scripts/jdk-modules.py` chooses them:
//! java.base; each module with a package that the program's sources name, in an import or a
//! qualified name in code or a string (`Class.forName`); each module that a `module-info.java` of
//! the program requires; and, in turn, the modules that those require and those that provide a
//! service that they use, as the JVM binds services when it resolves its boot layer. java.base's
//! services are the exception: their providers are in nearly every module of the JDK, and a module
//! that provides one is included only when the program names it or a module requires it. A module
//! that the program's own sources declare (javac's `jdk.compiler`, translated as a program) is not
//! a library.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The source roots of the JDK modules that a program with these source roots (its own and its
/// libraries') and files uses, java.base first, unpacking the JDK's sources first if they are not
/// in the cache yet.
pub fn libraries(sources: &[PathBuf], files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let root = unpacked()?;
    let mut module_of = BTreeMap::new();
    let mut requires: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut providers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut modules = BTreeSet::new();
    for entry in std::fs::read_dir(&root).map_err(|e| format!("{}: {e}", root.display()))?.flatten() {
        let Some(module) = entry.file_name().to_str().map(str::to_string) else { continue };
        if !entry.path().is_dir() {
            continue;
        }
        let mut packages = BTreeSet::new();
        package_dirs(&entry.path(), &entry.path(), &mut packages);
        for package in packages {
            module_of.insert(package, module.clone());
        }
        if let Ok(info) = std::fs::read_to_string(entry.path().join("module-info.java")) {
            let info = declarations(&without_comments(&info));
            requires.insert(module.clone(), info.requires);
            uses.insert(module.clone(), info.uses);
            for service in info.provides {
                providers.entry(service).or_default().push(module.clone());
            }
        }
        modules.insert(module);
    }

    let mut used = BTreeSet::from(["java.base".to_string()]);
    let mut own = BTreeSet::new();
    let mut java = Vec::new();
    for root in sources {
        java_files(root, &mut java);
    }
    java.extend(files.iter().cloned());
    for file in &java {
        let Ok(text) = std::fs::read(file) else { continue };
        let text = without_comments(&String::from_utf8_lossy(&text));
        if file.file_name().is_some_and(|f| f == "module-info.java") {
            let info = declarations(&text);
            own.extend(info.name);
            used.extend(info.requires.into_iter().filter(|m| modules.contains(m)));
        }
        for package in packages(&text) {
            if let Some(module) = module_of.get(&package) {
                used.insert(module.clone());
            }
        }
    }

    let mut pending: Vec<String> = used.iter().cloned().collect();
    while let Some(module) = pending.pop() {
        let bound =
            if module == "java.base" { &[][..] } else { uses.get(&module).map_or(&[][..], Vec::as_slice) };
        let needed = requires
            .get(&module)
            .into_iter()
            .flatten()
            .chain(bound.iter().flat_map(|s| providers.get(s).into_iter().flatten()));
        for needed in needed {
            if modules.contains(needed) && used.insert(needed.clone()) {
                pending.push(needed.clone());
            }
        }
    }
    Ok(std::iter::once("java.base".to_string())
        .chain(used.iter().filter(|m| *m != "java.base").cloned())
        .filter(|m| !own.contains(m))
        .map(|m| root.join(m))
        .collect())
}

/// The directory of the unpacked sources of the JDK that `java` runs: `jdk/<version>` in the
/// translator's cache (`embedded::cache`), unpacked from that JDK's `lib/src.zip` with its `jar`
/// tool the first time, into a directory of this process's own, then renamed into place.
fn unpacked() -> Result<PathBuf, String> {
    let properties = crate::lower::system::jdk_properties()
        .ok_or("--jdk needs `java` (a JDK) on PATH, and could not run it")?;
    let home = PathBuf::from(properties.get("java.home").ok_or("java reported no java.home")?);
    let version = properties.get("java.runtime.version").ok_or("java reported no java.runtime.version")?;
    let zip = home.join("lib/src.zip");
    if !zip.is_file() {
        return Err(format!(
            "--jdk: {} is missing; this JDK has no sources (install its source package, such as \
             openjdk-src, or name the module roots with --library)",
            zip.display()
        ));
    }
    let dir = crate::embedded::cache().join("jdk").join(version.replace(['/', '\\'], "_"));
    if dir.join("java.base").is_dir() {
        return Ok(dir);
    }
    unpack(&zip, &dir, |_| true).map_err(|e| format!("--jdk: {e}"))?;
    Ok(dir)
}

/// Unpacks the zip or jar file `zip` into `dir`, with the `jar` tool of the JDK that `java` runs,
/// then deletes the files that `keep` refuses. It unpacks into a directory of this process's own,
/// renamed into place at the end, so that translators started at the same time never read a
/// half-unpacked one; a `dir` that is already there is left as it is.
pub(crate) fn unpack(zip: &Path, dir: &Path, keep: impl Fn(&Path) -> bool) -> Result<(), String> {
    if dir.is_dir() {
        return Ok(());
    }
    let partial = dir.with_extension(format!("partial-{}", std::process::id()));
    let fail = |e: String| {
        let _ = std::fs::remove_dir_all(&partial);
        format!("cannot unpack {} into {}: {e}", zip.display(), dir.display())
    };
    std::fs::create_dir_all(&partial).map_err(|e| fail(e.to_string()))?;
    let jar = crate::lower::system::jdk_properties()
        .and_then(|p| p.get("java.home"))
        .map(|home| Path::new(home).join("bin/jar"))
        .filter(|jar| jar.is_file())
        .unwrap_or_else(|| PathBuf::from("jar"));
    let status = std::process::Command::new(&jar)
        .args(["--extract", "--file"])
        .arg(zip)
        .current_dir(&partial)
        .status()
        .map_err(|e| fail(format!("running jar: {e}")))?;
    if !status.success() {
        return Err(fail(format!("jar exited with {status}")));
    }
    remove_unkept(&partial, &partial, &keep);
    match std::fs::rename(&partial, dir) {
        Ok(()) => Ok(()),
        // Another translator unpacked the same file first.
        Err(_) if dir.is_dir() => {
            let _ = std::fs::remove_dir_all(&partial);
            Ok(())
        }
        Err(e) => Err(fail(e.to_string())),
    }
}

/// Deletes the files under `dir` whose path relative to `root` `keep` refuses.
fn remove_unkept(root: &Path, dir: &Path, keep: &impl Fn(&Path) -> bool) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            remove_unkept(root, &path, keep);
        } else if !keep(path.strip_prefix(root).unwrap_or(&path)) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The packages under `root` that hold a `.java` file, by their dotted names.
fn package_dirs(root: &Path, dir: &Path, packages: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut has_java = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            package_dirs(root, &path, packages);
        } else if path.extension().is_some_and(|e| e == "java") {
            has_java = true;
        }
    }
    if has_java && dir != root {
        let relative = dir.strip_prefix(root).expect("under its root");
        let names: Vec<_> = relative.components().map(|c| c.as_os_str().to_string_lossy()).collect();
        packages.insert(names.join("."));
    }
}

fn java_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            java_files(&path, files);
        } else if path.extension().is_some_and(|e| e == "java") {
            files.push(path);
        }
    }
}

/// `source` with its comments replaced by spaces and its string and character literals kept.
fn without_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            quote @ (b'"' | b'\'') => {
                out.push(quote);
                i += 1;
                while i < bytes.len() && bytes[i] != quote && bytes[i] != b'\n' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        out.push(bytes[i]);
                        i += 1;
                    }
                    out.push(bytes[i]);
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == quote {
                    out.push(quote);
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                out.push(b' ');
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i += 2;
                out.push(b' ');
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn is_package_part(part: &str) -> bool {
    let mut bytes = part.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// The package names in `text`: a dotted name of two or more lowercase parts followed by a part
/// that starts with a capital letter, `_` or `$` (a class), or by `*` (an import of a whole
/// package). A name is read from its start: in `a.java.util.X` the package is `a.java.util`,
/// which no module of the JDK has.
fn packages(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    let mut i = 0;
    while i < bytes.len() {
        let starts = bytes[i].is_ascii_lowercase()
            && (i == 0 || !(is_name_byte(bytes[i - 1]) || bytes[i - 1] == b'.'));
        if !starts {
            i += 1;
            continue;
        }
        let mut parts: Vec<&str> = Vec::new();
        let star = loop {
            let start = i;
            while i < bytes.len() && is_name_byte(bytes[i]) {
                i += 1;
            }
            parts.push(&text[start..i]);
            if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(|&b| is_name_byte(b)) {
                i += 1;
            } else {
                break bytes.get(i) == Some(&b'.') && bytes.get(i + 1) == Some(&b'*');
            }
        };
        let lower = parts.iter().take_while(|p| is_package_part(p)).count();
        let class_follows = parts
            .get(lower)
            .and_then(|p| p.bytes().next())
            .is_some_and(|b| b.is_ascii_uppercase() || b == b'_' || b == b'$');
        if lower >= 2 && (class_follows || (lower == parts.len() && star)) {
            found.insert(parts[..lower].join("."));
        }
    }
    found
}

/// What a `module-info.java` declares.
#[derive(Default)]
struct Declarations {
    name: Option<String>,
    requires: Vec<String>,
    uses: Vec<String>,
    provides: Vec<String>,
}

/// The declarations of a module's `module-info.java`, without its comments.
fn declarations(info: &str) -> Declarations {
    let words: Vec<&str> = info
        .split(|c: char| c.is_whitespace() || c == ';' || c == '{' || c == '}')
        .filter(|w| !w.is_empty())
        .collect();
    let mut out = Declarations::default();
    let mut i = 0;
    while i < words.len() {
        match words[i] {
            "module" if out.name.is_none() => {
                out.name = words.get(i + 1).map(|w| w.to_string());
            }
            "requires" => {
                let mut j = i + 1;
                while words.get(j).is_some_and(|w| *w == "transitive" || *w == "static") {
                    j += 1;
                }
                out.requires.extend(words.get(j).map(|w| w.to_string()));
            }
            "uses" => out.uses.extend(words.get(i + 1).map(|w| w.to_string())),
            "provides" => out.provides.extend(words.get(i + 1).map(|w| w.to_string())),
            _ => {}
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packages_are_found_where_a_class_or_star_follows() {
        let text = without_comments(
            "import java.util.logging.Logger;\nimport javax.swing.*;\n// java.sql.Date\n\
             Object o = java.net.URI.create(\"x\"); a.java.io.File f; Class.forName(\"java.awt.Point\");",
        );
        let found: Vec<String> = packages(&text).into_iter().collect();
        assert_eq!(found, ["a.java.io", "java.awt", "java.net", "java.util.logging", "javax.swing"]);
    }

    #[test]
    fn module_declarations_are_read() {
        let info = declarations(&without_comments(
            "module jdk.compiler { requires transitive java.compiler; requires static jdk.zipfs;\n\
             uses javax.annotation.processing.Processor; provides java.util.spi.ToolProvider with x.Y; }",
        ));
        assert_eq!(info.name.as_deref(), Some("jdk.compiler"));
        assert_eq!(info.requires, ["java.compiler", "jdk.zipfs"]);
        assert_eq!(info.uses, ["javax.annotation.processing.Processor"]);
        assert_eq!(info.provides, ["java.util.spi.ToolProvider"]);
    }
}
