//! The members that reflection reaches: the methods, fields and constructors that
//! `Class.getMethod`, `getField`, `getConstructor` and their relatives look up by name, which the
//! translation's reflection table holds (`lower/reflection.rs`).
//!
//! A translated program is closed, so a lookup finds a member of one of its classes, and the
//! classes it can look in are those that code can get a `Class` value of without naming them in
//! a type: the classes that string literals of reachable code name (`by_name`), the locale data
//! that a computed name finds, and, for methods and fields, the classes of the class literals of
//! reachable code. A lookup on a class literal, or on `Class.forName` of a name spelled out,
//! looks in that class only, and one on a `Class<? extends T>` in those that are or extend or
//! implement `T`. The names it can find are the one it
//! spells out as a string literal, or for a name that code computes, any string literal of
//! reachable code; `getMethods()` and its relatives find every member. Each member found is
//! reachable, with the members of the same name that the class inherits, and is noted in
//! `Reach::reflected` by the class it is looked up in, which lowering makes the table of.
//! A constructor looked up without parameter types is only the one without parameters.
//!
//! `ServiceLoader.load(S.class)` (and `loadInstalled`) finds the providers of `S` that the
//! sources declare (`modules::service_providers`): they are reachable, with what makes each (a
//! module's provider by its public static `provider()` if it has one, as Java makes it),
//! and noted in `Reach::services`, which lowering makes `ServiceLoader`'s natives of. A load of
//! a method's `Class` parameter loads the services that reachable calls of the method pass it, as
//! class literals or as their own such parameters (`JSSecurityManager.getProviders(c)`, which
//! `JDK13Services.getProviders(c)` calls, which `AudioSystem` calls with `MixerProvider.class`),
//! when only those calls give the parameter its values: the method is static, private or a
//! constructor, which no other method overrides, no method reference names it and reflection
//! finds it not, and its code never assigns the parameter. A load of any other `Class` value
//! loads any service whose class literal reachable code has.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::{
    Item, MethodKey, Walk, by_name::class_bound, by_name::class_for_name, has_flag, literal_class,
    strip_arguments, text,
};
use crate::value::Value;

const OBJECT: &str = "java.lang.Object";

/// What a lookup looks for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Kind {
    Method,
    Field,
    Constructor,
}

/// A lookup of members by name through a `Class` value.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Lookup {
    pub kind: Kind,
    /// The name looked up, spelled out; `None` for one that code computes, or for every member.
    pub name: Option<String>,
    /// Whether it finds every member of its kind (`getMethods()`).
    pub every: bool,
    /// Whether it is a constructor's without parameter types.
    pub no_parameters: bool,
    /// The class it looks in when the `Class` value is a class literal.
    pub literal: Option<String>,
    /// The type that bounds the classes the `Class` value can be (`by_name::class_bound`).
    pub bound: String,
}

/// The lookup that the call `node` makes, if it is one: a lookup of `Class`'s by name, or
/// `newInstance()`, which looks up the constructor without parameters.
pub fn lookup(node: &Value) -> Option<Lookup> {
    let symbol = &node["symbol"];
    if text(node, "kind") != "METHOD_INVOCATION" || text(symbol, "owner") != "java.lang.Class" {
        return None;
    }
    let (kind, named, every) = match text(symbol, "name") {
        "getMethod" | "getDeclaredMethod" => (Kind::Method, true, false),
        "getMethods" | "getDeclaredMethods" => (Kind::Method, false, true),
        "getField" | "getDeclaredField" => (Kind::Field, true, false),
        "getFields" | "getDeclaredFields" => (Kind::Field, false, true),
        "getConstructor" | "getDeclaredConstructor" | "newInstance" => (Kind::Constructor, false, false),
        "getConstructors" | "getDeclaredConstructors" => (Kind::Constructor, false, true),
        _ => return None,
    };
    let arguments = node["arguments"].as_array()?;
    let name = arguments
        .first()
        .filter(|_| named)
        .filter(|a| text(a, "kind") == "STRING_LITERAL")
        .and_then(|a| a["value"].as_str())
        .map(str::to_string);
    let select = &node["methodSelect"];
    let receiver = (text(select, "kind") == "MEMBER_SELECT").then(|| &select["expression"]);
    Some(Lookup {
        kind,
        name,
        every,
        no_parameters: kind == Kind::Constructor && !every && arguments.is_empty(),
        literal: receiver
            .and_then(|r| literal_class(r).or_else(|| class_for_name(r).map(|(class, _)| class))),
        bound: class_bound(receiver.map_or("", |r| text(r, "javaType"))),
    })
}

/// A `Class` value of reachable code that it loads a service of or passes to a method.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(super) enum ClassValue {
    /// A class literal, of this class.
    Literal(String),
    /// The parameter, by method and position, of the method whose code it is.
    Parameter(MethodKey, usize),
    /// Any other, which may be any class.
    Other,
}

/// The `Class` argument of the call `node` that loads a service, if it is `ServiceLoader.load`
/// or `loadInstalled`.
fn service_load(node: &Value) -> Option<&Value> {
    let symbol = &node["symbol"];
    let loads = text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.util.ServiceLoader"
        && matches!(text(symbol, "name"), "load" | "loadInstalled");
    if !loads {
        return None;
    }
    node["arguments"].as_array()?.iter().find(|a| is_class_value(a))
}

/// Whether the expression `node` is a `Class` value.
fn is_class_value(node: &Value) -> bool {
    let ty = text(node, "javaType");
    ty == "java.lang.Class" || ty.starts_with("java.lang.Class<")
}

/// Whether the code `node` assigns the variable `name`.
fn assigns(node: &Value, name: &str) -> bool {
    match node {
        Value::Object(map) => {
            let kind = text(node, "kind");
            let target = if kind == "ASSIGNMENT" || kind.ends_with("_ASSIGNMENT") {
                &node["variable"]
            } else {
                &Value::Null
            };
            (text(target, "kind") == "IDENTIFIER" && text(target, "name") == name)
                || map.values().any(|child| assigns(child, name))
        }
        Value::Array(items) => items.iter().any(|item| assigns(item, name)),
        _ => false,
    }
}

impl<'t> Walk<'t> {
    /// A node of reachable code: a string literal, a class literal, a lookup or a load of a
    /// service, which the lookups' fixed point (`reflect`) takes up.
    pub(super) fn reflection_scan(&mut self, node: &Value) {
        if let Some(lookup) = lookup(node) {
            self.lookups.insert(lookup);
        }
        if let Some(service) = service_load(node) {
            let service = self.class_value(service);
            self.service_loads.insert(service);
        }
        self.proxy_scan(node);
        match text(node, "kind") {
            "METHOD_INVOCATION" | "NEW_CLASS" => {
                let symbol = &node["symbol"];
                let callee = || -> MethodKey {
                    (
                        strip_arguments(text(symbol, "owner")),
                        text(symbol, "name").into(),
                        text(symbol, "type").into(),
                    )
                };
                for (index, argument) in node["arguments"].as_array().into_iter().flatten().enumerate() {
                    if is_class_value(argument) {
                        let value = self.class_value(argument);
                        self.class_values.entry((callee(), index)).or_default().insert(value);
                    }
                }
            }
            "MEMBER_REFERENCE" => {
                let symbol = &node["symbol"];
                let key = (
                    strip_arguments(text(symbol, "owner")),
                    text(symbol, "name").into(),
                    text(symbol, "type").into(),
                );
                self.referenced.insert(key);
            }
            _ => {}
        }
        if text(node, "kind") == "STRING_LITERAL"
            && let Some(literal) = node["value"].as_str()
            && !self.strings.contains(literal)
        {
            self.strings.insert(literal.to_string());
        }
        if let Some(class) = literal_class(node)
            && !self.class_literals.contains(&class)
        {
            self.class_literals.insert(class);
        }
    }

    /// Notes the classes of the class literals in `value`, the values of an annotation
    /// (`Walk::annotation_classes`).
    pub(super) fn note_annotation_classes(&mut self, value: &Value) {
        match value {
            Value::Object(fields) => {
                if let Some(class) = literal_class(value) {
                    self.annotation_classes.insert(class);
                }
                fields.values().for_each(|v| self.note_annotation_classes(v));
            }
            Value::Array(items) => items.iter().for_each(|v| self.note_annotation_classes(v)),
            _ => {}
        }
    }

    /// Makes reachable what the lookups of reachable code find among the classes found so far
    /// (see the module's comment), and whether that is anything new: the walk then goes on, as
    /// what it reaches may name more classes and more strings.
    pub(super) fn reflect(&mut self) -> bool {
        let loaded =
            self.load_services() | self.make_proxies() | self.make_services() | self.apply_reflect_config();
        if self.lookups.is_empty() {
            return loaded;
        }
        let (found, work) = self.reflected_members();
        for item in &work {
            if let Item::Method(key) = item {
                self.reflected_methods.insert(key.clone());
            }
        }
        for (class, name, key, declared) in &found {
            self.reflected_keys.insert(key.clone());
            if *declared {
                self.reach.reflected.insert((class.clone(), name.clone()));
            }
        }
        self.work.extend(work);
        loaded || !found.is_empty()
    }

    /// What the lookups find that they did not find before: each class and member name, its key
    /// in `Walk::reflected_keys`, and whether the class declares or inherits a member of that
    /// name; and the members to make reachable. The members are found by class and name
    /// (`Walk::methods_by_name`, `Walk::fields_by_class`), as a computed name can be any of
    /// thousands of strings in a library.
    #[allow(clippy::type_complexity)]
    fn reflected_members(&self) -> (Vec<(String, String, (String, String), bool)>, Vec<Item>) {
        let (methods, fields) = (&self.methods_by_name, &self.fields_by_class);
        let mut supertypes: HashMap<String, Vec<String>> = HashMap::new();
        let mut candidates: HashMap<(&str, bool), Vec<String>> = HashMap::new();
        let mut found = Vec::new();
        let mut work = Vec::new();
        let mut seen = HashSet::new();
        for lookup in &self.lookups {
            let classes: Vec<String> = match &lookup.literal {
                Some(class) => vec![class.clone()],
                None => {
                    let literals = lookup.kind != Kind::Constructor;
                    candidates
                        .entry((lookup.bound.as_str(), literals))
                        .or_insert_with(|| {
                            let mut all: BTreeSet<&String> = self.named.iter().collect();
                            all.extend(self.reach.found_by_name.iter());
                            if literals {
                                all.extend(self.class_literals.iter());
                            } else {
                                all.extend(self.annotation_classes.iter());
                            }
                            all.into_iter().filter(|c| self.is_subtype(c, &lookup.bound)).cloned().collect()
                        })
                        .clone()
                }
            };
            // `Object`'s `hashCode`, `equals` and `toString`, which the runtime's `Object` has
            // (`lower/reflection.rs`): the methods that a proxy class passes to its handler.
            if lookup.kind == Kind::Method && classes.iter().any(|c| c == OBJECT) {
                for (name, _, _) in super::proxies::OBJECT_METHODS {
                    let wanted = match &lookup.name {
                        Some(n) if !lookup.every => n == name,
                        _ => lookup.every || self.strings.contains(*name),
                    };
                    let key = (OBJECT.to_string(), name.to_string());
                    if wanted && !self.reflected_keys.contains(&key) && seen.insert(key.clone()) {
                        found.push((OBJECT.to_string(), name.to_string(), key, true));
                    }
                }
            }
            for class in classes.into_iter().filter(|c| self.classes.contains_key(c)) {
                let owners = supertypes.entry(class.clone()).or_insert_with(|| self.supertypes_of(&class));
                let names: Vec<String> = match (&lookup.name, lookup.kind) {
                    (_, Kind::Constructor) => vec!["<init>".to_string()],
                    (Some(name), _) if !lookup.every => vec![name.clone()],
                    // Every member, or for a computed name those that a string names.
                    (_, kind) => {
                        let mut all: BTreeSet<&str> = BTreeSet::new();
                        for owner in owners.iter() {
                            match kind {
                                Kind::Field => all.extend(
                                    fields.get(owner.as_str()).into_iter().flatten().map(String::as_str),
                                ),
                                _ => all.extend(
                                    methods
                                        .get(owner.as_str())
                                        .into_iter()
                                        .flat_map(|m| m.keys())
                                        .map(String::as_str)
                                        .filter(|n| !n.starts_with('<')),
                                ),
                            }
                        }
                        all.into_iter()
                            .filter(|n| lookup.every || self.strings.contains(*n))
                            .map(str::to_string)
                            .collect()
                    }
                };
                let parameters = lookup.kind == Kind::Constructor && !lookup.no_parameters;
                for name in names {
                    // A constructor looked up with parameter types after one without them is
                    // noted again, for the constructors that need them.
                    let key =
                        (class.clone(), if parameters { "<init>(..)".to_string() } else { name.clone() });
                    if self.reflected_keys.contains(&key) || !seen.insert(key.clone()) {
                        continue;
                    }
                    // A class's constructors are its own: they are not inherited.
                    let owners: &[String] =
                        if lookup.kind == Kind::Constructor { std::slice::from_ref(&class) } else { owners };
                    let mut declared = false;
                    for owner in owners {
                        match lookup.kind {
                            Kind::Field => {
                                if fields.get(owner.as_str()).is_some_and(|f| f.contains(name.as_str())) {
                                    declared = true;
                                    work.push(Item::Field((owner.clone(), name.clone())));
                                }
                            }
                            _ => {
                                for key in methods
                                    .get(owner.as_str())
                                    .and_then(|m| m.get(name.as_str()))
                                    .into_iter()
                                    .flatten()
                                {
                                    declared = true;
                                    if !(lookup.no_parameters && key.2 != "()void") {
                                        work.push(Item::Method((*key).clone()));
                                    }
                                }
                            }
                        }
                    }
                    found.push((class.clone(), name, key, declared));
                }
            }
        }
        (found, work)
    }

    /// What the `Class` value `node` of the code being scanned is.
    pub(super) fn class_value(&self, node: &Value) -> ClassValue {
        if let Some(class) = literal_class(node) {
            return ClassValue::Literal(class);
        }
        if text(node, "kind") == "IDENTIFIER"
            && text(&node["symbol"], "kind") == "PARAMETER"
            && let Some(method) = self.scanning.clone()
            && let Some(index) = self.parameter_index(&method, text(node, "name"))
        {
            return ClassValue::Parameter(method, index);
        }
        ClassValue::Other
    }

    /// Adds the classes that the `Class` value `value` can be to `out`, and whether it can only
    /// be those (see the module's comment); `seen` holds the parameters already followed.
    pub(super) fn classes_of(
        &self,
        value: &ClassValue,
        seen: &mut HashSet<(MethodKey, usize)>,
        out: &mut BTreeSet<String>,
    ) -> bool {
        let (method, index) = match value {
            ClassValue::Literal(class) => {
                out.insert(class.clone());
                return true;
            }
            ClassValue::Other => return false,
            ClassValue::Parameter(method, index) => (method, *index),
        };
        if !seen.insert((method.clone(), index)) {
            return true;
        }
        let Some(&node) = self.methods.get(method) else { return false };
        let direct = method.1 == "<init>"
            || node["symbol"]["static"].as_bool() == Some(true)
            || has_flag(node, "PRIVATE");
        let name = text(&node["parameters"][index], "name");
        if !direct
            || self.referenced.contains(method)
            || self.reflected_methods.contains(method)
            || assigns(&node["body"], name)
        {
            return false;
        }
        let passed = self.class_values.get(&(method.clone(), index)).cloned().unwrap_or_default();
        passed.iter().all(|value| self.classes_of(value, seen, out))
    }

    /// Makes reachable the providers of the services that reachable code loads, each with what
    /// `ServiceLoader` makes it with: its public static `provider()`, or its constructor without
    /// parameters; and whether there was a new one. A provider whose class no unit declares yet
    /// is missing, and what makes it is reached once a later round declares it.
    fn load_services(&mut self) -> bool {
        if self.service_loads.is_empty() {
            return false;
        }
        let (mut services, mut seen, mut any) = (BTreeSet::new(), HashSet::new(), false);
        for value in &self.service_loads {
            any |= !self.classes_of(value, &mut seen, &mut services);
        }
        let mut new = false;
        for (service, provider, modular) in self.providers.clone() {
            let loaded = services.contains(&service) || any && self.class_literals.contains(&service);
            if !loaded {
                continue;
            }
            let providers = self.reach.services.entry(service.clone()).or_default();
            if !providers.iter().any(|(p, _)| *p == provider) {
                providers.push((provider.clone(), modular));
                new = true;
                self.work.push(Item::Class(provider.clone()));
                self.unmade.insert(provider.clone());
            }
            if !self.unmade.contains(&provider) || !self.classes.contains_key(&provider) {
                continue;
            }
            self.unmade.remove(&provider);
            new = true;
            let made: Vec<_> = self
                .methods
                .iter()
                .filter(|((c, n, s), m)| {
                    *c == provider
                        && (modular && n == "provider" && s.starts_with("()") && super::has_flag(m, "STATIC")
                            || n == "<init>" && s == "()void")
                })
                .map(|(key, _)| key.clone())
                .collect();
            self.work.extend(made.into_iter().map(Item::Method));
        }
        new
    }

    /// The class `class` and its supertypes, directly or not.
    pub(super) fn supertypes_of(&self, class: &str) -> Vec<String> {
        let mut out = vec![class.to_string()];
        let mut at = 0;
        while at < out.len() {
            for s in self.supertypes.get(&out[at]).into_iter().flatten() {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
            at += 1;
        }
        out
    }
}
