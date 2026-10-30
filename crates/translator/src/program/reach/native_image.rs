//! The reflection metadata that libraries ship for GraalVM's native-image: the files
//! `META-INF/native-image/**/reflect-config.json` of a root (and the `reflection` part of
//! `reachability-metadata.json`, the newer form), which say which classes reflection finds by
//! name and which of their constructors, methods and fields it looks up. log4j-core lists its
//! plugins and their builders there, which it finds by names read from a file, and netty its
//! channel handlers, whose methods `ChannelHandlerMask` looks up.
//!
//! The translation takes such an entry as it takes the string literals of reachable code
//! (`reflection.rs`, `by_name.rs`): its class is named, as a literal naming it would, so that
//! the lookups on a `Class` value that is not a class literal and a computed `Class.forName`
//! can find it, and the names of the members it lists are names that a computed name can be.
//! A flag for all members of a kind (`allDeclaredMethods`, `allPublicFields`,
//! `queryAllPublicMethods` and the rest) gives the names of those that the class declares.
//! Constructors need no names. An entry applies only to a class that reachable code reaches
//! otherwise, and only once the class of its condition (`typeReachable`, `typeReached`) is
//! reached too: the metadata lists what a library may look up, and most of it is code that a
//! program does not use. An entry for a class that is not translated (`outside`), or that no
//! unit declares, or for an array class, never applies. Applied entries are noted in
//! `Reach::configured`. The class names in the roots' resource files come as entries too
//! (`resource_names.rs`), which apply as soon as a unit declares their class and ask for its
//! unit until one does, as a literal in reachable code would.

use std::path::{Path, PathBuf};

use super::{Walk, outside};

/// An entry of a library's reflection metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReflectEntry {
    /// The class, by binary name (`a.B$C`).
    pub class: String,
    /// The class that must be reached for the entry to apply, by binary name.
    pub condition: Option<String>,
    /// The names of the methods and fields that it lists.
    pub members: Vec<String>,
    /// Whether it asks for all methods, or all fields, of a kind.
    pub all_methods: bool,
    pub all_fields: bool,
    /// Whether a resource file names the class (`resource_names.rs`): the entry applies without
    /// the class being reached otherwise.
    pub named: bool,
}

/// The entries of the reflection metadata of the roots, root by root, each root's files in the
/// order of their paths, then those of the class names in their resource files.
pub fn reflect_config<'a>(roots: impl IntoIterator<Item = &'a PathBuf>) -> Vec<ReflectEntry> {
    let roots: Vec<&PathBuf> = roots.into_iter().collect();
    let mut out = Vec::new();
    for &root in &roots {
        let mut files = Vec::new();
        metadata_files(&root.join("META-INF").join("native-image"), &mut files);
        files.sort();
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else { continue };
            out.extend(entries(&text));
        }
    }
    out.extend(super::resource_names::resource_names(&roots));
    out
}

/// The metadata files under `dir`, at any depth.
fn metadata_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(items) = std::fs::read_dir(dir) else { return };
    for item in items.flatten() {
        let path = item.path();
        if path.is_dir() {
            metadata_files(&path, out);
        } else if matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some("reflect-config.json" | "reachability-metadata.json")
        ) {
            out.push(path);
        }
    }
}

/// The entries of a metadata file: an array of entries (`reflect-config.json`), or an object
/// whose `reflection` is one (`reachability-metadata.json`). A file that is not JSON has none.
pub fn entries(text: &str) -> Vec<ReflectEntry> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return Vec::new() };
    let list = match &json {
        serde_json::Value::Array(list) => list,
        serde_json::Value::Object(map) => match map.get("reflection") {
            Some(serde_json::Value::Array(list)) => list,
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    list.iter().filter_map(entry).collect()
}

/// An entry of the list, if it names a class (not an array, not a proxy).
fn entry(item: &serde_json::Value) -> Option<ReflectEntry> {
    let class = item.get("name").or_else(|| item.get("type"))?.as_str()?;
    if class.starts_with('[') || class.ends_with("[]") {
        return None;
    }
    let condition =
        item.get("condition").and_then(|c| c.get("typeReachable").or_else(|| c.get("typeReached")));
    let flag = |name: &str| item.get(name).and_then(serde_json::Value::as_bool).unwrap_or(false);
    let mut members = Vec::new();
    for list in ["methods", "queriedMethods", "fields"] {
        for member in item.get(list).and_then(serde_json::Value::as_array).into_iter().flatten() {
            if let Some(name) = member.get("name").and_then(serde_json::Value::as_str)
                && name != "<init>"
                && !members.iter().any(|m| m == name)
            {
                members.push(name.to_string());
            }
        }
    }
    Some(ReflectEntry {
        class: class.to_string(),
        condition: condition.and_then(serde_json::Value::as_str).map(str::to_string),
        members,
        all_methods: [
            "allDeclaredMethods",
            "allPublicMethods",
            "queryAllDeclaredMethods",
            "queryAllPublicMethods",
        ]
        .into_iter()
        .any(flag),
        all_fields: flag("allDeclaredFields") || flag("allPublicFields"),
        named: false,
    })
}

impl Walk<'_> {
    /// Applies the entries of the reflection metadata whose classes and conditions are reached
    /// now (see the module's comment), and whether there was one.
    pub(super) fn apply_reflect_config(&mut self) -> bool {
        let mut applied = false;
        for index in std::mem::take(&mut self.config_pending) {
            let entry = &self.reflect_config[index];
            let reached = |walk: &Self, binary: &str| {
                walk.declared_class(binary).filter(|class| walk.reach.classes.contains(class))
            };
            let condition = entry.condition.as_deref().is_none_or(|c| reached(self, c).is_some());
            let found =
                if entry.named { self.declared_class(&entry.class) } else { reached(self, &entry.class) };
            let Some(class) = found.filter(|_| condition) else {
                if entry.named {
                    // The unit of its top-level class, which javac gives in the next round.
                    let top = entry.class.split('$').next().unwrap_or(&entry.class).to_string();
                    self.missing(&top);
                }
                self.config_pending.push(index);
                continue;
            };
            let entry = entry.clone();
            applied = true;
            self.reach.configured.insert(class.clone());
            let mut names = entry.members;
            if entry.all_methods {
                let methods = self.methods_by_name.get(class.as_str());
                names.extend(
                    methods.into_iter().flat_map(|m| m.keys()).filter(|n| !n.starts_with('<')).cloned(),
                );
            }
            if entry.all_fields {
                names.extend(self.fields_by_class.get(class.as_str()).into_iter().flatten().cloned());
            }
            self.strings.extend(names);
            self.named(&class);
        }
        applied
    }

    /// The class that a unit declares with the binary name `binary`, by the name that the walk
    /// knows it by (`a.B.C` for `a.B$C`, a local or anonymous class keeping its `$`), if it is
    /// translated.
    pub(super) fn declared_class(&self, binary: &str) -> Option<String> {
        [binary.to_string(), binary.replace('$', ".")]
            .into_iter()
            .find(|class| self.classes.contains_key(class) && !outside(class))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_lists_its_class_condition_and_members() {
        let text = r#"[
            {"name": "a.B$Builder", "condition": {"typeReachable": "a.B"},
             "methods": [{"name": "newBuilder", "parameterTypes": []}, {"name": "<init>", "parameterTypes": []}],
             "fields": [{"name": "layout"}]},
            {"name": "a.Handler", "queryAllPublicMethods": true},
            {"name": "[Ljava.lang.String;"},
            {"name": "a.C", "allDeclaredFields": true, "allPublicConstructors": true}
        ]"#;
        let found = entries(text);
        assert_eq!(found.len(), 3);
        assert_eq!(
            found[0],
            ReflectEntry {
                class: "a.B$Builder".into(),
                condition: Some("a.B".into()),
                members: vec!["newBuilder".into(), "layout".into()],
                all_methods: false,
                all_fields: false,
                named: false,
            }
        );
        assert!(found[1].all_methods && !found[1].all_fields);
        assert!(!found[2].all_methods && found[2].all_fields);
    }

    #[test]
    fn the_newer_form_has_its_entries_under_reflection() {
        let text =
            r#"{"reflection": [{"type": "a.B", "condition": {"typeReached": "a.C"}}], "resources": []}"#;
        let found = entries(text);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].class.as_str(), found[0].condition.as_deref()), ("a.B", Some("a.C")));
        assert!(entries("not json").is_empty());
    }
}
