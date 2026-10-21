//! Exceptions: `throw`, `try`/`catch`, and the standard exception classes.
//!
//! The standard classes (`Throwable`, `Exception`, `IllegalArgumentException`, ...) are not read
//! from the JDK: the translator has its own sources for them, `java/Standard.java`, written in
//! the subset it supports, whose syntax tree `build.rs` prints. A program gets the classes it
//! names, with their superclasses, and of those only the constructors and methods it uses
//! (with what they use in turn), as classes of the program: they are lowered, analysed and
//! generated like the program's own, so a program's exception class extends them like any
//! superclass. A standard class is abstract unless the program creates its objects, so that the
//! enum of `Throwable` has a variant only for the classes whose objects exist.
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
    ("IOException", "java.io"),
    ("FileNotFoundException", "java.io"),
];

/// The standard classes that the runtime raises and a translation can catch, each with the
/// kinds of javac's tree of the operations that may raise it (a method call by its owner and
/// name: `owner#name`, or `owner#` for any method of the owner).
const RAISED: &[(&str, &[&str])] = &[
    ("java.lang.ArithmeticException", &["DIVIDE", "REMAINDER", "DIVIDE_ASSIGNMENT", "REMAINDER_ASSIGNMENT"]),
    ("java.lang.ArrayIndexOutOfBoundsException", &["ARRAY_ACCESS"]),
    ("java.lang.StringIndexOutOfBoundsException", &["java.lang.String#charAt", "java.lang.String#substring"]),
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

/// The package of the standard class with this simple name.
fn package_of(simple: &str) -> &'static str {
    OTHER_PACKAGES.iter().find(|(s, _)| *s == simple).map_or("java.lang", |(_, p)| p)
}

/// The class declarations of `java/Standard.java`, renamed from the package `standard` to their
/// real packages, as javac names the real classes in a program's tree.
fn template() -> &'static [Value] {
    static DECLS: OnceLock<Vec<Value>> = OnceLock::new();
    DECLS.get_or_init(|| {
        let text = include_str!(concat!(env!("OUT_DIR"), "/standard.json"));
        let mut tree: Value = serde_json::from_str(text).expect("build.rs writes the tree of Standard.java");
        rename(&mut tree);
        match tree["units"][0]["tree"]["typeDecls"].take() {
            Value::Array(decls) => decls,
            _ => panic!("the tree of Standard.java has its classes"),
        }
    })
}

/// Gives the classes of the package `standard` their real packages, in every name and type.
fn rename(v: &mut Value) {
    match v {
        Value::String(s) => {
            while let Some(at) = s.find("standard.") {
                let rest = &s[at + "standard.".len()..];
                let end = rest.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(rest.len());
                let simple = rest[..end].to_string();
                s.replace_range(at..at + "standard".len(), package_of(&simple));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(rename),
        Value::Object(map) => {
            // A class's symbol names its package as its owner.
            if map.get("kind").and_then(Value::as_str) == Some("CLASS")
                && map.get("owner").and_then(Value::as_str) == Some("standard")
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

/// The qualified name of a class declaration of the template.
fn qualified(decl: &Value) -> String {
    let simple = decl["simpleName"].as_str().unwrap_or_default();
    format!("{}.{simple}", package_of(simple))
}

/// Whether `name` is the qualified name of a standard class the translator provides.
pub fn is_standard(name: &str) -> bool {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| template().iter().map(qualified).collect()).contains(name)
}

/// Whether the standard class `name` is the standard class `ancestor` or a subclass of it.
pub fn standard_subclass(name: &str, ancestor: &str) -> bool {
    let mut at = Some(name.to_string());
    while let Some(c) = at {
        if c == ancestor {
            return true;
        }
        at = template()
            .iter()
            .find(|d| qualified(d) == c)
            .and_then(|d| d["extendsClause"]["javaType"].as_str())
            .map(str::to_string);
    }
    false
}

/// The qualified names of every standard class the translator provides.
pub fn standard_names() -> Vec<String> {
    template().iter().map(qualified).collect()
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
                        self.operations.insert(format!("{owner}#{}", text(symbol, "name")));
                        self.operations.insert(format!("{owner}#"));
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

impl Standard {
    /// What the code in these trees uses of the standard classes, with what that uses in turn.
    pub fn select<'v>(trees: impl IntoIterator<Item = &'v Value>) -> Standard {
        let mut uses = Uses::default();
        for tree in trees {
            uses.scan(tree);
        }
        // The classes the runtime raises where a `catch` or a `finally` might see them.
        let mut raised = HashSet::new();
        for (class, operations) in RAISED {
            let caught =
                uses.finally || uses.catches.iter().any(|c| is_standard(c) && standard_subclass(class, c));
            if caught && operations.iter().any(|o| uses.operations.contains(*o)) {
                uses.classes.insert(class.to_string());
                uses.members.insert((class.to_string(), "<init>".into(), "(java.lang.String)void".into()));
                uses.instantiated.insert(class.to_string());
                raised.insert(class.to_string());
            }
        }
        if uses.classes.is_empty() && uses.members.is_empty() {
            return Standard::default();
        }
        // `toString` is how an uncaught exception is printed.
        uses.members.insert(("java.lang.Throwable".into(), "toString".into(), "()java.lang.String".into()));
        let decls: Vec<(String, &'static Value)> = template().iter().map(|d| (qualified(d), d)).collect();
        let decl = |name: &str| decls.iter().find(|(n, _)| n == name).map(|(_, d)| *d);
        let mut scanned: HashSet<(String, String, String)> = HashSet::new();
        loop {
            let before = (uses.classes.len(), uses.members.len(), uses.fields.len());
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
            if before == (uses.classes.len(), uses.members.len(), uses.fields.len()) {
                break;
            }
        }
        let classes: Vec<(&'static str, &'static Value)> = decls
            .iter()
            .filter(|(name, _)| uses.classes.contains(name))
            .map(|(name, d)| (package_of(name.rsplit('.').next().unwrap_or_default()), *d))
            .collect();
        Standard {
            names: classes.iter().map(|(_, d)| qualified(d)).collect(),
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
        let catches: Vec<Catch> = node.list("catches").map(|c| self.catch(c)).collect::<Result<_, _>>()?;
        let finally = node.get("finallyBlock").map(|block| self.block(block)).transpose()?;
        if !resources.is_empty() && catches.is_empty() && finally.is_none() {
            return Ok(Stmt::Block(body));
        }
        Ok(Stmt::Try { body, catches, finally })
    }

    /// The statements `body` inside the resource `resource` of a try-with-resources: the
    /// resource's declaration, then `try { body } finally { r.close(); }`. That is what Java does
    /// when `r` is not null and its `close` cannot fail, which the resource being a new object
    /// and the analysis's check of `Class::closes` make sure of: otherwise Java would skip the
    /// `close` of a null and add the exception of a `close` to the body's as suppressed.
    fn with_resource(&self, resource: Node<'t>, body: Vec<Stmt>) -> Result<Vec<Stmt>, Error> {
        let Some(init) = resource.get("initializer").filter(|_| resource.kind() == "VARIABLE") else {
            return unsupported(resource, "try-with-resources over an existing variable");
        };
        if init.kind() != "NEW_CLASS" {
            return unsupported(init, "a resource that is not a new object");
        }
        let mut declared = Vec::new();
        self.stmt_into(resource, &mut declared)?;
        let [Stmt::Local { ty, name, pos, .. }] = &declared[..] else {
            return unsupported(resource, "this resource");
        };
        let (ty, name, pos) = (*ty, name.clone(), *pos);
        let class = match ty {
            Type::Class(class) => self.type_index(class),
            _ => None,
        };
        let Some(close) = class.and_then(|t| self.close_method(t)) else {
            return unsupported(resource, "a resource without a `close()` of the program");
        };
        self.closes.borrow_mut().push((close, pos));
        let receiver = Box::new(self.synth(ExprKind::Var(name), pos, ty));
        let call = ExprKind::MethodCall {
            receiver,
            name: "close".to_string(),
            args: Vec::new(),
            resolved: Some(close),
            is_super: false,
        };
        let close = Stmt::Expr(self.synth(call, pos, Type::Void));
        declared.push(Stmt::Try { body, catches: Vec::new(), finally: Some(vec![close]) });
        Ok(declared)
    }

    /// The program's `close()` of type `t`, declared there or inherited from a superclass.
    fn close_method(&self, t: usize) -> Option<usize> {
        let mut at = Some(t);
        while let Some(c) = at {
            let found =
                self.methods.iter().position(|m| m.owner == c && m.name == "close" && m.sig == "()void");
            if found.is_some() {
                return found;
            }
            at = self.superclass(c).ok().flatten();
        }
        None
    }

    fn catch(&self, node: Node<'t>) -> Result<Catch, Error> {
        let param = node.child("parameter")?;
        let ty_node = param.child("type")?;
        let names: Vec<&str> = if ty_node.kind() == "UNION_TYPE" {
            ty_node.list("typeAlternatives").map(|a| a.java_type()).collect()
        } else {
            vec![ty_node.java_type()]
        };
        let mut types = Vec::new();
        for name in names {
            match self.type_index(name) {
                Some(t) => types.push(t),
                None => return unsupported(ty_node, &format!("catching `{name}`")),
            }
        }
        let ty = self.ty(param, param.symbol().map_or("", |s| s.ty))?;
        let body = self.block(node.child("block")?)?;
        Ok(Catch { types, name: param.str("name").to_string(), ty, body, pos: node.pos() })
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
        if !matches!(object.ty, Type::Class(_)) {
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
