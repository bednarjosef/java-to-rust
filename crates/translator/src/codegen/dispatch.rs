//! Class trees with several concrete classes. A value of a class type that objects of several
//! concrete classes can be is an enum named after the type, with a variant per concrete class
//! holding that class's object (`Rc<RefCell<Circle>>`, or `Rc<Circle>` in an immutable family):
//!
//! - a call on such a value is a *dispatcher*, a method of the enum that matches the variant and
//!   calls what the object's class runs;
//! - a field is read through `base()`, which borrows the fields struct of the type's class from
//!   whichever object the value is, and written through `base_mut()`;
//! - `==` compares the objects' identity, `PartialEq` on the enum;
//! - printing goes through the enum's `Display`, which shows each object as its class does;
//! - a body inherited from the type that needs the whole object (not only its fields) is a
//!   method of the enum, taking `&self`;
//! - a subtype's value converts to a supertype's with `to_<supertype>()`, and an object of a
//!   concrete class is wrapped in its variant.
//!
//! Each item of an enum is written only when some code uses it. The generator learns which by
//! generating the program until no new item is asked for (see `generate`).

use super::*;
use std::collections::BTreeSet;

/// An item of the enum of a type that the code being generated uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Need {
    /// The enum of the type, with its `Clone` and its identity.
    Enum(usize),
    /// `base()`: the fields struct of the type's class, borrowed from the variant's object.
    Base(usize),
    /// `base_mut()`: the same, borrowed mutably.
    BaseMut(usize),
    /// The dispatcher on an enum for an override group (by group id).
    Dispatch(usize, usize),
    /// A method whose body is emitted on the enum of its type.
    Body(usize),
    /// `to_<t>()` on the enum of a subtype `s`: the same object as a value of `t`, as `(s, t)`.
    Convert(usize, usize),
    /// The enum's `Display`.
    Display(usize),
    /// `as_<t>()` on the enum of `s`: the object as a value of type `t` if it is one, as
    /// `(s, t)`.
    Narrow(usize, usize),
}

/// How a value of one class type becomes a value of another, in a cast or an `instanceof`.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Narrowing {
    /// The value's type is the target type or below it: every object of it is one of the target.
    Up,
    /// The value is of the enum of this type, whose variants are tested.
    Down(usize),
    /// The value's class is known, and whether it is of the target type.
    Known(bool),
}

impl<'a> Gen<'a> {
    /// Records that the code uses an item of an enum (and so the enum).
    pub(super) fn need(&self, need: Need) {
        let mut needs = self.needs.borrow_mut();
        match need {
            Need::Base(t) | Need::BaseMut(t) | Need::Dispatch(t, _) | Need::Display(t) => {
                needs.insert(Need::Enum(t));
            }
            Need::Convert(s, t) => {
                needs.insert(Need::Enum(s));
                needs.insert(Need::Enum(t));
            }
            Need::Narrow(s, _) => {
                needs.insert(Need::Enum(s));
            }
            Need::Body(i) => {
                if let Home::Enum(t) = self.homes[i] {
                    needs.insert(Need::Enum(t));
                }
            }
            Need::Enum(_) => {}
        }
        needs.insert(need);
    }

    /// The type whose enum represents the values of class type `ty`, if an enum does.
    pub(super) fn enum_type(&self, ty: Type) -> Option<usize> {
        self.enum_for(self.class.class_index(ty)?)
    }

    /// The type whose enum represents the values of class `t`, if an enum does.
    pub(super) fn enum_for(&self, t: usize) -> Option<usize> {
        match hierarchy::rep(self.class, self.facts, t) {
            hierarchy::Rep::Enum(t) => Some(t),
            hierarchy::Rep::Object(_) => None,
        }
    }

    /// The type whose enum the value of `e` is. `this` is the enum in a method of the enum, and
    /// an object of the class being written anywhere else.
    pub(super) fn enum_of(&self, e: &Expr) -> Option<usize> {
        if matches!(e.kind, ExprKind::This) {
            return (self.this_form == ThisForm::Enum).then_some(self.this_struct);
        }
        self.enum_type(e.ty)
    }

    /// The concrete classes whose objects the enum of type `t` holds, in `Class::types` order.
    pub(super) fn enum_variants(&self, t: usize) -> Vec<usize> {
        // An enum's constants exist without `new`.
        hierarchy::variants(self.class, self.facts, t)
            .into_iter()
            .filter(|&c| self.live[c] || self.holds_value(c))
            .collect()
    }

    /// The path of the variant of class `c` in the enum of `t`: `Shape::Circle`. A variant is
    /// named after its class, with the class's outer classes when two variants share a name.
    pub(super) fn variant(&self, t: usize, c: usize) -> String {
        self.need(Need::Enum(t));
        let simple =
            |c: usize| self.class.types[c].name.rsplit(['.', '$']).next().unwrap_or_default().to_string();
        let name = simple(c);
        let shared =
            hierarchy::variants(self.class, self.facts, t).into_iter().filter(|&o| simple(o) == name).count()
                > 1;
        let name =
            if shared { modules::split(&self.class.types[c].name).1.replace(['.', '$'], "") } else { name };
        format!("{}::{name}", struct_name(&self.class.types[t].name))
    }

    /// `variant`, when the enum of `t` has a variant for class `c`: it has none when no object of
    /// `c` is ever created, where code that has one never runs.
    fn variant_of(&self, t: usize, c: usize) -> Option<String> {
        self.enum_variants(t).contains(&c).then(|| self.variant(t, c))
    }

    /// The name of the conversion to the enum of `t`: `to_shape`.
    fn to_name(&self, t: usize) -> String {
        let simple = self.class.types[t].name.rsplit(['.', '$']).next().unwrap_or_default();
        format!("to_{}", snake_case(simple).trim_end_matches('_'))
    }

    /// The name of the checked conversion to type `t`: `as_circle`.
    fn as_name(&self, t: usize) -> String {
        let simple = self.class.types[t].name.rsplit(['.', '$']).next().unwrap_or_default();
        format!("as_{}", snake_case(simple).trim_end_matches('_'))
    }

    /// The binary name of type `t`, as the JVM shows it: `shapes.Outer$Inner`.
    fn binary_name(&self, t: usize) -> String {
        let decl = &self.class.types[t];
        if decl.kind == TypeKind::Runtime {
            return decl.name.clone();
        }
        if let Some((binary, _)) = decl.origin.java_names() {
            return binary.to_string();
        }
        match decl.name.strip_prefix(&decl.package).and_then(|rest| rest.strip_prefix('.')) {
            Some(rest) if !decl.package.is_empty() => format!("{}.{}", decl.package, rest.replace('.', "$")),
            _ => decl.name.replace('.', "$"),
        }
    }

    /// The type of the program that a class or enum type names.
    pub(super) fn type_of(&self, ty: Type) -> Option<usize> {
        self.class.type_of(ty)
    }

    /// The Java type of the values of type `t`.
    fn java_type(&self, t: usize) -> Type {
        let decl = &self.class.types[t];
        match decl.kind {
            TypeKind::Enum => Type::enum_type(&decl.name),
            TypeKind::Runtime => runtime_type(&decl.name).expect("a runtime class"),
            _ => Type::class(&decl.name),
        }
    }

    /// How the value of `value`, of a class or enum type, becomes a value of class or enum type
    /// `to`.
    pub(super) fn narrowing(&self, value: &Expr, to: Type) -> Narrowing {
        let h = &self.class.hierarchy;
        let (Some(s), Some(t)) = (self.type_of(value.ty), self.type_of(to)) else { return Narrowing::Up };
        if h.is_subtype(s, t) {
            return Narrowing::Up;
        }
        match self.enum_of(value) {
            Some(e) => Narrowing::Down(e),
            None => Narrowing::Known(h.is_subtype(self.value_class(value), t)),
        }
    }

    /// The variants of the enum of `s` whose objects are of type `t`.
    fn passing(&self, s: usize, t: usize) -> Vec<usize> {
        self.enum_variants(s).into_iter().filter(|&c| self.class.hierarchy.is_subtype(c, t)).collect()
    }

    /// The text of the value of `value` for a method to be called on it in a condition: a local
    /// or `this` as it is, anything else in a block of its own, so that no borrow it makes
    /// outlives the test (a let chain keeps its temporaries alive through the guarded block).
    fn tested(&mut self, value: &'a Expr, text: String) -> String {
        match self.peel(value).kind {
            ExprKind::Var(_) | ExprKind::This => text,
            _ => format!("{{ let value = {text}; value }}"),
        }
    }

    /// `(T) value`, a checked downcast, or a cast across, from the enum of `s`.
    fn downcast(&mut self, s: usize, to: Type, value: &'a Expr) -> Code {
        let t = self.type_of(to).expect("a cast to a type of the program");
        self.need(Need::Narrow(s, t));
        let name = self.as_name(t);
        let cast = self.runtime("cast");
        let target = rust_string(&self.binary_name(t));
        if self.nullable(value) {
            let option = self.object_place(value);
            let enum_name = struct_name(&self.class.types[s].name);
            return Code::atom(format!(
                "{option}.as_ref().map(|o| {cast}({enum_name}::{name}(o), {target}))"
            ));
        }
        let obj = self.object(value);
        Code::atom(format!("{cast}({obj}.{name}(), {target})"))
    }

    /// A cast of a value whose class is known to a type that it is not of, which always fails
    /// (unless the value is null).
    fn failing_cast(&mut self, to: Type, value: &'a Expr) -> Code {
        let t = self.type_of(to).expect("a cast to a type of the program");
        let failed = format!("{}({})", self.runtime("cast_failed"), rust_string(&self.binary_name(t)));
        if self.nullable(value) {
            let option = self.object_place(value);
            let ty = self.ty_name(to);
            return Code::atom(format!("{option}.as_ref().map(|_| -> {ty} {{ {failed} }})"));
        }
        let v = self.expr(value).text;
        Code::new(format!("{{ let _ = {v}; {failed} }}"), LOWEST)
    }

    /// `value instanceof T`.
    pub(super) fn instance_test(&mut self, value: &'a Expr, to: Type) -> Code {
        let nullable = self.nullable(value);
        let known = |g: &mut Self, passes: bool| {
            if nullable && passes {
                return Code::atom(format!("{}.is_some()", g.object_place(value)));
            }
            let text = passes.to_string();
            if matches!(g.peel(value).kind, ExprKind::Var(_) | ExprKind::This) {
                return Code::atom(text);
            }
            let v = g.expr(value).text;
            Code::new(format!("{{ let _ = {v}; {text} }}"), LOWEST)
        };
        match self.narrowing(value, to) {
            Narrowing::Up => known(self, true),
            Narrowing::Known(passes) => known(self, passes),
            Narrowing::Down(s) => {
                let t = self.type_of(to).expect("a test of a type of the program");
                let passing = self.passing(s, t);
                if passing.is_empty() {
                    return known(self, false);
                }
                let patterns: Vec<String> =
                    passing.iter().map(|&c| format!("{}(_)", self.variant(s, c))).collect();
                let patterns = patterns.join(" | ");
                let text = if nullable {
                    format!("matches!({}, Some({patterns}))", self.object_place(value))
                } else {
                    format!("matches!({}, {patterns})", self.object(value))
                };
                Code::atom(text)
            }
        }
    }

    /// The `Option` that `value instanceof T name` binds `name` from: the value as a `T` when the
    /// test passes.
    pub(super) fn pattern_source(&mut self, value: &'a Expr, to: Type) -> String {
        let nullable = self.nullable(value);
        match self.narrowing(value, to) {
            Narrowing::Up | Narrowing::Known(true) => {
                let v = self.class_cast(to, value).text;
                if nullable { v } else { format!("Some({v})") }
            }
            Narrowing::Known(false) => format!("None::<{}>", self.ty_name(to)),
            Narrowing::Down(s) => {
                let t = self.type_of(to).expect("a test of a type of the program");
                self.need(Need::Narrow(s, t));
                let name = self.as_name(t);
                let text = if nullable {
                    let enum_name = struct_name(&self.class.types[s].name);
                    format!("{}.as_ref().and_then({enum_name}::{name})", self.object_place(value))
                } else {
                    format!("{}.{name}()", self.object(value))
                };
                self.tested(value, text)
            }
        }
    }

    /// Whether the variant of `c` holds a value rather than a shared object of the program: the
    /// constant of an enum that implements the type, or a string or a box in an `Object`.
    pub(super) fn holds_value(&self, c: usize) -> bool {
        matches!(self.class.types[c].kind, TypeKind::Enum | TypeKind::Runtime)
    }

    /// The Java type of the values of runtime class `c`, if it is one.
    pub(super) fn runtime_of(&self, c: usize) -> Option<Type> {
        (self.class.types[c].kind == TypeKind::Runtime)
            .then(|| runtime_type(&self.class.types[c].name))
            .flatten()
    }

    /// The value that the variant of `c` bound to `o` holds, as a value of its own: `*o` for an
    /// enum's constant or a plain box, `o.clone()` for an object or a string.
    fn held(&self, c: usize) -> &'static str {
        let copied = match self.runtime_of(c) {
            Some(ty) => ty != Type::String && !self.faithful_box(ty),
            None => self.holds_value(c),
        };
        if copied { "*o" } else { "o.clone()" }
    }

    /// The class of the object, the enum of the constant, or the runtime class of the string or
    /// box, that `e` is: the variant it becomes.
    pub(super) fn value_class(&self, e: &Expr) -> usize {
        match e.ty {
            Type::Class(..) => self.struct_class(e).expect("an object has a class type"),
            _ => self.class.type_of(e.ty).expect("a value of a type of the program or of the runtime"),
        }
    }

    /// The variant's class of `e` (see `value_class`) and its handle or value.
    fn variant_value(&mut self, e: &'a Expr) -> (usize, String) {
        let c = self.value_class(e);
        let text = match self.runtime_of(c) {
            Some(ty) => self.value(e, ty).text,
            None if self.holds_value(c) => self.expr(e).text,
            None => self.object_handle(e),
        };
        (c, text)
    }

    /// The Rust type of the object of concrete class `c` that its variant holds.
    fn payload(&mut self, c: usize) -> String {
        if let Some(ty) = self.runtime_of(c) {
            return self.ty_name(ty);
        }
        if self.holds_value(c) {
            return struct_name(&self.class.types[c].name);
        }
        let structure = self.struct_of(c);
        if self.facts.immutable.contains(&self.class.types[c].name) {
            format!("Rc<{structure}>")
        } else {
            format!("Rc<RefCell<{structure}>>")
        }
    }

    /// Whether the objects of the enum of `t` are immutable: a family is immutable together.
    pub(super) fn immutable_enum(&self, t: usize) -> bool {
        let concretes = self.class.hierarchy.concretes(t);
        !concretes.is_empty()
            && concretes.iter().all(|&c| self.facts.immutable.contains(&self.class.types[c].name))
    }

    /// The class whose fields struct `base()` of the enum of `t` borrows: `t`'s own, or the
    /// nearest superclass's.
    fn fields_root(&self, t: usize) -> Option<usize> {
        if self.facts.fields_structs.contains(&t) {
            Some(t)
        } else {
            hierarchy::embedded(self.facts, self.class, t)
        }
    }

    /// The path of field `name` of a value of the enum of `t` from what `base()` borrows.
    pub(super) fn enum_field_path(&self, t: usize, ty: Type, name: &str) -> String {
        let field = snake_case(name);
        match (self.fields_root(t), self.class.field_owner(ty, name)) {
            (Some(root), Some(owner)) => hierarchy::base_path(self.facts, self.class, root, owner) + &field,
            _ => field,
        }
    }

    /// The borrow of the fields of a value of the enum of `t`: `.base()` or `.base_mut()`.
    pub(super) fn enum_borrow(&self, t: usize, mutable: bool) -> &'static str {
        if mutable {
            self.need(Need::BaseMut(t));
            ".base_mut()"
        } else {
            self.need(Need::Base(t));
            ".base()"
        }
    }

    /// `e` as a value of the enum of `t`, a type at or above its own, for a method of the enum
    /// to be called on: as it is, converted from a subtype's enum, or an object wrapped in its
    /// variant.
    pub(super) fn as_enum(&mut self, t: usize, e: &'a Expr) -> String {
        match self.enum_of(e) {
            Some(s) if s == t => self.object(e),
            Some(s) => {
                self.need(Need::Convert(s, t));
                format!("{}.{}()", self.object(e), self.to_name(t))
            }
            None => {
                let (c, handle) = self.variant_value(e);
                match self.variant_of(t, c) {
                    Some(variant) => format!("{variant}({handle})"),
                    None => {
                        let ty = struct_name(&self.class.types[t].name);
                        format!("{{ let value: {ty} = {}; value }}", absent(self, c))
                    }
                }
            }
        }
    }

    /// A conversion of an object to a supertype: the same object as a value of the supertype's
    /// enum. Where both types have the same representation it changes nothing.
    pub(super) fn class_cast(&mut self, to: Type, value: &'a Expr) -> Code {
        if matches!(value.kind, ExprKind::Null) {
            return Code::atom("None");
        }
        match self.narrowing(value, to) {
            Narrowing::Down(s) => return self.downcast(s, to, value),
            Narrowing::Known(false) => return self.failing_cast(to, value),
            Narrowing::Up | Narrowing::Known(true) => {}
        }
        let Some(t) = self.enum_type(to) else {
            // The values of `to` are the objects of one class: a value of another class is one
            // whose objects never reach `to`, so it is null.
            let target = self.class.class_index(to).map(|t| self.rep_class(t));
            if let (Some(target), Some(c)) = (target, self.struct_class(value))
                && target != c
            {
                let ty = self.ty_name(to);
                let text = self.expr(value).text;
                return Code::atom(if self.nullable(value) {
                    format!("{{ let _ = {text}; None::<{ty}> }}")
                } else {
                    format!("{{ let _ = {text}; let value: {ty} = {}; value }}", absent(self, c))
                });
            }
            return self.expr(value);
        };
        // A new object becomes a value of the target's enum directly.
        if let ExprKind::New { ctor: Some(ctor), args, .. } = &value.kind {
            return self.new_object_as(*ctor, args, Some(t));
        }
        let from = self.enum_of(value);
        if from == Some(t) {
            return self.expr(value);
        }
        if self.nullable(value) {
            let option = self.expr(value).receiver();
            return Code::atom(match from {
                Some(s) => {
                    self.need(Need::Convert(s, t));
                    format!(
                        "{option}.as_ref().map({}::{})",
                        struct_name(&self.class.types[s].name),
                        self.to_name(t)
                    )
                }
                None => {
                    let c = self.value_class(value);
                    match self.variant_of(t, c) {
                        Some(variant) => format!("{option}.map({variant})"),
                        None => {
                            format!("{option}.map(|_| -> {} {{ {} }})", self.ty_name(to), absent(self, c))
                        }
                    }
                }
            });
        }
        match from {
            Some(s) => {
                self.need(Need::Convert(s, t));
                Code::atom(format!("{}.{}()", self.object(value), self.to_name(t)))
            }
            None => {
                let c = self.value_class(value);
                // An object's last use moves its handle in.
                let Some(variant) = self.variant_of(t, c) else {
                    let ty = self.ty_name(to);
                    return Code::atom(format!("{{ let value: {ty} = {}; value }}", absent(self, c)));
                };
                let v = match self.runtime_of(c) {
                    Some(_) => self.variant_value(value).1,
                    None => self.expr(value).text,
                };
                Code::atom(format!("{variant}({v})"))
            }
        }
    }

    /// `a == b` and `a != b` on values of an enum: identity, through `PartialEq`.
    pub(super) fn enum_identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let symbol = if op == BinOp::Ne { "!=" } else { "==" };
        let text = if self.nullable(l) || self.nullable(r) {
            let (a, b) = (self.enum_option(l), self.enum_option(r));
            format!("{a} {symbol} {b}")
        } else {
            let (a, b) = (self.object_ref(l), self.object_ref(r));
            format!("{a} {symbol} {b}")
        };
        Code::new(text, CMP)
    }

    /// A value of an enum as an `Option<&..>`: `None` for null.
    fn enum_option(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "None".to_string(),
            _ if self.nullable(e) => format!("{}.as_ref()", self.object_place(e)),
            _ => format!("Some({})", self.object_ref(e)),
        }
    }

    /// A call of instance method `resolved` on a value of the enum of `t`: the method of the
    /// enum that runs what the object's class runs. `toString` is the enum's `Display`.
    pub(super) fn enum_call(
        &mut self,
        t: usize,
        receiver: &'a Expr,
        resolved: usize,
        args: &'a [Expr],
    ) -> Code {
        let m = &self.class.methods[resolved];
        if m.name == "toString" && m.params.is_empty() {
            self.need(Need::Display(t));
            return Code::atom(format!("{}.to_string()", self.object(receiver)));
        }
        let name = self.dispatch_name(t, resolved);
        let obj = self.object(receiver);
        let args = self.call_args(resolved, args).join(", ");
        Code::atom(format!("{obj}.{name}({args})"))
    }

    /// The implementations that the variants of the enum of `t` run for a call of `resolved`.
    fn enum_targets(&self, t: usize, resolved: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for c in self.enum_variants(t) {
            if let Some(i) = self.class.hierarchy.implementation(resolved, c)
                && !out.contains(&i)
            {
                out.push(i);
            }
        }
        out
    }

    /// The name of the method of the enum of `t` that a call of `resolved` calls: the body
    /// itself when every variant runs the one the enum has, else the dispatcher.
    fn dispatch_name(&self, t: usize, resolved: usize) -> String {
        let h = &self.class.hierarchy;
        if let [only] = self.enum_targets(t, resolved)[..]
            && self.homes[only] == Home::Enum(t)
            && self.enum_variants(t).into_iter().all(|c| h.implementation(resolved, c).is_some())
        {
            self.need(Need::Body(only));
            return self.fn_names[only].clone();
        }
        let group = self.class.hierarchy.group_id(resolved);
        self.need(Need::Dispatch(t, group));
        self.group_names[&group].clone()
    }

    /// A call of method `index`, emitted on the enum of a type or on a fields struct, from code
    /// whose receiver is not the struct that has it: `Base::Circle(c.clone()).describe_in_base()`
    /// or, in a method of an enum, `self.base_mut().get_name()`. `None` for any other call.
    pub(super) fn inherited_call(
        &mut self,
        receiver: &'a Expr,
        index: usize,
        args: &'a [Expr],
    ) -> Option<Code> {
        match self.homes[index] {
            Home::Enum(d) => {
                self.need(Need::Body(index));
                let obj = self.as_enum(d, receiver);
                let args = self.call_args(index, args).join(", ");
                Some(Code::atom(format!("{obj}.{}({args})", self.fn_names[index])))
            }
            Home::Fields(d) => {
                let t = self.enum_of(receiver)?;
                let root = self.fields_root(t).expect("a fields struct is at or above its subclasses");
                let path = hierarchy::base_path(self.facts, self.class, root, d);
                let borrow = self.enum_borrow(t, self.facts.mutating.contains(&index));
                let obj = self.object(receiver);
                let args = self.call_args(index, args).join(", ");
                Some(Code::atom(format!("{obj}{borrow}.{path}{}({args})", self.fn_names[index])))
            }
            _ => None,
        }
    }

    /// The text of a value of an enum, through its `Display`.
    pub(super) fn enum_text(&mut self, t: usize, obj: String) -> String {
        self.need(Need::Display(t));
        if self.lazy_text && !self.printing { obj } else { format!("{obj}.to_string()") }
    }

    /// Whether turning a value of the enum of `t` into text may write a shared object.
    pub(super) fn enum_text_writes(&self, t: usize) -> bool {
        self.enum_variants(t)
            .into_iter()
            .filter_map(|c| hierarchy::to_string_impl(self.class, c))
            .any(|i| self.facts.writers.contains(&i))
    }

    /// The enum of type `t` and the items of it that the code uses.
    pub(super) fn type_enum_items(&mut self, t: usize) {
        let name = struct_name(&self.class.types[t].name);
        let variants = self.enum_variants(t);
        self.gap();
        self.line("#[derive(Clone)]");
        self.line(&format!("{}enum {name} {{", self.vis()));
        self.indent += 1;
        for &c in &variants {
            let variant = self.variant(t, c);
            let short = variant.rsplit("::").next().unwrap_or_default().to_string();
            let payload = self.payload(c);
            self.line(&format!("{short}({payload}),"));
        }
        self.indent -= 1;
        self.line("}");
        let wanted: Vec<Need> = self.wanted.iter().copied().collect();
        let mark = self.out.len();
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        let start = self.out.len();
        for need in &wanted {
            match *need {
                Need::Base(s) if s == t => self.base_fn(t, false),
                Need::BaseMut(s) if s == t => self.base_fn(t, true),
                Need::Convert(s, to) if s == t => self.convert_fn(t, to),
                Need::Narrow(s, to) if s == t => self.narrow_fn(t, to),
                Need::Dispatch(s, group) if s == t => self.dispatcher(t, group),
                Need::Body(i) if self.homes[i] == Home::Enum(t) => {
                    self.gap();
                    self.method(i);
                }
                _ => {}
            }
        }
        self.indent -= 1;
        if self.out.len() == start {
            self.out.truncate(mark);
        } else {
            self.line("}");
        }
        self.identity_impl(t, &variants);
        if self.wanted.contains(&Need::Display(t)) {
            self.display_impl(t, &variants);
        }
        self.raised_impl(t);
    }

    /// The arms of a `match self` over the variants, each made by `arm` from the variant's class;
    /// a `match` with no arms for an enum without variants.
    fn match_arms(&mut self, t: usize, variants: &[usize], arm: &dyn Fn(&mut Self, usize) -> (String, bool)) {
        if variants.is_empty() {
            self.line("match *self {}");
            return;
        }
        let arms: Vec<(String, String, bool)> = variants
            .iter()
            .map(|&c| {
                let (text, uses) = arm(self, c);
                (self.variant(t, c), text, uses)
            })
            .collect();
        // The same call for every variant needs no `match`.
        if arms.iter().all(|(_, text, uses)| !uses && *text == arms[0].1) {
            self.line(&arms[0].1);
            return;
        }
        self.line("match self {");
        self.indent += 1;
        for (variant, text, uses) in arms {
            let binding = if uses { "o" } else { "_" };
            self.line(&format!("{variant}({binding}) => {text},"));
        }
        self.indent -= 1;
        self.line("}");
    }

    /// `base()` or `base_mut()`: the fields struct of the type's class in each variant's object.
    fn base_fn(&mut self, t: usize, mutable: bool) {
        let Some(root) = self.fields_root(t) else { return };
        let structure = self.struct_of(root);
        let variants = self.enum_variants(t);
        let immutable = self.immutable_enum(t);
        if mutable && immutable {
            return;
        }
        self.gap();
        let vis = self.vis();
        self.line(&match (mutable, immutable) {
            (false, true) => format!("{vis}fn base(&self) -> &{structure} {{"),
            (false, false) => format!("{vis}fn base(&self) -> Ref<'_, {structure}> {{"),
            _ => format!("{vis}fn base_mut(&self) -> RefMut<'_, {structure}> {{"),
        });
        self.indent += 1;
        self.match_arms(t, &variants, &|g: &mut Self, c| {
            let path = hierarchy::base_path(g.facts, g.class, c, root);
            let path = path.trim_end_matches('.');
            let text = match (mutable, immutable, path.is_empty()) {
                (false, true, true) => "o.as_ref()".to_string(),
                (false, true, false) => format!("&o.{path}"),
                (false, false, true) => "o.borrow()".to_string(),
                (false, false, false) => format!("Ref::map(o.borrow(), |o| &o.{path})"),
                (true, _, true) => "o.borrow_mut()".to_string(),
                (true, _, false) => format!("RefMut::map(o.borrow_mut(), |o| &mut o.{path})"),
            };
            (text, true)
        });
        self.indent -= 1;
        self.line("}");
    }

    /// `to_<to>()`: the same object as a value of the enum of a supertype.
    fn convert_fn(&mut self, t: usize, to: usize) {
        let variants = self.enum_variants(t);
        self.gap();
        let target = struct_name(&self.class.types[to].name);
        self.line(&format!("{}fn {}(&self) -> {target} {{", self.vis(), self.to_name(to)));
        self.indent += 1;
        self.match_arms(t, &variants, &|g: &mut Self, c| {
            (format!("{}({})", g.variant(to, c), g.held(c)), true)
        });
        self.indent -= 1;
        self.line("}");
    }

    /// `as_<to>()`: the object as a value of type `to` if it is one, else `None`.
    fn narrow_fn(&mut self, t: usize, to: usize) {
        let variants = self.enum_variants(t);
        let ty = self.java_type(to);
        let target = self.ty_name(ty);
        let rep = (self.class.types[to].kind != TypeKind::Enum).then(|| self.enum_for(to)).flatten();
        self.gap();
        self.line(&format!("{}fn {}(&self) -> Option<{target}> {{", self.vis(), self.as_name(to)));
        self.indent += 1;
        let passing = self.passing(t, to);
        if passing.is_empty() {
            self.line("None");
        } else {
            self.line("match self {");
            self.indent += 1;
            for &c in &passing {
                let value = match rep {
                    Some(e) => format!("{}({})", self.variant(e, c), self.held(c)),
                    None => self.held(c).to_string(),
                };
                self.line(&format!("{}(o) => Some({value}),", self.variant(t, c)));
            }
            if passing.len() < variants.len() {
                self.line("_ => None,");
            }
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The dispatcher on the enum of `t` for an override group: each variant runs what its class
    /// runs for the method.
    fn dispatcher(&mut self, t: usize, group: usize) {
        let h = &self.class.hierarchy;
        let mut chain = vec![t];
        chain.extend(h.ancestors(t));
        // The signature is that of the group's method declared nearest above the type; the
        // analyses give every method of a group the same one.
        let members = h.group(group);
        let above = |m: &usize| h.is_subtype(t, self.class.methods[*m].class);
        let Some(r) = chain
            .iter()
            .find_map(|&a| members.iter().copied().find(|&m| self.class.methods[m].class == a))
            .or_else(|| members.iter().copied().find(above))
        else {
            return;
        };
        self.enter_method(r);
        let m = &self.class.methods[r];
        // The arms bind the object to `o`, which must not hide a parameter of that name.
        let names: Vec<String> = m.params.iter().map(|p| self.local(&p.name)).collect();
        for p in &m.params {
            if self.local(&p.name) == "o" {
                let fresh = (1..).map(|n| format!("o{n}")).find(|f| !names.contains(f)).expect("a free name");
                self.locals.insert(p.name.clone(), fresh);
            }
        }
        let params = self.typed_params(r, false);
        let ret = self.ret_text(r);
        let args: Vec<String> = m.params.iter().map(|p| self.local(&p.name)).collect();
        let args = args.join(", ");
        let name = self.group_names[&group].clone();
        let variants = self.enum_variants(t);
        self.gap();
        let all: String = params.iter().map(|p| format!(", {p}")).collect();
        self.line(&format!("{}fn {name}(&self{all}){ret} {{", self.vis()));
        self.indent += 1;
        self.match_arms(t, &variants, &|g: &mut Self, c| match g.class.hierarchy.implementation(r, c) {
            Some(i) => g.arm_call(t, c, i, &args),
            None => (g.builtin_arm(r, c), true),
        });
        self.indent -= 1;
        self.line("}");
    }

    /// The arm for a value of class `c` bound to `o` that runs no method of the program for
    /// method `r`: `Object`'s `equals` or `hashCode`, or for an enum's constant a method that
    /// every enum has, which `lower` checks: `name()`, `toString()` or `ordinal()`.
    fn builtin_arm(&mut self, r: usize, c: usize) -> String {
        if self.is_object_method(r) {
            return self.object_arm(r, c);
        }
        if self.is_comparable_method(r) {
            return self.compare_arm(r, c);
        }
        match self.class.methods[r].name.as_str() {
            "ordinal" => "*o as i32".to_string(),
            "name" => "o.enum_name()".to_string(),
            _ => "o.to_string()".to_string(),
        }
    }

    /// The type of `Object`, when the program uses it.
    fn object_index(&self) -> Option<usize> {
        self.class.types.iter().position(|d| d.kind == TypeKind::Interface && d.name == OBJECT)
    }

    /// The index of `Object`'s method `name`, when the program uses `Object`.
    fn object_method(&self, name: &str) -> Option<usize> {
        let t = self.object_index()?;
        self.class.methods.iter().position(|m| m.class == t && m.name == name)
    }

    /// Whether method `r` is `Object`'s own abstract `equals` or `hashCode`.
    pub(super) fn is_object_method(&self, r: usize) -> bool {
        self.object_index() == Some(self.class.methods[r].class)
    }

    /// Whether method `r` is `Comparable`'s own abstract `compareTo`.
    fn is_comparable_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == COMPARABLE
    }

    /// `Comparable`'s `compareTo` (method `r`) for a value of class `c` bound to `o`, a string,
    /// a box or an enum's constant, compared with the argument, which must be one of the same
    /// class.
    fn compare_arm(&mut self, r: usize, c: usize) -> String {
        let p = &self.class.methods[r].params[0];
        let other = self.local(&p.name);
        let optional = self.optional_var(r, p.ty, &p.name);
        let place = if self.facts.borrowed_objects.contains(&(r, 0)) { other } else { format!("&{other}") };
        let comparable = self.class.methods[r].class;
        let compare = match self.runtime_of(c) {
            Some(ty) => {
                let star = if self.faithful_box(ty) { "**" } else { "*" };
                let (a, b) = (format!("{star}o"), format!("{star}b"));
                match ty.unboxed() {
                    Type::String => format!("{}(o, b)", self.runtime("compare_to")),
                    Type::Int | Type::Long => format!("({a}).cmp(&{b}) as i32"),
                    Type::Boolean => format!("{}({a}, {b})", self.runtime("bool_compare")),
                    Type::Double => format!("{}({a}, {b})", self.runtime("double_compare")),
                    Type::Float => {
                        format!("{}(f64::from({a}), f64::from({b}))", self.runtime("double_compare"))
                    }
                    _ => format!("i32::from({a}) - i32::from({b})"),
                }
            }
            // An enum's constants compare by their ordinals.
            None => "*o as i32 - *b as i32".to_string(),
        };
        let name = self.binary_name(c);
        let failed = format!("{}({})", self.runtime("cast_failed"), rust_string(&name));
        let Some(e) = self.enum_for(comparable) else { return failed };
        let variant = self.variant(e, c);
        let pattern = if optional { format!("Some({variant}(b))") } else { format!("{variant}(b)") };
        if !optional && self.enum_variants(e).len() == 1 {
            return format!("{{ let {pattern} = {place}; {compare} }}");
        }
        format!("match {place} {{ {pattern} => {compare}, _ => {failed} }}")
    }

    /// `Object`'s `equals` or `hashCode` (method `r`) for a value of class `c` bound to `o`
    /// whose class overrides neither: identity for an object, the value's own equality and hash
    /// for an enum's constant, a string or a box.
    fn object_arm(&mut self, r: usize, c: usize) -> String {
        let m = &self.class.methods[r];
        if m.name == "hashCode" {
            return self.value_hash(c, "o");
        }
        let p = &m.params[0];
        let other = self.local(&p.name);
        let optional = self.optional_var(r, p.ty, &p.name);
        let place = if self.facts.borrowed_objects.contains(&(r, 0)) { other } else { format!("&{other}") };
        let same = match self.runtime_of(c).map(Type::unboxed) {
            Some(Type::Double | Type::Float) => "o.to_bits() == b.to_bits()",
            _ if self.holds_value(c) => "o == b",
            _ => "Rc::ptr_eq(o, b)",
        };
        self.equals_test(c, &place, optional, same)
    }

    /// Whether `place`, a reference to an `Object` (an `Option` of one when `optional`), is a
    /// value of class `c` that binds `b` and passes `same`.
    fn equals_test(&mut self, c: usize, place: &str, optional: bool, same: &str) -> String {
        let Some(object) = self.object_index() else { return "false".to_string() };
        let Some(e) = self.enum_for(object) else {
            // Every `Object` is an object of one class.
            if self.holds_value(c)
                || hierarchy::rep(self.class, self.facts, object) != hierarchy::Rep::Object(c)
            {
                return "false".to_string();
            }
            return if optional {
                format!("{place}.as_ref().is_some_and(|b| {same})")
            } else {
                format!("{{ let b = {place}; {same} }}")
            };
        };
        if !self.enum_variants(e).contains(&c) {
            return "false".to_string();
        }
        let variant = self.variant(e, c);
        let pattern = if optional { format!("Some({variant}(b))") } else { format!("{variant}(b)") };
        format!("matches!({place}, {pattern} if {same})")
    }

    /// The address of the object of class `c` that `handle`, a `&Rc<..>`, refers to, which is
    /// its identity: that of the class's struct, also where `this` is a reference to it.
    pub(super) fn address(&self, c: usize, handle: &str) -> String {
        if self.facts.owned.contains(&self.class.types[c].name) {
            format!("std::ptr::from_ref({handle})")
        } else if self.facts.immutable.contains(&self.class.types[c].name) {
            format!("Rc::as_ptr({handle})")
        } else {
            // A method call borrows the handle itself; `&x.as_ptr()` would be a reference to the pointer.
            format!("{}.as_ptr()", handle.strip_prefix('&').unwrap_or(handle))
        }
    }

    /// The address of the object of class `c` that `receiver` is (see `address`).
    pub(super) fn receiver_address(&mut self, receiver: &'a Expr, c: usize) -> String {
        let receiver = self.peel(receiver);
        match receiver.kind {
            ExprKind::This if self.this_form == ThisForm::Local => "std::ptr::from_ref(&this)".to_string(),
            ExprKind::This if self.this_form == ThisForm::SelfRef || self.is_plain(receiver) => {
                format!("std::ptr::from_ref({})", self.this_name())
            }
            _ if self.nullable(receiver) => {
                let handle =
                    format!("{}({}.as_ref())", self.runtime("non_null"), self.object_place(receiver));
                self.address(c, &handle)
            }
            _ => {
                let handle = self.object_ref(receiver);
                self.address(c, &handle)
            }
        }
    }

    /// A call of `Object`'s `equals` or `hashCode` (method `r`) on an object of a class that
    /// overrides neither and that no enum represents: its identity.
    pub(super) fn identity_call(&mut self, receiver: &'a Expr, r: usize, args: &'a [Expr]) -> Code {
        let c = self.struct_class(receiver).expect("an object has a class type");
        let address = self.receiver_address(receiver, c);
        if self.class.methods[r].name == "hashCode" {
            return Code::atom(format!("{}({address})", self.runtime("address_hash")));
        }
        let arg = self.call_args(r, args).remove(0);
        let p = &self.class.methods[r].params[0];
        let optional = self.optional_var(r, p.ty, &p.name);
        let place = if self.facts.borrowed_objects.contains(&(r, 0)) { arg } else { format!("&{arg}") };
        // The object is found before the argument is evaluated, as Java does.
        let temp = self.temp("address");
        let same = format!("std::ptr::addr_eq({temp}, {})", self.address(c, "b"));
        let test = self.equals_test(c, &place, optional, &same);
        // An argument that cannot be the object is still evaluated.
        let test = if test == "false" && has_effects(&args[0]) {
            format!("{{ let _ = {place}; false }}")
        } else {
            test
        };
        Code::atom(format!("{{ let {temp} = {address}; {test} }}"))
    }

    /// `Object.toString()` on a value: the `Display` of its enum.
    pub(super) fn object_to_string(&mut self, receiver: &'a Expr) -> Code {
        let Some(t) = self.enum_of(receiver) else { return self.object_text(receiver) };
        self.need(Need::Display(t));
        Code::atom(format!("{}.to_string()", self.object(receiver)))
    }

    /// The hash code of a value of class `c`, reached through `handle`, a reference to what the
    /// variant holds, whose class does not override `hashCode`.
    pub(super) fn value_hash(&mut self, c: usize, handle: &str) -> String {
        let Some(ty) = self.runtime_of(c) else {
            if self.holds_value(c) {
                // An enum's constant hashes by its identity in Java, which nothing can depend on.
                return format!("*{handle} as i32");
            }
            return format!("{}({})", self.runtime("address_hash"), self.address(c, handle));
        };
        let value = if self.faithful_box(ty) { format!("**{handle}") } else { format!("*{handle}") };
        let function = match ty.unboxed() {
            Type::String => return format!("{}({handle})", self.runtime("string_hash")),
            Type::Int => return value,
            Type::Long => "long_hash",
            Type::Double => "double_hash",
            Type::Float => "float_hash",
            Type::Boolean => "bool_hash",
            _ => return format!("{value} as i32"),
        };
        format!("{}({value})", self.runtime(function))
    }

    /// The text that `Object.toString` makes for an object of class `c` bound to `o` in an arm of
    /// the enum of `t`: the class's binary name and its hash code in hexadecimal.
    fn default_arm_text(&mut self, t: usize, c: usize) -> String {
        let hash = match self.hash_index(c) {
            Some(i) => self.arm_call(t, c, i, "").0,
            None => self.value_hash(c, "o"),
        };
        self.default_text(c, hash)
    }

    /// The text that `Object.toString` makes for an object of class `c` whose hash code is `hash`.
    pub(super) fn default_text(&mut self, c: usize, hash: String) -> String {
        format!("{}({}, {hash})", self.runtime("default_text"), rust_string(&self.binary_name(c)))
    }

    /// The `hashCode` of the program that an object of class `c` runs, if it has one.
    pub(super) fn hash_index(&self, c: usize) -> Option<usize> {
        self.object_method("hashCode").and_then(|h| self.class.hierarchy.implementation(h, c))
    }

    /// The call that the dispatcher of the enum of `t` makes for an object of class `c` bound to
    /// `o`, running method `i`; and whether it uses `o`.
    fn arm_call(&mut self, t: usize, c: usize, i: usize, args: &str) -> (String, bool) {
        let name = self.fn_names[i].clone();
        let immutable = self.facts.immutable.contains(&self.class.types[c].name);
        let borrow = match () {
            _ if immutable || self.holds_value(c) => "",
            _ if self.facts.mutating.contains(&i) => ".borrow_mut()",
            _ => ".borrow()",
        };
        match self.homes[i] {
            Home::Class(_) if self.facts.self_methods.contains(&i) || self.holds_value(c) => {
                (format!("o{borrow}.{name}({args})"), true)
            }
            Home::Class(_) => {
                let all = if args.is_empty() { "o".to_string() } else { format!("o, {args}") };
                (format!("{}({all})", self.qualified_fn(i)), true)
            }
            Home::Fields(d) => {
                let path = hierarchy::base_path(self.facts, self.class, c, d);
                (format!("o{borrow}.{path}{name}({args})"), true)
            }
            Home::Enum(d) => {
                self.need(Need::Body(i));
                if d == t {
                    (format!("self.{name}({args})"), false)
                } else if !self.class.hierarchy.is_subtype(t, d) {
                    // A body inherited from a type below `t`: the object as a value of that type.
                    (format!("{}({}).{name}({args})", self.variant(d, c), self.held(c)), true)
                } else {
                    self.need(Need::Convert(t, d));
                    (format!("self.{}().{name}({args})", self.to_name(d)), false)
                }
            }
            Home::Free | Home::Nowhere => {
                unreachable!("a concrete class runs an instance method with a body")
            }
        }
    }

    /// Identity: two values are equal when they are the same object.
    fn identity_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        self.gap();
        self.line(&format!("impl PartialEq for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn eq(&self, other: &{name}) -> bool {{"));
        self.indent += 1;
        if variants.is_empty() {
            self.line("match *self {}");
        } else {
            self.line("match (self, other) {");
            self.indent += 1;
            for &c in variants {
                let v = self.variant(t, c);
                let same = if self.holds_value(c) { "a == b" } else { "Rc::ptr_eq(a, b)" };
                self.line(&format!("({v}(a), {v}(b)) => {same},"));
            }
            if variants.len() > 1 {
                self.line("_ => false,");
            }
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// `Display`: each object shows as its class's `toString` makes it.
    fn display_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        self.gap();
        self.line(&format!("impl fmt::Display for {name} {{"));
        self.indent += 1;
        self.line("fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {");
        self.indent += 1;
        self.match_arms(t, variants, &|g: &mut Self, c| {
            match g.runtime_of(c) {
                Some(Type::String) => return ("f.write_str(o)".to_string(), true),
                Some(ty) => {
                    let value = if g.faithful_box(ty) { "**o" } else { "*o" };
                    let shown = g.shown(ty.unboxed(), value.to_string());
                    return (format!("fmt::Display::fmt(&{shown}, f)"), true);
                }
                None => {}
            }
            if g.holds_value(c) {
                return ("fmt::Display::fmt(o, f)".to_string(), true);
            }
            match hierarchy::to_string_impl(g.class, c) {
                Some(i) => g.display_arm(t, c, i),
                None => {
                    let text = g.default_arm_text(t, c);
                    (format!("f.write_str(&{text})"), true)
                }
            }
        });
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// How the `Display` of the enum of `t` shows an object of class `c` bound to `o`, whose
    /// `toString` is method `i`; and whether it uses `o`.
    fn display_arm(&mut self, t: usize, c: usize, i: usize) -> (String, bool) {
        let immutable = self.facts.immutable.contains(&self.class.types[c].name);
        let (path, displays) = match self.homes[i] {
            Home::Class(_) => (String::new(), self.displays(i)),
            Home::Fields(d) => (hierarchy::base_path(self.facts, self.class, c, d), self.displays(i)),
            _ => (String::new(), false),
        };
        let path = path.trim_end_matches('.');
        if displays {
            let shown = match (immutable, path.is_empty()) {
                (true, true) => "&**o".to_string(),
                (true, false) => format!("&o.{path}"),
                (false, true) => "&*o.borrow()".to_string(),
                (false, false) => format!("&o.borrow().{path}"),
            };
            return (format!("fmt::Display::fmt({shown}, f)"), true);
        }
        let (text, uses) = self.arm_call(t, c, i, "");
        (format!("f.write_str(&{text})"), uses)
    }
}

/// The enum items that a pass of the generator asked for, which the next pass writes.
pub(super) type Needs = BTreeSet<Need>;

/// The value of an object of class `c` where the program creates none, so the code never runs.
fn absent(g: &Gen, c: usize) -> String {
    format!("unreachable!(\"no object of {} is created\")", g.class.types[c].name)
}
