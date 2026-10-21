//! Reachability: the declarations of a program that code reachable from `main` uses. It runs on
//! javac's trees, before lowering, so that what is unreachable is never lowered at all: a
//! library's unused classes and methods may use any Java feature.
//!
//! javac has resolved every name, call and `new` to its declaration (the `symbol` of a node), so
//! a method is scanned once, when it becomes reachable, and every symbol in it makes its
//! declaration reachable in turn: a worklist that runs until nothing new is found.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// A method or constructor: the qualified name of its class, its name (`<init>` for a
/// constructor) and javac's signature, such as `(int)void`.
pub type MethodKey = (String, String, String);

/// A field: the qualified name of its class and its name.
pub type FieldKey = (String, String);

/// What reachable code uses, by qualified name.
#[derive(Default, Debug)]
pub struct Reach {
    /// Classes that reachable code names, as a type or through a member, and their outer classes.
    pub classes: HashSet<String>,
    /// Classes whose static initialization runs: a static member is used or an object created.
    pub initialized: HashSet<String>,
    /// Classes whose objects are created: one of their constructors is reachable.
    pub instantiated: HashSet<String>,
    /// Classes that an instantiated class is or extends or implements, directly or not: the classes
    /// whose instance methods can run.
    pub live: HashSet<String>,
    pub methods: HashSet<MethodKey>,
    pub fields: HashSet<FieldKey>,
    /// Methods that reachable code calls but that are not among the sources: those of the
    /// classes the runtime provides (see `RUNTIME_CLASSES`), and any that are missing.
    pub external: HashSet<MethodKey>,
}

impl Reach {
    /// Whether a member of the class with qualified name `class` (a node of its `members`) is
    /// reachable and so is translated.
    pub fn member(&self, class: &str, member: &Value) -> bool {
        let name = text(member, "name");
        match text(member, "kind") {
            "METHOD" => self.methods.contains(&(class.into(), name.into(), symbol_type(member).into())),
            "VARIABLE" => self.fields.contains(&(class.into(), name.into())),
            // Initializer blocks run with the class's initialization or with every constructor.
            "BLOCK" if member["static"].as_bool() == Some(true) => self.initialized.contains(class),
            "BLOCK" => self.instantiated.contains(class),
            kind if CLASS_KINDS.contains(&kind) => self.classes.contains(&class_name(member)),
            _ => true,
        }
    }
}

/// The methods of `Object` that a class may override, which code outside the program (printing,
/// string concatenation, collections) calls on its objects.
pub(super) const OBJECT_METHODS: &[(&str, &str)] =
    &[("toString", "()java.lang.String"), ("equals", "(java.lang.Object)boolean"), ("hashCode", "()int")];

/// The classes that the runtime provides and the translator maps to it, such as `String` and
/// `System`: their sources are never read, even when a library root has them, as their code
/// reaches deep into the JVM.
pub const RUNTIME_CLASSES: &[&str] = &[
    "java.io.PrintStream",
    "java.lang.Boolean",
    "java.lang.Byte",
    "java.lang.Character",
    "java.lang.Class",
    "java.lang.ClassLoader",
    "java.lang.Double",
    "java.lang.Enum",
    "java.lang.Float",
    "java.lang.Integer",
    "java.lang.Long",
    "java.lang.Math",
    "java.lang.Number",
    "java.lang.Object",
    "java.lang.Runtime",
    "java.lang.Short",
    "java.lang.StrictMath",
    "java.lang.String",
    "java.lang.StringBuffer",
    "java.lang.StringBuilder",
    "java.lang.System",
    "java.lang.Thread",
    "java.lang.ref.PhantomReference",
    "java.lang.ref.Reference",
    "java.lang.ref.ReferenceQueue",
    "java.lang.ref.SoftReference",
    "java.lang.ref.WeakReference",
    "java.util.Arrays",
    "sun.util.logging.PlatformLogger",
];

/// What is not translated, by the prefix of qualified class names, with the feature it is:
/// reachability stops there, as at the runtime's classes, and the survey counts its use. Library
/// code reaches these on paths that only its data rules out, such as `TreeMap.addAll` reading
/// from a stream that is null there.
pub const UNTRANSLATED: &[(&str, &str)] = &[
    ("java.io.ObjectInputStream", "serialization"),
    ("java.io.ObjectOutputStream", "serialization"),
    ("java.io.ObjectStreamClass", "serialization"),
    ("java.lang.invoke.", "reflection"),
    ("java.lang.reflect.", "reflection"),
    ("java.security.", "security"),
    ("jdk.internal.reflect.", "reflection"),
    ("sun.security.", "security"),
];

/// Static fields that only Java Flight Recorder sets, through reflection when it starts
/// (`jdk.internal.event.JFRTracing`): no code sets them otherwise, so in a translated program they
/// stay `false` and the code they guard, which records events through `jdk.internal.event`, never
/// runs.
pub const FALSE_FLAGS: &[(&str, &str)] = &[
    ("java.io.FileInputStream", "jfrTracing"),
    ("java.io.FileOutputStream", "jfrTracing"),
    ("java.io.RandomAccessFile", "jfrTracing"),
    ("java.lang.Throwable", "jfrTracing"),
    ("sun.nio.ch.FileChannelImpl", "jfrTracing"),
];

/// Whether an `if` statement's condition is never true, so that only its `else` branch can run:
/// it reads one of the `FALSE_FLAGS`, alone or as an operand of `&&`.
pub fn never_true(condition: &Value) -> bool {
    match text(condition, "kind") {
        "PARENTHESIZED" => never_true(&condition["expression"]),
        "CONDITIONAL_AND" => never_true(&condition["leftOperand"]) || never_true(&condition["rightOperand"]),
        "IDENTIFIER" | "MEMBER_SELECT" => {
            let symbol = &condition["symbol"];
            text(symbol, "kind") == "FIELD"
                && FALSE_FLAGS.contains(&(text(symbol, "owner"), text(symbol, "name")))
        }
        _ => false,
    }
}

/// The feature that the class with qualified name `class` belongs to, if it is not translated.
pub fn untranslated(class: &str) -> Option<&'static str> {
    UNTRANSLATED.iter().find(|(prefix, _)| class.starts_with(prefix)).map(|&(_, feature)| feature)
}

/// Whether reachability stops at the class with qualified name `class`: the runtime provides it,
/// or it is not translated.
pub fn outside(class: &str) -> bool {
    provided(class) || untranslated(class).is_some()
}

/// Whether the translator provides the class with qualified name `class`: one of the runtime's
/// classes, or a standard exception class (see `lower::exceptions`).
pub fn provided(class: &str) -> bool {
    RUNTIME_CLASSES.contains(&class) || crate::lower::is_standard(class)
}

/// The node kinds of class declarations, and the symbol kinds of classes.
pub const CLASS_KINDS: &[&str] = &["CLASS", "INTERFACE", "ENUM", "RECORD", "ANNOTATION_TYPE"];

/// What reachable code uses in the program made of these compilation units, starting from the
/// `main` method of the class with qualified name `main`.
pub fn reach(units: &[&Value], main: &str) -> Result<Reach, String> {
    let mut walk = Walk::default();
    for unit in units {
        for decl in unit["typeDecls"].as_array().into_iter().flatten() {
            walk.declare(decl, None);
        }
    }
    if !walk.classes.contains_key(main) {
        return Err(format!("the main class `{main}` is not among the sources"));
    }
    let entry = (main.to_string(), "main".to_string(), "(java.lang.String[])void".to_string());
    if !walk.methods.contains_key(&entry) {
        return Err(format!("`{main}` has no `public static void main(String[])`"));
    }
    walk.work.push(Item::Init(main.to_string()));
    walk.work.push(Item::Method(entry));
    walk.run();
    Ok(walk.reach)
}

/// A declaration that has become reachable and is still to be visited.
enum Item {
    Class(String),
    Init(String),
    Method(MethodKey),
    Field(FieldKey),
}

/// The declarations of the program, and the worklist over them.
#[derive(Default)]
struct Walk<'t> {
    /// Every class, with its declaration and its outer class.
    classes: HashMap<String, (&'t Value, Option<String>)>,
    methods: HashMap<MethodKey, &'t Value>,
    fields: HashMap<FieldKey, &'t Value>,
    /// The direct supertypes of each class.
    supertypes: HashMap<String, Vec<String>>,
    /// The methods that override each method, as javac reports them.
    overriders: HashMap<MethodKey, Vec<MethodKey>>,
    /// Overriders of reached methods whose class is not live yet, by that class: they become
    /// reachable when it is.
    waiting: HashMap<String, Vec<MethodKey>>,
    work: Vec<Item>,
    reach: Reach,
}

impl<'t> Walk<'t> {
    fn declare(&mut self, decl: &'t Value, outer: Option<&str>) {
        if !CLASS_KINDS.contains(&text(decl, "kind")) {
            return;
        }
        let class = class_name(decl);
        if outside(&class) {
            return;
        }
        for member in members(decl) {
            let name = text(member, "name").to_string();
            if !CLASS_KINDS.contains(&text(member, "kind")) {
                let mut locals = Vec::new();
                local_classes(member, &mut locals);
                for local in locals {
                    self.declare(local, Some(&class));
                }
            }
            match text(member, "kind") {
                "METHOD" => {
                    let key = (class.clone(), name, symbol_type(member).to_string());
                    for overridden in member["symbol"]["overrides"].as_array().into_iter().flatten() {
                        let overridden = (
                            text(overridden, "owner").to_string(),
                            key.1.clone(),
                            text(overridden, "type").to_string(),
                        );
                        self.overriders.entry(overridden).or_default().push(key.clone());
                    }
                    self.methods.insert(key, member);
                }
                "VARIABLE" => {
                    self.fields.insert((class.clone(), name), member);
                }
                _ => self.declare(member, Some(&class)),
            }
        }
        let mut supertypes: Vec<String> = ["extendsClause", "implementsClause"]
            .iter()
            .flat_map(|header| match &decl[header] {
                Value::Array(items) => items.iter().collect(),
                one => vec![one],
            })
            .filter_map(supertype)
            .collect();
        let implicit = match text(decl, "kind") {
            "ENUM" => Some("java.lang.Enum"),
            "RECORD" => Some("java.lang.Record"),
            "CLASS" if decl["extendsClause"].is_null() && class != "java.lang.Object" => {
                Some("java.lang.Object")
            }
            _ => None,
        };
        supertypes.extend(implicit.map(str::to_string));
        self.supertypes.insert(class.clone(), supertypes);
        self.classes.insert(class, (decl, outer.map(str::to_string)));
    }

    fn run(&mut self) {
        while let Some(item) = self.work.pop() {
            match item {
                Item::Class(class) => self.class(class),
                Item::Init(class) => self.init(class),
                Item::Method(key) => self.method(key),
                Item::Field(key) => self.field(key),
            }
        }
    }

    /// A class named by reachable code: it exists as a type, with its outer class and the types in
    /// its header.
    fn class(&mut self, class: String) {
        let Some((decl, outer)) = self.classes.get(&class).map(|(d, o)| (*d, o.clone())) else { return };
        if !self.reach.classes.insert(class) {
            return;
        }
        if let Some(outer) = outer {
            self.work.push(Item::Class(outer));
        }
        for header in ["extendsClause", "implementsClause", "permitsClause", "typeParameters"] {
            self.scan(&decl[header]);
        }
    }

    /// A class whose static initialization runs: its static initializer blocks run, and its static
    /// fields with initializers that may do something besides computing a value are kept even if
    /// nothing reads them. (When classes can extend others, the superclass is initialized first.)
    fn init(&mut self, class: String) {
        let Some(&(decl, _)) = self.classes.get(&class) else { return };
        if !self.reach.initialized.insert(class.clone()) {
            return;
        }
        self.work.push(Item::Class(class.clone()));
        self.initializers(decl, &class, true);
    }

    /// The initializers that run for the class's static initialization (`statics`) or for every
    /// object it creates.
    fn initializers(&mut self, decl: &'t Value, class: &str, statics: bool) {
        for member in members(decl) {
            let is_static = match text(member, "kind") {
                "BLOCK" => member["static"].as_bool() == Some(true),
                "VARIABLE" => has_flag(member, "STATIC"),
                _ => continue,
            };
            if is_static != statics {
                continue;
            }
            if text(member, "kind") == "BLOCK" {
                self.scan(member);
            } else if has_effects(&member["initializer"]) {
                self.work.push(Item::Field((class.to_string(), text(member, "name").to_string())));
            }
        }
    }

    fn method(&mut self, key: MethodKey) {
        let Some(&node) = self.methods.get(&key) else {
            // What overrides a method of the runtime, such as `Object.toString`, can still run.
            if self.reach.external.insert(key.clone()) {
                for overrider in self.overriders(&key) {
                    self.work.push(Item::Method(overrider));
                }
            }
            return;
        };
        if !self.reach.methods.insert(key.clone()) {
            return;
        }
        let class = key.0.clone();
        let is_ctor = key.1 == "<init>";
        let is_static = node["symbol"]["static"].as_bool() == Some(true);
        self.work.push(Item::Class(class.clone()));
        if is_static || is_ctor {
            self.work.push(Item::Init(class.clone()));
        }
        if is_ctor && self.reach.instantiated.insert(class.clone()) {
            self.live(class.clone());
            let decl = self.classes[&class].0;
            self.initializers(decl, &class, false);
            // Library code calls these on any object, as string conversion calls `toString`.
            for (name, signature) in OBJECT_METHODS {
                self.work.push(Item::Method((class.clone(), name.to_string(), signature.to_string())));
            }
        }
        if !is_static && !is_ctor {
            for overrider in self.overriders(&key) {
                self.work.push(Item::Method(overrider));
            }
        }
        self.scan(node);
    }

    /// The methods that a call of the instance method `method` may run instead of it: those that
    /// override it in live classes. Those in classes that are not live yet wait until they are.
    fn overriders(&mut self, method: &MethodKey) -> Vec<MethodKey> {
        let mut now = Vec::new();
        for overrider in self.overriders.get(method).cloned().unwrap_or_default() {
            if self.reach.live.contains(&overrider.0) {
                now.push(overrider);
            } else {
                self.waiting.entry(overrider.0.clone()).or_default().push(overrider);
            }
        }
        now
    }

    /// Makes a class live, and its supertypes: the overriders that waited for one become
    /// reachable.
    fn live(&mut self, class: String) {
        let mut work = vec![class];
        while let Some(class) = work.pop() {
            if !self.reach.live.insert(class.clone()) {
                continue;
            }
            for overrider in self.waiting.remove(&class).unwrap_or_default() {
                self.work.push(Item::Method(overrider));
            }
            work.extend(self.supertypes.get(&class).cloned().unwrap_or_default());
        }
    }

    fn field(&mut self, key: FieldKey) {
        let Some(&node) = self.fields.get(&key) else { return };
        if !self.reach.fields.insert(key.clone()) {
            return;
        }
        self.work.push(Item::Class(key.0.clone()));
        if has_flag(node, "STATIC") {
            self.work.push(Item::Init(key.0));
        }
        self.scan(node);
    }

    /// Makes every declaration that a node of this subtree refers to reachable. Local and
    /// anonymous classes inside are scanned whole, which over-approximates. Annotations are
    /// skipped: without reflection they do nothing at run time. So is the branch of an `if` whose
    /// condition is never true (`never_true`).
    fn scan(&mut self, value: &'t Value) {
        match value {
            Value::Object(_) if matches!(text(value, "kind"), "ANNOTATION" | "TYPE_ANNOTATION") => {}
            // The branch that a condition that is never true guards is not reached.
            Value::Object(_) if text(value, "kind") == "IF" && never_true(&value["condition"]) => {
                self.scan(&value["condition"]);
                self.scan(&value["elseStatement"]);
            }
            Value::Object(fields) => {
                for (field, child) in fields {
                    if field == "symbol" {
                        self.symbol(child);
                    } else {
                        self.scan(child);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.scan(item)),
            _ => {}
        }
    }

    fn symbol(&mut self, symbol: &Value) {
        let (owner, name) = (text(symbol, "owner").to_string(), text(symbol, "name").to_string());
        match text(symbol, "kind") {
            "METHOD" | "CONSTRUCTOR" => {
                self.work.push(Item::Method((owner, name, text(symbol, "type").to_string())));
            }
            "FIELD" | "ENUM_CONSTANT" => self.work.push(Item::Field((owner, name))),
            kind if CLASS_KINDS.contains(&kind) => {
                self.work.push(Item::Class(strip_arguments(text(symbol, "type"))))
            }
            _ => {}
        }
    }
}

/// The local and anonymous class declarations in the code of a member, outside the bodies of
/// the classes found: they are declared with the member's class as their outer class, and
/// `Walk::scan` reaches their members when it scans the code.
fn local_classes<'t>(value: &'t Value, out: &mut Vec<&'t Value>) {
    match value {
        Value::Object(fields) => {
            if CLASS_KINDS.contains(&text(value, "kind")) {
                // javac names the body of an enum constant `<anonymous Planet>`: it belongs to
                // the enum (see `lower::constant_bodies`).
                if !symbol_type(value).starts_with('<') {
                    out.push(value);
                }
                return;
            }
            for (field, child) in fields {
                if field != "symbol" {
                    local_classes(child, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| local_classes(item, out)),
        _ => {}
    }
}

/// The qualified name of a class declaration, from the type javac gives its symbol
/// (`com.example.Box<T>` for a generic class).
pub fn class_name(decl: &Value) -> String {
    strip_arguments(symbol_type(decl))
}

/// The qualified name of the class that a type in a class header names: `List<E>` names
/// `java.util.List`.
fn supertype(ty: &Value) -> Option<String> {
    match text(ty, "kind") {
        "PARAMETERIZED_TYPE" => supertype(&ty["type"]),
        _ => ty["symbol"]["type"].as_str().map(strip_arguments),
    }
}

/// A type without its type arguments: `java.util.ArrayList<E>.Itr` is `java.util.ArrayList.Itr`.
fn strip_arguments(ty: &str) -> String {
    let mut depth = 0;
    ty.chars()
        .filter(|&c| {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ => return depth == 0,
            }
            false
        })
        .collect()
}

pub(super) fn members(decl: &Value) -> impl Iterator<Item = &Value> {
    decl["members"].as_array().into_iter().flatten()
}

pub(super) fn text<'v>(node: &'v Value, field: &str) -> &'v str {
    node[field].as_str().unwrap_or("")
}

pub(super) fn symbol_type(node: &Value) -> &str {
    text(&node["symbol"], "type")
}

pub(super) fn has_flag(node: &Value, flag: &str) -> bool {
    if flag == "STATIC"
        && let Some(s) = node["symbol"]["static"].as_bool()
    {
        return s;
    }
    node["modifiers"]["flags"].as_array().is_some_and(|f| f.iter().any(|x| x == flag))
}

/// Whether evaluating an initializer may do more than compute a value: call a method, create an
/// object, assign, or throw on a division.
fn has_effects(value: &Value) -> bool {
    match value {
        Value::Object(fields) => {
            let kind = text(value, "kind");
            matches!(
                kind,
                "METHOD_INVOCATION"
                    | "NEW_CLASS"
                    | "ASSIGNMENT"
                    | "PREFIX_INCREMENT"
                    | "PREFIX_DECREMENT"
                    | "POSTFIX_INCREMENT"
                    | "POSTFIX_DECREMENT"
                    | "DIVIDE"
                    | "REMAINDER"
            ) || kind.ends_with("_ASSIGNMENT")
                || fields.values().any(has_effects)
        }
        Value::Array(items) => items.iter().any(has_effects),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A static call of `class.name()`, as a statement of javac's tree.
    fn call(class: &str, name: &str) -> Value {
        json!({"kind": "EXPRESSION_STATEMENT", "expression": {
            "kind": "METHOD_INVOCATION",
            "symbol": {"kind": "METHOD", "owner": class, "name": name, "type": "()void", "static": true},
        }})
    }

    /// A class with static methods of no arguments (`main` takes its `String[]`), each with the
    /// statements given.
    fn class(name: &str, methods: &[(&str, Vec<Value>)]) -> Value {
        let members: Vec<Value> = methods
            .iter()
            .map(|(method, statements)| {
                let ty = if *method == "main" { "(java.lang.String[])void" } else { "()void" };
                json!({"kind": "METHOD", "name": method, "symbol": {"type": ty, "static": true},
                       "body": {"kind": "BLOCK", "statements": statements}})
            })
            .collect();
        json!({"kind": "CLASS", "symbol": {"type": name}, "members": members})
    }

    #[test]
    fn a_branch_guarded_by_a_flag_only_flight_recorder_sets_is_not_reached() {
        let flag = json!({"kind": "MEMBER_SELECT",
            "symbol": {"kind": "FIELD", "owner": "java.lang.Throwable", "name": "jfrTracing", "static": true}});
        let guarded = json!({"kind": "IF",
            "condition": {"kind": "PARENTHESIZED", "expression": {"kind": "CONDITIONAL_AND",
                "leftOperand": flag, "rightOperand": {"kind": "LITERAL"}}},
            "thenStatement": call("Tracer", "trace"),
            "elseStatement": call("Other", "run")});
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![guarded])]),
            class("Tracer", &[("trace", vec![])]),
            class("Other", &[("run", vec![])]),
        ]});
        let reach = reach(&[&unit], "Main").unwrap();
        let method = |class: &str, name: &str| (class.to_string(), name.to_string(), "()void".to_string());
        assert!(reach.methods.contains(&method("Other", "run")), "{reach:?}");
        assert!(!reach.methods.contains(&method("Tracer", "trace")), "{reach:?}");
        assert!(!reach.classes.contains("Tracer"), "{reach:?}");
    }
}
