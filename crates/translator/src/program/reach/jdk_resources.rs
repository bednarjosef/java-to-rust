//! The data files of the JDK that reachable code reads as resources of a class: files made when
//! the JDK is built, which are not in its sources, such as the IDN profile `sun/net/idn/uidna.spp`
//! that `IDN` reads (`StringPrep.class.getResourceAsStream("/sun/net/idn/uidna.spp")`), which
//! every HTTPS connection's host name goes through, and ICU's property and normalization data,
//! which `ICUBinary.getRequiredData(itemPath)` reads for the names its callers pass it. A call of
//! `Class.getResourceAsStream` or `getResource` on a class literal, or on a `Class<C>` of a class
//! `C` (no wildcard: the class itself), reads a resource whose name is a constant, or a parameter
//! of the method whose code it is, which has the constants that reachable calls pass it, through
//! their own parameters too; the name is resolved as Java resolves it (from the class's package
//! unless it starts with `/`). After reachability the translator reads the files of those names
//! that the JDK it runs with holds (`java/JdkResources.java`, over the `jrt:` file system of its
//! image), kept in `Reach::jdk_resources`, and the program carries them: `Module.jdkResource0`
//! gives them by name (`lower/modules.rs`), which the resources of the JDK's classes are read
//! from (`TranslatedModules.classResourceAsStream`).

use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{MethodKey, Reach, Walk, literal_class, strip_arguments, text};
use crate::value::Value;

/// A string that reachable code passes or reads a resource by: a constant, or a method's
/// parameter, by position.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Name {
    Constant(String),
    Parameter(MethodKey, usize),
    /// A concatenation of names (`"/data/" + name + ".nrm"`).
    Concat(Vec<Name>),
}

/// The constant string that `node` is: a literal, a constant variable, or a concatenation of
/// constants.
fn constant(node: &Value) -> Option<String> {
    match text(node, "kind") {
        "STRING_LITERAL" => node["value"].as_str().map(str::to_string),
        "IDENTIFIER" | "MEMBER_SELECT" => node["symbol"]["constant"].as_str().map(str::to_string),
        "PARENTHESIZED" => constant(&node["expression"]),
        "PLUS" if text(node, "javaType") == "java.lang.String" => {
            Some(constant(&node["leftOperand"])? + &constant(&node["rightOperand"])?)
        }
        _ => None,
    }
}

/// The initializer of the local variable `name` that the code `node` declares, if it has one.
fn declaration<'v>(node: &'v Value, name: &str) -> Option<&'v Value> {
    match node {
        Value::Object(map) => {
            if text(node, "kind") == "VARIABLE"
                && text(node, "name") == name
                && !node["initializer"].is_null()
            {
                return Some(&node["initializer"]);
            }
            map.values().find_map(|child| declaration(child, name))
        }
        Value::Array(items) => items.iter().find_map(|item| declaration(item, name)),
        _ => None,
    }
}

/// The class `C` of a value of type `Class<C>`: a class itself, not a wildcard or a variable's.
fn class_of_type(ty: &str) -> Option<String> {
    let inner = ty.strip_prefix("java.lang.Class<")?.strip_suffix('>')?;
    (inner.contains('.') && !inner.contains(['<', '?', ' ', ','])).then(|| inner.to_string())
}

impl Walk<'_> {
    /// The name that the string `node` of the code being scanned is, if a constant or a parameter.
    fn name(&self, node: &Value) -> Option<Name> {
        if let Some(constant) = constant(node) {
            return Some(Name::Constant(constant));
        }
        match (text(node, "kind"), text(&node["symbol"], "kind")) {
            ("PARENTHESIZED", _) => self.name(&node["expression"]),
            ("PLUS", _) if text(node, "javaType") == "java.lang.String" => {
                let (left, right) = (self.name(&node["leftOperand"])?, self.name(&node["rightOperand"])?);
                Some(Name::Concat(vec![left, right]))
            }
            ("IDENTIFIER", "PARAMETER") => {
                let method = self.scanning.clone()?;
                let index = self.parameter_index(&method, text(node, "name"))?;
                Some(Name::Parameter(method, index))
            }
            // A local that its declaration sets and no code assigns.
            ("IDENTIFIER", "LOCAL_VARIABLE") => {
                let method = self.methods.get(self.scanning.as_ref()?)?;
                let name = text(node, "name");
                let init = declaration(&method["body"], name)?;
                (!super::reflection::assigns(&method["body"], name)).then(|| self.name(init))?
            }
            _ => None,
        }
    }

    /// A call of reachable code, which may read a resource of a class or pass a string (see the
    /// module's comment).
    pub(super) fn jdk_resource_scan(&mut self, node: &Value) {
        if !matches!(text(node, "kind"), "METHOD_INVOCATION" | "NEW_CLASS") {
            return;
        }
        let symbol = &node["symbol"];
        let arguments = node["arguments"].as_array().map_or(&[][..], Vec::as_slice);
        let callee: MethodKey = (
            strip_arguments(text(symbol, "owner")),
            text(symbol, "name").into(),
            text(symbol, "type").into(),
        );
        for (index, argument) in arguments.iter().enumerate() {
            if text(argument, "javaType") == "java.lang.String"
                && let Some(name) = self.name(argument)
            {
                let passed = self.reach.string_arguments.entry((callee.clone(), index)).or_default();
                if !passed.contains(&name) {
                    passed.insert(name);
                }
            }
        }
        let reads = callee.0 == "java.lang.Class"
            && matches!(callee.1.as_str(), "getResourceAsStream" | "getResource");
        if !reads {
            return;
        }
        let receiver = &node["methodSelect"]["expression"];
        let Some(class) = literal_class(receiver).or_else(|| class_of_type(text(receiver, "javaType")))
        else {
            return;
        };
        let Some(name) = arguments.first().and_then(|a| self.name(a)) else { return };
        let package = self.package_of(&class);
        if !self.reach.resource_reads.contains(&(package.clone(), name.clone())) {
            self.reach.resource_reads.insert((package, name));
        }
    }
}

impl Reach {
    /// The names of the resources that reachable code reads from classes (see the module's
    /// comment), resolved as Java resolves them; class files aside.
    pub fn jdk_resource_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for (package, name) in &self.resource_reads {
            let mut constants = BTreeSet::new();
            self.constants_of(name, &mut HashSet::new(), &mut constants);
            for name in constants {
                let resolved = match name.strip_prefix('/') {
                    Some(absolute) => absolute.to_string(),
                    None if package.is_empty() => name,
                    None => format!("{}/{name}", package.replace('.', "/")),
                };
                if !resolved.ends_with(".class") {
                    names.insert(resolved);
                }
            }
        }
        names
    }

    /// The constants that `name` can be, into `out`; `seen` holds the parameters followed.
    fn constants_of(&self, name: &Name, seen: &mut HashSet<Name>, out: &mut BTreeSet<String>) {
        match name {
            Name::Constant(constant) => {
                out.insert(constant.clone());
            }
            Name::Parameter(method, index) => {
                if !seen.insert(name.clone()) {
                    return;
                }
                for passed in self.string_arguments.get(&(method.clone(), *index)).into_iter().flatten() {
                    self.constants_of(passed, seen, out);
                }
                seen.remove(name);
            }
            Name::Concat(parts) => {
                let mut texts = BTreeSet::from([String::new()]);
                for part in parts {
                    let mut constants = BTreeSet::new();
                    self.constants_of(part, seen, &mut constants);
                    texts =
                        texts.iter().flat_map(|t| constants.iter().map(move |c| format!("{t}{c}"))).collect();
                    // A name of too many parts is no file's.
                    if texts.len() > 64 {
                        return;
                    }
                }
                out.extend(texts);
            }
        }
    }
}

/// The files of `names` that the JDK on the path holds, by name; none where it cannot be asked.
pub fn read(names: &BTreeSet<String>) -> BTreeMap<String, Vec<u8>> {
    if names.is_empty() {
        return BTreeMap::new();
    }
    let output = std::process::Command::new("java")
        .args(["-cp", env!("OUT_DIR"), "JdkResources"])
        .args(names)
        .output();
    let Some(output) = output.ok().filter(|o| o.status.success()) else { return BTreeMap::new() };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| {
            let (name, hex) = line.split_once('\t')?;
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
                .collect::<Option<Vec<u8>>>()?;
            Some((name.to_string(), bytes))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::json;

    #[test]
    fn a_constant_is_a_literal_a_constant_variable_or_their_concatenation() {
        let literal = json!({"kind": "STRING_LITERAL", "value": "/a/"});
        let variable = json!({"kind": "IDENTIFIER", "symbol": {"constant": "b.dat"}});
        let both = json!({"kind": "PLUS", "javaType": "java.lang.String", "leftOperand": literal, "rightOperand": variable});
        assert_eq!(constant(&both).as_deref(), Some("/a/b.dat"));
        let computed = json!({"kind": "IDENTIFIER", "symbol": {}});
        assert_eq!(constant(&computed), None);
    }

    #[test]
    fn a_class_type_names_its_class_and_a_wildcard_none() {
        assert_eq!(class_of_type("java.lang.Class<a.B>").as_deref(), Some("a.B"));
        assert_eq!(class_of_type("java.lang.Class<? extends a.B>"), None);
        assert_eq!(class_of_type("java.lang.Class<T>"), None);
    }

    #[test]
    fn a_missing_file_is_left_out() {
        // `-` for a name that the image does not hold; the answer depends on the JDK on the path.
        let names = BTreeSet::from(["no/such/resource.dat".to_string()]);
        assert!(read(&names).is_empty());
    }
}
