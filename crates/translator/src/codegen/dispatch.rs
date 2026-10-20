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

    /// The name of the conversion to the enum of `t`: `to_shape`.
    fn to_name(&self, t: usize) -> String {
        let simple = self.class.types[t].name.rsplit(['.', '$']).next().unwrap_or_default();
        format!("to_{}", snake_case(simple).trim_end_matches('_'))
    }

    /// Whether the variant of `c` holds a value of an enum that implements the type, which is
    /// `Copy`, rather than a shared object.
    pub(super) fn holds_value(&self, c: usize) -> bool {
        self.class.types[c].kind == TypeKind::Enum
    }

    /// The value that the variant of `c` bound to `o` holds, as a value of its own: `*o` for an
    /// enum's constant, `o.clone()` for an object.
    fn held(&self, c: usize) -> &'static str {
        if self.holds_value(c) { "*o" } else { "o.clone()" }
    }

    /// The class of the object, or the enum of the constant, that `e` is, and its handle or value.
    fn variant_value(&mut self, e: &'a Expr) -> (usize, String) {
        if let Type::Enum(name) = e.ty {
            let c = self.class.type_index(name).expect("an enum of the program");
            return (c, self.expr(e).text);
        }
        let c = self.struct_class(e).expect("an object has a class type");
        (c, self.object_handle(e))
    }

    /// The Rust type of the object of concrete class `c` that its variant holds.
    fn payload(&self, c: usize) -> String {
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
                format!("{}({handle})", self.variant(t, c))
            }
        }
    }

    /// A conversion of an object to a supertype: the same object as a value of the supertype's
    /// enum. Where both types have the same representation it changes nothing.
    pub(super) fn class_cast(&mut self, to: Type, value: &'a Expr) -> Code {
        let Some(t) = self.enum_type(to) else { return self.expr(value) };
        if matches!(value.kind, ExprKind::Null) {
            return Code::atom("None");
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
                    let c = self.struct_class(value).expect("an object has a class type");
                    format!("{option}.map({})", self.variant(t, c))
                }
            });
        }
        match from {
            Some(s) => {
                self.need(Need::Convert(s, t));
                Code::atom(format!("{}.{}()", self.object(value), self.to_name(t)))
            }
            None => {
                let c = match value.ty {
                    Type::Enum(name) => self.class.type_index(name).expect("an enum of the program"),
                    _ => self.struct_class(value).expect("an object has a class type"),
                };
                let v = self.expr(value).text;
                Code::atom(format!("{}({v})", self.variant(t, c)))
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
            self.line(&format!("{short}({}),", self.payload(c)));
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
            None => (g.enum_builtin_arm(r), true),
        });
        self.indent -= 1;
        self.line("}");
    }

    /// The arm for an enum's constant bound to `o` whose method `r` is one that every enum has,
    /// which `lower` checks: `name()`, `toString()` or `ordinal()`.
    fn enum_builtin_arm(&self, r: usize) -> String {
        match self.class.methods[r].name.as_str() {
            "ordinal" => "*o as i32".to_string(),
            "name" => "o.enum_name()".to_string(),
            _ => "o.to_string()".to_string(),
        }
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
            if g.holds_value(c) {
                return ("fmt::Display::fmt(o, f)".to_string(), true);
            }
            let i = hierarchy::to_string_impl(g.class, c)
                .expect("lowering checks that printed classes have toString");
            g.display_arm(t, c, i)
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
