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
//!
//! An object is also created from a `Class` value: `c.getConstructor().newInstance()`, with
//! `getDeclaredConstructor` or the older `c.newInstance()`, as `ResourceBundle` creates the
//! locale data that `Bundles` finds. The candidates are the same, with `T` the bound of `c`'s
//! type (`Class<? extends T>`, `Object` for `Class<?>`). A class name that code computes finds
//! the locale data of the libraries: `Class.forName(module, name)`, which `Bundles` calls with
//! the names that `ResourceBundle.Control.toBundleName` makes of a base name such as
//! `sun.text.resources.cldr.FormatData` and a locale, finds one of the classes of the
//! `LOCALE_DATA_PACKAGES` for the root locale, `en` and `en_US`, the locales of the translation
//! (`locale_data_classes`), which are then candidates as if a literal named them. A name that
//! `Class.forName(name)` computes is one of the classes named so far, or of those named later,
//! and each of them is initialized, as the call would.
//!
//! A name that code makes of a package, a computed part and the rest of a class name, as `URL`
//! finds the handler of a protocol, `"sun.net.www.protocol." + protocol + ".Handler"`, names
//! every class of the source roots that fits it, with an identifier for the computed part
//! (`name_pattern`, `pattern_classes`): `sun.net.www.protocol.https.Handler` and the other
//! protocols' handlers, as if a literal named each.

use std::path::PathBuf;

use super::{Item, Walk, class_name, has_flag, text};
use crate::value::Value;

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

/// The binary name of the element class (`a.B$C`) of the array that the call `node`,
/// `Class.forName("[La.B$C;")` or `Class.forName("[La.B$C;", initialize, loader)`, names by a
/// descriptor spelled out as a string literal, and the array's dimensions. `None` for any other
/// call or name, a primitive array's too.
pub fn array_for_name(node: &Value) -> Option<(String, usize)> {
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
    let element = name.trim_start_matches('[');
    let dimensions = name.len() - element.len();
    let element = element.strip_prefix('L')?.strip_suffix(';')?;
    let named = dimensions > 0 && element.split(['.', '$']).all(is_identifier);
    named.then(|| (element.to_string(), dimensions))
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

/// The packages of the locale data that a computed `Class.forName(module, name)` finds: the
/// resource bundles of the JDK's two locale providers, the CLDR's and the JRE's.
pub const LOCALE_DATA_PACKAGES: &[&str] =
    &["sun.text.resources", "sun.text.resources.cldr", "sun.util.resources", "sun.util.resources.cldr"];

/// The suffixes of the names of the locale data for the locales of the translation: the root
/// locale, `en` and `en_US`.
pub const LOCALE_DATA_SUFFIXES: &[&str] = &["", "_en", "_en_US"];

/// The classes of the `LOCALE_DATA_PACKAGES` in these source roots whose names have one of the
/// `LOCALE_DATA_SUFFIXES`: `FormatData`, `FormatData_en`, but not `FormatData_en_US_POSIX`.
pub fn locale_data_classes(roots: &[PathBuf]) -> Vec<String> {
    let mut out = Vec::new();
    for root in roots {
        for package in LOCALE_DATA_PACKAGES {
            let dir = package.split('.').fold(root.clone(), |dir, part| dir.join(part));
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let file = entry.file_name().to_string_lossy().to_string();
                let Some(simple) = file.strip_suffix(".java") else { continue };
                let base = simple.find('_').map_or(simple, |i| &simple[..i]);
                if LOCALE_DATA_SUFFIXES.iter().any(|suffix| format!("{base}{suffix}") == simple) {
                    out.push(format!("{package}.{simple}"));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The prefix and suffix of the class name that the string concatenation `node` makes of a
/// constant package (`"sun.net.www.protocol."`), one computed part and a constant rest of a name
/// that starts with a dot (`".Handler"`). `None` for any other node.
pub fn name_pattern(node: &Value) -> Option<(String, String)> {
    fn operands<'v>(node: &'v Value, out: &mut Vec<&'v Value>) {
        let node = unparenthesized(node);
        if text(node, "kind") == "PLUS" && text(node, "javaType") == "java.lang.String" {
            operands(&node["leftOperand"], out);
            operands(&node["rightOperand"], out);
        } else {
            out.push(node);
        }
    }
    fn constant(node: &Value) -> Option<&str> {
        match text(node, "kind") {
            "STRING_LITERAL" => node["value"].as_str(),
            "IDENTIFIER" | "MEMBER_SELECT" => node["symbol"]["constant"].as_str(),
            _ => None,
        }
    }
    if text(node, "kind") != "PLUS" || text(node, "javaType") != "java.lang.String" {
        return None;
    }
    let mut parts = Vec::new();
    operands(node, &mut parts);
    let [prefix, computed, suffix] = parts[..] else { return None };
    let (prefix, suffix) = (constant(prefix)?, constant(suffix)?);
    let package = prefix.strip_suffix('.')?;
    let rest = suffix.strip_prefix('.')?;
    let fits = constant(computed).is_none()
        && package.split('.').all(is_identifier)
        && rest.split('.').all(|part| is_identifier(part) && !part.contains('$'));
    fits.then(|| (prefix.to_string(), suffix.to_string()))
}

/// The top-level classes of these source roots whose names are `prefix`, an identifier and
/// `suffix` (`name_pattern`): for each directory of the package `prefix` names, the class that
/// the rest of the name names in it, if its file is there.
pub fn pattern_classes(roots: &[PathBuf], (prefix, suffix): (&str, &str)) -> Vec<String> {
    let mut out = Vec::new();
    for root in roots {
        let dir = prefix.split('.').filter(|p| !p.is_empty()).fold(root.clone(), |dir, part| dir.join(part));
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let part = entry.file_name().to_string_lossy().to_string();
            let file = suffix[1..].split('.').fold(dir.join(&part), |dir, part| dir.join(part));
            if is_identifier(&part) && file.with_extension("java").is_file() {
                out.push(format!("{prefix}{part}{suffix}"));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The call `node` is `Class.forName(module, name)`: `Some` of the name when the code spells it
/// out, `Some(None)` when it computes it, `None` for any other node.
pub fn module_for_name(node: &Value) -> Option<Option<String>> {
    let symbol = &node["symbol"];
    let is_for_name = text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.lang.Class"
        && text(symbol, "name") == "forName"
        && text(symbol, "type").starts_with("(java.lang.Module,java.lang.String)");
    if !is_for_name {
        return None;
    }
    let name = unparenthesized(node["arguments"].as_array()?.get(1)?);
    Some(
        (text(name, "kind") == "STRING_LITERAL")
            .then(|| name["value"].as_str().map(str::to_string))
            .flatten(),
    )
}

/// Whether the call `node` is `Class.forName(name)` or `Class.forName(name, initialize, loader)`
/// with a name that the code computes.
pub fn computed_for_name(node: &Value) -> bool {
    let symbol = &node["symbol"];
    let is_for_name = text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.lang.Class"
        && text(symbol, "name") == "forName"
        && text(symbol, "type").starts_with("(java.lang.String");
    let name = node["arguments"].as_array().and_then(|a| a.first()).map(unparenthesized);
    is_for_name && name.is_some_and(|name| text(name, "kind") != "STRING_LITERAL")
}

/// The call `node` creates an object from a `Class` value without naming its class:
/// `c.getConstructor()`, `c.getDeclaredConstructor()` or `c.newInstance()`, with no arguments.
/// The class that the object is of extends or implements the bound of `c`'s type, which this
/// gives (`java.lang.Object` for `Class<?>` or a type variable). `None` for any other node.
pub fn created_from_class(node: &Value) -> Option<String> {
    let symbol = &node["symbol"];
    let creates = text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.lang.Class"
        && matches!(text(symbol, "name"), "getConstructor" | "getDeclaredConstructor" | "newInstance")
        && node["arguments"].as_array().is_some_and(Vec::is_empty);
    if !creates {
        return None;
    }
    let select = &node["methodSelect"];
    let receiver =
        if text(select, "kind") == "MEMBER_SELECT" { text(&select["expression"], "javaType") } else { "" };
    Some(class_bound(receiver))
}

/// The type that a type `Class<? extends T>`, `Class<T>` or `Constructor<? extends T>` bounds
/// its values' classes with: `T`, which may be a type variable, or `java.lang.Object` for
/// `Class<?>`, a raw `Class` or a generic `T`.
pub fn class_bound(ty: &str) -> String {
    let inner = ty.split_once('<').and_then(|(_, rest)| rest.strip_suffix('>')).unwrap_or("");
    let inner = inner.strip_prefix("? extends ").unwrap_or(inner);
    let inner = inner.rsplit(" of ? extends ").next().unwrap_or(inner);
    if !inner.is_empty() && !inner.contains(['<', '?', ' ']) {
        inner.to_string()
    } else {
        "java.lang.Object".to_string()
    }
}

impl Walk<'_> {
    /// The reflection that reachable code finds classes and creates objects with
    /// (`module_for_name`, `created_from_class`), as the module's comment says.
    pub(super) fn reflective(&mut self, node: &Value) {
        if module_for_name(node).is_some() {
            let (method, signature) = super::IN_MODULE;
            self.work.push(Item::Method((super::TRANSLATED_MODULES.into(), method.into(), signature.into())));
        }
        match module_for_name(node) {
            Some(Some(name)) => self.work.push(Item::Class(name)),
            Some(None) if !self.finds_locale_data => {
                self.finds_locale_data = true;
                for class in self.locale_data.clone() {
                    self.found(class);
                }
            }
            _ => {}
        }
        if computed_for_name(node) && !self.finds_named {
            self.finds_named = true;
            for class in self.named.clone() {
                self.work.push(Item::Init(class));
            }
        }
        if let Some(target) = created_from_class(node) {
            // A bound that is no class, a type variable `T`, is `Object`'s.
            let class = target.contains('.') || self.classes.contains_key(&target);
            self.created_by_target(if class { target } else { "java.lang.Object".to_string() });
        }
    }

    /// A class name that code makes of a prefix, a computed part and a suffix: the classes of the
    /// source roots that fit it are named, as if literals named them.
    pub(super) fn named_by_pattern(&mut self, pattern: (String, String)) {
        if !self.patterns.insert(pattern.clone()) {
            return;
        }
        for class in pattern_classes(&self.class_roots, (&pattern.0, &pattern.1)) {
            self.named(&class);
        }
    }

    /// A class that a computed name finds: it is reached, and named as a literal would name it.
    /// One that no unit declares yet is asked for, and found when a unit does (`add_named`).
    fn found(&mut self, class: String) {
        self.reach.found_by_name.insert(class.clone());
        if self.classes.contains_key(&class) {
            self.work.push(Item::Class(class.clone()));
        } else {
            self.missing(&class);
        }
        self.named(&class);
    }

    /// The cast `node` creates an object of a class named by a string: the candidates named so
    /// far become reachable, and those named later will.
    pub(super) fn created_by_name(&mut self, node: &Value) {
        self.created_by_target(super::strip_arguments(text(node, "javaType")));
    }

    /// Code creates an object of a `target` from a class name or a `Class` value: the
    /// candidates named so far become reachable, and those named later will.
    fn created_by_target(&mut self, target: String) {
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
        // A provider's registration names its class for the services alone (`services.rs`).
        if self.registered.contains(literal) {
            return;
        }
        if !self.classes.contains_key(literal) {
            if could_name_class(literal) && !self.literals.contains(literal) {
                self.literals.insert(literal.to_string());
            }
            return;
        }
        if !self.named.insert(literal.to_string()) {
            return;
        }
        self.reach.named.insert(literal.to_string());
        if self.finds_named {
            self.work.push(Item::Init(literal.to_string()));
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
            // It is in the reflection table, which `newInstance()` finds it in (`reflection.rs`).
            self.reach.reflected.insert((name.to_string(), "<init>".to_string()));
        }
    }

    /// When reachable code creates objects by class name, the qualified names in its string
    /// literals that no unit declares are classes it may need (`Walk::missing`): javac gives
    /// their units in the next round, where they are candidates like any other.
    pub(super) fn note_named(&mut self) {
        if self.by_name.is_empty() {
            return;
        }
        for literal in self.literals.clone() {
            self.missing(&literal);
        }
    }

    /// After a round's units were declared (`Walk::add`): the literals that name a class they
    /// declare name it now, and every creation by name looks at the classes named again, as the
    /// supertypes that make one a candidate may be among the new units.
    pub(super) fn add_named(&mut self) {
        let declared: Vec<String> =
            self.literals.iter().filter(|literal| self.classes.contains_key(*literal)).cloned().collect();
        for literal in declared {
            self.literals.remove(&literal);
            if self.reach.found_by_name.contains(&literal) {
                self.work.push(Item::Class(literal.clone()));
            }
            self.named(&literal);
        }
        let named: Vec<String> = self.named.iter().cloned().collect();
        for target in self.by_name.clone() {
            for name in &named {
                self.create_by_name(&target, name);
            }
        }
    }

    /// Whether the class `class` is `target` or extends or implements it, directly or not.
    pub(super) fn is_subtype(&self, class: &str, target: &str) -> bool {
        if target == "java.lang.Object" {
            return true;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bound_of_a_class_value_is_its_type_argument() {
        assert_eq!(
            class_bound("java.lang.Class<? extends java.util.ResourceBundle>"),
            "java.util.ResourceBundle"
        );
        assert_eq!(
            class_bound("java.lang.reflect.Constructor<capture#1 of ? extends java.util.ResourceBundle>"),
            "java.util.ResourceBundle"
        );
        assert_eq!(class_bound("java.lang.Class<Shape>"), "Shape");
        assert_eq!(class_bound("java.lang.Class<capture#1 of ?>"), "java.lang.Object");
        assert_eq!(class_bound("java.lang.Class"), "java.lang.Object");
    }

    #[test]
    fn a_pattern_names_the_classes_that_fit_it() {
        let root = std::env::temp_dir().join(format!("name-pattern-{}", std::process::id()));
        for dir in ["https", "jar", "notes"] {
            std::fs::create_dir_all(root.join("sun/net/www/protocol").join(dir)).unwrap();
        }
        for file in ["https/Handler.java", "jar/Handler.java", "notes/Other.java"] {
            std::fs::write(root.join("sun/net/www/protocol").join(file), "").unwrap();
        }
        let classes = pattern_classes(std::slice::from_ref(&root), ("sun.net.www.protocol.", ".Handler"));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(classes, ["sun.net.www.protocol.https.Handler", "sun.net.www.protocol.jar.Handler"]);
    }

    #[test]
    fn a_pattern_is_a_package_a_computed_part_and_a_name() {
        use crate::value::json;
        let prefix = json!({"kind": "IDENTIFIER", "javaType": "java.lang.String",
            "symbol": {"constant": "sun.net.www.protocol."}});
        let computed = json!({"kind": "IDENTIFIER", "javaType": "java.lang.String", "symbol": {}});
        let suffix = json!({"kind": "STRING_LITERAL", "javaType": "java.lang.String", "value": ".Handler"});
        let inner = json!({"kind": "PLUS", "javaType": "java.lang.String",
            "leftOperand": prefix, "rightOperand": computed});
        let node = json!({"kind": "PLUS", "javaType": "java.lang.String",
            "leftOperand": inner.clone(), "rightOperand": suffix});
        assert_eq!(name_pattern(&node), Some(("sun.net.www.protocol.".to_string(), ".Handler".to_string())));
        assert_eq!(name_pattern(&inner), None);
    }

    #[test]
    fn locale_data_is_that_of_the_root_locale_and_english() {
        let root = std::env::temp_dir().join(format!("locale-data-{}", std::process::id()));
        let dir = root.join("sun/text/resources/cldr");
        std::fs::create_dir_all(&dir).unwrap();
        for file in
            ["FormatData.java", "FormatData_en.java", "FormatData_en_US_POSIX.java", "FormatData_fr.java"]
        {
            std::fs::write(dir.join(file), "").unwrap();
        }
        let classes = locale_data_classes(std::slice::from_ref(&root));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(classes, ["sun.text.resources.cldr.FormatData", "sun.text.resources.cldr.FormatData_en"]);
    }
}
