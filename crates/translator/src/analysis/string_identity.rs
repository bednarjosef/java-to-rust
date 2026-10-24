//! `a == b` of two strings in the program's own code (`Builtin::StringSame`), which compares
//! references: where each side can come from, and so what decides it.
//!
//! The analysis follows strings backwards from each comparison through the places that can hold
//! them: variables, parameters (from the arguments of every call that can run the method),
//! return values (per override group), fields, statics and array elements (all arrays of
//! strings and objects together), and through `Object` and the other types a string is a value
//! of. Where a string is made, it is one of three kinds, as Java makes strings:
//!
//! - Interned: a literal, a constant expression, `intern()`, an enum constant's name, a class's
//!   name. Java keeps one object for each text of these, so two of them are the same object
//!   exactly when their texts are equal.
//! - New: `new String(..)`, a concatenation that is not a constant, `StringBuilder.toString`,
//!   `String.format` and the other library methods that always make a new string.
//! - The receiver itself, or a new or interned string: the `String` methods that the JDK makes
//!   return the string itself under a condition (`substring` of all of it, `replace` with no
//!   match, `trim` with nothing to trim, `concat("")`, `repeat(1)`, `toString()`, `split` with
//!   no match), or an interned `""`.
//! - The text of an object (`String.valueOf(o)`, `Object.toString`): the string itself, the
//!   literal `"null"`, or what a `toString` of the program returns.
//!
//! A string that Java keeps for a key (a system property, `System.lineSeparator()`, an
//! environment variable, a class's simple or canonical name) is the same object for the same key
//! and never a literal, so it counts as new. What the analysis cannot place (a library method's
//! string that it does not know, a parameter of a method the runtime calls) is a source of its
//! own, named, and a comparison that can see one is rejected with the source named.
//!
//! A comparison of two interned strings compares their texts. One where no string can be on both
//! sides (an interned string and a new one, or a side that is only null) is true only when both
//! are null. Any other is decided by the objects at run time: the program's strings are then
//! shared (`Class::shared_strings`), each made as Java makes it (`codegen::shared_strings`).

use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// A node of the flow: a place that holds strings, or a kind of source.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Key {
    Var(usize, String),
    /// The return values of an override group.
    Return(usize),
    Field(usize, String),
    Static(usize, String),
    /// The elements of every `String[]` (`true`), or of every other array whose elements may be
    /// strings (`Object[]`, `T[]`). A `String[]` used as an `Object[]` is an `Object[]` throughout
    /// (`lower::covariant`); the builtins that copy between the two connect them.
    Elements(bool),
    Interned,
    New,
    /// What the analysis does not follow, by what it is.
    Unknown(String),
}

/// The types whose values may be strings: `String`, the types above it, the type variables of
/// Rust generics, and types the program does not declare.
const ABOVE_STRING: [&str; 13] = [
    OBJECT,
    COMPARABLE,
    CHAR_SEQUENCE,
    "Serializable",
    "Constable",
    "ConstantDesc",
    "java.lang.Object",
    "java.lang.Comparable",
    "java.lang.CharSequence",
    "java.io.Serializable",
    "java.lang.constant.Constable",
    "java.lang.constant.ConstantDesc",
    "java.lang.String",
];

struct Graph<'c> {
    class: &'c Class,
    ids: HashMap<Key, usize>,
    keys: Vec<Key>,
    /// The nodes whose strings flow into each node.
    preds: Vec<Vec<usize>>,
    /// The sites of each unknown source, for its message.
    unknown_sites: HashMap<usize, Vec<Pos>>,
    /// The override groups of every `toString()`, whose results `String.valueOf(o)` may be.
    to_strings: Vec<usize>,
}

impl<'c> Graph<'c> {
    fn node(&mut self, key: Key) -> usize {
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        let id = self.keys.len();
        self.keys.push(key.clone());
        self.ids.insert(key, id);
        self.preds.push(Vec::new());
        id
    }

    fn unknown(&mut self, what: String, pos: Pos) -> usize {
        let id = self.node(Key::Unknown(what));
        let sites = self.unknown_sites.entry(id).or_default();
        if !sites.contains(&pos) {
            sites.push(pos);
        }
        id
    }

    fn flow(&mut self, from: &[usize], to: usize) {
        for &f in from {
            if f != to && !self.preds[to].contains(&f) {
                self.preds[to].push(f);
            }
        }
    }

    /// Whether values of type `ty` may be strings.
    fn holds(&self, ty: Type) -> bool {
        match ty {
            Type::String => true,
            Type::Class(name, _) => match self.class.type_index(name) {
                Some(t) => {
                    let decl = &self.class.types[t];
                    decl.kind == TypeKind::Param || ABOVE_STRING.contains(&decl.name.as_str())
                }
                None => true,
            },
            _ => false,
        }
    }

    /// The node of the elements of arrays of type `array`, if they may be strings.
    fn elements(&mut self, array: Type) -> Option<usize> {
        let element = array.element();
        self.holds(element).then(|| self.node(Key::Elements(element == Type::String)))
    }

    /// The nodes of the innermost elements of arrays of type `ty`, at any depth, if they may be
    /// strings.
    fn innermost(&mut self, mut ty: Type) -> Option<usize> {
        if !matches!(ty, Type::Array(_)) {
            return None;
        }
        while let Type::Array(element) = ty {
            ty = *element;
        }
        self.holds(ty).then(|| self.node(Key::Elements(ty == Type::String)))
    }

    fn field_nodes(&mut self, object: &Expr, name: &str) -> Vec<usize> {
        match self.class.field_owner(object.ty, name) {
            Some(owner) => vec![self.node(Key::Field(owner, name.to_string()))],
            None => {
                let owners: Vec<usize> = (0..self.class.types.len())
                    .filter(|&t| self.class.types[t].fields.iter().any(|f| f.name == name))
                    .collect();
                owners.into_iter().map(|t| self.node(Key::Field(t, name.to_string()))).collect()
            }
        }
    }

    /// The type that declares the static `name` that `Class.name` reads, `c` or above it.
    fn static_owner(&self, c: usize, name: &str) -> usize {
        let mut work = vec![c];
        let mut seen = HashSet::new();
        while let Some(t) = work.pop() {
            if !seen.insert(t) {
                continue;
            }
            let decl = &self.class.types[t];
            if decl.statics.iter().any(|f| f.name == name) {
                return t;
            }
            work.extend(decl.extends.iter().chain(&decl.implements));
        }
        c
    }

    fn place_nodes(&mut self, m: usize, place: &Place) -> Vec<usize> {
        match place {
            Place::Var(name) => vec![self.node(Key::Var(m, name.clone()))],
            Place::Field { object, name } => self.field_nodes(object, name),
            Place::Static { class, name } => {
                let owner = self.static_owner(*class, name);
                vec![self.node(Key::Static(owner, name.clone()))]
            }
            Place::Index(array, _) => self.elements(array.ty).into_iter().collect(),
        }
    }

    /// The nodes that the string `e`, in method `m`, comes from.
    fn origins(&mut self, m: usize, e: &Expr) -> Vec<usize> {
        if !self.holds(e.ty) {
            return Vec::new();
        }
        match &e.kind {
            ExprKind::Str(_) | ExprKind::Units(_) => vec![self.node(Key::Interned)],
            ExprKind::Null | ExprKind::This | ExprKind::New { .. } => Vec::new(),
            ExprKind::Var(name) => vec![self.node(Key::Var(m, name.clone()))],
            ExprKind::Field { object, name } => self.field_nodes(object, name),
            ExprKind::Static { class, name } => {
                let owner = self.static_owner(*class, name);
                vec![self.node(Key::Static(owner, name.clone()))]
            }
            ExprKind::Index(array, _) => self.elements(array.ty).into_iter().collect(),
            ExprKind::Assign { op: None, value, .. } | ExprKind::Cast(_, value) => self.origins(m, value),
            ExprKind::Assign { op: Some(_), .. } => vec![self.node(Key::New)],
            ExprKind::Conditional(_, a, b) => {
                let mut nodes = self.origins(m, a);
                nodes.extend(self.origins(m, b));
                nodes
            }
            ExprKind::Switch { cases, .. } => {
                let mut yields = Vec::new();
                for s in cases.iter().flat_map(|c| &c.body) {
                    visit_stmt_outside_switches(s, &mut |s| {
                        if let Stmt::Yield(y) = s {
                            yields.push(y);
                        }
                    });
                }
                yields.into_iter().flat_map(|y| self.origins(m, y)).collect()
            }
            ExprKind::Binary(..) if e.ty == Type::String => match constant(self.class, e) {
                true => vec![self.node(Key::Interned)],
                false => vec![self.node(Key::New)],
            },
            ExprKind::Call { resolved: Some(_), .. } | ExprKind::MethodCall { resolved: Some(_), .. } => {
                let callees = self.class.callees(e);
                callees
                    .into_iter()
                    .map(|c| self.node(Key::Return(self.class.hierarchy.group_id(c))))
                    .collect()
            }
            ExprKind::Builtin(builtin, args) => self.builtin(m, e, *builtin, args),
            _ => vec![self.unknown(format!("{:?}", std::mem::discriminant(&e.kind)), e.pos)],
        }
    }

    /// The nodes that the string a builtin returns comes from.
    fn builtin(&mut self, m: usize, e: &Expr, builtin: Builtin, args: &[Expr]) -> Vec<usize> {
        let interned = self.node(Key::Interned);
        let new = self.node(Key::New);
        match made(builtin) {
            Made::Interned => vec![interned],
            Made::New | Made::PerKey => vec![new],
            Made::NewOrEmpty | Made::ArrayText | Made::TypeName => vec![interned, new],
            Made::Itself => args.last().map(|a| self.origins(m, a)).unwrap_or_default(),
            Made::Kept => {
                let mut nodes = self.origins(m, &args[0]);
                nodes.extend([interned, new]);
                nodes
            }
            // `String.valueOf(o)`: the string itself, `"null"`, or what `o.toString()` returns.
            Made::Text => {
                let x = value_of_operand(builtin, args);
                let mut nodes = self.origins(m, x);
                nodes.push(interned);
                if x.ty != Type::String {
                    nodes.push(new);
                    for group in self.to_strings.clone() {
                        nodes.push(self.node(Key::Return(group)));
                    }
                }
                nodes
            }
            Made::Never => Vec::new(),
            Made::Element => vec![self.node(Key::Elements(false)), self.node(Key::Elements(true))],
            Made::Unknown => vec![self.unknown(format!("{builtin:?}"), e.pos)],
        }
    }

    /// The flows that the expression `x` itself makes, in method `m`, not counting those of the
    /// expressions in it.
    fn stores(&mut self, m: usize, x: &Expr) {
        match &x.kind {
            ExprKind::Assign { op, target, value } => {
                if !self.holds(x.ty) && !self.holds(value.ty) {
                    return;
                }
                let from = match op {
                    None => self.origins(m, value),
                    Some(_) if x.ty == Type::String => vec![self.node(Key::New)],
                    Some(_) => return,
                };
                for to in self.place_nodes(m, target) {
                    self.flow(&from, to);
                }
            }
            ExprKind::ArrayLit(items) if self.holds(x.ty.element()) => {
                let elements = self.elements(x.ty).expect("elements that hold strings");
                for item in items {
                    let from = self.origins(m, item);
                    self.flow(&from, elements);
                }
            }
            ExprKind::InstanceOf { value, binding: Some(name), .. } => {
                let from = self.origins(m, value);
                let to = self.node(Key::Var(m, name.clone()));
                self.flow(&from, to);
            }
            ExprKind::Call { args, .. }
            | ExprKind::New { args, .. }
            | ExprKind::ThisCall { args, .. }
            | ExprKind::SuperInit { args, .. }
            | ExprKind::MethodCall { args, .. } => {
                let class = self.class;
                let callees = class.callees(x);
                let froms: Vec<Vec<usize>> = args.iter().map(|a| self.origins(m, a)).collect();
                for c in callees {
                    for (p, from) in class.methods[c].params.iter().zip(&froms) {
                        if self.holds(p.ty) {
                            let to = self.node(Key::Var(c, p.name.clone()));
                            self.flow(from, to);
                        }
                    }
                }
            }
            ExprKind::Builtin(builtin, args) => {
                // The elements of the arrays that a builtin makes.
                if let Some(elements) = self.innermost(x.ty) {
                    let from = match builtin {
                        Builtin::String(StringOp::Split | StringOp::Lines) => {
                            let mut nodes = self.origins(m, &args[0]);
                            nodes.extend([self.node(Key::Interned), self.node(Key::New)]);
                            nodes
                        }
                        Builtin::MainArgs => vec![self.node(Key::New)],
                        Builtin::StringArrayOf
                        | Builtin::ObjectArrayOf
                        | Builtin::ArraysCopyOf
                        | Builtin::Arrays(_)
                        | Builtin::ObjectClone
                        | Builtin::NullChecked(_) => Vec::new(),
                        _ => vec![self.unknown(format!("the elements of {builtin:?}"), x.pos)],
                    };
                    self.flow(&from, elements);
                }
                // A builtin may store its arguments, or the elements of the arrays it is given,
                // into the arrays it is given or makes.
                let arrays: Vec<usize> =
                    args.iter().map(|a| a.ty).chain([x.ty]).filter_map(|ty| self.innermost(ty)).collect();
                if !arrays.is_empty() {
                    let mut from: Vec<usize> = args.iter().flat_map(|a| self.origins(m, a)).collect();
                    from.extend(&arrays);
                    for &to in &arrays {
                        self.flow(&from, to);
                    }
                }
            }
            _ => {}
        }
    }

    /// The flows of method `m`'s statements.
    fn method(&mut self, m: usize) {
        let class = self.class;
        let method = &class.methods[m];
        let group = class.hierarchy.group_id(m);
        for s in &method.body {
            let mut exprs = Vec::new();
            let mut stmts = Vec::new();
            visit_stmt(s, &mut |e| exprs.push(e), &mut |s| stmts.push(s));
            for e in exprs {
                visit_expr(e, &mut |x| self.stores(m, x));
            }
            for s in stmts {
                match s {
                    Stmt::Local { ty, name, init: Some(init), .. } if self.holds(*ty) => {
                        let from = self.origins(m, init);
                        let to = self.node(Key::Var(m, name.clone()));
                        self.flow(&from, to);
                    }
                    Stmt::ForEach { ty, name, array, .. } if self.holds(*ty) => {
                        let from: Vec<usize> = self.elements(array.ty).into_iter().collect();
                        let to = self.node(Key::Var(m, name.clone()));
                        self.flow(&from, to);
                    }
                    Stmt::Return(Some(e), _) if self.holds(method.ret) => {
                        let from = self.origins(m, e);
                        let to = self.node(Key::Return(group));
                        self.flow(&from, to);
                    }
                    _ => {}
                }
            }
        }
    }

    /// The flows of the code outside methods: field initializers and enum constants.
    fn outside_methods(&mut self) {
        let class = self.class;
        for (t, decl) in class.types.iter().enumerate() {
            for (f, is_static) in
                decl.fields.iter().map(|f| (f, false)).chain(decl.statics.iter().map(|f| (f, true)))
            {
                let Some(init) = &f.init else { continue };
                visit_expr(init, &mut |x| self.stores(usize::MAX, x));
                if self.holds(f.ty) {
                    let from = self.origins(usize::MAX, init);
                    let to = match is_static {
                        true => self.node(Key::Static(t, f.name.clone())),
                        false => self.node(Key::Field(t, f.name.clone())),
                    };
                    self.flow(&from, to);
                }
            }
            // A local or anonymous class or a lambda holds the locals it captures in fields.
            let enclosing = match &decl.origin {
                Origin::Local { method, .. }
                | Origin::Anonymous { method, .. }
                | Origin::Lambda { method, .. } => method.unwrap_or(usize::MAX),
                _ => usize::MAX,
            };
            for capture in &decl.captures {
                if !self.holds(capture.ty) {
                    continue;
                }
                let from = vec![self.node(Key::Var(enclosing, capture.name.clone()))];
                let owner = class.hierarchy.field_owner(t, &capture.field);
                let to = self.node(Key::Field(owner, capture.field.clone()));
                self.flow(&from, to);
            }
            for constant in decl.enum_constants.iter().flatten() {
                for (field, value) in decl.fields.iter().zip(&constant.values) {
                    visit_expr(value, &mut |x| self.stores(usize::MAX, x));
                    if self.holds(field.ty) {
                        let from = self.origins(usize::MAX, value);
                        let to = self.node(Key::Field(t, field.name.clone()));
                        self.flow(&from, to);
                    }
                }
            }
        }
    }

    /// The parameters of the methods the runtime may call itself, with what it holds: unknown.
    fn runtime_calls(&mut self) {
        let class = self.class;
        let groups = super::object_values::runtime_called(class);
        for (m, method) in class.methods.iter().enumerate() {
            if !groups.contains(&class.hierarchy.group_id(m)) {
                continue;
            }
            for p in &method.params {
                let to = match self.holds(p.ty) {
                    true => Some(self.node(Key::Var(m, p.name.clone()))),
                    false => self.innermost(p.ty),
                };
                if let Some(to) = to {
                    let from = vec![
                        self.unknown("a parameter of a method that the runtime calls".into(), method.pos),
                    ];
                    self.flow(&from, to);
                }
            }
        }
        // `Type.getTypeName()` of a class held as a `Type` is `Class`'s own (see
        // `codegen::dispatch`), the class's interned name or an array's new type name.
        for (m, method) in class.methods.iter().enumerate() {
            if method.name == "getTypeName" && class.types[method.class].name == "java.lang.reflect.Type" {
                let to = self.node(Key::Return(class.hierarchy.group_id(m)));
                let from = vec![self.node(Key::Interned), self.node(Key::New)];
                self.flow(&from, to);
            }
        }
        // The arguments of `main` are new strings.
        for method in &class.methods {
            if method.is_main
                && let Some(p) = method.params.first()
                && let Some(to) = self.innermost(p.ty)
            {
                let from = vec![self.node(Key::New)];
                self.flow(&from, to);
            }
        }
    }

    /// The sources that each node's strings may come from: whether interned, whether new, and
    /// whether unknown.
    fn kinds(&self) -> Vec<u8> {
        let n = self.keys.len();
        let mut succs = vec![Vec::new(); n];
        for (to, preds) in self.preds.iter().enumerate() {
            for &p in preds {
                succs[p].push(to);
            }
        }
        let mut kinds = vec![0u8; n];
        let mut work = Vec::new();
        for (id, key) in self.keys.iter().enumerate() {
            kinds[id] = match key {
                Key::Interned => INTERNED,
                Key::New => NEW,
                Key::Unknown(_) => UNKNOWN,
                _ => continue,
            };
            work.push(id);
        }
        while let Some(id) = work.pop() {
            for &s in &succs[id] {
                if kinds[s] | kinds[id] != kinds[s] {
                    kinds[s] |= kinds[id];
                    work.push(s);
                }
            }
        }
        kinds
    }

    /// The nodes that the strings of `from` may come from, `from` among them.
    fn closure(&self, from: &[usize]) -> Vec<usize> {
        let mut seen: HashSet<usize> = from.iter().copied().collect();
        let mut work: Vec<usize> = from.to_vec();
        while let Some(id) = work.pop() {
            for &p in &self.preds[id] {
                if seen.insert(p) {
                    work.push(p);
                }
            }
        }
        let mut nodes: Vec<usize> = seen.into_iter().collect();
        nodes.sort();
        nodes
    }
}

/// How Java makes the string that a builtin returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Made {
    /// The one object Java keeps for its text: `intern()`, an enum constant's name, a class's
    /// name (HotSpot interns those), a charset's canonical name (a literal of its provider).
    Interned,
    /// A new string on every call.
    New,
    /// The interned `""` when empty, a new string otherwise: what a builder's `toString` and
    /// `substring` make.
    NewOrEmpty,
    /// `Arrays.toString`: the literal `"null"` or `"[]"`, a new string otherwise.
    ArrayText,
    /// `Class.getTypeName()`: the class's interned name, or an array's new type name, which the
    /// generator makes as such (`java::Class::shared_type_name`).
    TypeName,
    /// One object for each key, the same on every call: a system property, which Java stores
    /// once, `System.lineSeparator()`, an environment variable, a class's simple and canonical
    /// names (cached in its reflection data).
    PerKey,
    /// The string method of the receiver (the first argument) that returns the receiver itself
    /// under a condition (see `java::kept`).
    Kept,
    /// The string it is given, its last argument.
    Itself,
    /// `String.valueOf(o)`: the string itself, `"null"`, or what `o.toString()` returns.
    Text,
    /// An element of an array.
    Element,
    /// No string: it never returns.
    Never,
    /// What the translation does not know.
    Unknown,
}

/// How Java makes the string that `builtin` returns, where it returns one (checked against the
/// JDK's source).
pub(crate) fn made(builtin: Builtin) -> Made {
    match builtin {
        Builtin::String(StringOp::Intern)
        | Builtin::EnumName
        | Builtin::ClassName { simple: false }
        | Builtin::ClassLiteralName { simple: false, .. }
        | Builtin::ClassMethod(ClassOp::GetName | ClassOp::GetPackageName)
        | Builtin::Charset(CharsetOp::Name | CharsetOp::DisplayName | CharsetOp::ToString) => Made::Interned,
        Builtin::String(
            StringOp::Identity
            | StringOp::Join
            | StringOp::JoinRange
            | StringOp::FromChars
            | StringOp::FromCodePoints
            | StringOp::FromHighByte
            | StringOp::FromBytes,
        )
        | Builtin::Boxed(..)
        | Builtin::Bits(..)
        | Builtin::Unsigned(..)
        | Builtin::CodePoint(_)
        | Builtin::ClassMethod(ClassOp::ToString | ClassOp::DescriptorString) => Made::New,
        Builtin::Native(Native::SystemProperty(_) | Native::LineSeparator | Native::Getenv)
        | Builtin::ClassName { simple: true }
        | Builtin::ClassLiteralName { simple: true, .. }
        | Builtin::ClassMethod(ClassOp::GetSimpleName | ClassOp::GetCanonicalName) => Made::PerKey,
        Builtin::ClassMethod(ClassOp::GetTypeName) => Made::TypeName,
        // What a native method of the JDK returns is a new string on every call.
        Builtin::Native(_) => Made::New,
        Builtin::Format | Builtin::Builder(BuilderOp::ToString | BuilderOp::Substring) => Made::NewOrEmpty,
        Builtin::ArraysToString | Builtin::Arrays(_) => Made::ArrayText,
        Builtin::String(
            StringOp::Substring
            | StringOp::Trim
            | StringOp::Strip
            | StringOp::StripLeading
            | StringOp::StripTrailing
            | StringOp::ToUpperCase
            | StringOp::ToLowerCase
            | StringOp::Replace
            | StringOp::ReplaceAll
            | StringOp::ReplaceFirst
            | StringOp::Concat
            | StringOp::Repeat
            | StringOp::StripIndent
            | StringOp::TranslateEscapes,
        ) => Made::Kept,
        Builtin::String(StringOp::ToString) | Builtin::NullChecked(_) | Builtin::Kept(_) => Made::Itself,
        Builtin::ObjectText | Builtin::ValueOf => Made::Text,
        Builtin::ElementUpdate(_) => Made::Element,
        Builtin::NoObject { .. }
        | Builtin::UnknownClass
        | Builtin::UpdaterRefused
        | Builtin::MissingResource
        | Builtin::Unmatched => Made::Never,
        _ => Made::Unknown,
    }
}

const INTERNED: u8 = 1;
const NEW: u8 = 2;
const UNKNOWN: u8 = 4;

/// Whether the string expression `e` is a constant expression, which Java interns.
fn constant(class: &Class, e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Str(_)
        | ExprKind::Units(_)
        | ExprKind::Int(_)
        | ExprKind::Long(_)
        | ExprKind::Double(_)
        | ExprKind::Char(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_) => true,
        ExprKind::Static { class: c, name } => {
            class.types[*c].statics.iter().any(|f| &f.name == name && f.constant)
        }
        ExprKind::Binary(_, l, r) => constant(class, l) && constant(class, r),
        ExprKind::Unary(_, x) => constant(class, x),
        ExprKind::Cast(ty, x) => {
            (ty.is_numeric() || matches!(ty, Type::String | Type::Boolean)) && constant(class, x)
        }
        ExprKind::Conditional(c, a, b) => constant(class, c) && constant(class, a) && constant(class, b),
        _ => false,
    }
}

/// How a comparison is decided.
enum Decision {
    /// Both sides are interned: by their texts.
    Texts,
    /// By the objects, at run time: every string of the program is shared
    /// (`Class::shared_strings`).
    Objects,
    /// No object can be on both sides: only two nulls are the same.
    BothNull,
}

/// Decides each `Builtin::StringSame` of the program, or rejects it.
pub fn string_identity(class: &mut Class) -> Result<(), Vec<Error>> {
    let result = decide(class);
    // `String.valueOf` is the text of its argument where strings are not shared.
    if !class.shared_strings {
        let mut unwrap = |x: &mut Expr| {
            if let ExprKind::Builtin(Builtin::ValueOf, args) = &mut x.kind {
                *x = args.pop().expect("the text of `String.valueOf`");
            }
        };
        for method in &mut class.methods {
            for s in &mut method.body {
                rewrite_stmt(s, &mut unwrap);
            }
        }
        for decl in &mut class.types {
            for f in decl.fields.iter_mut().chain(decl.statics.iter_mut()) {
                if let Some(init) = &mut f.init {
                    rewrite_expr(init, &mut unwrap);
                }
            }
        }
    }
    result
}

/// The value that `String.valueOf` (`Builtin::ValueOf`, whose argument is `"" + x`) or
/// `Object.toString` (`Builtin::ObjectText`) makes the string of.
pub(crate) fn value_of_operand(builtin: Builtin, args: &[Expr]) -> &Expr {
    match (builtin, &args[0].kind) {
        (Builtin::ValueOf, ExprKind::Binary(BinOp::Add, _, x)) => x,
        _ => &args[0],
    }
}

/// Decides each `==` on strings (see `string_identity`).
fn decide(class: &mut Class) -> Result<(), Vec<Error>> {
    let mut sites = Vec::new();
    for (m, method) in class.methods.iter().enumerate() {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Builtin(Builtin::StringSame, args) = &x.kind {
                            sites.push((Some(m), x.id, x.pos, args));
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    for decl in &class.types {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init {
                visit_expr(init, &mut |x| {
                    if let ExprKind::Builtin(Builtin::StringSame, args) = &x.kind {
                        sites.push((None, x.id, x.pos, args));
                    }
                });
            }
        }
    }
    // A check of the generator: every program as if it compared strings by reference.
    if std::env::var_os("JAVA_TO_RUST_SHARED_STRINGS").is_some() {
        class.shared_strings = true;
    }
    if sites.is_empty() {
        if class.shared_strings {
            keep_receivers(class);
        }
        return Ok(());
    }
    let to_strings = class
        .methods
        .iter()
        .enumerate()
        .filter(|(_, method)| method.name == "toString" && method.params.is_empty() && !method.is_static)
        .map(|(m, _)| class.hierarchy.group_id(m))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut graph = Graph {
        class,
        ids: HashMap::new(),
        keys: Vec::new(),
        preds: Vec::new(),
        unknown_sites: HashMap::new(),
        to_strings,
    };
    for m in 0..class.methods.len() {
        graph.method(m);
    }
    graph.outside_methods();
    graph.runtime_calls();
    // The nodes of both sides of each comparison, before the kinds are known.
    let all_sides: Vec<Vec<Vec<usize>>> = sites
        .iter()
        .map(|(m, _, _, args)| args.iter().map(|a| graph.origins(m.unwrap_or(usize::MAX), a)).collect())
        .collect();
    let kinds = graph.kinds();
    let mut decisions = HashMap::new();
    let mut errors = Vec::new();
    for ((_, id, pos, _), sides) in sites.iter().zip(all_sides) {
        let kind = |nodes: &[usize]| nodes.iter().fold(0, |k, &n| k | kinds[n]);
        let (l, r) = (kind(&sides[0]), kind(&sides[1]));
        if (l | r) & UNKNOWN != 0 {
            let closures: Vec<Vec<usize>> = sides.iter().map(|s| graph.closure(s)).collect();
            let names: Vec<String> = closures
                .iter()
                .flatten()
                .filter_map(|&n| match &graph.keys[n] {
                    Key::Unknown(what) => Some(what.clone()),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            errors.push(Error {
                pos: *pos,
                message: format!(
                    "`==` on strings, where a string may come from where the translation does not follow it: {}",
                    names.join(", ")
                ),
            });
            continue;
        }
        let decision = if l & NEW == 0 && r & NEW == 0 {
            Decision::Texts
        } else if (l == INTERNED && r == NEW) || (l == NEW && r == INTERNED) || l == 0 || r == 0 {
            Decision::BothNull
        } else {
            Decision::Objects
        };
        decisions.insert(*id, decision);
    }
    class.shared_strings |= decisions.values().any(|d| matches!(d, Decision::Objects));
    let mut next_id = super::live_methods::largest_id(class) + 1;
    let mut decide = |x: &mut Expr| {
        let Some(decision) = decisions.get(&x.id) else { return };
        if matches!(decision, Decision::Objects) {
            return;
        }
        let ExprKind::Builtin(_, args) = &mut x.kind else { return };
        let args = std::mem::take(args);
        x.kind = match decision {
            Decision::Texts => ExprKind::Builtin(Builtin::ObjectsEquals, args),
            Decision::Objects => unreachable!("left for the generator"),
            Decision::BothNull => {
                let mut is_null = |a: Expr| {
                    let null = Expr { kind: ExprKind::Null, pos: a.pos, ty: a.ty, id: next_id };
                    let pos = a.pos;
                    next_id += 2;
                    Box::new(Expr {
                        kind: ExprKind::Binary(BinOp::Eq, Box::new(a), Box::new(null)),
                        pos,
                        ty: Type::Boolean,
                        id: next_id - 1,
                    })
                };
                let mut args = args.into_iter();
                let (a, b) = (args.next().expect("two sides"), args.next().expect("two sides"));
                ExprKind::Binary(BinOp::BitAnd, is_null(a), is_null(b))
            }
        };
    };
    for method in &mut class.methods {
        for s in &mut method.body {
            rewrite_stmt(s, &mut decide);
        }
    }
    for decl in &mut class.types {
        for f in decl.fields.iter_mut().chain(decl.statics.iter_mut()) {
            if let Some(init) = &mut f.init {
                rewrite_expr(init, &mut decide);
            }
        }
    }
    if class.shared_strings {
        keep_receivers(class);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Where every string is shared, gives each call of a `String` method that may return its
/// receiver itself (`Made::Kept`) the receiver as an object: the receiver and the arguments are
/// held in locals, as `lower::argument_order` holds them, and `Builtin::Kept` of them and the
/// call's text is the receiver itself or a new string, as the JDK's condition says.
fn keep_receivers(class: &mut Class) {
    let mut next_id = super::live_methods::largest_id(class) + 1;
    let mut held = 0;
    let mut keep = |x: &mut Expr| {
        let ExprKind::Builtin(Builtin::String(op), args) = &x.kind else { return };
        if x.ty != Type::String
            || made(Builtin::String(*op)) != Made::Kept
            || args.iter().any(|a| a.ty == Type::Null)
        {
            return;
        }
        let op = *op;
        let (pos, ty) = (x.pos, x.ty);
        let mut synth = |kind: ExprKind, ty: Type| {
            next_id += 1;
            Expr { kind, pos, ty, id: next_id - 1 }
        };
        let ExprKind::Builtin(_, args) = std::mem::replace(&mut x.kind, ExprKind::Null) else {
            unreachable!()
        };
        let mut body = Vec::new();
        let mut names = Vec::new();
        for (i, arg) in args.into_iter().enumerate() {
            let prefix = if i == 0 { crate::lower::RECEIVER } else { "argument$" };
            let name = format!("{prefix}kept{held}_{i}");
            names.push((name.clone(), arg.ty));
            body.push(Stmt::Local { ty: arg.ty, name, init: Some(arg), pos });
        }
        held += 1;
        let mut reads: Vec<Expr> = names.iter().map(|(n, ty)| synth(ExprKind::Var(n.clone()), *ty)).collect();
        let call_args = names.iter().map(|(n, ty)| synth(ExprKind::Var(n.clone()), *ty)).collect();
        reads.push(synth(ExprKind::Builtin(Builtin::String(op), call_args), ty));
        let kept = synth(ExprKind::Builtin(Builtin::Kept(op), reads), ty);
        body.push(Stmt::Yield(kept));
        let selector = Box::new(synth(ExprKind::Int(0), Type::Int));
        x.kind = ExprKind::Switch { selector, cases: vec![Case { labels: vec![CaseLabel::Default], body }] };
    };
    for method in &mut class.methods {
        for s in &mut method.body {
            rewrite_stmt(s, &mut keep);
        }
    }
    for decl in &mut class.types {
        for f in decl.fields.iter_mut().chain(decl.statics.iter_mut()) {
            if let Some(init) = &mut f.init {
                rewrite_expr(init, &mut keep);
            }
        }
    }
}
