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
        match self.rep(t) {
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
        self.variants(t).iter().copied().filter(|&c| self.live[c] || self.holds_value(c)).collect()
    }

    /// The path of the variant of class `c` in the enum of `t`: `Shape::Circle`. A variant is
    /// named after its class, with the class's outer classes when two variants share a name, and
    /// with its package too when two classes of different packages share that
    /// (`JavaUtilStreamAbstractSetInEntrySet`).
    pub(super) fn variant(&self, t: usize, c: usize) -> String {
        self.need(Need::Enum(t));
        let names = self.variant_names_of(t);
        let name = match names.get(&c) {
            Some(name) => name.clone(),
            None => self.variant_name(&self.variants(t), c),
        };
        format!("{}::{name}", struct_name(&self.class.types[t].name))
    }

    /// `hierarchy::rep`, worked out once for each type.
    pub(super) fn rep(&self, t: usize) -> hierarchy::Rep {
        if let Some(&rep) = self.reps.borrow().get(&t) {
            return rep;
        }
        let rep = hierarchy::rep(self.class, self.facts, t);
        self.reps.borrow_mut().insert(t, rep);
        rep
    }

    /// `hierarchy::variants`, worked out once for each type.
    pub(super) fn variants(&self, t: usize) -> std::rc::Rc<Vec<usize>> {
        if let Some(variants) = self.variant_lists.borrow().get(&t) {
            return variants.clone();
        }
        let variants = std::rc::Rc::new(hierarchy::variants(self.class, self.facts, t));
        self.variant_lists.borrow_mut().insert(t, variants.clone());
        variants
    }

    /// The names of the variants of the enum of `t`, made once for all of them.
    fn variant_names_of(&self, t: usize) -> std::rc::Rc<HashMap<usize, String>> {
        if let Some(names) = self.variant_names.borrow().get(&t) {
            return names.clone();
        }
        let variants = self.variants(t);
        let count = |name: &dyn Fn(usize) -> String| {
            let mut counts: HashMap<String, usize> = HashMap::new();
            for &o in variants.iter() {
                *counts.entry(name(o)).or_default() += 1;
            }
            counts
        };
        let (simple, in_package) = (count(&|c| self.simple_name(c)), count(&|c| self.in_package_name(c)));
        let names: HashMap<usize, String> = variants
            .iter()
            .map(|&c| {
                let name = if simple[&self.simple_name(c)] <= 1 {
                    self.simple_name(c)
                } else if in_package[&self.in_package_name(c)] <= 1 {
                    self.in_package_name(c)
                } else {
                    self.full_variant_name(c)
                };
                (c, name)
            })
            .collect();
        let names = std::rc::Rc::new(names);
        self.variant_names.borrow_mut().insert(t, names.clone());
        names
    }

    /// The name of the variant for class `c` among `variants`: its simple name, unless another
    /// variant shares it, then its name in its package, then its whole name.
    fn variant_name(&self, variants: &[usize], c: usize) -> String {
        let shared = |name: &dyn Fn(usize) -> String| {
            let own = name(c);
            variants.iter().filter(|&&o| name(o) == own).count() > 1
        };
        if !shared(&|c| self.simple_name(c)) {
            self.simple_name(c)
        } else if !shared(&|c| self.in_package_name(c)) {
            self.in_package_name(c)
        } else {
            self.full_variant_name(c)
        }
    }

    fn in_package_name(&self, c: usize) -> String {
        modules::split(&self.class.types[c].name).1.replace(['.', '$'], "")
    }

    fn full_variant_name(&self, c: usize) -> String {
        self.class.types[c].name.split(['.', '$']).map(capitalized).collect()
    }

    /// `variant`, when the enum of `t` has a variant for class `c`: it has none when no object of
    /// `c` is ever created, where code that has one never runs.
    pub(super) fn variant_of(&self, t: usize, c: usize) -> Option<String> {
        self.enum_variants(t).contains(&c).then(|| self.variant(t, c))
    }

    /// The name of the conversion to the enum of `t`: `to_shape`.
    pub(super) fn to_name(&self, t: usize) -> String {
        format!("to_{}", snake_case(&self.distinct_name(t)).trim_end_matches('_'))
    }

    /// The name of type `t` in the names of its conversions: its simple name, or with its outer
    /// classes when another type has that simple name too (`ImageReaderNode` and
    /// `ConcurrentHashMapNode`), or with its package as well when that is shared too.
    fn distinct_name(&self, t: usize) -> String {
        let simple = self.simple_name(t);
        if !self.shared_names.contains(&simple) || self.class.array_of(t).is_some() {
            return simple;
        }
        let in_package = self.in_package_name(t);
        if !self.shared_names.contains(&in_package) {
            return in_package;
        }
        self.full_variant_name(t)
    }

    /// The simple names, and the names with outer classes, that more than one type has.
    pub(super) fn shared_names(&self) -> HashSet<String> {
        let mut seen = HashSet::new();
        let mut shared = HashSet::new();
        for t in (0..self.class.types.len()).filter(|&t| self.class.array_of(t).is_none()) {
            for name in [self.simple_name(t), format!("{}\0", self.in_package_name(t))] {
                if !seen.insert(name.clone()) {
                    shared.insert(name.trim_end_matches('\0').to_string());
                }
            }
        }
        shared
    }

    /// The simple name of type `t`, which names its variants and conversions: `Circle` for
    /// `shapes.Circle`, `IntArray` for the arrays `int[]` that become `Object`s.
    fn simple_name(&self, t: usize) -> String {
        if self.class.array_of(t).is_some() {
            let name = &self.class.types[t].name;
            let element = name.trim_end_matches("[]");
            let simple = element.rsplit(['.', '$']).next().unwrap_or_default();
            let dimensions = (name.len() - element.len()) / 2;
            return capitalized(simple) + &"Array".repeat(dimensions);
        }
        self.class.types[t].name.rsplit(['.', '$']).next().unwrap_or_default().to_string()
    }

    /// The name of the checked conversion to type `t`: `as_circle`.
    fn as_name(&self, t: usize) -> String {
        format!("as_{}", snake_case(&self.distinct_name(t)).trim_end_matches('_'))
    }

    /// The binary name of type `t`, as the JVM shows it: `shapes.Outer$Inner`.
    fn binary_name(&self, t: usize) -> String {
        let decl = &self.class.types[t];
        if let Some(array) = self.class.array_of(t) {
            return self.descriptor(array);
        }
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
            TypeKind::Runtime => self.runtime_of(t).expect("a runtime class"),
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

    /// `(T) value`, a checked downcast, or a cast across, from the enum of `s`; `checked` when
    /// a `catch` or a `finally` sees its failure (see `checked_class_cast`).
    fn downcast(&mut self, s: usize, to: Type, value: &'a Expr, checked: Option<ExprId>) -> Code {
        let t = self.type_of(to).expect("a cast to a type of the program");
        // No class of the enum is a `T` (no object of one is created): only null passes.
        if self.passing(s, t).is_empty() {
            return self.failing_cast(to, value, checked);
        }
        self.need(Need::Narrow(s, t));
        let name = self.as_name(t);
        let cast = self.runtime(if checked.is_some() { "try_cast" } else { "cast" });
        let target = rust_string(&self.binary_name(t));
        let text = if self.nullable(value) {
            let option = self.object_place(value);
            let enum_name = struct_name(&self.class.types[s].name);
            format!("{option}.as_ref().map(|o| {cast}({enum_name}::{name}(o), {target}))")
        } else {
            let obj = self.object(value);
            format!("{cast}({obj}.{name}(), {target})")
        };
        self.cast_result(text, checked, self.nullable(value))
    }

    /// A cast of a value whose class is known to a type that it is not of, which always fails
    /// (unless the value is null).
    fn failing_cast(&mut self, to: Type, value: &'a Expr, checked: Option<ExprId>) -> Code {
        let t = self.type_of(to).expect("a cast to a type of the program");
        if checked.is_some() {
            let ty = self.ty_name(to);
            let fail =
                format!("{}(None::<{ty}>, {})", self.runtime("try_cast"), rust_string(&self.binary_name(t)));
            let text = if self.nullable(value) {
                format!("{}.as_ref().map(|_| {fail})", self.object_place(value))
            } else {
                format!("{{ let _ = {}; {fail} }}", self.expr(value).text)
            };
            return self.cast_result(text, checked, self.nullable(value));
        }
        let failed = format!("{}({})", self.runtime("cast_failed"), rust_string(&self.binary_name(t)));
        if self.nullable(value) {
            let option = self.object_place(value);
            let ty = self.ty_name(to);
            return Code::atom(format!("{option}.as_ref().map(|_| -> {ty} {{ {failed} }})"));
        }
        // The value is typed, as `cast_failed` returns `!` and a local it initializes may be used.
        let v = self.expr(value).text;
        let ty = self.ty_name(to);
        Code::atom(format!("{{ let _ = {v}; None::<{ty}>.unwrap_or_else(|| {failed}) }}"))
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

    /// The pattern that binds `name`, of type `ty`, from `source` (`pattern_source`):
    /// `Some(name)`. Where no value of the program is a `T` (`None::<T>`), the code that would run
    /// with it may still assign fields of it, as of a value of a class that no object of the
    /// program is: it is bound `mut` there, as it is where the method assigns the variable. A
    /// variable that the method may set to null is an `Option` like any other local, bound whole
    /// (`mut name @ Some(_)`).
    pub(super) fn pattern_binding(&self, name: &str, source: &str, ty: Type) -> String {
        let local = self.local(name);
        let assigned = assigns_any(&self.class.methods[self.method].body, name);
        if self.optional_var(self.method, ty, name) {
            return format!("mut {local} @ Some(_)");
        }
        if source.starts_with("None::<") || assigned {
            format!("Some(mut {local})")
        } else {
            format!("Some({local})")
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
                if self.passing(s, t).is_empty() {
                    return format!("None::<{}>", self.ty_name(to));
                }
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

    /// The string `s` (a `String`) as an `Object`.
    pub(super) fn string_as_object(&mut self, s: &str) -> String {
        let object = self.object_index().expect("an `Object[]` in a program that uses `Object`");
        let string = self.type_of(Type::String).expect("strings among `Object`'s runtime classes");
        format!("{}({s})", self.variant(object, string))
    }

    /// `StringArrayOf` and `ObjectArrayOf`: a copy of array `e` (an `Object[]` or a `String[]`) as
    /// an array of type `ty` (the other), each string cast back from its `Object` or put in one.
    pub(super) fn array_conversion(&mut self, builtin: Builtin, e: &'a Expr, ty: Type) -> Code {
        let from_nullable = self.nullable_element(crate::codegen::arrays::element(e.ty));
        let to_nullable = self.nullable_element(crate::codegen::arrays::element(ty));
        // Each element of the new array: from a plain element `s` (`plain`), or from an `Option`
        // (`optional`, an `Option` of the new element).
        let (plain, optional) = if builtin == Builtin::ObjectArrayOf {
            (self.string_as_object("s.clone()"), format!("s.clone().map(|s| {})", self.string_as_object("s")))
        } else {
            let object = self.object_index().expect("an `Object[]` in a program that uses `Object`");
            let string = self.type_of(Type::String).expect("strings among `Object`'s runtime classes");
            self.need(Need::Narrow(object, string));
            let (cast, name) = (self.runtime("cast"), self.as_name(string));
            let target = rust_string("java.lang.String");
            (
                format!("{cast}(s.{name}(), {target})"),
                format!("s.as_ref().map(|o| {cast}(o.{name}(), {target}))"),
            )
        };
        let each = match (from_nullable, to_nullable) {
            (false, false) => plain,
            (false, true) => format!("Some({plain})"),
            (true, true) => optional,
            (true, false) => format!("{}({optional})", self.runtime("non_null")),
        };
        let slice = self.slice_of(e);
        let maker = self.array_maker("from_vec");
        Code::atom(format!("{maker}(({slice}).iter().map(|s| {each}).collect::<Vec<_>>())"))
    }

    /// `System.arraycopy(src, srcPos, dest, destPos, length)` where `src` and `dest` are
    /// `Object`s: the copy between their arrays when both are arrays of one type, else Java's
    /// `ArrayStoreException`. The positions are computed first; a null `src` or `dest` throws
    /// `NullPointerException`. Where a `catch` or a `finally` sees the copy's failure
    /// (`Facts::raising`), the checked form, whose failure goes on from here.
    pub(super) fn object_array_copy(&mut self, args: &'a [Expr], id: ExprId) -> Code {
        let [src, src_pos, dest, dest_pos, length] = args else {
            unreachable!("arraycopy has five arguments")
        };
        let ints: Vec<String> =
            [src_pos, dest_pos, length].iter().map(|e| self.value(e, Type::Int).text).collect();
        let (s, d) = (self.object(src), self.object(dest));
        let checked = self.facts.raising.contains(&id);
        let copy = self.runtime(if checked { "try_arraycopy" } else { "arraycopy" });
        let throw = if checked {
            format!("Err::<(), _>({})", self.runtime("Exception::new"))
        } else {
            self.runtime("throw")
        };
        // Names that no local of the method has, as `src` and `dest` may read one.
        let [p, q, n] = ["src_pos", "dest_pos", "length"].map(|base| self.temp(base));
        let mut arms = Vec::new();
        if let Some(t) = self.object_index() {
            for c in self.enum_variants(t).into_iter().filter(|&c| self.class.array_of(c).is_some()) {
                let variant = self.variant(t, c);
                arms.push(format!("({variant}(src), {variant}(dest)) => {copy}(src, {p}, dest, {q}, {n}),"));
            }
        }
        arms.push(format!("_ => {throw}(\"java.lang.ArrayStoreException\", \"arraycopy: type mismatch\"),"));
        let code = Code::atom(format!(
            "{{ let ({p}, {q}, {n}) = ({}, {}, {}); match (&{s}, &{d}) {{ {} }} }}",
            ints[0],
            ints[1],
            ints[2],
            arms.join(" ")
        ));
        if !checked {
            return code;
        }
        Code::atom(self.passed_on(code))
    }

    /// The JVM's name of an array class, as `getClass().getName()` gives it: `[I`,
    /// `[Ljava.lang.String;`, `[[Lshapes.Point;`.
    fn descriptor(&self, ty: Type) -> String {
        match ty {
            Type::Array(element) => format!("[{}", self.descriptor(*element)),
            Type::Int => "I".to_string(),
            Type::Long => "J".to_string(),
            Type::Double => "D".to_string(),
            Type::Float => "F".to_string(),
            Type::Byte => "B".to_string(),
            Type::Short => "S".to_string(),
            Type::Char => "C".to_string(),
            Type::Boolean => "Z".to_string(),
            Type::String => "Ljava.lang.String;".to_string(),
            Type::StringBuilder => "Ljava.lang.StringBuilder;".to_string(),
            Type::Boxed(_) => format!("Ljava.lang.{ty};"),
            Type::Class(name, _) if name == OBJECT => "Ljava.lang.Object;".to_string(),
            Type::Class(name, _) | Type::Enum(name) => match self.class.type_index(name) {
                Some(t) => format!("L{};", self.binary_name(t)),
                None => format!("L{name};"),
            },
            _ => ty.to_string(),
        }
    }

    /// The Java type of the values of runtime class `c`, if it is one: a string, a box, or the
    /// arrays of an array class.
    pub(super) fn runtime_of(&self, c: usize) -> Option<Type> {
        if let Some(shared) = self.class.shared_of(c) {
            return Some(shared);
        }
        if self.class.types[c].kind == TypeKind::Runtime && self.class.types[c].name == CLASS_VALUE {
            return Some(Type::JavaClass);
        }
        (self.class.types[c].kind == TypeKind::Runtime)
            .then(|| runtime_type(&self.class.types[c].name))
            .flatten()
    }

    /// The value that the variant of `c` bound to `o` holds, as a value of its own: `*o` for an
    /// enum's constant or a plain box, `o.clone()` for an object or a string.
    fn held(&self, c: usize) -> &'static str {
        let copied = match self.runtime_of(c) {
            Some(ty) => {
                !matches!(ty, Type::String | Type::StringBuilder | Type::Array(_)) && !self.faithful_box(ty)
            }
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
        let (pointer, cell) = (self.pointer(c), self.cell(c));
        if self.facts.immutable.contains(&self.class.types[c].name) {
            format!("{pointer}<{structure}>")
        } else {
            format!("{pointer}<{cell}<{structure}>>")
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
        match (self.fields_root(t), self.class.field_owner(ty, name)) {
            (Some(root), Some(owner)) => {
                hierarchy::base_path(self.facts, self.class, root, owner)
                    + &field_name(&self.class.types[owner], name)
            }
            (None, Some(owner)) => field_name(&self.class.types[owner], name),
            _ => snake_case(name),
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
                        format!("None::<{ty}>.unwrap_or_else(|| {})", absent(self, c))
                    }
                }
            }
        }
    }

    /// A conversion of an object to a supertype: the same object as a value of the supertype's
    /// enum. Where both types have the same representation it changes nothing.
    pub(super) fn class_cast(&mut self, to: Type, value: &'a Expr) -> Code {
        let checked = self.checked_cast.take();
        if matches!(value.kind, ExprKind::Null) {
            return Code::atom("None");
        }
        // A string, a builder or a box as a value of a type of the program whose values do not
        // include it, which `lower::hierarchy::convert` leaves to the generator.
        if matches!(value.ty, Type::String | Type::StringBuilder | Type::Boxed(_))
            && !self
                .type_of(value.ty)
                .zip(self.type_of(to))
                .is_some_and(|(s, t)| self.class.hierarchy.is_subtype(s, t))
        {
            let message = format!("a `{}` as a `{to}` not supported yet", value.ty);
            self.rejected.push(Error { pos: value.pos, message });
            return Code::atom("unreachable!()");
        }
        match self.narrowing(value, to) {
            Narrowing::Down(s) => return self.downcast(s, to, value, checked),
            Narrowing::Known(false) => return self.failing_cast(to, value, checked),
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
                    format!("{{ let _ = {text}; None::<{ty}>.unwrap_or_else(|| {}) }}", absent(self, c))
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
                        // A borrowed optional string is an `Option<&str>`.
                        Some(variant) if matches!(&value.kind, ExprKind::Var(name) if self.borrowed_optional.contains(name)) =>
                        {
                            format!("{option}.map(|s| {variant}(s.to_string()))")
                        }
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
                    return Code::atom(format!("None::<{ty}>.unwrap_or_else(|| {})", absent(self, c)));
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

    /// A call at `pos` of instance method `resolved` on a value of the enum of `t`: the method of
    /// the enum that runs what the object's class runs. `toString` is the enum's `Display`.
    pub(super) fn enum_call(
        &mut self,
        t: usize,
        pos: Pos,
        receiver: &'a Expr,
        resolved: usize,
        args: &'a [Expr],
    ) -> Code {
        let m = &self.class.methods[resolved];
        // A class of the program that has none of `CharSequence`'s methods (`lower` links its own
        // or the default `getChars`, so none should) rejects each call that may reach it, and
        // the dispatcher's arm for it never runs.
        if self.is_char_sequence_method(resolved)
            && self.enum_variants(t).into_iter().any(|c| self.missing_sequence_method(resolved, c))
        {
            let message = format!("`CharSequence.{}` of a class without one not supported yet", m.name);
            self.rejected.push(Error { pos, message });
            return Code::atom("unreachable!()".to_string());
        }
        if m.name == "toString" && m.params.is_empty() {
            self.need(Need::Display(t));
            let text = format!("{}.to_string()", self.object(receiver));
            // A `toString` that an implementation may return null from is an `Option` where its
            // value is used (`nullness`); the `Display` shows such a null as "null".
            if self.facts.nullable_returns.contains(&resolved) {
                return Code::atom(format!("Some({text})"));
            }
            return Code::atom(text);
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
    pub(super) fn dispatch_name(&self, t: usize, resolved: usize) -> String {
        let h = &self.class.hierarchy;
        if let [only] = self.enum_targets(t, resolved)[..]
            && self.homes[only] == Home::Enum(t)
            && self.same_ret(only, resolved)
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
                let name = self.fn_names[index].clone();
                // The object is borrowed before the arguments run, so arguments that may reach an
                // object of the class run first, as `bound_call` runs them.
                let Type::Class(class, _) = receiver.ty else { unreachable!("a receiver has a class type") };
                if args.iter().any(|a| has_effects(a) || reads_class(self.class, a, class)) {
                    let outer = std::mem::replace(&mut self.guarded, false);
                    let mut lines = Vec::new();
                    let obj = match &receiver.kind {
                        ExprKind::This | ExprKind::Var(_) => self.object(receiver),
                        _ => {
                            let temp = self.temp("object");
                            lines.push(format!("let {temp} = {};", self.object(receiver)));
                            temp
                        }
                    };
                    let texts = self.call_args(index, args);
                    let mut bound = Vec::new();
                    for (text, p) in texts.into_iter().zip(&self.class.methods[index].params) {
                        let mut temp = self.temp(&snake_case(&p.name));
                        while bound.contains(&temp) || temp == obj {
                            temp.push('_');
                        }
                        lines.push(format!("let {temp} = {text};"));
                        bound.push(temp);
                    }
                    self.guarded = outer;
                    lines.push(format!("{obj}{borrow}.{path}{name}({})", bound.join(", ")));
                    return Some(Code::atom(format!("{{ {} }}", lines.join(" "))));
                }
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
        if self.enum_text_throws(t) {
            // A `toString` that throws is called, through `try_text`.
            return self.passed_on(Code::atom(format!("{obj}.try_text()")));
        }
        if self.lazy_text && !self.printing { obj } else { format!("{obj}.to_string()") }
    }

    /// Whether turning a value of the enum of `t` into text may throw an exception that the
    /// program observes: some variant's `toString` throws.
    pub(super) fn enum_text_throws(&self, t: usize) -> bool {
        self.enum_variants(t)
            .into_iter()
            .filter_map(|c| hierarchy::to_string_impl(self.class, c))
            .any(|i| self.facts.throwing.contains(&i))
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
        if self.facts.closure_types.contains(&t) {
            return self.closure_alias(t);
        }
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
        self.monitor_impl(t, &variants);
        if self.wanted.contains(&Need::Display(t)) {
            self.display_impl(t, &variants);
            if self.enum_text_throws(t) {
                self.try_text_impl(t, &variants);
            }
        }
        self.raised_impl(t);
        self.chain_enum_items(t, &variants);
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
        // The borrows of a `java::Locked` are the runtime's.
        let java = if self.facts.thread_shared.contains(&root) { "java::" } else { "" };
        self.line(&match (mutable, immutable) {
            (false, true) => format!("{vis}fn base(&self) -> &{structure} {{"),
            (false, false) => format!("{vis}fn base(&self) -> {java}Ref<'_, {structure}> {{"),
            _ => format!("{vis}fn base_mut(&self) -> {java}RefMut<'_, {structure}> {{"),
        });
        self.indent += 1;
        self.match_arms(t, &variants, &|g: &mut Self, c| {
            let path = hierarchy::base_path(g.facts, g.class, c, root);
            let path = path.trim_end_matches('.');
            let text = match (mutable, immutable, path.is_empty()) {
                (false, true, true) => "o.as_ref()".to_string(),
                (false, true, false) => format!("&o.{path}"),
                (false, false, true) => "o.borrow()".to_string(),
                (false, false, false) => format!("{java}Ref::map(o.borrow(), |o| &o.{path})"),
                (true, _, true) => "o.borrow_mut()".to_string(),
                (true, _, false) => format!("{java}RefMut::map(o.borrow_mut(), |o| &mut o.{path})"),
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
        // The flows of the analysis may keep a class's objects from ever becoming values of `to`
        // (`hierarchy::flows`); that arm never runs.
        self.match_arms(t, &variants, &|g: &mut Self, c| match g.variant_of(to, c) {
            Some(variant) => (format!("{variant}({})", g.held(c)), true),
            None => {
                let (from, to) = (&g.class.types[c].name, &g.class.types[to].name);
                (format!("unreachable!(\"an object of {from} never becomes a value of {to}\")"), true)
            }
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
            Some(i) => {
                let (text, uses) = g.arm_call(t, c, i, &args);
                (g.covariant_ret(i, r, text), uses)
            }
            // What runs no method of the program cannot throw, but the group may (a `hashCode`
            // that iterates a map), and then every arm gives a `Result`.
            None if g.facts.throwing.contains(&r) => (format!("Ok({})", g.builtin_arm(r, c)), true),
            None => (g.builtin_arm(r, c), true),
        });
        self.indent -= 1;
        self.line("}");
    }

    /// The arm for a value of class `c` bound to `o` that runs no method of the program for
    /// method `r`: `Object`'s `equals` or `hashCode`, or for an enum's constant a method that
    /// every enum has, which `lower` checks: `name()`, `toString()` or `ordinal()`.
    fn builtin_arm(&mut self, r: usize, c: usize) -> String {
        // An interface may declare `Object`'s `equals` or `hashCode` again (as `Comparator` does).
        if self.class.hierarchy.group(r).into_iter().any(|g| self.is_object_method(g)) {
            return self.object_arm(r, c);
        }
        if self.is_comparable_method(r) {
            return self.compare_arm(r, c);
        }
        if self.is_number_method(r) {
            return self.number_arm(r, c);
        }
        if self.is_char_sequence_method(r) {
            return self.char_sequence_arm(r, c);
        }
        if self.is_appendable_method(r) {
            return self.appendable_arm(r, c);
        }
        match self.class.methods[r].name.as_str() {
            "ordinal" => "*o as i32".to_string(),
            "name" => "o.enum_name()".to_string(),
            _ => "o.to_string()".to_string(),
        }
    }

    /// The type of `Object`, when the program uses it.
    pub(super) fn object_index(&self) -> Option<usize> {
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

    /// Whether method `r` is one of `CharSequence`'s own abstract methods.
    fn is_char_sequence_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == CHAR_SEQUENCE
    }

    /// Whether a value of class `c` has no method for `CharSequence`'s method `r`: it is no
    /// string or builder, and no method of the program implements `r` for it.
    fn missing_sequence_method(&self, r: usize, c: usize) -> bool {
        self.runtime_of(c).is_none() && self.class.hierarchy.implementation(r, c).is_none()
    }

    /// `CharSequence`'s `length()`, `charAt(index)`, `subSequence(start, end)` or `getChars(..)`
    /// (method `r`) for a string or a builder of class `c` bound to `o`. What a string's throws
    /// is thrown for a `java::catching` around the call (see `analysis::exceptions`), as a
    /// builder's is.
    fn char_sequence_arm(&mut self, r: usize, c: usize) -> String {
        let method = &self.class.methods[r];
        let builder = self.runtime_of(c) == Some(Type::StringBuilder);
        if self.missing_sequence_method(r, c) {
            // Each call that may reach it is rejected (`enum_call`).
            return "unreachable!()".to_string();
        }
        if method.name == "subSequence" {
            return self.sub_sequence_arm(r, builder);
        }
        if method.name == "getChars" {
            return self.get_chars_arm(r, builder);
        }
        match (method.name.as_str(), builder) {
            ("length", true) => "o.length()".to_string(),
            ("length", false) => format!("{}(o)", self.runtime("string_length")),
            (_, true) => format!("o.char_at({})", self.local(&method.params[0].name)),
            (_, false) => {
                let index = self.local(&method.params[0].name);
                format!(
                    "{}(o, {index}).unwrap_or_else(|e| {}(e))",
                    self.runtime("try_char_at"),
                    self.runtime("throw_exception")
                )
            }
        }
    }

    /// `getChars(srcBegin, srcEnd, dst, dstBegin)` (method `r`) of a string or a builder bound to
    /// `o`, whose `dst` may be null. What it throws is thrown for a `java::catching` around the
    /// call (see `analysis::exceptions`).
    fn get_chars_arm(&mut self, r: usize, builder: bool) -> String {
        let method = &self.class.methods[r];
        let names: Vec<String> = method.params.iter().map(|p| self.local(&p.name)).collect();
        let [begin, end, dst, dst_begin] = &names[..] else { unreachable!("`getChars` has four parameters") };
        let dst = match self.optional_var(r, method.params[2].ty, &method.params[2].name) {
            true => format!("{dst}.as_ref()"),
            false => format!("Some(&{dst})"),
        };
        if builder {
            format!("o.sequence_get_chars({begin}, {end}, {dst}, {dst_begin})")
        } else {
            format!("{}(o, {begin}, {end}, {dst}, {dst_begin})", self.runtime("sequence_get_chars"))
        }
    }

    /// `subSequence(start, end)` (method `r`) of a string or a builder bound to `o`: its
    /// `substring`, as the string variant of `CharSequence`'s enum.
    fn sub_sequence_arm(&mut self, r: usize, builder: bool) -> String {
        let method = &self.class.methods[r];
        let (start, end) = (self.local(&method.params[0].name), self.local(&method.params[1].name));
        let text = if builder {
            format!("o.substring({start}, {end})")
        } else {
            let thrown = self.runtime("throw_exception");
            format!("{}(o, {start}, {end}).unwrap_or_else(|e| {thrown}(e))", self.runtime("try_substring"))
        };
        let char_sequence = method.class;
        let string =
            self.class.type_of(Type::String).filter(|&s| self.enum_variants(char_sequence).contains(&s));
        let Some(string) = string else {
            let message = "`subSequence` of a builder held as a `CharSequence` where no string is one not supported yet";
            self.rejected.push(Error { pos: method.pos, message: message.to_string() });
            return "unreachable!()".to_string();
        };
        let value = format!("{}({text})", self.variant(char_sequence, string));
        if self.facts.nullable_returns.contains(&r) { format!("Some({value})") } else { value }
    }

    /// Whether method `r` is one of the JDK's `Appendable`'s own `append` methods, which a builder
    /// answers (see `lower::object`).
    fn is_appendable_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == "java.lang.Appendable"
    }

    /// `Appendable`'s `append(csq)`, `append(csq, start, end)` or `append(c)` (method `r`) for a
    /// builder of class `c` bound to `o`: the builder's own, which returns the builder itself. A
    /// null `csq` appends `"null"`, as Java's does.
    fn appendable_arm(&mut self, r: usize, c: usize) -> String {
        let method = &self.class.methods[r];
        let appendable = method.class;
        if self.runtime_of(c) != Some(Type::StringBuilder) {
            return "unreachable!()".to_string();
        }
        let names: Vec<String> = method.params.iter().map(|p| self.local(&p.name)).collect();
        let append = match (&method.params[..], &names[..]) {
            ([p], [x]) if p.ty == Type::Char => format!("o.append_char({x})"),
            ([p, ..], [csq, rest @ ..]) if matches!(p.ty, Type::Class(..)) => {
                if let Some(t) = self.type_of(p.ty) {
                    self.need(Need::Display(t));
                }
                let text = if self.optional_var(r, p.ty, &p.name) {
                    format!("match {csq} {{ Some(s) => s.to_string(), None => \"null\".to_string() }}")
                } else {
                    format!("{csq}.to_string()")
                };
                match rest {
                    [] => format!("o.append_str(&{text})"),
                    [start, end] => format!("o.append_range(&{text}, {start}, {end})"),
                    _ => return "unreachable!()".to_string(),
                }
            }
            _ => return "unreachable!()".to_string(),
        };
        let builder = format!("{}(o.clone())", self.variant(appendable, c));
        let value =
            if self.facts.nullable_returns.contains(&r) { format!("Some({builder})") } else { builder };
        format!("{{ {append}; {value} }}")
    }

    /// Whether method `r` is one of `Number`'s own abstract `byteValue()` to `doubleValue()`.
    fn is_number_method(&self, r: usize) -> bool {
        let t = &self.class.types[self.class.methods[r].class];
        t.kind == TypeKind::Interface && t.name == NUMBER
    }

    /// `Number`'s `intValue()` and the others (method `r`) for a box of class `c` bound to `o`:
    /// its value converted as Java's primitive conversions convert it, which Rust's `as` does
    /// (wrapping to narrower integers, saturating from floating point, NaN to 0). Floating point
    /// becomes a `byte` or a `short` through an `int`, as in Java.
    fn number_arm(&self, r: usize, c: usize) -> String {
        let ret = self.class.methods[r].ret;
        let Some(ty) = self.runtime_of(c) else { return "unreachable!()".to_string() };
        let value = if self.faithful_box(ty) { "**o" } else { "*o" };
        let from = ty.unboxed();
        if from == ret {
            return value.to_string();
        }
        if matches!(from, Type::Float | Type::Double) && matches!(ret, Type::Byte | Type::Short) {
            return format!("(({value}) as i32) as {}", rust_type(ret));
        }
        format!("({value}) as {}", rust_type(ret))
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
            // Arrays are `Arc`s in a program with threads, as `array_identity` compares them.
            Some(Type::Array(_)) if self.facts.threaded => "Arc::ptr_eq(o, b)",
            Some(Type::Array(_)) => "Rc::ptr_eq(o, b)",
            Some(Type::StringBuilder) => "o.same(b)",
            // A box whose identity is kept (`java::Boxed`) compares the values it holds.
            _ if self.runtime_of(c).is_some_and(|ty| self.faithful_box(ty)) => "**o == **b",
            _ if self.holds_value(c) => "o == b",
            _ if self.facts.thread_shared.contains(&c) => "Arc::ptr_eq(o, b)",
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
            if self.holds_value(c) || self.rep(object) != hierarchy::Rep::Object(c) {
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
            format!("{}::as_ptr({handle})", self.pointer(c))
        } else {
            // A method call borrows the handle itself; `&x.as_ptr()` would be a reference to the pointer.
            format!("{}.as_ptr()", handle.strip_prefix('&').unwrap_or(handle))
        }
    }

    /// The address of the object of class `c` that `receiver` is (see `address`).
    pub(super) fn receiver_address(&mut self, receiver: &'a Expr, c: usize) -> String {
        let id = receiver.id;
        let receiver = self.peel(receiver);
        match receiver.kind {
            ExprKind::This if self.this_form == ThisForm::Local => "std::ptr::from_ref(&this)".to_string(),
            ExprKind::This if self.this_form == ThisForm::SelfRef || self.is_plain(receiver) => {
                format!("std::ptr::from_ref({})", self.this_name())
            }
            _ if self.nullable(receiver) => {
                let option = format!("{}.as_ref()", self.object_place(receiver));
                let handle = self.dereference(&[id, receiver.id], &option);
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
        let obj = self.object(receiver);
        if self.enum_text_throws(t) {
            return Code::atom(self.passed_on(Code::atom(format!("{obj}.try_text()"))));
        }
        Code::atom(format!("{obj}.to_string()"))
    }

    /// The hash code of a value of class `c`, reached through `handle`, a reference to what the
    /// variant holds, whose class does not override `hashCode`.
    pub(super) fn value_hash(&mut self, c: usize, handle: &str) -> String {
        if self.class.shared_of(c).is_some() {
            return format!("{}({handle})", self.runtime("identity_hash"));
        }
        let Some(ty) = self.runtime_of(c) else {
            if self.holds_value(c) {
                // An enum's constant hashes by its identity in Java, which nothing can depend on.
                return format!("*{handle} as i32");
            }
            return format!("{}({})", self.runtime("address_hash"), self.address(c, handle));
        };
        // A class's hash is its identity's in Java: there is one class object for each class.
        if ty == Type::JavaClass {
            return format!("{handle}.hash_code()");
        }
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
            Some(i) => {
                let call = self.arm_call(t, c, i, "").0;
                self.shown_hash(i, call)
            }
            None => self.value_hash(c, "o"),
        };
        self.default_text(c, hash)
    }

    /// The call `call` of `hashCode` method `i` for the text that `Object.toString` makes: what it
    /// throws ends the program, as what a `toString` throws does where an object's text is made
    /// (`analysis::exceptions`).
    pub(super) fn shown_hash(&mut self, i: usize, call: String) -> String {
        if !self.facts.throwing.contains(&i) {
            return call;
        }
        format!("{call}.unwrap_or_else({})", self.uncaught("uncaught"))
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
                // An array is a shared handle, the same array only when it is the same handle; in a
                // program with threads, an `Arc`, as a shared object is.
                let array = self.class.array_of(c).is_some();
                // A box whose identity is kept is the same box only when it is the same object.
                let faithful = self.runtime_of(c).is_some_and(|ty| self.faithful_box(ty));
                let same_box = format!("{}(Some(a), Some(b))", self.runtime("same_optional"));
                // A floating box held as a value is compared as `equals` compares it, so that
                // where the translation compares values for Java's identity (a shortcut before
                // `equals`), `-0.0` differs from `0.0` and NaN equals NaN.
                let same_bits = format!("{}(f64::from(*a), f64::from(*b))", self.runtime("double_equals"));
                let floating = matches!(self.runtime_of(c), Some(Type::Boxed(b)) if matches!(*b, Type::Double | Type::Float));
                let same = match () {
                    _ if self.runtime_of(c) == Some(Type::StringBuilder) => "a.same(b)",
                    _ if faithful => &same_box,
                    _ if floating && self.holds_value(c) => &same_bits,
                    _ if self.holds_value(c) && !array => "a == b",
                    _ if self.facts.thread_shared.contains(&c) || array && self.facts.threaded => {
                        "Arc::ptr_eq(a, b)"
                    }
                    _ => "Rc::ptr_eq(a, b)",
                };
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

    /// The monitor of an object of the type, where the program keeps the monitors of its objects
    /// (`synchronized (node)` on a `ConcurrentHashMap` bin): the monitor of the variant's handle.
    /// A variant that is not such a handle (a value, such as a string or a box held as an
    /// `Object`, or an immutable object shared as it is) has none, and ends the program
    /// (`java::unkept_monitor`): `synchronized (LOCK)` on an `Object` that is a plain object
    /// keeps its monitor although the type's other values have none.
    fn monitor_impl(&mut self, t: usize, variants: &[usize]) {
        if variants.is_empty() || !self.facts.kept_monitor_of(t) {
            return;
        }
        // A builder's handle has its cell's monitor (`java::Builder`'s `HasMonitor`).
        let handles: Vec<bool> = variants
            .iter()
            .map(|&c| {
                let payload = self.payload(c);
                payload.starts_with("Rc<")
                    || payload.starts_with("Arc<java::Locked<")
                    || matches!(payload.as_str(), "java::StringBuilder" | "java::SharedStringBuilder")
            })
            .collect();
        if !handles.contains(&true) {
            return;
        }
        let name = struct_name(&self.class.types[t].name);
        let (has, of) = (self.runtime("HasMonitor"), self.runtime("MonitorOf"));
        self.gap();
        self.line(&format!("impl {has} for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn monitor(&self) -> {of}<'_> {{"));
        self.indent += 1;
        self.line("match self {");
        self.indent += 1;
        let unkept = self.runtime("unkept_monitor");
        for (&c, handle) in variants.iter().zip(handles) {
            let v = self.variant(t, c);
            if handle {
                self.line(&format!("{v}(o) => {has}::monitor(o),"));
            } else {
                self.line(&format!("{v}(_) => {unkept}(),"));
            }
        }
        self.indent -= 1;
        self.line("}");
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
                // A builder shows its text, as its `toString` makes it.
                Some(Type::StringBuilder) => return ("fmt::Display::fmt(o, f)".to_string(), true),
                // An array shows as `[I@` and its identity hash.
                Some(Type::Array(_)) => {
                    let text = g.default_arm_text(t, c);
                    return (format!("f.write_str(&{text})"), true);
                }
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

    /// `try_text()`: the text of a value whose class's `toString` may throw, or its exception;
    /// the other classes' text is their `Display`.
    fn try_text_impl(&mut self, t: usize, variants: &[usize]) {
        let name = struct_name(&self.class.types[t].name);
        let throwable = self.throwable_type();
        self.gap();
        self.line(&format!("impl {name} {{"));
        self.indent += 1;
        self.line(&format!("fn try_text(&self) -> Result<String, {throwable}> {{"));
        self.indent += 1;
        self.match_arms(t, variants, &|g: &mut Self, c| match hierarchy::to_string_impl(g.class, c) {
            Some(i) if g.facts.throwing.contains(&i) => {
                let (call, uses) = g.arm_call(t, c, i, "");
                if g.facts.nullable_returns.contains(&i) {
                    (format!("{call}.map(|text| text.unwrap_or_else(|| \"null\".to_string()))"), uses)
                } else {
                    (call, uses)
                }
            }
            _ => ("Ok(self.to_string())".to_string(), false),
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
        let (mut text, uses) = self.arm_call(t, c, i, "");
        // Where the runtime makes the text, what a `toString` throws ends the program (see
        // `analysis::exceptions`); the generator's own text calls it (`try_text`).
        if self.facts.throwing.contains(&i) {
            if let Some(t) = self.class.type_index("java.lang.Throwable").and_then(|t| self.enum_for(t)) {
                self.need(Need::Display(t));
            }
            text = format!("{text}.unwrap_or_else({})", self.runtime("uncaught"));
        }
        // A `toString` that may return null shows "null", as `String.valueOf` does.
        if self.facts.nullable_returns.contains(&i) {
            return (format!("f.write_str({text}.as_deref().unwrap_or(\"null\"))"), uses);
        }
        (format!("f.write_str(&{text})"), uses)
    }
}

/// The enum items that a pass of the generator asked for, which the next pass writes.
pub(super) type Needs = BTreeSet<Need>;

/// The value of an object of class `c` where the program creates none, so the code never runs.
fn absent(g: &Gen, c: usize) -> String {
    format!("unreachable!(\"no object of {} is created\")", g.class.types[c].name)
}

/// `name` with its first letter in upper case: `Int` for `int`.
fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}
