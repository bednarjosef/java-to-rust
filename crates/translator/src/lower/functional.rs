//! Functional interfaces: the interfaces that lambdas and method references convert to.
//!
//! A program's own functional interfaces, and the JDK's when the program is translated with the
//! JDK's sources, are interfaces like any other. Without those sources, the JDK's functional
//! interfaces (`Runnable`, `Comparator`, `java.util.function.*`, and `Iterable` and `Iterator`)
//! come from the translator's own declarations of their abstract methods, `java/Functional.java`,
//! whose syntax tree `build.rs` prints: a program gets the ones it names, with the interfaces they
//! extend, as interfaces of the program, the way it gets the standard exception classes (see
//! `exceptions`). Their items are named by their simple names at the crate's root.
//!
//! Marker interfaces (`Serializable`, `Cloneable`, `RandomAccess`) declare nothing, so they are
//! dropped where a class implements them; `JavaTree` drops them from intersection casts such as
//! `(Comparator<T> & Serializable)`.

use super::*;
use std::collections::HashSet;
use std::sync::OnceLock;

/// The interfaces that declare no methods.
const MARKERS: &[&str] = &["java.io.Serializable", "java.lang.Cloneable", "java.util.RandomAccess"];

/// Whether `name` is a marker interface, which a class implements without declaring anything.
pub(super) fn is_marker(name: &str) -> bool {
    MARKERS.contains(&name)
}

/// The package of the interface of `java/Functional.java` with this simple name.
fn package_of(simple: &str) -> &'static str {
    match simple {
        "Runnable" | "Iterable" => "java.lang",
        "Comparator" | "Iterator" => "java.util",
        "Callable" => "java.util.concurrent",
        _ => "java.util.function",
    }
}

/// The interface declarations of `java/Functional.java`, renamed from the package `functional` to
/// their real packages, as javac names the real interfaces in a program's tree.
fn template() -> &'static [Value] {
    static DECLS: OnceLock<Vec<Value>> = OnceLock::new();
    DECLS.get_or_init(|| {
        let text = include_str!(concat!(env!("OUT_DIR"), "/functional.json"));
        let mut tree: Value =
            serde_json::from_str(text).expect("build.rs writes the tree of Functional.java");
        rename(&mut tree);
        match tree["units"][0]["tree"]["typeDecls"].take() {
            Value::Array(decls) => decls,
            _ => panic!("the tree of Functional.java has its interfaces"),
        }
    })
}

/// Gives the interfaces of the package `functional` their real packages, in every name and type.
fn rename(v: &mut Value) {
    match v {
        Value::String(s) => {
            while let Some(at) = s.find("functional.") {
                let rest = &s[at + "functional.".len()..];
                let end = rest.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(rest.len());
                let simple = rest[..end].to_string();
                s.replace_range(at..at + "functional".len(), package_of(&simple));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(rename),
        Value::Object(map) => {
            if map.get("kind").and_then(Value::as_str) == Some("INTERFACE")
                && map.get("owner").and_then(Value::as_str) == Some("functional")
            {
                let simple = map.get("name").and_then(Value::as_str).unwrap_or_default();
                let package = package_of(simple);
                map.insert("owner".into(), Value::String(package.into()));
            }
            map.values_mut().for_each(rename);
        }
        _ => {}
    }
}

/// The qualified name of an interface of the template.
fn qualified(decl: &Value) -> String {
    let simple = decl["simpleName"].as_str().unwrap_or_default();
    format!("{}.{simple}", package_of(simple))
}

/// Whether `name` is the qualified name of a functional interface the translator provides.
pub fn is_functional(name: &str) -> bool {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| template().iter().map(qualified).collect()).contains(name)
}

/// The interfaces of the template that a program uses and does not declare itself.
#[derive(Default)]
pub(super) struct Functional {
    /// The declarations to add, with their packages.
    pub classes: Vec<(&'static str, &'static Value)>,
    names: HashSet<String>,
}

impl Functional {
    /// The interfaces of the template that the code in these trees names, with the interfaces
    /// they extend, leaving out those that `declared` (the program's own classes) has.
    pub fn select<'v>(trees: impl IntoIterator<Item = &'v Value>, declared: &HashSet<String>) -> Functional {
        let decls: Vec<(String, &'static Value)> = template().iter().map(|d| (qualified(d), d)).collect();
        let mut used = HashSet::new();
        for tree in trees {
            names(tree, &decls, &mut used);
        }
        // An interface extends others of the template (`UnaryOperator` extends `Function`).
        loop {
            let before = used.len();
            for (name, decl) in &decls {
                if used.contains(name) {
                    for clause in decl["implementsClause"].as_array().into_iter().flatten() {
                        names(clause, &decls, &mut used);
                    }
                }
            }
            if used.len() == before {
                break;
            }
        }
        let classes: Vec<(&'static str, &'static Value)> = decls
            .iter()
            .filter(|(name, _)| used.contains(name) && !declared.contains(name))
            .map(|(name, d)| (package_of(name.rsplit('.').next().unwrap_or_default()), *d))
            .collect();
        Functional { names: classes.iter().map(|(_, d)| qualified(d)).collect(), classes }
    }

    /// Whether `class` is an interface of the template that the program uses.
    pub fn has(&self, class: &str) -> bool {
        self.names.contains(class)
    }
}

/// Adds to `used` every interface of the template that a type in `v` mentions, or the owner of
/// a member it uses or of a lambda's method. The types in the signatures of the methods it calls
/// do not count: `String.join` takes an `Iterable` that the program need not have.
fn names(v: &Value, decls: &[(String, &'static Value)], used: &mut HashSet<String>) {
    match v {
        Value::Array(items) => items.iter().for_each(|i| names(i, decls, used)),
        Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("javaType" | "owner" | "target", Value::String(s)) => mentioned(s, decls, used),
                    _ => names(value, decls, used),
                }
            }
        }
        _ => {}
    }
}

/// Adds to `used` every interface of the template that the type `s` names.
fn mentioned(s: &str, decls: &[(String, &'static Value)], used: &mut HashSet<String>) {
    match s {
        s if s.contains("java.") => {
            for (name, _) in decls {
                let mut rest = s;
                while let Some(at) = rest.find(name.as_str()) {
                    let after = rest[at + name.len()..].chars().next();
                    if after.is_none_or(|c| !c.is_ascii_alphanumeric()) {
                        used.insert(name.clone());
                        break;
                    }
                    rest = &rest[at + name.len()..];
                }
            }
        }
        _ => {}
    }
}

impl<'t> Lowering<'t> {
    /// Declares the interfaces of the template that the program uses, after its own classes.
    pub(super) fn declare_functional(&mut self) -> Result<(), Error> {
        // An interface of the template is an item of the crate's root named by its simple name,
        // which a class of the program in the unnamed package must not have.
        for (_, decl) in &self.functional.classes {
            let simple = Node(decl).str("simpleName");
            if let Some(clash) = self.types.iter().find(|t| t.name == simple) {
                return unsupported(clash.node, &format!("a class named like the interface `{simple}`"));
            }
        }
        for (package, decl) in self.functional.classes.clone() {
            self.declare_type(Node(decl), Some(package))?;
        }
        Ok(())
    }

    /// The declarations of the interfaces of the template that the program uses, which planning
    /// `Object` looks at with the program's own.
    pub(super) fn functional_decls(&self) -> Vec<Node<'t>> {
        self.functional.classes.iter().map(|(_, d)| Node(d)).collect()
    }
}
