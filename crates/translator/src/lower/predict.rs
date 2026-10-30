//! The covariant arrays and the failing Rust generics, predicted before the first lowering from
//! the trees, so that it starts with them and finds nothing more: the program is lowered once
//! (see "Real Rust generics" in `docs/object-model.md`). Whatever the prediction misses, lowering
//! notes as before, and the program is lowered again (`real_generics::retry`): a miss costs time,
//! never a wrong translation.
//!
//! With `JAVA_TO_RUST_PREDICTION_CHECK` set, the predicted arrays are not used, and `--timings`
//! compares the prediction with what the lowerings found: how many of the array types they noted
//! and of the Rust generics that failed it predicted, and each one it missed or added. The
//! lowerings still start without the Rust generics predicted to fail, as the arrays they note
//! depend on which are Rust generics; set to `unseeded`, they start with all the candidates, which
//! shows a Rust generic predicted to fail that does not. With `JAVA_TO_RUST_PREDICTION_WHY` set,
//! the prediction prints why it predicts each Rust generic to fail.

use super::real_generics::{Owner, Real};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

/// The array types predicted to be `Object[]`s and the Rust generics predicted to fail.
#[derive(Clone, Default)]
pub(super) struct Prediction {
    pub(super) arrays: Vec<Type>,
    arrays_made: bool,
    pub(super) failed: Real,
}

/// Whether the prediction prints why it predicts each Rust generic to fail.
fn tracing() -> bool {
    static WHY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *WHY.get_or_init(|| std::env::var_os("JAVA_TO_RUST_PREDICTION_WHY").is_some())
}

/// Whether the prediction is only compared with what lowering finds, not used: with
/// `JAVA_TO_RUST_PREDICTION_CHECK` set, its arrays are not used; set to `unseeded`, nor are its
/// Rust generics.
pub(super) fn checking() -> bool {
    std::env::var_os("JAVA_TO_RUST_PREDICTION_CHECK").is_some()
}

/// Whether the predicted Rust generics are not used either (`checking`).
pub(super) fn unseeded() -> bool {
    std::env::var_os("JAVA_TO_RUST_PREDICTION_CHECK").is_some_and(|v| v == "unseeded")
}

impl<'t> Lowering<'t> {
    /// Sets the Rust generics this lowering tries: `real`, or, on a lowering that is given none
    /// (the first, or one after that only noted covariant arrays), the candidates without those
    /// predicted to fail. The prediction is made once and kept with the covariant arrays
    /// (`Covariant::predicted`); its array types are `Object[]`s from the first lowering on.
    pub(super) fn start_real(&mut self, real: Option<&Real>) {
        let candidates = self.candidates();
        let cell = Rc::clone(&self.covariant.predicted);
        let mut predicted = cell.borrow_mut();
        let prediction = predicted
            .get_or_insert_with(|| crate::timings::phase("prediction", || self.predict(&candidates)));
        if unseeded() {
            self.real = real.cloned().unwrap_or(candidates);
            return;
        }
        self.real = real.cloned().unwrap_or_else(|| candidates.without(&prediction.failed));
    }

    /// The prediction with the Rust generics `candidates`; its arrays are predicted once the
    /// Rust generics are set (`predict_arrays`).
    fn predict(&self, candidates: &Real) -> Prediction {
        Prediction { arrays: Vec::new(), arrays_made: false, failed: self.predict_failed(candidates) }
    }

    /// Predicts, on the first lowering, once its Rust generics are set and the plan for
    /// `Object` is made, the array types that will be `Object[]`s, which are from then on.
    /// Lowering's own rules decide: the scan of the trees (`Meets`) hands each place where two
    /// types meet to the conversions that lowering notes covariant arrays in
    /// (`meets_object_array`, `note_object_array`, `object_array`), `noted_later` closes them,
    /// and what that noted and recorded is then forgotten, as this lowering has not lowered
    /// anything yet.
    pub(super) fn predict_arrays(&mut self) {
        let cell = Rc::clone(&self.covariant.predicted);
        let mut predicted = cell.borrow_mut();
        let Some(prediction) = predicted.as_mut() else { return };
        if !prediction.arrays_made {
            prediction.arrays_made = true;
            prediction.arrays = crate::timings::phase("prediction of covariant arrays", || {
                let (current, method) = (self.current.get(), self.method_ctx.get());
                let methods_by_node =
                    self.methods.iter().enumerate().map(|(m, i)| (i.node.0 as *const Value, m)).collect();
                let types_by_node =
                    self.types.iter().enumerate().map(|(t, i)| (i.node.0 as *const Value, t)).collect();
                let meets = Meets {
                    lowering: self,
                    methods_by_node: &methods_by_node,
                    types_by_node: &types_by_node,
                };
                meets.program();
                self.noted_later(&mut Real::default());
                self.current.set(current);
                self.method_ctx.set(method);
                let arrays = self.covariant.noted.take();
                self.covariant.forget_meets();
                self.object_arrays.take();
                self.object_tests.take();
                self.failed.take();
                self.uses_real.take();
                // The tables of reflection that the scan made, and what making them recorded.
                self.reflection_tables.borrow_mut().clear();
                arrays
            });
        }
        if !checking() {
            self.covariant.known.extend(prediction.arrays.iter().copied());
        }
    }

    /// The candidates predicted to fail, as `verify` fails them, until no more do: a method none
    /// of whose parameters' types names one of its type variables where Rust sees it (Rust infers
    /// a method's type parameters from them), and a class none of whose instance fields' types
    /// does for one of its own (a Rust struct uses each of its type parameters). A type names a
    /// type variable where Rust sees it at its top, as an array's element, or as an argument of a
    /// Rust generic class, not of an erased one (`List<T>` is `List` once `List` is erased).
    /// Then a method that overrides or is overridden, and what the code does with the values of
    /// each one (`Flow`, as `verify`'s `Check` and lowering's own failures).
    fn predict_failed(&self, candidates: &Real) -> Real {
        let mut real = candidates.clone();
        let methods_by_node =
            self.methods.iter().enumerate().map(|(m, i)| (i.node.0 as *const Value, m)).collect();
        let types_by_node =
            self.types.iter().enumerate().map(|(t, i)| (i.node.0 as *const Value, t)).collect();
        loop {
            let mut failed = Real::default();
            for &m in &real.methods {
                let node = self.methods[m].node;
                let params: Vec<JavaType> = node
                    .list("parameters")
                    .map(|p| JavaType::parse(p.symbol().map_or("", |s| s.ty)))
                    .collect();
                if node
                    .list("typeParameters")
                    .any(|v| !params.iter().any(|ty| self.sees(ty, v.str("name"), &real, None)))
                {
                    failed.insert(Owner::Method(m));
                }
                // A method that overrides another, or that another overrides, is called through
                // the dispatch of its class's objects, which no type parameter of its own fits.
                if node.get("symbol").and_then(|s| s.0["overrides"].as_array()).is_some_and(|o| !o.is_empty())
                {
                    failed.insert(Owner::Method(m));
                    for o in self.overrides(node) {
                        if real.methods.contains(&o) {
                            failed.insert(Owner::Method(o));
                        }
                    }
                }
            }
            for &t in &real.classes {
                let node = self.types[t].node;
                let fields: Vec<JavaType> = node
                    .list("members")
                    .filter(|m| m.kind() == "VARIABLE" && m.symbol().is_some_and(|s| !s.is_static))
                    .map(|m| JavaType::parse(m.symbol().map_or("", |s| s.ty)))
                    .collect();
                if node
                    .list("typeParameters")
                    .any(|v| !fields.iter().any(|ty| self.sees(ty, v.str("name"), &real, Some(t))))
                {
                    failed.insert(Owner::Class(t));
                }
            }
            let flow = Flow {
                lowering: self,
                real: &real,
                failed: RefCell::new(failed),
                place: RefCell::default(),
                methods_by_node: &methods_by_node,
                types_by_node: &types_by_node,
            };
            flow.program();
            let failed = flow.failed.into_inner();
            if failed.is_empty() {
                return candidates.without(&real);
            }
            real = real.without(&failed);
        }
    }

    /// Whether `ty` names the type variable `var` where Rust sees it, with the Rust generic
    /// classes of `real`, other than as an argument of class `own`.
    fn sees(&self, ty: &JavaType, var: &str, real: &Real, own: Option<usize>) -> bool {
        match ty {
            JavaType::Array(element) => self.sees(element, var, real, own),
            JavaType::Named(name, args) if args.is_empty() => *name == var,
            JavaType::Named(name, args) => self.type_index(name).is_some_and(|t| {
                Some(t) != own
                    && real.classes.contains(&t)
                    && args.iter().any(|a| self.sees(a, var, real, own))
            }),
            JavaType::Wild | JavaType::Other => false,
        }
    }
}

/// A type as lowering has it with the Rust generics `real`, as far as the prediction needs.
#[derive(Clone, PartialEq)]
enum Lowered<'a> {
    /// A type variable of a Rust generic.
    Param(Owner),
    /// A Rust generic class with its type arguments.
    Real(usize, Vec<Lowered<'a>>),
    /// A Rust generic class without type arguments, or with a wildcard among them.
    Raw(usize),
    Array(Box<Lowered<'a>>),
    /// Any other class, a primitive, or an erased type variable.
    Erased(&'a str),
    /// What the prediction does not read.
    Other,
}

impl Lowered<'_> {
    fn has_param(&self) -> bool {
        match self {
            Lowered::Param(_) => true,
            Lowered::Real(_, args) => args.iter().any(Lowered::has_param),
            Lowered::Array(element) => element.has_param(),
            _ => false,
        }
    }

    fn mentions_real(&self) -> bool {
        match self {
            Lowered::Param(_) | Lowered::Real(..) | Lowered::Raw(_) => true,
            Lowered::Array(element) => element.mentions_real(),
            _ => false,
        }
    }

    fn owners(&self, out: &mut Vec<Owner>) {
        match self {
            Lowered::Param(owner) => out.push(*owner),
            Lowered::Real(t, args) => {
                out.push(Owner::Class(*t));
                args.iter().for_each(|a| a.owners(out));
            }
            Lowered::Raw(t) => out.push(Owner::Class(*t)),
            Lowered::Array(element) => element.owners(out),
            _ => {}
        }
    }
}

/// The type variables in scope where code is, each with the Rust generic that declares it, or
/// `None` where that one is erased.
#[derive(Clone, Default)]
struct Scope<'t> {
    vars: Vec<(&'t str, Option<Owner>)>,
    /// The declared result of the method whose code it is.
    ret: Option<&'t str>,
}

/// One run of the prediction's check of what the code does with the values of the Rust
/// generics `real`, over the trees, as `verify` checks the lowered code: the generics it fails
/// are in `failed`.
struct Flow<'l, 't> {
    lowering: &'l Lowering<'t>,
    real: &'l Real,
    failed: RefCell<Real>,
    /// For `JAVA_TO_RUST_PREDICTION_WHY`: the method being checked.
    place: RefCell<String>,
    /// The method and the class of each declaration node.
    methods_by_node: &'l std::collections::HashMap<*const Value, usize>,
    types_by_node: &'l std::collections::HashMap<*const Value, usize>,
}

impl<'t> Flow<'_, 't> {
    fn fail(&self, ty: &Lowered, why: &str) {
        let mut owners = Vec::new();
        ty.owners(&mut owners);
        owners.into_iter().for_each(|o| self.fail_owner(o, why));
    }

    fn fail_owner(&self, owner: Owner, why: &str) {
        let new = match owner {
            Owner::Class(t) => !self.failed.borrow().classes.contains(&t),
            Owner::Method(m) => !self.failed.borrow().methods.contains(&m),
        };
        if new && tracing() {
            let l = self.lowering;
            let name = match owner {
                Owner::Class(t) => l.types[t].name.clone(),
                Owner::Method(m) => format!("{}.{}", l.types[l.methods[m].owner].name, l.methods[m].name),
            };
            eprintln!("prediction why: {name}: {why} in {}", self.place.borrow());
        }
        self.failed.borrow_mut().insert(owner);
    }

    fn is_real(&self, owner: Owner) -> bool {
        match owner {
            Owner::Class(t) => self.real.classes.contains(&t),
            Owner::Method(m) => self.real.methods.contains(&m),
        }
    }

    /// The type variables in scope in method `m` (when given) of class `t`, whose result is
    /// `ret`: the method's, then, unless it is static, the class's and those of the classes an
    /// inner class is in.
    fn scope_of(&self, t: usize, m: Option<usize>, ret: Option<&'t str>) -> Scope<'t> {
        let l = self.lowering;
        let mut vars = Vec::new();
        let mut owned = |node: Node<'t>, owner: Owner| {
            let real = self.is_real(owner).then_some(owner);
            vars.extend(node.list("typeParameters").map(|p| (p.str("name"), real)));
        };
        if let Some(m) = m {
            owned(l.methods[m].node, Owner::Method(m));
        }
        if !m.is_some_and(|m| l.methods[m].node.has_flag("STATIC")) {
            owned(l.types[t].node, Owner::Class(t));
            // An inner class sees the type variables of the classes it is in, erased for it
            // (`around` fails the Rust generics among them that it names).
            let mut class = t;
            while let Some(nested) = l.nesting(class).filter(|n| n.kind == super::inner::NestKind::Inner) {
                class = nested.enclosing;
                vars.extend(l.types[class].node.list("typeParameters").map(|p| (p.str("name"), None)));
            }
        }
        Scope { vars, ret }
    }

    /// Type `text` as lowering has it in `scope`.
    fn lowered<'a>(&self, text: &'a str, scope: &Scope) -> Lowered<'a> {
        self.lower(&JavaType::parse(text), scope)
    }

    fn lower<'a>(&self, ty: &JavaType<'a>, scope: &Scope) -> Lowered<'a> {
        match ty {
            JavaType::Array(element) => Lowered::Array(Box::new(self.lower(element, scope))),
            JavaType::Named(name, args) if args.is_empty() => {
                if let Some(&(_, owner)) = scope.vars.iter().find(|(v, _)| v == name) {
                    return owner.map_or(Lowered::Erased("java.lang.Object"), Lowered::Param);
                }
                match self.lowering.type_index(name) {
                    Some(t) if self.real.classes.contains(&t) => Lowered::Raw(t),
                    _ => Lowered::Erased(name),
                }
            }
            JavaType::Named(name, args) => match self.lowering.type_index(name) {
                Some(t) if self.real.classes.contains(&t) => {
                    if args.contains(&JavaType::Wild) {
                        return Lowered::Raw(t);
                    }
                    Lowered::Real(t, args.iter().map(|a| self.lower(a, scope)).collect())
                }
                _ => Lowered::Erased(name),
            },
            JavaType::Wild | JavaType::Other => Lowered::Other,
        }
    }

    /// Checks every method and field initializer of the program.
    fn program(&self) {
        let l = self.lowering;
        for t in 0..l.types.len() {
            // A local or anonymous class and a lambda are checked with the code that declares
            // them.
            if l.nesting(t).is_some_and(|n| n.method.is_some() || n.kind != super::inner::NestKind::Inner)
                || l.lambda_of_type(t).is_some()
            {
                continue;
            }
            self.class(t);
        }
    }

    /// Checks the members of class `t`.
    fn class(&self, t: usize) {
        let l = self.lowering;
        let node = l.types[t].node;
        // An inner class's code that names a type variable of a class it is in fails it.
        let mut outer = Vec::new();
        let mut class = t;
        while let Some(nested) = l.nesting(class).filter(|n| n.kind == super::inner::NestKind::Inner) {
            class = nested.enclosing;
            let owner = Owner::Class(class);
            let real = self.is_real(owner).then_some(owner);
            outer.extend(l.types[class].node.list("typeParameters").map(|p| (p.str("name"), real)));
        }
        self.around(node, &Scope { vars: outer, ret: None });
        let class_scope = self.scope_of(t, None, None);
        for member in node.list("members") {
            match member.kind() {
                "VARIABLE" => {
                    let is_static = member.has_flag("STATIC");
                    let scope = if is_static { Scope::default() } else { class_scope.clone() };
                    self.types_in(member, &scope);
                    if let Some(init) = member.get("initializer") {
                        let slot = self.lowered(member.symbol().map_or("", |s| s.ty), &scope);
                        self.expr(init, &scope);
                        self.flows(init, &slot, &scope);
                    }
                }
                "METHOD" => {
                    let Some(&m) = self.methods_by_node.get(&(member.0 as *const Value)) else { continue };
                    if tracing() {
                        *self.place.borrow_mut() = format!("{}.{}", l.types[t].name, l.methods[m].name);
                    }
                    let ret = member.get("returnType").map(|r| r.java_type());
                    let scope = self.scope_of(t, Some(m), ret);
                    self.types_in(member, &scope);
                    if let Some(body) = member.get("body") {
                        self.stmt(body, &scope);
                    }
                }
                "BLOCK" if member.has_flag("STATIC") => self.stmt(member, &Scope::default()),
                "BLOCK" => self.stmt(member, &class_scope),
                _ => {}
            }
        }
    }

    /// The declared types of a member's parameters, result and variable: a Rust generic class
    /// without its type arguments fails (`Check::ty`).
    fn types_in(&self, member: Node<'t>, scope: &Scope) {
        for p in member.list("parameters") {
            self.declared(p.symbol().map_or("", |s| s.ty), scope);
        }
        if member.kind() == "VARIABLE" {
            self.declared(member.symbol().map_or("", |s| s.ty), scope);
        }
        if let Some(ret) = member.get("returnType") {
            self.declared(ret.java_type(), scope);
        }
    }

    /// A type that occurs in the lowered code.
    fn declared(&self, text: &str, scope: &Scope) {
        let ty = self.lowered(text, scope);
        self.raw(&ty);
    }

    fn raw(&self, ty: &Lowered) {
        match ty {
            Lowered::Raw(t) => {
                self.fail_owner(Owner::Class(*t), "raw");
            }
            Lowered::Real(_, args) => args.iter().for_each(|a| self.raw(a)),
            Lowered::Array(element) => self.raw(element),
            _ => {}
        }
    }

    fn stmt(&self, node: Node<'t>, scope: &Scope<'t>) {
        match node.kind() {
            "BLOCK" => node.list("statements").for_each(|s| self.stmt(s, scope)),
            "VARIABLE" => {
                let slot = self.lowered(node.symbol().map_or("", |s| s.ty), scope);
                self.raw(&slot);
                if let Some(init) = node.get("initializer") {
                    self.expr(init, scope);
                    self.flows(init, &slot, scope);
                }
            }
            "RETURN" => {
                if let Some(e) = node.get("expression") {
                    self.expr(e, scope);
                    let slot = scope.ret.map_or(Lowered::Other, |r| self.lowered(r, scope));
                    self.flows(e, &slot, scope);
                }
            }
            "EXPRESSION_STATEMENT" => {
                if let Some(e) = node.get("expression") {
                    self.expr(e, scope);
                }
            }
            "CLASS" => self.nested_class(node, scope),
            _ => self.children(node, scope),
        }
    }

    /// Every child of `node`: statements as statements, expressions as values that go
    /// nowhere in particular.
    fn children(&self, node: Node<'t>, scope: &Scope<'t>) {
        let Value::Object(map) = node.0 else { return };
        for (field, value) in map {
            if field == "symbol" {
                continue;
            }
            for child in nodes_in(value) {
                if child.0.get("kind").is_none() {
                    continue;
                }
                if is_statement(child.kind()) {
                    self.stmt(child, scope);
                } else {
                    self.expr(child, scope);
                    self.used(child, scope);
                }
            }
        }
    }

    /// The value of `node` used for something other than a store (an operand, a receiver, a
    /// condition): a value of a type variable fails its generic.
    fn used(&self, node: Node<'t>, scope: &Scope) {
        if matches!(node.kind(), "MEMBER_SELECT" | "IDENTIFIER")
            && node.symbol().is_some_and(|s| {
                !matches!(
                    s.kind,
                    "LOCAL_VARIABLE" | "PARAMETER" | "FIELD" | "EXCEPTION_PARAMETER" | "RESOURCE_VARIABLE"
                )
            })
        {
            return;
        }
        let ty = self.lowered(node.java_type(), scope);
        if matches!(ty, Lowered::Param(_)) {
            self.fail(&ty, "a value used");
        }
    }

    /// The value of `node` going where a value of type `slot` goes (`Check::store`), converted
    /// to it where the types differ (lowering's `Cast`, which `Check` fails).
    fn flows(&self, node: Node<'t>, slot: &Lowered, scope: &Scope) {
        if node.kind() == "PARENTHESIZED"
            && let Some(inner) = node.get("expression")
        {
            return self.flows(inner, slot, scope);
        }
        if node.kind() == "CONDITIONAL_EXPRESSION" {
            for branch in ["trueExpression", "falseExpression"] {
                if let Some(b) = node.get(branch) {
                    self.flows(b, slot, scope);
                }
            }
            return;
        }
        if *slot == Lowered::Other || node.kind() == "NULL_LITERAL" {
            return;
        }
        let value = self.lowered(node.java_type(), scope);
        if value == Lowered::Other || value == *slot {
            return;
        }
        if matches!(value, Lowered::Param(_)) && !matches!(slot, Lowered::Param(_)) {
            self.fail(&value, "a value stored");
        } else if value.mentions_real() && !matches!(slot, Lowered::Param(_)) {
            self.fail(&value, "a value converted");
            if slot.has_param() {
                self.fail(slot, "a value converted to");
            }
        }
    }

    fn expr(&self, node: Node<'t>, scope: &Scope<'t>) {
        match node.kind() {
            "METHOD_INVOCATION" | "NEW_CLASS" => self.call(node, scope),
            "ASSIGNMENT" => {
                let (Some(target), Some(value)) = (node.get("variable"), node.get("expression")) else {
                    return;
                };
                self.expr(value, scope);
                if target.kind() == "MEMBER_SELECT"
                    && let Some(object) = target.get("expression")
                {
                    self.expr(object, scope);
                    self.used(object, scope);
                }
                let slot = self.lowered(target.java_type(), scope);
                self.flows(value, &slot, scope);
                // The field as its class declares it takes the value as it is (`Check::store`).
                if let Some(declared) = self.field_type(target)
                    && matches!(self.lowered(value.java_type(), scope), Lowered::Param(_))
                    && !matches!(declared, Lowered::Param(_))
                {
                    self.fail(&self.lowered(value.java_type(), scope), "a value stored in a field");
                }
            }
            "CONDITIONAL_EXPRESSION" => {
                if let Some(c) = node.get("condition") {
                    self.expr(c, scope);
                }
                let slot = self.lowered(node.java_type(), scope);
                for branch in ["trueExpression", "falseExpression"] {
                    if let Some(b) = node.get(branch) {
                        self.expr(b, scope);
                        self.flows(b, &slot, scope);
                    }
                }
            }
            "PARENTHESIZED" => {
                if let Some(e) = node.get("expression") {
                    self.expr(e, scope);
                }
            }
            "EQUAL_TO" | "NOT_EQUAL_TO" => {
                let (Some(a), Some(b)) = (node.get("leftOperand"), node.get("rightOperand")) else { return };
                self.expr(a, scope);
                self.expr(b, scope);
                let null = |x: Node| x.kind() == "NULL_LITERAL";
                if !null(b) {
                    self.used(a, scope);
                }
                if !null(a) {
                    self.used(b, scope);
                }
            }
            "TYPE_CAST" => {
                let Some(e) = node.get("expression") else { return };
                self.expr(e, scope);
                let to = self.lowered(node.java_type(), scope);
                let from = self.lowered(e.java_type(), scope);
                if to != from {
                    if to.has_param() || from.has_param() {
                        self.fail(&to, "a cast to");
                        self.fail(&from, "a cast from");
                    }
                    self.fail(&from, "a cast of");
                }
            }
            "INSTANCE_OF" => {
                if let Some(e) = node.get("expression") {
                    self.expr(e, scope);
                    self.fail(&self.lowered(e.java_type(), scope), "instanceof of");
                }
                for field in ["type", "pattern"] {
                    if let Some(ty) = node.get(field) {
                        self.fail(&self.lowered(ty.java_type(), scope), "instanceof");
                    }
                }
            }
            "MEMBER_SELECT" => {
                if let Some(object) = node.get("expression") {
                    self.expr(object, scope);
                    if node.symbol().is_some_and(|s| s.kind == "FIELD") {
                        self.used(object, scope);
                        self.read(node, scope);
                    }
                }
            }
            "IDENTIFIER" => {
                if node.symbol().is_some_and(|s| s.kind == "FIELD") {
                    self.read(node, scope);
                }
            }
            "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE" => self.nested_code(node, scope),
            _ => self.children(node, scope),
        }
    }

    /// The declared type of the field that `target` names, as its class has it.
    fn field_type(&self, target: Node<'t>) -> Option<Lowered<'t>> {
        let symbol = target.symbol().filter(|s| s.kind == "FIELD")?;
        let t = self.lowering.type_index(symbol.owner)?;
        let scope = self.scope_of(t, None, None);
        Some(self.lowered(symbol.ty, &scope))
    }

    /// A field read: where its declared type has no type variable of a Rust generic, a value
    /// that javac gives one is cast back to it (`erased_use`, `Check::read`).
    fn read(&self, node: Node<'t>, scope: &Scope) {
        let Some(declared) = self.field_type(node) else { return };
        let used = self.lowered(node.java_type(), scope);
        if !declared.has_param() && used.has_param() && declared != used {
            self.fail(&used, "a field read");
        }
    }

    /// A call: its receiver, its arguments, each going to its parameter, and its result.
    fn call(&self, node: Node<'t>, scope: &Scope<'t>) {
        let l = self.lowering;
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        for &a in &args {
            self.expr(a, scope);
        }
        let select = node.get("methodSelect");
        if let Some(object) = select.filter(|s| s.kind() == "MEMBER_SELECT").and_then(|s| s.get("expression"))
        {
            self.expr(object, scope);
            self.used(object, scope);
        }
        if let Some(object) = node.get("enclosingExpression") {
            self.expr(object, scope);
        }
        if let Some(body) = node.get("classBody") {
            self.nested_class(body, scope);
        }
        let Some(symbol) = node.symbol() else { return };
        let Some(m) = l.method_index(&symbol) else {
            args.iter().for_each(|&a| self.flows(a, &Lowered::Erased("void"), scope));
            return;
        };
        let callee = l.methods[m].owner;
        let declared_scope = self.scope_of(callee, Some(m), None);
        let (declared_params, declared_ret) = signature(symbol.ty);
        // The receiver's class runs a method it inherits on its object as a value of the
        // method's class (`Check`'s `MethodCall`).
        if let Some(object) = select.filter(|s| s.kind() == "MEMBER_SELECT").and_then(|s| s.get("expression"))
            && let Lowered::Real(t, _) | Lowered::Raw(t) = self.lowered(object.java_type(), scope)
            && t != callee
        {
            self.fail_owner(Owner::Class(t), "an inherited method");
        }
        let substituted = select.map(|s| signature(s.java_type()).0);
        for (i, &a) in args.iter().enumerate() {
            let last = declared_params.len().saturating_sub(1);
            let Some(&param) = declared_params.get(i.min(last)) else { continue };
            let mut declared = self.lowered(param, &declared_scope);
            // The arguments that a variable-arity call packs into an array go to its elements.
            if symbol.varargs && i >= last {
                let passes_array = args.len() == declared_params.len()
                    && matches!(self.lowered(a.java_type(), scope), Lowered::Array(_) | Lowered::Other);
                if !passes_array {
                    let Lowered::Array(element) = declared else { continue };
                    declared = *element;
                }
                if declared.has_param() {
                    continue;
                }
            }
            let slot = if declared.has_param() {
                match substituted.as_ref().and_then(|s| s.get(i)) {
                    Some(&sub) => self.lowered(sub, scope),
                    None => Lowered::Other,
                }
            } else {
                declared
            };
            self.flows(a, &slot, scope);
        }
        // A result whose declared type has no type variable of a Rust generic is cast to what
        // javac substituted (`erased_use`).
        if node.kind() == "METHOD_INVOCATION" {
            let declared = self.lowered(declared_ret, &declared_scope);
            let used = self.lowered(node.java_type(), scope);
            if !declared.has_param() && used.has_param() && declared != used {
                self.fail(&used, "a result");
            }
        }
    }

    /// A lambda or a method reference: its code is a class of its own, whose uses of the type
    /// variables around fail them (`type_var`); its code is checked with them erased.
    fn nested_code(&self, node: Node<'t>, scope: &Scope<'t>) {
        self.around(node, scope);
        let inner = Scope { vars: scope.vars.iter().map(|&(v, _)| (v, None)).collect(), ret: None };
        self.children(node, &inner);
    }

    /// A local or anonymous class (its `CLASS` node) declared in code with `scope`.
    fn nested_class(&self, node: Node<'t>, scope: &Scope<'t>) {
        self.around(node, scope);
        let Some(&t) = self.types_by_node.get(&(node.0 as *const Value)) else { return };
        self.class(t);
    }

    /// Fails the Rust generics of `scope` whose type variables the code `node` of a class of its
    /// own names.
    fn around(&self, node: Node<'t>, scope: &Scope) {
        let real: Vec<(&str, Owner)> = scope.vars.iter().filter_map(|&(v, o)| Some((v, o?))).collect();
        if real.is_empty() {
            return;
        }
        let mut names = Vec::new();
        type_texts(node, &mut names);
        for (v, owner) in real {
            if names.iter().any(|ty| super::real_generics::mentions_var(ty, v)) {
                self.fail_owner(owner, "named in a class of its own");
            }
        }
    }
}

/// The scan of the trees for the places where lowering converts a value of one type to another,
/// each handed to lowering's own rules for covariant arrays (`Lowering::predict_arrays`).
struct Meets<'l, 't> {
    lowering: &'l Lowering<'t>,
    methods_by_node: &'l std::collections::HashMap<*const Value, usize>,
    types_by_node: &'l std::collections::HashMap<*const Value, usize>,
}

impl<'t> Meets<'_, 't> {
    fn ty(&self, node: Node<'t>, text: &str) -> Option<Type> {
        if text.is_empty() || text.starts_with(['(', '<']) {
            return None;
        }
        self.lowering.ty(node, text).ok()
    }

    fn value(&self, node: Node<'t>) -> Option<Type> {
        match node.kind() {
            "PARENTHESIZED" => self.value(node.get("expression")?),
            "NULL_LITERAL" => None,
            _ => self.ty(node, node.java_type()),
        }
    }

    /// A value of type `value` converted to type `to` (`Lowering::convert`).
    fn convert(&self, value: Type, to: Type) {
        let l = self.lowering;
        if l.param_owner(to).is_some() || value == to || matches!(value, Type::Null | Type::Void) {
            return;
        }
        l.meets_object_array(value, to);
        if Lowering::is_object(to) {
            l.note_object_array(value);
        }
    }

    /// The value of `node` going where a value of type `to` goes.
    fn flows(&self, node: Node<'t>, to: Option<Type>) {
        let Some(to) = to else { return };
        match node.kind() {
            "PARENTHESIZED" => {
                if let Some(inner) = node.get("expression") {
                    self.flows(inner, Some(to));
                }
            }
            "CONDITIONAL_EXPRESSION" => {
                for branch in ["trueExpression", "falseExpression"] {
                    if let Some(b) = node.get(branch) {
                        self.flows(b, Some(to));
                    }
                }
            }
            _ => {
                if let Some(value) = self.value(node) {
                    self.convert(value, to);
                }
            }
        }
    }

    fn program(&self) {
        let l = self.lowering;
        for t in 0..l.types.len() {
            if l.nesting(t).is_some_and(|n| n.method.is_some() || n.kind != super::inner::NestKind::Inner)
                || l.lambda_of_type(t).is_some()
            {
                continue;
            }
            self.class(t);
        }
    }

    fn class(&self, t: usize) {
        let l = self.lowering;
        let (current, method) = (l.current.get(), l.method_ctx.get());
        l.current.set(t);
        for member in l.types[t].node.list("members") {
            l.method_ctx.set(None);
            match member.kind() {
                "VARIABLE" => {
                    if let Some(init) = member.get("initializer") {
                        let to = self.ty(member, member.symbol().map_or("", |s| s.ty));
                        self.expr(init);
                        self.flows(init, to);
                    }
                }
                "METHOD" => {
                    let Some(&m) = self.methods_by_node.get(&(member.0 as *const Value)) else { continue };
                    l.method_ctx.set(Some(m));
                    self.bridges(m, member);
                    let ret = member.get("returnType").and_then(|r| self.ty(r, r.java_type()));
                    if let Some(body) = member.get("body") {
                        self.stmt(body, ret);
                    }
                }
                "BLOCK" => self.stmt(member, None),
                _ => {}
            }
        }
        l.current.set(current);
        l.method_ctx.set(method);
    }

    /// The parameters and the result of method `m` and of each method it overrides, which meet
    /// in the bridge that calls one for the other (`object::bridges`).
    fn bridges(&self, m: usize, node: Node<'t>) {
        let l = self.lowering;
        let Ok(params) = l.param_types(m) else { return };
        let ret = |m: usize| {
            let info = &l.methods[m];
            let ret = info.node.get("returnType")?;
            l.declared_type(ret, l.return_java_type(m, ret), info.owner, Some(m)).ok()
        };
        for o in l.overrides(node) {
            let Ok(overridden) = l.param_types(o) else { continue };
            for (&p, &q) in params.iter().zip(&overridden) {
                if p != q && !Lowering::is_object(q) {
                    l.meets_object_array(p, q);
                }
            }
            if let (Some(a), Some(b)) = (ret(m), ret(o)) {
                l.meets_object_array(a, b);
            }
        }
    }

    fn stmt(&self, node: Node<'t>, ret: Option<Type>) {
        match node.kind() {
            "BLOCK" => node.list("statements").for_each(|s| self.stmt(s, ret)),
            "VARIABLE" => {
                if let Some(init) = node.get("initializer") {
                    let to = self.ty(node, node.symbol().map_or("", |s| s.ty));
                    self.expr(init);
                    self.flows(init, to);
                }
            }
            "RETURN" => {
                if let Some(e) = node.get("expression") {
                    self.expr(e);
                    self.flows(e, ret);
                }
            }
            "CLASS" => self.nested_class(node),
            _ => self.children(node, ret),
        }
    }

    fn children(&self, node: Node<'t>, ret: Option<Type>) {
        let Value::Object(map) = node.0 else { return };
        for (field, value) in map {
            if field == "symbol" {
                continue;
            }
            for child in nodes_in(value) {
                if child.0.get("kind").is_none() {
                    continue;
                }
                if is_statement(child.kind()) {
                    self.stmt(child, ret);
                } else {
                    self.expr(child);
                }
            }
        }
    }

    fn nested_class(&self, node: Node<'t>) {
        if let Some(&t) = self.types_by_node.get(&(node.0 as *const Value)) {
            self.class(t);
        }
    }

    fn expr(&self, node: Node<'t>) {
        let l = self.lowering;
        match node.kind() {
            "METHOD_INVOCATION" | "NEW_CLASS" => self.call(node),
            "ASSIGNMENT" => {
                let (Some(target), Some(value)) = (node.get("variable"), node.get("expression")) else {
                    return;
                };
                self.expr(target);
                self.expr(value);
                self.flows(value, self.ty(target, target.java_type()));
            }
            "CONDITIONAL_EXPRESSION" => {
                self.children(node, None);
                let to = self.ty(node, node.java_type());
                for branch in ["trueExpression", "falseExpression"] {
                    if let Some(b) = node.get(branch) {
                        self.flows(b, to);
                    }
                }
            }
            "TYPE_CAST" => {
                let Some(e) = node.get("expression") else { return };
                self.expr(e);
                let (Some(to), Some(value)) = (self.ty(node, node.java_type()), self.value(e)) else {
                    return;
                };
                if Lowering::is_object(value) {
                    let _ = l.object_array(node, to);
                }
                if value != to {
                    l.meets_object_array(value, to);
                }
            }
            "INSTANCE_OF" => {
                self.children(node, None);
                for field in ["type", "pattern"] {
                    if let Some(tested) = node.get(field).and_then(|t| self.ty(t, t.java_type()))
                        && matches!(tested, Type::Array(_))
                    {
                        let _ = l.object_array(node, tested);
                    }
                }
            }
            "PLUS" if node.java_type() == "java.lang.String" => {
                self.children(node, None);
                for operand in ["leftOperand", "rightOperand"] {
                    if let Some(ty) = node.get(operand).and_then(|o| self.value(o))
                        && matches!(ty, Type::Array(_))
                    {
                        let _ = l.object_array(node, ty);
                        l.note_object_array(ty);
                    }
                }
            }
            "NEW_ARRAY" => {
                self.children(node, None);
                if let Some(Type::Array(element)) = self.ty(node, node.java_type()) {
                    for init in node.list("initializers") {
                        self.flows(init, Some(*element));
                    }
                }
            }
            "MEMBER_SELECT" | "IDENTIFIER" => {
                self.children(node, None);
                if let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD")
                    && let Some(class) = l.type_index(symbol.owner)
                    && let Ok(declared) = l.declared_type(node, symbol.ty, class, None)
                {
                    self.read(node, declared);
                }
            }
            _ => self.children(node, None),
        }
    }

    /// A use of a member whose declared type is `declared`, where javac gave the use another:
    /// lowering casts it back (`erased_use`).
    fn read(&self, node: Node<'t>, declared: Type) {
        let l = self.lowering;
        let Some(used) = self.ty(node, node.java_type()) else { return };
        if declared == used || used == Type::Void || l.has_param(declared) {
            return;
        }
        if matches!(declared, Type::Array(_)) {
            l.meets_object_array(used, declared);
        } else if Lowering::is_object(declared) {
            let _ = l.object_array(node, used);
        }
    }

    fn call(&self, node: Node<'t>) {
        let l = self.lowering;
        self.children(node, None);
        let Some(symbol) = node.symbol() else { return };
        // An access of a field through a handle, which lowering makes of the field, not a call.
        if let Some((class, field, is_static)) = l.handle_read(node, &symbol) {
            let declared = l.types[class].node.list("members").find(|m| {
                m.kind() == "VARIABLE" && m.str("name") == field && m.has_flag("STATIC") == is_static
            });
            if let Some(member) = declared
                && let Ok(ty) = l.declared_type(member, member.symbol().map_or("", |s| s.ty), class, None)
            {
                self.convert(ty, self.ty(node, node.java_type()).unwrap_or(ty));
            }
            return;
        }
        for (ty, unboxed) in l.reflected_conversions(node, &symbol) {
            if unboxed {
                if matches!(ty, Type::Array(_)) {
                    let _ = l.object_array(node, ty);
                }
            } else {
                self.convert(ty, Type::class(crate::ast::OBJECT));
            }
        }
        let Some(m) = l.method_index(&symbol) else { return };
        let Ok(params) = l.param_types(m) else { return };
        let args: Vec<Node<'t>> = node.list("arguments").collect();
        let last = params.len().saturating_sub(1);
        for (i, &a) in args.iter().enumerate() {
            let Some(&param) = params.get(i.min(last)) else { continue };
            let mut to = param;
            if symbol.varargs && i >= last {
                let packed = args.len() != params.len()
                    || !matches!(self.value(a), Some(Type::Array(_)) | Some(Type::Null) | None);
                if packed {
                    let Type::Array(element) = param else { continue };
                    to = *element;
                }
            }
            self.flows(a, Some(to));
        }
        if node.kind() == "METHOD_INVOCATION" {
            let info = &l.methods[m];
            if let Some(ret) = info.node.get("returnType")
                && let Ok(declared) = l.declared_type(ret, l.return_java_type(m, ret), info.owner, Some(m))
            {
                self.read(node, declared);
            }
        }
    }
}

/// The types that lowering reads under `node`: each node's type, but a method's, and each
/// variable's and method's declared type.
fn type_texts<'t>(node: Node<'t>, out: &mut Vec<&'t str>) {
    let ty = node.java_type();
    if !ty.is_empty() && !ty.starts_with(['(', '<']) {
        out.push(ty);
    }
    if matches!(node.kind(), "VARIABLE") {
        out.push(node.symbol().map_or("", |s| s.ty));
    }
    let Value::Object(map) = node.0 else { return };
    for (field, value) in map {
        if field != "symbol" {
            nodes_in(value).into_iter().for_each(|child| type_texts(child, out));
        }
    }
}

/// The nodes that the value `v` of a field of a node is: itself, or the items of a list.
fn nodes_in(v: &Value) -> Vec<Node<'_>> {
    match v {
        Value::Object(_) => vec![Node(v)],
        Value::Array(items) => items.iter().flat_map(nodes_in).collect(),
        _ => Vec::new(),
    }
}

fn is_statement(kind: &str) -> bool {
    matches!(
        kind,
        "BLOCK"
            | "VARIABLE"
            | "RETURN"
            | "EXPRESSION_STATEMENT"
            | "IF"
            | "WHILE_LOOP"
            | "DO_WHILE_LOOP"
            | "FOR_LOOP"
            | "ENHANCED_FOR_LOOP"
            | "SWITCH"
            | "TRY"
            | "CATCH"
            | "THROW"
            | "SYNCHRONIZED"
            | "LABELED_STATEMENT"
            | "CLASS"
            | "CASE"
            | "BREAK"
            | "CONTINUE"
            | "ASSERT"
            | "YIELD"
            | "EMPTY_STATEMENT"
    )
}

/// The parameter types and the result of javac's method type `(A,B)R` or `<T>(A,B)R`.
fn signature(ty: &str) -> (Vec<&str>, &str) {
    let ty = match ty.strip_prefix('<') {
        Some(_) => ty.find(">(").map_or(ty, |i| &ty[i + 1..]),
        None => ty,
    };
    let Some(rest) = ty.strip_prefix('(') else { return (Vec::new(), "") };
    let mut depth = 0;
    let close = rest
        .char_indices()
        .find_map(|(i, c)| {
            match c {
                '<' | '(' => depth += 1,
                '>' => depth -= 1,
                ')' if depth == 0 => return Some(i),
                ')' => depth -= 1,
                _ => {}
            }
            None
        })
        .unwrap_or(rest.len());
    let params = &rest[..close];
    let ret = rest.get(close + 1..).unwrap_or("");
    let params = if params.is_empty() { Vec::new() } else { super::generics::split_args(params) };
    (params, ret)
}

/// A type as javac writes it (`java.util.Map<K,Main.Box<V>>`, `T[]`), read as far as the
/// prediction needs: a wildcard, a capture or an intersection is `Other`.
#[derive(Debug, PartialEq)]
enum JavaType<'a> {
    Named(&'a str, Vec<JavaType<'a>>),
    Array(Box<JavaType<'a>>),
    /// A wildcard, a capture or an intersection.
    Wild,
    Other,
}

impl<'a> JavaType<'a> {
    fn parse(text: &'a str) -> JavaType<'a> {
        let text = text.trim();
        if let Some(element) = text.strip_suffix("[]").or_else(|| text.strip_suffix("...")) {
            return JavaType::Array(Box::new(JavaType::parse(element)));
        }
        if text.starts_with('?')
            || text.starts_with("capture#")
            || super::generics::intersection_bounds(text).is_some()
        {
            return JavaType::Wild;
        }
        if text.is_empty() || text.starts_with(['(', '<']) {
            return JavaType::Other;
        }
        match text.find('<') {
            Some(open) if text.ends_with('>') => {
                let args = super::generics::split_args(&text[open + 1..text.len() - 1]);
                JavaType::Named(&text[..open], args.into_iter().map(JavaType::parse).collect())
            }
            // An inner class of a generic class, written with the outer's arguments
            // (`Outer<T>.Inner`), is read as the erasure lowering makes of it.
            Some(_) => JavaType::Other,
            None => JavaType::Named(text, Vec::new()),
        }
    }
}

/// For `--timings` with `JAVA_TO_RUST_PREDICTION_CHECK`: the prediction against what the
/// lowerings found, the array types `noted` and the Rust generics that `failed`, each miss and
/// each addition on a line of its own.
pub(super) fn report(prediction: &Prediction, noted: &[Type], failed: &Real, class: &Class) {
    let name = |owner: Owner| match owner {
        Owner::Class(t) => class.types[t].name.clone(),
        Owner::Method(m) => {
            let method = &class.methods[m];
            format!("{}.{}", class.types[method.class].name, method.name)
        }
    };
    let owners = |real: &Real| -> Vec<Owner> {
        real.classes
            .iter()
            .map(|&t| Owner::Class(t))
            .chain(real.methods.iter().map(|&m| Owner::Method(m)))
            .collect()
    };
    let predicted_owners = owners(&prediction.failed);
    let failed_owners = owners(failed);
    let missed: Vec<String> =
        failed_owners.iter().filter(|o| !predicted_owners.contains(o)).map(|&o| name(o)).collect();
    let added: Vec<String> =
        predicted_owners.iter().filter(|o| !failed_owners.contains(o)).map(|&o| name(o)).collect();
    let missed_arrays: Vec<String> =
        noted.iter().filter(|t| !prediction.arrays.contains(t)).map(Type::to_string).collect();
    let added_arrays: Vec<String> =
        prediction.arrays.iter().filter(|t| !noted.contains(t)).map(Type::to_string).collect();
    eprintln!(
        "timing: prediction: {} of {} covariant array types, {} more; {} of {} failed Rust generics, {} more",
        noted.len() - missed_arrays.len(),
        noted.len(),
        added_arrays.len(),
        failed_owners.len() - missed.len(),
        failed_owners.len(),
        added.len()
    );
    for (what, list) in [
        ("missed array", &missed_arrays),
        ("added array", &added_arrays),
        ("missed generic", &missed),
        ("added generic", &added),
    ] {
        for item in list {
            eprintln!("timing: prediction {what} {item}");
        }
    }
}
