//! Objects created from a class name: `(T) Class.forName(name).getDeclaredConstructor()
//! .newInstance()`, or the older `(T) Class.forName(name).newInstance()`, as the JDK's locale
//! providers create their adapters from the class names in `LocaleProviderAdapter.Type`.
//!
//! A translated program is closed: every class it can create is one of its own. Such a creation
//! creates an object of the program's class with that binary name, which the lowering selects
//! with a `switch` over the name (`lower::by_name`), and a name of no such class fails loudly
//! there. The candidates are the program's top-level classes that are not abstract or generic,
//! extend or implement `T` and have a constructor without parameters. Reachability makes that
//! constructor reachable for each candidate whose name reachable code spells out as a string
//! literal, the way the names of the adapters are written; a name that code computes, by
//! concatenation or from a file, finds nothing, and the creation fails loudly when it runs. As
//! javac gives a library's units only for the classes that reachability names, the literals that
//! could name a class no unit declares are noted as missing (`Walk::note_named`), so that the
//! next round has their units.

use super::{Item, Walk, class_name, has_flag, text};
use serde_json::Value;

/// The expression that gives the class name, when the cast `node` casts an object created from
/// a class name.
pub fn created_by_name(node: &Value) -> Option<&Value> {
    if text(node, "kind") != "TYPE_CAST" {
        return None;
    }
    let creation = unparenthesized(&node["expression"]);
    let class = match call(creation, "newInstance")? {
        ("java.lang.reflect.Constructor", constructor) => {
            match call(constructor, "getDeclaredConstructor")? {
                ("java.lang.Class", class) => class,
                _ => return None,
            }
        }
        ("java.lang.Class", class) => class,
        _ => return None,
    };
    let class = unparenthesized(class);
    let symbol = &class["symbol"];
    let arguments = class["arguments"].as_array()?;
    let is_for_name = text(class, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.lang.Class"
        && text(symbol, "name") == "forName"
        && text(symbol, "type").starts_with("(java.lang.String");
    // `forName(name, initialize, loader)` finds the same class as `forName(name)`.
    (is_for_name && matches!(arguments.len(), 1 | 3)).then(|| &arguments[0])
}

/// The class that the call `node`, `Class.forName(name)` or `Class.forName(name, initialize,
/// loader)`, finds when it spells the name out as a string literal, and whether the call
/// initializes it: the class's name as the program's classes have it (the binary name `a.B$C`
/// of a member class is `a.B.C`). `SharedSecrets` runs a class's static initializer this way,
/// `Class.forName("java.util.ImmutableCollections$Access", true, null)`, and in a closed program
/// such a name is a class of the program, or of none. `None` for a computed name, a computed
/// `initialize`, or the name of a local or anonymous class.
pub fn class_for_name(node: &Value) -> Option<(String, bool)> {
    let symbol = &node["symbol"];
    let is_for_name = text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.lang.Class"
        && text(symbol, "name") == "forName"
        && text(symbol, "type").starts_with("(java.lang.String");
    let arguments = node["arguments"].as_array()?;
    if !is_for_name || !matches!(arguments.len(), 1 | 3) {
        return None;
    }
    let name = unparenthesized(&arguments[0]);
    let name = (text(name, "kind") == "STRING_LITERAL").then(|| name["value"].as_str())??;
    let initialize = match arguments.get(1).map(unparenthesized) {
        None => true,
        Some(flag) if text(flag, "kind") == "BOOLEAN_LITERAL" => flag["value"].as_bool()?,
        Some(_) => return None,
    };
    let named = name.split(['.', '$']).all(|part| is_identifier(part) && !part.contains('$'));
    named.then(|| (name.replace('$', "."), initialize))
}

/// The class that declares the method of a call `node` named `name` without arguments, and the
/// receiver it is called on.
fn call<'v>(node: &'v Value, name: &str) -> Option<(&'v str, &'v Value)> {
    let symbol = &node["symbol"];
    let select = &node["methodSelect"];
    let no_arguments = node["arguments"].as_array().is_some_and(Vec::is_empty);
    (text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "name") == name
        && no_arguments
        && text(select, "kind") == "MEMBER_SELECT")
        .then(|| (text(symbol, "owner"), &select["expression"]))
}

fn unparenthesized(node: &Value) -> &Value {
    match text(node, "kind") {
        "PARENTHESIZED" => unparenthesized(&node["expression"]),
        _ => node,
    }
}

impl Walk<'_> {
    /// The cast `node` creates an object of a class named by a string: the candidates named so
    /// far become reachable, and those named later will.
    pub(super) fn created_by_name(&mut self, node: &Value) {
        let target = super::strip_arguments(text(node, "javaType"));
        if !self.by_name.insert(target.clone()) {
            return;
        }
        let named: Vec<String> = self.named.iter().cloned().collect();
        for name in named {
            self.create_by_name(&target, &name);
        }
    }

    /// A string literal of reachable code, which may name a class that a cast creates. One that
    /// could name a class that no unit declares is kept for `note_named`.
    pub(super) fn named(&mut self, literal: &str) {
        if !self.classes.contains_key(literal) {
            if could_name_class(literal) && !self.literals.contains(literal) {
                self.literals.insert(literal.to_string());
            }
            return;
        }
        if !self.named.insert(literal.to_string()) {
            return;
        }
        let targets: Vec<String> = self.by_name.iter().cloned().collect();
        for target in targets {
            self.create_by_name(&target, literal);
        }
    }

    /// Makes the constructor without parameters of the class `name` reachable, if a creation of
    /// a `target` by name can create an object of it.
    fn create_by_name(&mut self, target: &str, name: &str) {
        let Some(&(decl, ref outer)) = self.classes.get(name) else { return };
        let candidate = outer.is_none()
            && text(decl, "kind") == "CLASS"
            && !has_flag(decl, "ABSTRACT")
            && decl["typeParameters"].as_array().is_none_or(Vec::is_empty)
            && class_name(decl) == name
            && self.is_subtype(name, target);
        let constructor = (name.to_string(), "<init>".to_string(), "()void".to_string());
        if candidate && self.methods.contains_key(&constructor) {
            self.work.push(Item::Method(constructor));
        }
    }

    /// When reachable code creates objects by class name, the qualified names in its string
    /// literals that no unit declares are classes it may need (`Walk::missing`): javac gives
    /// their units in the next round, where they are candidates like any other.
    pub(super) fn note_named(&mut self) {
        if self.by_name.is_empty() {
            return;
        }
        for literal in std::mem::take(&mut self.literals) {
            self.missing(&literal);
        }
    }

    /// Whether the class `class` is `target` or extends or implements it, directly or not.
    fn is_subtype(&self, class: &str, target: &str) -> bool {
        let mut work = vec![class.to_string()];
        let mut seen = std::collections::HashSet::new();
        while let Some(class) = work.pop() {
            if class == target {
                return true;
            }
            if seen.insert(class.clone()) {
                work.extend(self.supertypes.get(&class).cloned().unwrap_or_default());
            }
        }
        false
    }
}

/// Whether a string could be the binary name of a top-level class in a package: identifiers
/// joined by dots, at least two (`sun.util.cldr.CLDRLocaleProviderAdapter`).
fn could_name_class(text: &str) -> bool {
    text.contains('.') && text.split('.').all(is_identifier)
}

/// Whether a string is a Java identifier, as far as names of classes need.
fn is_identifier(part: &str) -> bool {
    part.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && part.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}
