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
//! a `Class` value loads any service whose class literal reachable code has.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{Item, MethodKey, Walk, by_name::class_bound, by_name::class_for_name, literal_class, text};
use crate::value::Value;

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

/// The service that the call `node` loads, if it is `ServiceLoader.load` or `loadInstalled`:
/// `Some` of the class of a class literal, `Some(None)` for a `Class` value.
fn service_load(node: &Value) -> Option<Option<String>> {
    let symbol = &node["symbol"];
    let loads = text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.util.ServiceLoader"
        && matches!(text(symbol, "name"), "load" | "loadInstalled");
    if !loads {
        return None;
    }
    let service =
        node["arguments"].as_array()?.iter().find(|a| text(a, "javaType").starts_with("java.lang.Class"))?;
    Some(literal_class(service))
}

impl<'t> Walk<'t> {
    /// A node of reachable code: a string literal, a class literal, a lookup or a load of a
    /// service, which the lookups' fixed point (`reflect`) takes up.
    pub(super) fn reflection_scan(&mut self, node: &Value) {
        if let Some(lookup) = lookup(node) {
            self.lookups.insert(lookup);
        }
        if let Some(service) = service_load(node) {
            self.service_loads.insert(service);
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

    /// Makes reachable what the lookups of reachable code find among the classes found so far
    /// (see the module's comment), and whether that is anything new: the walk then goes on, as
    /// what it reaches may name more classes and more strings.
    pub(super) fn reflect(&mut self) -> bool {
        let loaded = self.load_services();
        if self.lookups.is_empty() {
            return loaded;
        }
        let (found, work) = self.reflected_members();
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
    /// name; and the members to make reachable. The members are indexed by class and name once,
    /// as a computed name can be any of thousands of strings in a library.
    #[allow(clippy::type_complexity)]
    fn reflected_members(&self) -> (Vec<(String, String, (String, String), bool)>, Vec<Item>) {
        let mut methods: HashMap<&str, BTreeMap<&str, Vec<&MethodKey>>> = HashMap::new();
        for key in self.methods.keys() {
            methods.entry(key.0.as_str()).or_default().entry(key.1.as_str()).or_default().push(key);
        }
        let mut fields: HashMap<&str, BTreeSet<&str>> = HashMap::new();
        for (class, name) in self.fields.keys() {
            fields.entry(class.as_str()).or_default().insert(name.as_str());
        }
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
                            }
                            all.into_iter().filter(|c| self.is_subtype(c, &lookup.bound)).cloned().collect()
                        })
                        .clone()
                }
            };
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
                                Kind::Field => all.extend(fields.get(owner.as_str()).into_iter().flatten()),
                                _ => all.extend(
                                    methods
                                        .get(owner.as_str())
                                        .into_iter()
                                        .flat_map(|m| m.keys())
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

    /// Makes reachable the providers of the services that reachable code loads, each with what
    /// `ServiceLoader` makes it with: its public static `provider()`, or its constructor without
    /// parameters; and whether there was a new one.
    fn load_services(&mut self) -> bool {
        if self.service_loads.is_empty() {
            return false;
        }
        let any = self.service_loads.contains(&None);
        let mut new = false;
        for (service, provider, modular) in self.providers.clone() {
            let loaded = self.service_loads.contains(&Some(service.clone()))
                || any && self.class_literals.contains(&service);
            if !loaded {
                continue;
            }
            let providers = self.reach.services.entry(service.clone()).or_default();
            if providers.iter().any(|(p, _)| *p == provider) {
                continue;
            }
            providers.push((provider.clone(), modular));
            new = true;
            self.work.push(Item::Class(provider.clone()));
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
    fn supertypes_of(&self, class: &str) -> Vec<String> {
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
