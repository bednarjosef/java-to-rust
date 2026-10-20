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
    pub methods: HashSet<MethodKey>,
    pub fields: HashSet<FieldKey>,
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
const OBJECT_METHODS: &[(&str, &str)] =
    &[("toString", "()java.lang.String"), ("equals", "(java.lang.Object)boolean"), ("hashCode", "()int")];

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
    work: Vec<Item>,
    reach: Reach,
}

impl<'t> Walk<'t> {
    fn declare(&mut self, decl: &'t Value, outer: Option<&str>) {
        if !CLASS_KINDS.contains(&text(decl, "kind")) {
            return;
        }
        let class = class_name(decl);
        for member in members(decl) {
            let name = text(member, "name").to_string();
            match text(member, "kind") {
                "METHOD" => {
                    self.methods.insert((class.clone(), name, symbol_type(member).to_string()), member);
                }
                "VARIABLE" => {
                    self.fields.insert((class.clone(), name), member);
                }
                _ => self.declare(member, Some(&class)),
            }
        }
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
        let Some(&node) = self.methods.get(&key) else { return };
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
    /// override it in instantiated subclasses. Classes cannot extend others or implement
    /// interfaces yet, so there are none. When they can, this is the hook: return the overriders
    /// of `method` in every instantiated subclass, and when a class becomes instantiated, also
    /// reach its overriders of the instance methods reached so far.
    fn overriders(&self, _method: &MethodKey) -> Vec<MethodKey> {
        Vec::new()
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
    /// anonymous classes inside are scanned whole, which over-approximates.
    fn scan(&mut self, value: &'t Value) {
        match value {
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

/// The qualified name of a class declaration, from the type javac gives its symbol
/// (`com.example.Box<T>` for a generic class).
pub fn class_name(decl: &Value) -> String {
    strip_arguments(symbol_type(decl))
}

fn strip_arguments(ty: &str) -> String {
    ty.split('<').next().unwrap_or(ty).to_string()
}

fn members(decl: &Value) -> impl Iterator<Item = &Value> {
    decl["members"].as_array().into_iter().flatten()
}

fn text<'v>(node: &'v Value, field: &str) -> &'v str {
    node[field].as_str().unwrap_or("")
}

fn symbol_type(node: &Value) -> &str {
    text(&node["symbol"], "type")
}

fn has_flag(node: &Value, flag: &str) -> bool {
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
