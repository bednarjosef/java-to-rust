//! The members that reflection reaches: the methods, fields and constructors that
//! `Class.getMethod`, `getField`, `getConstructor` and their relatives look up by name, which the
//! translation's reflection table holds (`lower/reflection.rs`).
//!
//! A translated program is closed, so a lookup finds a member of one of its classes, and the
//! classes it can look in are those that code can get a `Class` value of without naming them in
//! a type: the classes that string literals of reachable code name (`by_name`), the locale data
//! that a computed name finds, and, for methods and fields, the classes of the class literals of
//! reachable code, as for constructors on a `Class<? extends T>` of a `T` other than `Object`;
//! and the classes that the flow of `Class` values of an earlier translation found that the
//! lookups of the kind get (`Walk::flow_classes`, `analysis::reflection_table::missing`): an
//! object's `getClass()`, or its `getSuperclass()`, with no class literal anywhere. A lookup on a
//! class literal, or on `Class.forName` of a name spelled out, looks in that class only, and one
//! on a `Class<? extends T>` in those that are or extend or implement `T`. The names it can find
//! are the one it spells out as a string literal, or for a name that code computes, any string
//! literal of reachable code; `getMethods()` and its relatives find every member. Each member
//! found is reachable, with the members of the same name that the class inherits, and is noted
//! in `Reach::reflected` by the class it is looked up in, which lowering makes the table of. A
//! constructor looked up without parameter types is only the one without parameters.
//!
//! A read of records' components (`getRecordComponents`, called, or found by its name on
//! `Class.class` as gson finds it) looks in the classes that a lookup of fields does, and in the
//! classes that the component types of the records found name, with their type arguments: gson
//! reads a record held in a `Map<String, Endpoint>` component through the field's generic type.
//! Each record found has its fields, its accessors and its constructors made reachable and noted
//! in `Reach::reflected`, and is noted in `Reach::records`, whose components lowering lists. The
//! methods of `Class` itself (`isRecord`, `getRecordComponents`) and of `RecordComponent` are
//! found as `Object`'s are, also on a `Class` value that may be `Class.class` or
//! `RecordComponent`'s, which code gets by its name.
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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{
    Item, MethodKey, Walk, by_name::class_bound, by_name::class_for_name, has_flag, literal_class,
    strip_arguments, text,
};
use crate::value::Value;

const OBJECT: &str = "java.lang.Object";
const CLASS: &str = "java.lang.Class";
/// The boxed classes with a `valueOf(String)`.
const BOXES: [&str; 7] = [
    "java.lang.Boolean",
    "java.lang.Byte",
    "java.lang.Short",
    "java.lang.Integer",
    "java.lang.Long",
    "java.lang.Float",
    "java.lang.Double",
];
const RECORD_COMPONENT: &str = "java.lang.reflect.RecordComponent";

/// The methods of `Class` itself that the reflection table has (`lower/reflection.rs`).
const CLASS_METHODS: [&str; 2] = ["isRecord", "getRecordComponents"];

/// The methods of the translator's `RecordComponent` that the reflection table has, which gson
/// looks up by name to run on JDKs without records.
pub const RECORD_COMPONENT_METHODS: [&str; 6] =
    ["getName", "getType", "getGenericType", "getAccessor", "getDeclaringRecord", "toString"];

/// The key in `Walk::reflected_keys` of a record whose components are found.
const RECORD_KEY: &str = "<record>";

/// The classes that the flow of `Class` values found that the lookups of each kind look in
/// (`Walk::flow_classes`).
pub type FlowClasses = BTreeMap<Kind, BTreeSet<String>>;

/// What a lookup looks for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Kind {
    Method,
    Field,
    Constructor,
    /// The components of a record (`getRecordComponents()`), with their fields, accessors and
    /// constructors.
    Record,
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
        "getRecordComponents" => (Kind::Record, false, true),
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

/// Whether the bound of a lookup's `Class` value allows any class.
fn is_subtype_of_object(bound: &str) -> bool {
    bound == OBJECT
}

/// The bound of the classes that a lookup without a class literal looks in: any record's
/// components are read, whatever the `Class` value's type says.
fn candidate_bound(lookup: &Lookup) -> &str {
    if lookup.kind == Kind::Record { OBJECT } else { lookup.bound.as_str() }
}

/// What `Walk::reflected_members` keeps from one call to the next (`Walk::reflect_cache`), so
/// that a call looks only where the walk may have added something to find since the last. The
/// walk calls it each time its work runs out, about a hundred times over the Minecraft server's
/// rounds, and each call looked through every lookup's classes and their members again.
///
/// The sets of classes a lookup looks in only grow (`Walk::named`, `Walk::class_literals` and
/// the others), so the classes of each bound are found by checking only those not checked
/// before (`Candidates`). A lookup that spells out its name, a constructor's, or one of every
/// member finds everything it will in a class the first time it looks there: each key it finds
/// goes into `Walk::reflected_keys`, which later calls skip, so it looks only in the classes added
/// since, in order of name, which finds the same members in the same order as looking in all of
/// them would. A lookup of a computed name looks in all of them again, as a new string may name
/// another member; the names of each class's members are kept.
///
/// What the cache keeps of a class holds once the class and all its supertypes are declared, or
/// are the runtime's (`outside`), which are never declared: its supertypes, whether it is below a
/// bound and its members do not change after (`Walk::chain_of`). The cache keeps the classes it
/// met before then (`unsettled`), with how many supertypes of theirs were known and declared.
/// Once more classes are declared (`Walk::declarations`), each of those whose supertypes changed
/// is looked at again as if new: checked against the bounds it was not below, and looked in again
/// by the lookups of those it was, which find only what they did not find before
/// (`Walk::reflected_keys`). So is a class declared again with more members (`Walk::redeclared`),
/// whose keys `redeclare_units` took out. The cache thus lasts across the rounds, where it used to
/// start over in each, as each round declares its units.
#[derive(Default)]
pub(super) struct ReflectCache {
    /// `Walk::declarations` and the length of `Walk::redeclared` when the cache was last brought
    /// up to date.
    declarations: usize,
    redeclared: usize,
    /// The classes met whose supertypes are not all known and declared yet, with how many of them
    /// were known and declared then (`Walk::chain_of`), and those met whose supertypes are.
    unsettled: HashMap<String, (usize, usize)>,
    settled: HashSet<String>,
    /// The classes that the lookups without a class literal of each bound and kind look in.
    candidates: HashMap<(String, Kind), Candidates>,
    /// How many of its classes (`Candidates::order`, or its class literal) each lookup whose
    /// names do not depend on the strings has looked in.
    looked: HashMap<Lookup, usize>,
    /// The supertypes of each class looked in, itself first (`Walk::supertypes_of`).
    supertypes: HashMap<String, Vec<String>>,
    /// The names of the methods and of the fields of each class looked in for every member or a
    /// computed name, with those it inherits (`Walk::member_names`).
    method_names: HashMap<String, Vec<String>>,
    field_names: HashMap<String, Vec<String>>,
}

/// The classes that the lookups of one bound and kind look in (`ReflectCache::candidates`).
#[derive(Default)]
struct Candidates {
    /// The classes that are or extend or implement the bound, in the order found, and in order
    /// of name.
    order: Vec<String>,
    sorted: BTreeSet<String>,
    /// The classes checked against the bound, and the sizes of the sets they come from when they
    /// last were.
    checked: HashSet<String>,
    sizes: [usize; 6],
}

/// Notes a class that the cache keeps something of among the settled or the unsettled ones
/// (`ReflectCache::unsettled`), unless it is already.
fn note_chain(
    walk: &Walk<'_>,
    unsettled: &mut HashMap<String, (usize, usize)>,
    settled: &mut HashSet<String>,
    class: &str,
) {
    if settled.contains(class) || unsettled.contains_key(class) {
        return;
    }
    match walk.chain_of(class) {
        (_, true) => settled.insert(class.to_string()),
        (chain, false) => unsettled.insert(class.to_string(), chain).is_none(),
    };
}

/// The fields of the record `decl` that are its components: those that are not static, in order.
fn record_fields(decl: &Value) -> impl Iterator<Item = &Value> {
    decl["members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| text(m, "kind") == "VARIABLE" && !has_flag(m, "STATIC"))
}

/// Whether the expression `node` is a `Class` value.
fn is_class_value(node: &Value) -> bool {
    let ty = text(node, "javaType");
    ty == "java.lang.Class" || ty.starts_with("java.lang.Class<")
}

/// Whether the code `node` assigns the variable `name`.
pub(super) fn assigns(node: &Value, name: &str) -> bool {
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
        let mut cache = std::mem::take(&mut self.reflect_cache);
        self.update_reflect_cache(&mut cache);
        let (found, work) = self.reflected_members(&mut cache);
        self.reflect_cache = cache;
        for item in &work {
            if let Item::Method(key) = item {
                self.reflected_methods.insert(key.clone());
            }
        }
        let mut more = false;
        for (class, name, key, declared) in &found {
            self.reflected_keys.insert(key.clone());
            if *declared {
                self.reach.reflected.insert((class.clone(), name.clone()));
            }
            // A record whose components are found: the classes that their types name are
            // `Class` values that reflection gives, which may be records read the same way.
            if key.1 == RECORD_KEY && self.reach.records.insert(class.clone()) {
                for name in self.record_component_types(class) {
                    more |= self.record_types.insert(name);
                }
            }
            // `Class.getRecordComponents` found by reflection reads the components of any record.
            if (class.as_str(), name.as_str()) == (CLASS, "getRecordComponents") {
                more |= self.lookups.insert(Lookup {
                    kind: Kind::Record,
                    name: None,
                    every: true,
                    no_parameters: false,
                    literal: None,
                    bound: OBJECT.to_string(),
                });
            }
        }
        self.work.extend(work);
        loaded || more || !found.is_empty()
    }

    /// The classes of the program that the declared types of the components of the record
    /// `class` name, with their type arguments.
    fn record_component_types(&self, class: &str) -> Vec<String> {
        let Some((decl, _)) = self.classes.get(class) else { return Vec::new() };
        let mut out = Vec::new();
        for member in record_fields(decl) {
            let ty = text(&member["symbol"], "type");
            for name in ty.split(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_' || c == '$')) {
                if self.classes.contains_key(name) && !out.iter().any(|o| o == name) {
                    out.push(name.to_string());
                }
            }
        }
        out
    }

    /// What the lookups find that they did not find before: each class and member name, its key
    /// in `Walk::reflected_keys`, and whether the class declares or inherits a member of that
    /// name; and the members to make reachable. The members are found by class and name
    /// (`Walk::methods_by_name`, `Walk::fields_by_class`), as a computed name can be any of
    /// thousands of strings in a library. What `cache` holds from the calls before spares
    /// looking again where nothing can be found (`ReflectCache`).
    #[allow(clippy::type_complexity)]
    fn reflected_members(
        &self,
        cache: &mut ReflectCache,
    ) -> (Vec<(String, String, (String, String), bool)>, Vec<Item>) {
        let (methods, fields) = (&self.methods_by_name, &self.fields_by_class);
        let mut found = Vec::new();
        let mut work = Vec::new();
        let mut seen = HashSet::new();
        let ReflectCache {
            candidates,
            looked,
            supertypes,
            method_names,
            field_names,
            unsettled,
            settled,
            ..
        } = cache;
        for lookup in &self.lookups {
            // The classes it can look in: its class literal, or those of its bound and kind.
            let group = lookup
                .literal
                .is_none()
                .then(|| &candidates[&(candidate_bound(lookup).to_string(), lookup.kind)]);
            let has = |class: &str| match (group, &lookup.literal) {
                (Some(group), _) => group.sorted.contains(class),
                (None, literal) => literal.as_deref() == Some(class),
            };
            // A lookup whose names do not depend on the strings finds all it ever finds in a
            // class the first time it looks there: it looks only in the classes added since, in
            // order of name. One of a computed name looks in all of them again, in order of name.
            let computed = lookup.kind != Kind::Constructor && !lookup.every && lookup.name.is_none();
            let before = if computed { 0 } else { looked.get(lookup).copied().unwrap_or(0) };
            let classes: Vec<&String> = match (group, &lookup.literal) {
                (Some(group), _) if computed => group.sorted.iter().collect(),
                (Some(group), _) => {
                    let mut added: Vec<&String> =
                        group.order[before.min(group.order.len())..].iter().collect();
                    added.sort();
                    added
                }
                (None, literal) => literal.iter().skip(before).collect(),
            };
            if !computed {
                let count = match (group, &lookup.literal) {
                    (Some(group), _) => group.order.len(),
                    // A class literal's class is looked in again until its supertypes are settled.
                    (None, literal) => usize::from(literal.as_deref().is_some_and(|c| self.chain_of(c).1)),
                };
                if before != count {
                    looked.insert(lookup.clone(), count);
                }
            }
            // `Object`'s `hashCode`, `equals` and `toString`, which the runtime's `Object` has
            // (`lower/reflection.rs`): the methods that a proxy class passes to its handler.
            if lookup.kind == Kind::Method && has(OBJECT) {
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
            // The boxed classes' `valueOf(String)`, which the runtime's boxes have
            // (`lower/reflection.rs`): jopt-simple's converter of an option's values.
            if lookup.kind == Kind::Method {
                // In order of name, as the classes are looked in.
                let mut boxes = BOXES;
                boxes.sort_unstable();
                for class in boxes.into_iter().filter(|c| has(c)) {
                    let wanted = match &lookup.name {
                        Some(n) if !lookup.every => n == "valueOf",
                        _ => lookup.every || self.strings.contains("valueOf"),
                    };
                    let key = (class.to_string(), "valueOf".to_string());
                    if wanted && !self.reflected_keys.contains(&key) && seen.insert(key.clone()) {
                        found.push((class.to_string(), "valueOf".to_string(), key, true));
                    }
                }
            }
            // `Class`'s own `isRecord` and `getRecordComponents`, and the methods of a record's
            // component, which the translator's `RecordComponent` answers (`lower/reflection.rs`).
            for (owner, names) in
                [(CLASS, &CLASS_METHODS[..]), (RECORD_COMPONENT, &RECORD_COMPONENT_METHODS[..])]
            {
                // A `Class` value that the lookup cannot tell may be `Class.class` or
                // `RecordComponent`'s, which code gets by its name.
                let candidate = has(owner)
                    || lookup.literal.is_none()
                        && is_subtype_of_object(&lookup.bound)
                        && match owner {
                            CLASS => self.class_literals.contains(CLASS),
                            _ => self.strings.contains(RECORD_COMPONENT),
                        };
                if lookup.kind != Kind::Method || !candidate {
                    continue;
                }
                for name in names {
                    let wanted = match &lookup.name {
                        Some(n) if !lookup.every => n == name,
                        _ => lookup.every || self.strings.contains(*name),
                    };
                    let key = (owner.to_string(), name.to_string());
                    if wanted && !self.reflected_keys.contains(&key) && seen.insert(key.clone()) {
                        found.push((owner.to_string(), name.to_string(), key, true));
                    }
                }
            }
            if lookup.kind == Kind::Record {
                for &class in classes.iter().filter(|c| self.classes.contains_key(c.as_str())) {
                    let decl = self.classes[class.as_str()].0;
                    let key = (class.clone(), RECORD_KEY.to_string());
                    if text(decl, "kind") != "RECORD"
                        || self.reflected_keys.contains(&key)
                        || !seen.insert(key.clone())
                    {
                        continue;
                    }
                    // Its fields, the accessors of the same names, and its constructors, which
                    // gson makes a record with: the canonical one, by the components' types.
                    let mut names: Vec<String> = Vec::new();
                    for field in record_fields(decl) {
                        let name = text(field, "name").to_string();
                        work.push(Item::Field((class.clone(), name.clone())));
                        for key in methods
                            .get(class.as_str())
                            .and_then(|m| m.get(name.as_str()))
                            .into_iter()
                            .flatten()
                        {
                            if key.2.starts_with("()") {
                                work.push(Item::Method((*key).clone()));
                            }
                        }
                        names.push(name);
                    }
                    for key in methods.get(class.as_str()).and_then(|m| m.get("<init>")).into_iter().flatten()
                    {
                        work.push(Item::Method((*key).clone()));
                    }
                    names.push("<init>".to_string());
                    for name in names {
                        found.push((class.clone(), name, key.clone(), true));
                    }
                }
                continue;
            }
            for &class in classes.iter().filter(|c| self.classes.contains_key(c.as_str())) {
                if !supertypes.contains_key(class) {
                    supertypes.insert(class.clone(), self.supertypes_of(class));
                    note_chain(self, unsettled, settled, class);
                }
                let owners = &supertypes[class];
                let names: Vec<String> = match (&lookup.name, lookup.kind) {
                    (_, Kind::Constructor) => vec!["<init>".to_string()],
                    (Some(name), _) if !lookup.every => vec![name.clone()],
                    // Every member, or for a computed name those that a string names.
                    (_, kind) => {
                        let cached = if kind == Kind::Field { &mut *field_names } else { &mut *method_names };
                        if !cached.contains_key(class) {
                            cached.insert(class.clone(), self.member_names(owners, kind));
                        }
                        cached[class]
                            .iter()
                            .filter(|n| lookup.every || self.strings.contains(n.as_str()))
                            .cloned()
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
                        if lookup.kind == Kind::Constructor { std::slice::from_ref(class) } else { owners };
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

    /// The names of the members of kind `kind` (fields, or methods other than constructors and
    /// initializers) that `owners`, a class and its supertypes, declare, in order.
    fn member_names(&self, owners: &[String], kind: Kind) -> Vec<String> {
        let mut all: BTreeSet<&str> = BTreeSet::new();
        for owner in owners {
            match kind {
                Kind::Field => all.extend(
                    self.fields_by_class.get(owner.as_str()).into_iter().flatten().map(String::as_str),
                ),
                _ => all.extend(
                    self.methods_by_name
                        .get(owner.as_str())
                        .into_iter()
                        .flat_map(|m| m.keys())
                        .map(String::as_str)
                        .filter(|n| !n.starts_with('<')),
                ),
            }
        }
        all.into_iter().map(str::to_string).collect()
    }

    /// Brings `cache` up to the walk: it starts over when a class was declared since it began
    /// (`Walk::declarations`), and each bound and kind of a lookup without a class literal has
    /// the classes of the sets it looks in that are or extend or implement the bound.
    fn update_reflect_cache(&self, cache: &mut ReflectCache) {
        if cache.declarations != self.declarations || cache.redeclared != self.redeclared.len() {
            cache.declarations = self.declarations;
            // The classes declared again, and those whose supertypes changed, are looked at again.
            let mut changed: BTreeSet<String> = self.redeclared[cache.redeclared..].iter().cloned().collect();
            cache.redeclared = self.redeclared.len();
            cache.unsettled.retain(|class, was| {
                let (now, settled) = self.chain_of(class);
                if now != *was {
                    changed.insert(class.clone());
                    *was = now;
                }
                if settled {
                    cache.settled.insert(class.clone());
                }
                !settled
            });
            for class in &changed {
                cache.supertypes.remove(class);
                cache.method_names.remove(class);
                cache.field_names.remove(class);
                for group in cache.candidates.values_mut() {
                    if group.sorted.contains(class) {
                        group.order.push(class.clone());
                    } else if group.checked.remove(class) {
                        group.sizes = [usize::MAX; 6];
                    }
                }
            }
        }
        let flow = |kind: Kind| self.flow_classes.get(&kind).into_iter().flatten();
        let flows: usize = self.flow_classes.values().map(BTreeSet::len).sum();
        let sizes = [
            self.named.len(),
            self.reach.found_by_name.len(),
            flows,
            self.annotation_classes.len(),
            self.class_literals.len(),
            self.record_types.len(),
        ];
        for lookup in self.lookups.iter().filter(|l| l.literal.is_none()) {
            let bound = candidate_bound(lookup);
            let key = (bound.to_string(), lookup.kind);
            if cache.candidates.get(&key).is_some_and(|group| group.sizes == sizes) {
                continue;
            }
            let group = cache.candidates.entry(key).or_default();
            group.sizes = sizes;
            let mut sources: Vec<&String> = Vec::new();
            sources.extend(self.named.iter());
            sources.extend(self.reach.found_by_name.iter());
            sources.extend(flow(lookup.kind));
            match lookup.kind {
                Kind::Constructor => {
                    sources.extend(self.annotation_classes.iter());
                    // A `Class<? extends T>` below `Object` holds the class literals of `T`'s
                    // classes that code passes it, whose objects it makes
                    // (`KeyStoreDelegator`'s `PKCS12KeyStore.class`).
                    if !is_subtype_of_object(bound) {
                        sources.extend(self.class_literals.iter());
                    }
                }
                Kind::Record => {
                    sources.extend(self.class_literals.iter());
                    sources.extend(self.record_types.iter());
                }
                _ => sources.extend(self.class_literals.iter()),
            }
            for class in sources {
                if !group.checked.contains(class) {
                    group.checked.insert(class.clone());
                    note_chain(self, &mut cache.unsettled, &mut cache.settled, class);
                    if self.is_subtype(class, bound) {
                        group.order.push(class.clone());
                        group.sorted.insert(class.clone());
                    }
                }
            }
        }
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

    /// The classes that a lookup of each kind on a `Class` value that is not a class literal may
    /// look in, whatever the bound of its type (`Reach::lookup_candidates`), as `reflected_members`
    /// takes them: for constructors, without the class literals that a lookup on a
    /// `Class<? extends T>` below `Object` looks in too, so that one on any other `Class` value
    /// that gets one of those has it put in (at the cost of a translation made again).
    pub(super) fn lookup_candidates(&self) -> BTreeMap<Kind, BTreeSet<String>> {
        let mut out = BTreeMap::new();
        for kind in [Kind::Method, Kind::Field, Kind::Constructor] {
            let mut all: BTreeSet<String> = self.named.iter().cloned().collect();
            all.extend(self.reach.found_by_name.iter().cloned());
            all.extend(self.flow_classes.get(&kind).into_iter().flatten().cloned());
            match kind {
                Kind::Constructor => all.extend(self.annotation_classes.iter().cloned()),
                _ => all.extend(self.class_literals.iter().cloned()),
            }
            out.insert(kind, all);
        }
        out
    }

    /// How many of the supertypes of `class` (`supertypes_of`) are known, and how many of those
    /// are declared; and whether they are settled: each declared, or the runtime's (`outside`),
    /// which is never declared.
    pub(super) fn chain_of(&self, class: &str) -> ((usize, usize), bool) {
        let chain = self.supertypes_of(class);
        let declared = chain.iter().filter(|c| self.supertypes.contains_key(c.as_str())).count();
        let settled = chain.iter().all(|c| self.supertypes.contains_key(c.as_str()) || super::outside(c));
        ((chain.len(), declared), settled)
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
