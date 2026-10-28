//! The class names that libraries write in their resource files, which their code reads and
//! finds the classes of by name: log4j's plugins, listed in the binary
//! `META-INF/org/apache/logging/log4j/core/config/plugins/Log4j2Plugins.dat` that its
//! `PluginRegistry` reads and whose classes `Loader.loadClass` finds, are its configuration's
//! parsers, layouts and appenders. A computed `Class.forName` finds only the classes that the
//! program names (`by_name.rs`), so such a name is taken as the string literals of reachable code
//! are: its class is named.
//!
//! The names are the runs of a file's bytes that could be a class's binary name (`a.b.C`,
//! `a.b.C$D`) and that name a class whose source a root has (`a/b/C.java`), so that the words of
//! a manifest or of a properties file that are not classes name nothing. The files are those under
//! a root's `META-INF`, where libraries keep what their code reads about themselves, but for its
//! service files, which `ServiceLoader` reads (`modules::service_providers`), its reflection
//! metadata, which lists what a library may look up rather than what it does
//! (`native_image.rs`), and its class files.

use std::path::{Path, PathBuf};

use super::ReflectEntry;

/// The largest file that is read for names: plugin lists and manifests are far smaller.
const LARGEST: u64 = 8 << 20;

/// The class names in the resource files of the roots, root by root, each root's files in the
/// order of their paths, each name once, as entries that name their class (`ReflectEntry::named`).
pub fn resource_names(roots: &[&PathBuf]) -> Vec<ReflectEntry> {
    let mut out: Vec<ReflectEntry> = Vec::new();
    for root in roots {
        let mut files = Vec::new();
        resource_files(&root.join("META-INF"), &mut files);
        files.sort();
        for file in files {
            let Ok(bytes) = std::fs::read(&file) else { continue };
            for name in class_names(&bytes) {
                if !out.iter().any(|e| e.class == name) && has_source(roots, &name) {
                    out.push(ReflectEntry { class: name, named: true, ..ReflectEntry::default() });
                }
            }
        }
    }
    out
}

/// The files under `dir`, at any depth, that may hold class names (see the module's comment).
fn resource_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(items) = std::fs::read_dir(dir) else { return };
    for item in items.flatten() {
        let path = item.path();
        let name = item.file_name();
        if path.is_dir() {
            if !matches!(name.to_str(), Some("services" | "native-image" | "versions")) {
                resource_files(&path, out);
            }
        } else if path.extension().is_none_or(|x| x != "class")
            && item.metadata().is_ok_and(|m| m.len() <= LARGEST)
        {
            out.push(path);
        }
    }
}

/// The runs of `bytes` that could be a class's binary name: identifiers joined by dots, at least
/// two, a nested class's after a `$`.
fn class_names(bytes: &[u8]) -> Vec<String> {
    let part = |b: &u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'.');
    bytes
        .split(|b| !part(b))
        .filter_map(|run| std::str::from_utf8(run).ok())
        .map(|run| run.trim_matches('.'))
        .filter(|run| could_name_class(run))
        .map(str::to_string)
        .collect()
}

/// Whether `text` could be a binary name: two identifiers or more joined by dots, none starting
/// with a digit, the last starting with a capital as classes do.
fn could_name_class(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| p.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_'))
        && parts.last().is_some_and(|p| p.starts_with(|c: char| c.is_ascii_uppercase()))
}

/// Whether a root has the source of the class `binary` names: the file of its top-level class.
fn has_source(roots: &[&PathBuf], binary: &str) -> bool {
    let top = binary.split('$').next().unwrap_or(binary);
    let file = format!("{}.java", top.replace('.', "/"));
    roots.iter().any(|root| root.join(&file).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_runs_that_look_like_classes() {
        let bytes = b"\x00\x0aappender\x00\x2aorg.apache.logging.log4j.core.Appender$Nested\x00\
            Export-Package: org.apache.logging.log4j.core,a.b;version=2.26 x.Y. 1.2.Three";
        assert_eq!(class_names(bytes), vec!["org.apache.logging.log4j.core.Appender$Nested", "x.Y"]);
    }

    #[test]
    fn a_name_counts_only_with_its_source_in_a_root() {
        let root = std::env::temp_dir().join(format!("resource-names-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::create_dir_all(root.join("META-INF/plugins")).unwrap();
        std::fs::create_dir_all(root.join("META-INF/services")).unwrap();
        std::fs::write(root.join("a/b/C.java"), "package a.b; public class C {}").unwrap();
        std::fs::write(root.join("META-INF/plugins/list.dat"), b"\x00\x05a.b.C$D\x00\x05a.b.Gone\x00")
            .unwrap();
        std::fs::write(root.join("META-INF/services/a.b.Spi"), "a.b.C\n").unwrap();
        let found = resource_names(&[&root]);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(found.iter().map(|e| e.class.as_str()).collect::<Vec<_>>(), vec!["a.b.C$D"]);
        assert!(found[0].named);
    }
}
