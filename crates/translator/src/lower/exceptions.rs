//! Exceptions: `throw`, `try`/`catch`, and the standard exception classes.
//!
//! The standard classes (`Throwable`, `Exception`, `IllegalArgumentException`, ...) are not read
//! from the JDK: the translator has its own sources for them, `java/Standard.java`, written in
//! the subset it supports, whose syntax tree `build.rs` prints. A program gets the classes it
//! names, with their superclasses, and of those only the constructors and methods it uses
//! (with what they use in turn), as classes of the program: they are lowered, analysed and
//! generated like the program's own, so a program's exception class extends them like any
//! superclass. A standard class is abstract unless the program creates its objects, so that the
//! enum of `Throwable` has a variant only for the classes whose objects exist. `java.lang.Thread`
//! comes the same way from `java/Threads.java` (see docs/multi-threading.md): it is a standard
//! class too, though not an exception.
//!
//! The runtime raises some of them itself (`RAISED`): a division by zero, an index out of
//! bounds, a malformed number. Where a `catch` or a `finally` of the program might see such a
//! failure (an operation that may fail and a `catch` of the class or one above it, or any
//! `finally`, found in the syntax tree), the class is declared with its constructor from a
//! message and counts as created; `Class::raised` holds the `new` expressions that build its
//! objects, which the analysis decides are needed or not.

use super::*;
use std::collections::HashSet;
use std::sync::OnceLock;

/// The standard classes that are not in `java.lang`, by simple name.
const OTHER_PACKAGES: &[(&str, &str)] = &[
    ("NoSuchElementException", "java.util"),
    ("ConcurrentModificationException", "java.util"),
    ("InputMismatchException", "java.util"),
    ("EmptyStackException", "java.util"),
    ("MissingResourceException", "java.util"),
    ("IOException", "java.io"),
    ("FileNotFoundException", "java.io"),
    ("EOFException", "java.io"),
    ("UnsupportedEncodingException", "java.io"),
    ("UncheckedIOException", "java.io"),
];

/// The standard classes that the runtime raises and a translation can catch, each with the
/// kinds of javac's tree of the operations that may raise it (a method call by its owner and
/// name: `owner#name`, `owner#` for any method of the owner, or `METHOD_INVOCATION` for any
/// call; `enum#valueOf` for the `valueOf` of any enum of the program).
const RAISED: &[(&str, &[&str])] = &[
    (
        "java.lang.ArithmeticException",
        &[
            "DIVIDE",
            "REMAINDER",
            "DIVIDE_ASSIGNMENT",
            "REMAINDER_ASSIGNMENT",
            "java.lang.Math#floorDiv",
            "java.lang.Math#floorMod",
            "java.lang.StrictMath#floorDiv",
            "java.lang.StrictMath#floorMod",
        ],
    ),
    (
        "java.lang.ArrayIndexOutOfBoundsException",
        &["ARRAY_ACCESS", "java.lang.Character#toChars", "java.lang.System#arraycopy"],
    ),
    ("java.lang.NegativeArraySizeException", &["NEW_ARRAY", "java.util.Arrays#copyOf"]),
    // `super.clone()` of an object whose class is not `Cloneable`.
    ("java.lang.CloneNotSupportedException", &["java.lang.Object#clone"]),
    (
        "java.lang.IndexOutOfBoundsException",
        &[
            "java.lang.Integer#parseInt",
            "java.lang.Long#parseLong",
            "java.lang.Integer#parseUnsignedInt",
            "java.lang.Long#parseUnsignedLong",
        ],
    ),
    // A call, a field access, an array access or a switch on a reference that may be null.
    (
        "java.lang.NullPointerException",
        &["METHOD_INVOCATION", "MEMBER_SELECT", "ARRAY_ACCESS", "SWITCH", "SWITCH_EXPRESSION"],
    ),
    ("java.lang.ClassCastException", &["TYPE_CAST"]),
    (
        "java.lang.IllegalArgumentException",
        &["enum#valueOf", "java.lang.Character#toChars", "java.lang.Object#wait"],
    ),
    // A monitor that the thread does not hold.
    (
        "java.lang.IllegalMonitorStateException",
        &["java.lang.Object#wait", "java.lang.Object#notify", "java.lang.Object#notifyAll"],
    ),
    (
        "java.lang.StringIndexOutOfBoundsException",
        &[
            "java.lang.String#charAt",
            "java.lang.String#getChars",
            "java.lang.String#substring",
            "java.lang.String#valueOf",
            "java.lang.String#copyValueOf",
        ],
    ),
    // The natives of the descriptor streams (`fd.rs`): reading or writing a closed or failing
    // descriptor, and opening a file that cannot be opened.
    (
        "java.io.IOException",
        &[
            "java.io.FileInputStream#read0",
            "java.io.FileInputStream#readBytes",
            "java.io.FileInputStream#available0",
            "java.io.FileInputStream#skip0",
            "java.io.FileOutputStream#write",
            "java.io.FileOutputStream#writeBytes",
        ],
    ),
    ("java.io.FileNotFoundException", &["java.io.FileInputStream#open0", "java.io.FileOutputStream#open0"]),
    (
        "java.lang.NumberFormatException",
        &[
            "java.lang.Integer#",
            "java.lang.Long#",
            "java.lang.Short#",
            "java.lang.Byte#",
            "java.lang.Double#",
            "java.lang.Float#",
        ],
    ),
];

/// The operation without which the runtime never raises `InterruptedException`.
const INTERRUPTS: &str = "java.lang.Thread#interrupt";

/// More of `RAISED`, in a program that interrupts a thread (`INTERRUPTS`): `sleep`, `join` and
/// `wait` end with an interrupt, and `sleep` has a checked form for its negative time as well.
/// (Elsewhere `sleep` never throws `InterruptedException`, and a `catch` that sees its
/// `IllegalArgumentException` runs it in `java::catching`.)
const RAISED_WHEN_INTERRUPTED: &[(&str, &[&str])] = &[
    (
        "java.lang.InterruptedException",
        &["java.lang.Thread#sleep", "java.lang.Thread#join", "java.lang.Object#wait"],
    ),
    ("java.lang.IllegalArgumentException", &["java.lang.Thread#sleep"]),
];

/// The classes of `RAISED` that the runtime may raise without a message as well, which are built
/// with the constructor without arguments then, so that `getMessage()` is null.
const BARE: &[&str] = &["java.lang.InterruptedException"];

/// The package of the standard class with this simple name.
fn package_of(simple: &str) -> &'static str {
    OTHER_PACKAGES.iter().find(|(s, _)| *s == simple).map_or("java.lang", |(_, p)| p)
}

/// The class declarations of `java/Standard.java` and `java/Threads.java`, renamed from the
/// packages `standard` and `threads` to their real packages, as javac names the real classes in a
/// program's tree.
fn template() -> &'static [Value] {
    static DECLS: OnceLock<Vec<Value>> = OnceLock::new();
    DECLS.get_or_init(|| {
        let files = [
            ("standard", include_str!(concat!(env!("OUT_DIR"), "/standard.json"))),
            ("threads", include_str!(concat!(env!("OUT_DIR"), "/threads.json"))),
        ];
        let mut decls = Vec::new();
        for (package, text) in files {
            let mut tree: Value = serde_json::from_str(text).expect("build.rs writes the templates' trees");
            rename(&mut tree, package);
            match tree["units"][0]["tree"]["typeDecls"].take() {
                Value::Array(found) => decls.extend(found),
                _ => panic!("the tree of the template `{package}` has its classes"),
            }
        }
        decls
    })
}

/// Gives the classes of the template's package `from` their real packages, in every name and type.
fn rename(v: &mut Value, from: &str) {
    let prefix = format!("{from}.");
    match v {
        Value::String(s) => {
            while let Some(at) = s.find(&prefix) {
                let rest = &s[at + prefix.len()..];
                let end = rest.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(rest.len());
                let simple = rest[..end].to_string();
                s.replace_range(at..at + from.len(), package_of(&simple));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|i| rename(i, from)),
        Value::Object(map) => {
            // A class's symbol names its package as its owner.
            if map.get("kind").and_then(Value::as_str) == Some("CLASS")
                && map.get("owner").and_then(Value::as_str) == Some(from)
            {
                let simple = map.get("name").and_then(Value::as_str).unwrap_or_default();
                let package = package_of(simple);
                map.insert("owner".into(), Value::String(package.into()));
            }
            map.values_mut().for_each(|v| rename(v, from));
        }
        _ => {}
    }
}

/// The qualified name of a class declaration of the template.
fn qualified(decl: &Value) -> String {
    let simple = decl["simpleName"].as_str().unwrap_or_default();
    format!("{}.{simple}", package_of(simple))
}

/// Every class declaration of the template, with its qualified name: the top-level ones, and the
/// classes and interfaces declared in them (`java.lang.Thread.UncaughtExceptionHandler`), which
/// lowering declares with their outer class.
fn template_types() -> &'static [(String, &'static Value)] {
    static TYPES: OnceLock<Vec<(String, &'static Value)>> = OnceLock::new();
    TYPES.get_or_init(|| {
        fn nested(outer: &str, decl: &'static Value, out: &mut Vec<(String, &'static Value)>) {
            out.push((outer.to_string(), decl));
            for member in decl["members"].as_array().into_iter().flatten() {
                if matches!(member["kind"].as_str(), Some("CLASS" | "INTERFACE" | "ENUM" | "RECORD")) {
                    let simple = member["simpleName"].as_str().unwrap_or_default();
                    nested(&format!("{outer}.{simple}"), member, out);
                }
            }
        }
        let mut out = Vec::new();
        for decl in template() {
            nested(&qualified(decl), decl, &mut out);
        }
        out
    })
}

/// The declaration of the template's class with qualified name `name`.
fn template_type(name: &str) -> Option<&'static Value> {
    template_types().iter().find(|(n, _)| n == name).map(|(_, d)| *d)
}

/// Whether `name` is the qualified name of a standard class the translator provides.
pub fn is_standard(name: &str) -> bool {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| template_types().iter().map(|(n, _)| n.clone()).collect()).contains(name)
}

/// The name of a standard class inside its package: `Thread`, or `Thread.UncaughtExceptionHandler`
/// for a class declared in another.
pub fn standard_local_name(name: &str) -> &str {
    let mut at = 0;
    for segment in name.split('.') {
        if segment.starts_with(|c: char| c.is_ascii_uppercase()) {
            return &name[at..];
        }
        at += segment.len() + 1;
    }
    name
}

/// Whether the standard class `name` is the standard class `ancestor` or a subclass of it.
pub fn standard_subclass(name: &str, ancestor: &str) -> bool {
    let mut at = Some(name.to_string());
    while let Some(c) = at {
        if c == ancestor {
            return true;
        }
        at = template_type(&c).and_then(|d| d["extendsClause"]["javaType"].as_str()).map(str::to_string);
    }
    false
}

/// The qualified names of every standard class the translator provides.
pub fn standard_names() -> Vec<String> {
    template_types().iter().map(|(n, _)| n.clone()).collect()
}

/// What a program uses of the standard classes: the classes to declare, and their members.
#[derive(Default)]
pub(super) struct Standard {
    /// The declarations to add, with their packages, superclasses first.
    pub classes: Vec<(&'static str, &'static Value)>,
    names: HashSet<String>,
    /// Constructors and methods, as (class, name, javac signature).
    members: HashSet<(String, String, String)>,
    /// Fields, as (class, name).
    fields: HashSet<(String, String)>,
    /// The classes that the program creates objects of.
    pub instantiated: HashSet<String>,
    /// The classes of `RAISED` that the runtime may raise where the program could catch them.
    raised: HashSet<String>,
}

/// `Throwable.addSuppressed`, which each try-with-resources calls (see `with_resource`).
fn suppress_member() -> (String, String, String) {
    ("java.lang.Throwable".into(), "addSuppressed".into(), "(java.lang.Throwable)void".into())
}

/// The classes that a static initializer's failure is seen as (see `lower/clinit_errors.rs`): a
/// program that catches one of these, or a supertype of them, declares them, with the constructors
/// that the rewritten `<clinit>` calls.
fn linkage_classes(uses: &mut Uses) {
    const CAUGHT: [&str; 5] = [
        "java.lang.Throwable",
        "java.lang.Error",
        "java.lang.LinkageError",
        "java.lang.ExceptionInInitializerError",
        "java.lang.NoClassDefFoundError",
    ];
    if !uses.catches.iter().any(|c| CAUGHT.contains(&c.as_str())) {
        return;
    }
    let members = [
        ("java.lang.ExceptionInInitializerError", "(java.lang.Throwable)void"),
        ("java.lang.ExceptionInInitializerError", "(java.lang.String)void"),
        ("java.lang.NoClassDefFoundError", "(java.lang.String,java.lang.Throwable)void"),
    ];
    for (class, signature) in members {
        uses.classes.insert(class.into());
        uses.members.insert((class.into(), "<init>".into(), signature.into()));
        uses.instantiated.insert(class.into());
    }
}

/// The uses of the standard classes found in a tree.
#[derive(Default)]
struct Uses {
    classes: HashSet<String>,
    members: HashSet<(String, String, String)>,
    fields: HashSet<(String, String)>,
    instantiated: HashSet<String>,
    /// The types of the `catch` clauses, and whether there is a `finally`.
    catches: HashSet<String>,
    finally: bool,
    /// The operations that may fail, as `RAISED` names them.
    operations: HashSet<String>,
}

impl Uses {
    fn scan(&mut self, v: &Value) {
        match v {
            Value::Array(items) => items.iter().for_each(|i| self.scan(i)),
            Value::Object(map) => {
                let text = |v: &Value, field: &str| v[field].as_str().unwrap_or_default().to_string();
                if let Some(ty) = map.get("javaType").and_then(Value::as_str)
                    && is_standard(ty)
                {
                    self.classes.insert(ty.to_string());
                }
                let symbol = &v["symbol"];
                let owner = text(symbol, "owner");
                let kind = text(v, "kind");
                match kind.as_str() {
                    "TRY" => {
                        self.finally |= !v["finallyBlock"].is_null();
                        // A try-with-resources catches a `Throwable` to add a failing `close`
                        // to the body's exception as suppressed (see `with_resource`).
                        if v["resources"].as_array().is_some_and(|r| !r.is_empty()) {
                            self.finally = true;
                            self.members.insert(suppress_member());
                        }
                        for catch in v["catches"].as_array().into_iter().flatten() {
                            let ty = &catch["parameter"]["type"];
                            for t in
                                ty["typeAlternatives"].as_array().map_or(vec![ty], |a| a.iter().collect())
                            {
                                self.catches.insert(text(t, "javaType"));
                            }
                        }
                    }
                    "METHOD_INVOCATION" => {
                        self.operations.insert(kind.clone());
                        self.operations.insert(format!("{owner}#{}", text(symbol, "name")));
                        self.operations.insert(format!("{owner}#"));
                        // `E.valueOf(String)` of an enum `E` of the program.
                        if text(symbol, "name") == "valueOf"
                            && text(symbol, "type") == format!("(java.lang.String){owner}")
                            && !owner.starts_with("java.")
                        {
                            self.operations.insert("enum#valueOf".to_string());
                        }
                    }
                    _ => _ = self.operations.insert(kind),
                }
                match symbol["kind"].as_str() {
                    Some("METHOD" | "CONSTRUCTOR") if is_standard(&owner) => {
                        self.members.insert((owner.clone(), text(symbol, "name"), text(symbol, "type")));
                        if map.get("kind").and_then(Value::as_str) == Some("NEW_CLASS") {
                            self.instantiated.insert(owner);
                        }
                    }
                    Some("FIELD") if is_standard(&owner) => {
                        self.fields.insert((owner, text(symbol, "name")));
                    }
                    Some("CLASS") if is_standard(&text(symbol, "type")) => {
                        self.classes.insert(text(symbol, "type"));
                    }
                    _ => {}
                }
                // A method of the program that overrides a standard one: the standard one must
                // be there for calls on it to dispatch to the override.
                if map.get("kind").and_then(Value::as_str) == Some("METHOD") {
                    for o in symbol["overrides"].as_array().into_iter().flatten() {
                        let o_owner = text(o, "owner");
                        if is_standard(&o_owner) {
                            self.members.insert((o_owner, text(symbol, "name"), text(o, "type")));
                        }
                    }
                }
                map.values().for_each(|x| self.scan(x));
            }
            _ => {}
        }
    }
}

/// Declares the classes of `RAISED` that the runtime may raise where a `catch` or a `finally`
/// might see them, and records them in `raised`.
fn raise_classes(uses: &mut Uses, raised: &mut HashSet<String>) {
    let interrupted = if uses.operations.contains(INTERRUPTS) { RAISED_WHEN_INTERRUPTED } else { &[] };
    // An uncaught-exception handler sees every failure that nothing else catches (see
    // `Native::InstallUncaught`).
    let handled = uses.members.iter().any(|m| m.0 == "java.lang.Thread" && m.1 == "installUncaught0");
    for (class, operations) in RAISED.iter().chain(interrupted) {
        let caught = handled
            || uses.finally
            || uses.catches.iter().any(|c| is_standard(c) && standard_subclass(class, c));
        if caught && operations.iter().any(|o| uses.operations.contains(*o)) {
            uses.classes.insert(class.to_string());
            uses.members.insert((class.to_string(), "<init>".into(), "(java.lang.String)void".into()));
            if BARE.contains(class) {
                uses.members.insert((class.to_string(), "<init>".into(), "()void".into()));
            }
            uses.instantiated.insert(class.to_string());
            raised.insert(class.to_string());
        }
    }
}

impl Standard {
    /// What the code in these trees uses of the standard classes, with what that uses in turn.
    pub fn select<'v>(trees: impl IntoIterator<Item = &'v Value>) -> Standard {
        let mut uses = Uses::default();
        for tree in trees {
            uses.scan(tree);
        }
        let mut raised = HashSet::new();
        raise_classes(&mut uses, &mut raised);
        linkage_classes(&mut uses);
        if uses.classes.is_empty() && uses.members.is_empty() {
            return Standard::default();
        }
        // `toString` is how an uncaught exception is printed.
        let throwable = |c: &String| standard_subclass(c, "java.lang.Throwable");
        if uses.classes.iter().any(throwable) || uses.members.iter().any(|m| throwable(&m.0)) {
            uses.members.insert((
                "java.lang.Throwable".into(),
                "toString".into(),
                "()java.lang.String".into(),
            ));
        }
        let decls: Vec<(String, &'static Value)> = template().iter().map(|d| (qualified(d), d)).collect();
        let decl = template_type;
        let mut scanned: HashSet<(String, String, String)> = HashSet::new();
        let mut scanned_fields: HashSet<(String, String)> = HashSet::new();
        let text = |v: &Value, field: &str| v[field].as_str().unwrap_or_default().to_string();
        loop {
            // The operations of the standard members scanned so far may raise (`addSuppressed`
            // indexes an array), so the classes they raise are decided again on every round.
            raise_classes(&mut uses, &mut raised);
            // The checks of `addSuppressed` throw these (see `lower/suppress.rs`).
            if uses.members.contains(&suppress_member()) {
                for class in super::suppress::CLASSES {
                    uses.classes.insert(class.into());
                    uses.members.insert((class.into(), "<init>".into(), "(java.lang.String)void".into()));
                    uses.instantiated.insert(class.into());
                }
            }
            let before = (uses.classes.len(), uses.members.len(), uses.fields.len(), uses.operations.len());
            let owners: Vec<String> = uses
                .members
                .iter()
                .map(|m| m.0.clone())
                .chain(uses.fields.iter().map(|f| f.0.clone()))
                .collect();
            uses.classes.extend(owners);
            for class in uses.classes.clone() {
                let Some(d) = decl(&class) else { continue };
                if let Some(s) = d["extendsClause"]["javaType"].as_str() {
                    uses.classes.insert(s.to_string());
                }
                let interfaces = d["implementsClause"].as_array().into_iter().flatten();
                uses.classes.extend(interfaces.filter_map(|i| i["javaType"].as_str()).map(str::to_string));
                // A class declared in another is declared with it.
                if let Some((outer, _)) = class.rsplit_once('.')
                    && decl(outer).is_some()
                {
                    uses.classes.insert(outer.to_string());
                }
                // Only a class whose objects the program may create runs its methods, and only an
                // interface that the program uses is implemented: a field's type alone is neither.
                let interface = d["kind"] == "INTERFACE";
                let live = uses.members.iter().any(|m| m.0 == class && (interface || m.1 == "<init>"));
                let methods = d["members"].as_array().into_iter().flatten().filter(|m| m["kind"] == "METHOD");
                for m in methods.filter(|_| live) {
                    let member = (class.clone(), text(m, "name"), text(&m["symbol"], "type"));
                    // An interface's abstract methods, which its lambdas implement.
                    let is_abstract = interface && m["body"].is_null();
                    // A method that implements an interface's method that is used, which a call of
                    // that one may run (`ThreadGroup.uncaughtException`).
                    let overrides = m["symbol"]["overrides"].as_array().into_iter().flatten().any(|o| {
                        let owner = text(o, "owner");
                        decl(&owner).is_some_and(|i| i["kind"] == "INTERFACE")
                            && uses.members.contains(&(owner, member.1.clone(), text(o, "type")))
                    });
                    if is_abstract || overrides {
                        uses.members.insert(member);
                    }
                }
            }
            // The thread that `start0` starts runs the thread's `run()`.
            if uses.members.iter().any(|m| m.0 == "java.lang.Thread" && m.1 == "start0") {
                uses.members.insert(("java.lang.Thread".into(), "run".into(), "()void".into()));
            }
            // The runtime runs `dispatchUncaught()` once a handler is set (see `lower/natives.rs`).
            if uses.members.iter().any(|m| m.0 == "java.lang.Thread" && m.1 == "installUncaught0") {
                uses.members.insert(("java.lang.Thread".into(), "dispatchUncaught".into(), "()void".into()));
            }
            // A thread or thread group prints as its `toString`, which string conversion calls
            // unseen.
            for printed in ["java.lang.Thread", "java.lang.ThreadGroup"] {
                if uses.classes.contains(printed) {
                    uses.members.insert((printed.into(), "toString".into(), "()java.lang.String".into()));
                }
            }
            for member in uses.members.clone() {
                if !scanned.insert(member.clone()) {
                    continue;
                }
                let Some(d) = decl(&member.0) else { continue };
                let node = d["members"].as_array().into_iter().flatten().find(|m| {
                    m["kind"] == "METHOD"
                        && m["name"].as_str() == Some(member.1.as_str())
                        && m["symbol"]["type"].as_str() == Some(member.2.as_str())
                });
                if let Some(node) = node {
                    uses.scan(node);
                }
            }
            // A field's initializer runs with its class's initialization (`ThreadGroup.MAIN`).
            for field in uses.fields.clone() {
                if !scanned_fields.insert(field.clone()) {
                    continue;
                }
                let Some(d) = decl(&field.0) else { continue };
                let node = d["members"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|m| m["kind"] == "VARIABLE" && m["name"].as_str() == Some(field.1.as_str()));
                if let Some(node) = node {
                    uses.scan(node);
                }
            }
            if before == (uses.classes.len(), uses.members.len(), uses.fields.len(), uses.operations.len()) {
                break;
            }
        }
        let classes: Vec<(&'static str, &'static Value)> = decls
            .iter()
            .filter(|(name, _)| uses.classes.contains(name))
            .map(|(name, d)| (package_of(name.rsplit('.').next().unwrap_or_default()), *d))
            .collect();
        Standard {
            names: uses.classes.iter().filter(|c| is_standard(c)).cloned().collect(),
            classes,
            members: uses.members,
            fields: uses.fields,
            instantiated: uses.instantiated,
            raised,
        }
    }

    /// Whether `class` is a standard class that the program uses.
    pub fn has(&self, class: &str) -> bool {
        self.names.contains(class)
    }

    /// Whether a member of the standard class `class` is lowered: the program uses it.
    pub fn keeps(&self, member: Node, class: &str) -> bool {
        let name = member.str("name").to_string();
        match member.kind() {
            "METHOD" => {
                let sig = member.symbol().map_or("", |s| s.ty).to_string();
                self.members.contains(&(class.to_string(), name, sig))
            }
            "VARIABLE" => self.fields.contains(&(class.to_string(), name)),
            "CLASS" | "INTERFACE" | "ENUM" | "RECORD" => {
                self.names.contains(&format!("{class}.{}", member.str("simpleName")))
            }
            _ => false,
        }
    }
}

impl<'t> Lowering<'t> {
    /// `new C(message)` for each class `C` that the runtime may raise where the program could
    /// catch it (see `Class::raised`).
    pub(super) fn raised_objects(&self, types: &[TypeDecl], methods: &[Method]) -> Vec<Expr> {
        let mut out = Vec::new();
        for (t, decl) in types.iter().enumerate() {
            if !self.standard.raised.contains(&decl.name) {
                continue;
            }
            let ctor = methods.iter().position(|m| {
                m.is_ctor && m.class == t && m.params.len() == 1 && m.params[0].ty == Type::String
            });
            let Some(ctor) = ctor else { continue };
            let expr = |kind: ExprKind, ty: Type| {
                let id = self.next_id.get();
                self.next_id.set(id + 1);
                Expr { kind, pos: Pos::default(), ty, id }
            };
            // Without a message first: the generator matches it when the message is null.
            let bare = methods.iter().position(|m| m.is_ctor && m.class == t && m.params.is_empty());
            if BARE.contains(&decl.name.as_str())
                && let Some(bare) = bare
            {
                out.push(expr(
                    ExprKind::New { class: t, args: Vec::new(), ctor: Some(bare) },
                    Type::class(&decl.name),
                ));
            }
            let message = expr(ExprKind::Var("raisedMessage".to_string()), Type::String);
            out.push(expr(
                ExprKind::New { class: t, args: vec![message], ctor: Some(ctor) },
                Type::class(&decl.name),
            ));
        }
        out
    }

    /// Declares the standard classes that the program uses (see `Standard`).
    pub(super) fn declare_standard(&mut self) -> Result<(), Error> {
        // A standard class is an item of the crate's root named by its simple name, which a class
        // of the program in the unnamed package must not have.
        for (_, decl) in &self.standard.classes {
            let simple = Node(decl).str("simpleName");
            if let Some(clash) = self.types.iter().find(|t| t.name == simple) {
                return unsupported(clash.node, &format!("a class named like the standard class `{simple}`"));
            }
        }
        for (package, decl) in self.standard.classes.clone() {
            self.declare_type(Node(decl), Some(package))?;
        }
        Ok(())
    }

    /// `throw value;`: the value as a `Throwable`.
    pub(super) fn throw(&self, node: Node<'t>) -> Result<Stmt, Error> {
        let value = self.expr(node.child("expression")?)?;
        let Some(throwable) = self.type_index("java.lang.Throwable") else {
            return unsupported(node, "this `throw`");
        };
        let ty = Type::class(&self.types[throwable].name);
        Ok(Stmt::Throw(self.convert(value, ty)))
    }

    /// `try { .. } catch (..) { .. } finally { .. }`, and try-with-resources (`resources`).
    pub(super) fn try_stmt(&self, node: Node<'t>) -> Result<Stmt, Error> {
        let mut body = self.block(node.child("block")?)?;
        let resources: Vec<Node<'t>> = node.list("resources").collect();
        for &resource in resources.iter().rev() {
            body = self.with_resource(resource, body)?;
        }
        let catches: Vec<Catch> =
            node.list("catches").filter_map(|c| self.catch(c).transpose()).collect::<Result<_, _>>()?;
        let finally = node.get("finallyBlock").map(|block| self.block(block)).transpose()?;
        if catches.is_empty() && finally.is_none() {
            return Ok(Stmt::Block(body));
        }
        Ok(Stmt::Try { body, catches, finally })
    }

    /// The statements `body` inside the resource `resource` of a try-with-resources: the
    /// resource's declaration, then the closing that Java does. The body's completion closes the
    /// resource, and a `close` that throws replaces it, unless the body threw: then the `close`'s
    /// exception is added to the body's as suppressed and the body's is thrown.
    ///
    /// ```text
    /// R r = new R(..);
    /// boolean r$failed = false;
    /// try {
    ///     try { body } catch (Throwable r$thrown) {
    ///         r$failed = true;
    ///         try { r.close(); } catch (Throwable r$suppressed) { r$thrown.addSuppressed(r$suppressed); }
    ///         throw r$thrown;
    ///     }
    /// } finally {
    ///     if (!r$failed) { r.close(); }
    /// }
    /// ```
    ///
    /// A resource that is a new object is not null, as Java requires for `close` to run. An
    /// existing variable (`try (r)`, `try (this.r)`) is `final var r$resource = r`; it, and a
    /// resource that any other expression gives (`try (Stream<R> s = mapper.apply(e))`), may be
    /// null, which is not closed, as Java skips it: the `close` calls then test the resource.
    fn with_resource(&self, resource: Node<'t>, body: Vec<Stmt>) -> Result<Vec<Stmt>, Error> {
        let existing = resource.kind() != "VARIABLE";
        let mut guard = existing;
        let mut declared = Vec::new();
        if existing {
            let name = resource.symbol().map_or("resource", |s| s.name);
            let value = self.expr(resource)?;
            let ty = self.ty(resource, resource.java_type())?;
            let value = self.convert(value, ty);
            let name = format!("{name}$resource");
            declared.push(Stmt::Local { ty, name, init: Some(value), pos: resource.pos() });
        } else {
            let Some(init) = resource.get("initializer") else {
                return unsupported(resource, "this resource");
            };
            guard = init.kind() != "NEW_CLASS";
            self.stmt_into(resource, &mut declared)?;
        }
        let [Stmt::Local { ty, name, pos, .. }] = &declared[..] else {
            return unsupported(resource, "this resource");
        };
        let (ty, name, pos) = (*ty, name.clone(), *pos);
        let class = match ty {
            Type::Class(class, _) => self.type_index(class),
            _ => None,
        };
        let Some(close) = class.and_then(|t| self.close_method(t)) else {
            return unsupported(resource, "a resource without a `close()` of the program");
        };
        let Some(throwable) = self.type_index("java.lang.Throwable") else {
            return unsupported(resource, "a try-with-resources without `Throwable`");
        };
        let Some(add) = self.methods.iter().position(|m| m.owner == throwable && m.name == "addSuppressed")
        else {
            return unsupported(resource, "a try-with-resources without `addSuppressed`");
        };
        let throwable_ty = Type::class("java.lang.Throwable");
        let failed = format!("{name}$failed");
        let thrown = format!("{name}$thrown");
        let suppressed = format!("{name}$suppressed");
        let add_call = ExprKind::MethodCall {
            receiver: Box::new(self.synth(ExprKind::Var(thrown.clone()), pos, throwable_ty)),
            name: "addSuppressed".to_string(),
            args: vec![self.synth(ExprKind::Var(suppressed.clone()), pos, throwable_ty)],
            resolved: Some(add),
            is_super: false,
        };
        let set_failed = ExprKind::Assign {
            op: None,
            target: Place::Var(failed.clone()),
            value: Box::new(self.synth(ExprKind::Bool(true), pos, Type::Boolean)),
        };
        let suppress = Catch {
            types: vec![throwable],
            name: suppressed,
            ty: throwable_ty,
            body: vec![Stmt::Expr(self.synth(add_call, pos, Type::Void))],
            pos,
        };
        let on_throw = vec![
            Stmt::Expr(self.synth(set_failed, pos, Type::Void)),
            Stmt::Try {
                body: vec![self.close_stmt(&name, ty, pos, close, guard)],
                catches: vec![suppress],
                finally: None,
            },
            Stmt::Throw(self.synth(ExprKind::Var(thrown.clone()), pos, throwable_ty)),
        ];
        let inner = Stmt::Try {
            body,
            catches: vec![Catch {
                types: vec![throwable],
                name: thrown,
                ty: throwable_ty,
                body: on_throw,
                pos,
            }],
            finally: None,
        };
        let not_failed = ExprKind::Unary(
            UnOp::Not,
            Box::new(self.synth(ExprKind::Var(failed.clone()), pos, Type::Boolean)),
        );
        let close_unless_failed = Stmt::If {
            cond: self.synth(not_failed, pos, Type::Boolean),
            then: Box::new(Stmt::Block(vec![self.close_stmt(&name, ty, pos, close, guard)])),
            otherwise: None,
        };
        declared.push(Stmt::Local {
            ty: Type::Boolean,
            name: failed,
            init: Some(self.synth(ExprKind::Bool(false), pos, Type::Boolean)),
            pos,
        });
        declared.push(Stmt::Try {
            body: vec![inner],
            catches: Vec::new(),
            finally: Some(vec![close_unless_failed]),
        });
        Ok(declared)
    }

    /// `name.close();` for the program's `close` method `close` of the resource's type `ty`; with
    /// `guard`, `if (name != null) name.close();`, for a resource that may be null.
    fn close_stmt(&self, name: &str, ty: Type, pos: Pos, close: usize, guard: bool) -> Stmt {
        let receiver = Box::new(self.synth(ExprKind::Var(name.to_string()), pos, ty));
        let call = ExprKind::MethodCall {
            receiver,
            name: "close".to_string(),
            args: Vec::new(),
            resolved: Some(close),
            is_super: false,
        };
        let close = Stmt::Expr(self.synth(call, pos, Type::Void));
        if !guard {
            return close;
        }
        let value = Box::new(self.synth(ExprKind::Var(name.to_string()), pos, ty));
        let null = Box::new(self.synth(ExprKind::Null, pos, Type::Null));
        let not_null = ExprKind::Binary(BinOp::Ne, value, null);
        Stmt::If {
            cond: self.synth(not_null, pos, Type::Boolean),
            then: Box::new(Stmt::Block(vec![close])),
            otherwise: None,
        }
    }

    /// The program's `close()` of type `t`, declared there or inherited from a superclass, or
    /// declared by an interface that it implements or extends (`AutoCloseable`'s, which a call
    /// dispatches to the implementation).
    fn close_method(&self, t: usize) -> Option<usize> {
        let declared = |c: usize| {
            self.methods.iter().position(|m| m.owner == c && m.name == "close" && m.sig == "()void")
        };
        let mut chain = Vec::new();
        let mut at = Some(t);
        while let Some(c) = at {
            if let Some(found) = declared(c) {
                return Some(found);
            }
            chain.push(c);
            at = self.superclass(c).ok().flatten();
        }
        let mut work: Vec<usize> =
            chain.iter().flat_map(|&c| self.interfaces(c).unwrap_or_default()).collect();
        let mut seen = Vec::new();
        while let Some(i) = work.pop() {
            if seen.contains(&i) {
                continue;
            }
            seen.push(i);
            if let Some(found) = declared(i) {
                return Some(found);
            }
            work.extend(self.interfaces(i).unwrap_or_default());
        }
        None
    }

    /// A `catch` clause, or `None` when every class it catches is one that a whole program does
    /// not translate (`program::untranslated`), such as reflection's
    /// `InvocationTargetException`: no translated code can throw one, so such an alternative is
    /// left out.
    fn catch(&self, node: Node<'t>) -> Result<Option<Catch>, Error> {
        let param = node.child("parameter")?;
        let ty_node = param.child("type")?;
        let mut names: Vec<&str> = if ty_node.kind() == "UNION_TYPE" {
            ty_node.list("typeAlternatives").map(|a| a.java_type()).collect()
        } else {
            vec![ty_node.java_type()]
        };
        if self.reach.is_some() {
            names.retain(|name| crate::program::untranslated(name).is_none());
            if names.is_empty() {
                return Ok(None);
            }
        }
        let mut types = Vec::new();
        for name in names {
            match self.type_index(name) {
                Some(t) => types.push(t),
                None => return unsupported(ty_node, &format!("catching `{name}`")),
            }
        }
        let ty = self.ty(param, param.symbol().map_or("", |s| s.ty))?;
        let body = self.block(node.child("block")?)?;
        Ok(Some(Catch { types, name: param.str("name").to_string(), ty, body, pos: node.pos() }))
    }

    /// `o.getClass().getName()` or `o.getClass().getSimpleName()`, as `Builtin::ClassName` of
    /// `o`; `None` for any other call.
    pub(super) fn class_name_call(
        &self,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
    ) -> Result<Option<ExprKind>, Error> {
        let simple = match (symbol.owner, symbol.name) {
            ("java.lang.Class", "getName") => false,
            ("java.lang.Class", "getSimpleName") => true,
            _ => return Ok(None),
        };
        let is_get_class = |r: &Node| {
            r.kind() == "METHOD_INVOCATION"
                && r.symbol().is_some_and(|s| s.owner == "java.lang.Object" && s.name == "getClass")
        };
        let Some(get_class) = receiver.filter(is_get_class) else { return Ok(None) };
        let select = get_class.child("methodSelect")?;
        let object = match select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT") {
            Some(o) => self.expr(o)?,
            None => self.this(get_class),
        };
        if !matches!(object.ty, Type::Class(..)) {
            return unsupported(get_class, "getClass() of this type");
        }
        Ok(Some(ExprKind::Builtin(Builtin::ClassName { simple }, vec![object])))
    }

    /// Rejects a constructor or method of a standard class that the translator does not have,
    /// such as `getStackTrace`.
    pub(super) fn check_standard_member(&self, node: Node, symbol: &Symbol) -> Result<(), Error> {
        if is_standard(symbol.owner) && self.method_index(symbol).is_none() {
            let simple = symbol.owner.rsplit('.').next().unwrap_or_default();
            let what = if symbol.name == "<init>" {
                format!("this constructor of `{simple}`")
            } else {
                format!("`{simple}.{}`", symbol.name)
            };
            return unsupported(node, &what);
        }
        Ok(())
    }
}
