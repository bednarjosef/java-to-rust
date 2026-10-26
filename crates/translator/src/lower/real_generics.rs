//! Generic classes and methods that become Rust generics. Lowering first tries every generic
//! class and method that could be one (`candidates`), lowers the program, and then checks what
//! the lowered code does with each one (`verify`); those that fail are erased (see `generics`)
//! and the program is lowered again, until every remaining one passes.
//!
//! A type variable of a real generic is a type of its own, of kind `TypeKind::Param`, named
//! `Owner#T`, which nothing implements: its values are of the Rust type parameter, so every
//! analysis sees an opaque value that is stored, moved and cloned. A use of a real generic class
//! has its type arguments (`Type::Class("Box", [Integer])`), and a use of a member keeps the type
//! javac substituted, with no cast: Rust substitutes the same way.
//!
//! What a real generic may do with a value of its type variable: store it in a variable, field
//! or element of the same type variable, return it, pass it to a parameter of a type variable,
//! and compare it with `null`. Anything else (a cast, a call on it, a conversion to `Object` or
//! to text, an array of it) erases the generic. A class qualifies when it extends and implements
//! nothing, nothing extends it, and its type parameters have no bounds and each occurs in the type
//! of one of its instance fields, as a Rust struct must use them; its uses must all give it
//! type arguments, and its objects must not become `Object`s. A method qualifies when its type
//! parameters have no bounds and each occurs in a parameter's type, so that Rust can infer it.

use super::*;
use std::collections::BTreeSet;

/// The generic classes (by type index) and methods (by method index) that are Rust generics.
#[derive(Clone, Default, Debug, PartialEq)]
pub(super) struct Real {
    pub(super) classes: BTreeSet<usize>,
    pub(super) methods: BTreeSet<usize>,
}

impl Real {
    pub(super) fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.methods.is_empty()
    }

    fn without(&self, failed: &Real) -> Real {
        Real {
            classes: self.classes.difference(&failed.classes).copied().collect(),
            methods: self.methods.difference(&failed.methods).copied().collect(),
        }
    }
}

/// What declares a type variable.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Owner {
    Class(usize),
    Method(usize),
}

/// Lowers with every candidate, then without those that fail, until none does; an error with
/// real generics is retried with all of them erased. A lowering that finds arrays that meet an
/// `Object[]` is retried with them as `Object[]`s first (see `covariant`).
pub(super) fn retry<E: std::fmt::Debug>(
    mut run: impl FnMut(Option<&Real>, &super::covariant::Covariant) -> Result<(Class, Real, Real), E>,
) -> Result<Class, E> {
    let mut real: Option<Real> = None;
    let mut covariant = super::covariant::Covariant::default();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let result =
            crate::timings::phase(&format!("lowering attempt {attempt}"), || run(real.as_ref(), &covariant));
        let noted: Vec<Type> =
            covariant.noted.take().into_iter().filter(|ty| !covariant.known.contains(ty)).collect();
        if !noted.is_empty() {
            if crate::timings::enabled() {
                eprintln!(
                    "timing: lowering attempt {attempt} is lowered again for {}",
                    covariant_types(&noted)
                );
            }
            covariant.known.extend(noted);
            continue;
        }
        match result {
            Ok((class, _, failed)) if failed.is_empty() => return Ok(class),
            Ok((class, used, failed)) => {
                if crate::timings::enabled() {
                    eprintln!(
                        "timing: lowering attempt {attempt} is lowered again without {}",
                        failed_generics(&class, &used, &failed)
                    );
                }
                real = Some(used.without(&failed))
            }
            Err(e) if real.as_ref().is_some_and(Real::is_empty) => return Err(e),
            Err(e) => {
                if crate::timings::enabled() {
                    eprintln!(
                        "timing: lowering attempt {attempt} is lowered again without Rust generics, for an error: {e:?}"
                    );
                }
                real = Some(Real::default())
            }
        }
    }
}

/// For `--timings`: how many array types became `Object[]`s, with the first few.
fn covariant_types(noted: &[Type]) -> String {
    let first: Vec<String> = noted.iter().take(5).map(Type::to_string).collect();
    let more = if noted.len() > first.len() { ", …" } else { "" };
    format!("{} new covariant array types ({}{more})", noted.len(), first.join(", "))
}

/// For `--timings`: how many of the Rust generics `used` failed, with the first few.
fn failed_generics(class: &Class, used: &Real, failed: &Real) -> String {
    let classes = failed.classes.iter().map(|&t| class.types[t].name.clone());
    let methods = failed.methods.iter().map(|&m| {
        let method = &class.methods[m];
        format!("{}.{}", class.types[method.class].name, method.name)
    });
    let names: Vec<String> = classes.chain(methods).collect();
    let first = names[..names.len().min(5)].join(", ");
    let more = if names.len() > 5 { ", …" } else { "" };
    format!(
        "{} of {} Rust generic classes and {} of {} methods ({first}{more})",
        failed.classes.len(),
        used.classes.len(),
        failed.methods.len(),
        used.methods.len()
    )
}

/// Whether every type variable of class `node` occurs in the type of one of its instance fields:
/// a Rust struct uses each of its type parameters.
fn held(node: Node) -> bool {
    let fields: Vec<&str> = node
        .list("members")
        .filter(|m| m.kind() == "VARIABLE" && m.symbol().is_some_and(|s| !s.is_static))
        .map(|m| m.symbol().map_or("", |s| s.ty))
        .collect();
    node.list("typeParameters").all(|v| fields.iter().any(|ty| mentions_var(ty, v.str("name"))))
}

fn has_bounds(node: Node) -> bool {
    node.list("typeParameters").any(|p| p.list("bounds").next().is_some())
}

impl<'t> Lowering<'t> {
    /// The generic classes and methods that may be Rust generics, before lowering checks them.
    pub(super) fn candidates(&self) -> Real {
        let generic = |node: Node| node.list("typeParameters").next().is_some();
        let extended: BTreeSet<&str> = self
            .types
            .iter()
            .filter_map(|t| t.node.get("extendsClause"))
            .map(|c| c.java_type().split('<').next().unwrap_or_default())
            .collect();
        let classes = (0..self.types.len())
            .filter(|&t| {
                let node = self.types[t].node;
                node.kind() == "CLASS"
                    && generic(node)
                    && !has_bounds(node)
                    && !node.has_flag("ABSTRACT")
                    && node.get("extendsClause").is_none()
                    && node.list("implementsClause").next().is_none()
                    && !extended.contains(self.types[t].name.as_str())
                    && held(node)
            })
            .collect();
        let methods = (0..self.methods.len())
            .filter(|&m| {
                let node = self.methods[m].node;
                generic(node) && !has_bounds(node) && self.inferable(m)
            })
            .collect();
        Real { classes, methods }
    }

    /// Whether every type variable of method `m` occurs in the type of one of its parameters.
    /// (A class's must occur in one of its fields: see `held`.)
    fn inferable(&self, m: usize) -> bool {
        let node = self.methods[m].node;
        let params: Vec<&str> = node.list("parameters").map(|p| p.symbol().map_or("", |s| s.ty)).collect();
        node.list("typeParameters").all(|v| {
            let name = v.str("name");
            params.iter().any(|ty| mentions_var(ty, name))
        })
    }

    pub(super) fn is_real(&self, owner: Owner) -> bool {
        match owner {
            Owner::Class(t) => self.real.classes.contains(&t),
            Owner::Method(m) => self.real.methods.contains(&m),
        }
    }

    /// The type of the type variable `var` of a real generic.
    pub(super) fn param_type(&self, owner: Owner, var: &str) -> Type {
        match owner {
            Owner::Class(t) => Type::class(&format!("{}#{var}", self.types[t].name)),
            Owner::Method(m) => Type::class(&format!("@{m}#{var}")),
        }
    }

    /// The type parameters of class `t` when it is a Rust generic.
    pub(super) fn class_type_params(&self, t: usize) -> Vec<Type> {
        if !self.is_real(Owner::Class(t)) {
            return Vec::new();
        }
        self.types[t]
            .node
            .list("typeParameters")
            .map(|p| self.param_type(Owner::Class(t), p.str("name")))
            .collect()
    }

    /// The type parameters of method `m` when it is a Rust generic.
    pub(super) fn method_type_params(&self, m: usize) -> Vec<Type> {
        if !self.is_real(Owner::Method(m)) {
            return Vec::new();
        }
        self.methods[m]
            .node
            .list("typeParameters")
            .map(|p| self.param_type(Owner::Method(m), p.str("name")))
            .collect()
    }

    /// The types of the type variables of the real generics, appended to the program's types.
    pub(super) fn param_decls(&self, types: &mut Vec<TypeDecl>) {
        let classes = self.real.classes.iter().flat_map(|&t| self.class_type_params(t));
        let methods = self.real.methods.iter().flat_map(|&m| self.method_type_params(m));
        for ty in classes.chain(methods).collect::<Vec<_>>() {
            let Type::Class(name, _) = ty else { continue };
            types.push(TypeDecl {
                name: name.to_string(),
                package: String::new(),
                kind: TypeKind::Param,
                is_abstract: true,
                extends: None,
                implements: Vec::new(),
                fields: Vec::new(),
                statics: Vec::new(),
                enum_constants: None,
                type_params: Vec::new(),
                outer: None,
                captures: Vec::new(),
                origin: Origin::Member,
                cloneable: false,
                modifiers: 0,
                enclosing: None,
                interfaces: Vec::new(),
            });
        }
    }

    /// The type of the class `t` as `this` sees it: with its own type variables as arguments when
    /// it is a Rust generic.
    pub(super) fn self_type(&self, t: usize) -> Type {
        let name = &self.types[t].name;
        if self.enum_index(name).is_some() {
            return Type::enum_type(name);
        }
        Type::generic(name, &self.class_type_params(t))
    }

    /// The pointers of the `typeParameters` of the real generics, whose type variables do not
    /// make the program use `Object`.
    pub(super) fn real_type_parameters(&self) -> std::collections::HashSet<*const Value> {
        let classes = self.real.classes.iter().map(|&t| self.types[t].node);
        let methods = self.real.methods.iter().map(|&m| self.methods[m].node);
        classes.chain(methods).map(|n| &n.0["typeParameters"] as *const Value).collect()
    }

    /// Records that a real generic cannot stay one; lowering goes on, and is run again.
    pub(super) fn fail(&self, owner: Owner) {
        let mut failed = self.failed.borrow_mut();
        match owner {
            Owner::Class(t) => failed.classes.insert(t),
            Owner::Method(m) => failed.methods.insert(m),
        };
    }

    /// The real generic whose type variable `ty` is, if it is one.
    pub(super) fn param_owner(&self, ty: Type) -> Option<Owner> {
        let Type::Class(name, _) = ty else { return None };
        let (owner, _) = name.rsplit_once('#')?;
        match owner.strip_prefix('@') {
            Some(m) => m.parse().ok().map(Owner::Method),
            None => self.type_index(owner).map(Owner::Class),
        }
    }

    /// The real generics that a type mentions: its own class and the type variables in it.
    pub(super) fn owners_in(&self, ty: Type, out: &mut Vec<Owner>) {
        match ty {
            Type::Array(element) => self.owners_in(*element, out),
            Type::Class(name, args) => {
                if let Some(owner) = self.param_owner(ty) {
                    out.push(owner);
                } else if let Some(t) = self.type_index(name)
                    && self.is_real(Owner::Class(t))
                {
                    out.push(Owner::Class(t));
                }
                args.iter().for_each(|&a| self.owners_in(a, out));
            }
            _ => {}
        }
    }

    /// Records that the real generics `ty` mentions cannot stay ones.
    fn fail_all_of(&self, ty: Type) {
        let mut owners = Vec::new();
        self.owners_in(ty, &mut owners);
        owners.into_iter().for_each(|o| self.fail(o));
    }

    /// Whether a type mentions a type variable of a real generic.
    pub(super) fn has_param(&self, ty: Type) -> bool {
        match ty {
            Type::Array(element) => self.has_param(*element),
            Type::Class(..) if self.param_owner(ty).is_some() => true,
            Type::Class(_, args) => args.iter().any(|&a| self.has_param(a)),
            _ => false,
        }
    }

    /// Checks what the lowered program does with the values of each real generic (see the
    /// module's comment), and returns those that cannot stay Rust generics.
    pub(super) fn verify(&self, types: &[TypeDecl], methods: &[Method]) -> Real {
        let check = Check { lowering: self, methods };
        for decl in types {
            // A Rust struct must use each type parameter in a field other than one of its own
            // type (`next: TableStack<K, V>`): a field whose class was erased uses none.
            for &p in &decl.type_params {
                if !decl.fields.iter().any(|f| mentions_outside(f.ty, p, &decl.name)) {
                    self.fail_all_of(p);
                }
            }
            for f in decl.fields.iter().chain(&decl.statics) {
                check.ty(f.ty);
                if let Some(init) = &f.init {
                    visit_expr(init, &mut |x| check.expr(x));
                    check.store(init, f.ty);
                }
            }
        }
        for (m, method) in methods.iter().enumerate() {
            method.params.iter().for_each(|p| check.ty(p.ty));
            check.ty(method.ret);
            for stmt in &method.body {
                visit_stmt(stmt, &mut |e| visit_expr(e, &mut |x| check.expr(x)), &mut |s| check.stmt(s, m));
            }
            // Rust infers a generic method's type parameters from its parameters' types.
            for &p in &method.type_params {
                if !method.params.iter().any(|q| mentions(q.ty, p)) {
                    self.fail_all_of(p);
                }
            }
        }
        self.failed.borrow().clone()
    }
}

/// Whether type `ty` mentions type `p`.
fn mentions(ty: Type, p: Type) -> bool {
    ty == p
        || match ty {
            Type::Array(element) => mentions(*element, p),
            Type::Class(_, args) => args.iter().any(|&a| mentions(a, p)),
            _ => false,
        }
}

/// Whether type `ty` mentions type `p` other than as an argument of the class `own`.
fn mentions_outside(ty: Type, p: Type, own: &str) -> bool {
    ty == p
        || match ty {
            Type::Array(element) => mentions_outside(*element, p, own),
            Type::Class(name, _) if name == own => false,
            Type::Class(_, args) => args.iter().any(|&a| mentions_outside(a, p, own)),
            _ => false,
        }
}

/// Whether javac's type string `ty` mentions the type variable `name`.
fn mentions_var(ty: &str, name: &str) -> bool {
    ty.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$' || c == '.')).any(|w| w == name)
}

struct Check<'l, 't> {
    lowering: &'l Lowering<'t>,
    methods: &'l [Method],
}

impl Check<'_, '_> {
    fn fail_all(&self, ty: Type) {
        let mut owners = Vec::new();
        self.lowering.owners_in(ty, &mut owners);
        owners.into_iter().for_each(|o| self.lowering.fail(o));
    }

    /// A type that occurs in the program: a real generic class must have its arguments, and an
    /// array of a type variable is not supported.
    fn ty(&self, ty: Type) {
        match ty {
            Type::Array(element) if self.lowering.has_param(*element) => self.fail_all(ty),
            Type::Array(element) => self.ty(*element),
            Type::Class(name, args) => {
                if let Some(t) = self.lowering.type_index(name)
                    && self.lowering.is_real(Owner::Class(t))
                    && args.len() != self.lowering.class_type_params(t).len()
                {
                    self.lowering.fail(Owner::Class(t));
                }
                args.iter().for_each(|&a| self.ty(a));
            }
            _ => {}
        }
    }

    /// Whether `ty` is a bare type variable of a real generic.
    fn is_param(&self, ty: Type) -> bool {
        self.lowering.param_owner(ty).is_some()
    }

    /// A value stored where a value of type `to` goes.
    fn store(&self, value: &Expr, to: Type) {
        if self.is_param(value.ty) && !self.is_param(to) {
            self.fail_all(value.ty);
        }
    }

    fn stmt(&self, stmt: &Stmt, m: usize) {
        match stmt {
            Stmt::Local { ty, init, .. } => {
                self.ty(*ty);
                if let Some(init) = init {
                    self.store(init, *ty);
                }
            }
            Stmt::Return(Some(e), _) => self.store(e, self.methods[m].ret),
            Stmt::Expr(_) | Stmt::Return(None, _) => {}
            Stmt::If { cond: e, .. } | Stmt::While { cond: e, .. } | Stmt::DoWhile { cond: e, .. } => {
                self.store(e, Type::Boolean)
            }
            Stmt::For { cond, update, .. } => {
                cond.iter().for_each(|c| self.store(c, Type::Boolean));
                // An update's value is dropped.
                let _ = update;
            }
            Stmt::ForEach { array, ty, .. } => {
                self.ty(*ty);
                self.ty(array.ty);
            }
            Stmt::Switch { selector, .. } => self.store(selector, Type::Int),
            _ => {}
        }
    }

    /// An expression and the places its operands go.
    fn expr(&self, e: &Expr) {
        self.ty(e.ty);
        let param = |x: &Expr| self.is_param(x.ty);
        match &e.kind {
            ExprKind::Cast(to, x) => {
                if self.lowering.has_param(*to) || self.lowering.has_param(x.ty) {
                    self.fail_all(*to);
                    self.fail_all(x.ty);
                }
                // An object of a real generic class becomes a value of another type.
                self.fail_all(x.ty);
                self.ty(*to);
            }
            ExprKind::InstanceOf { value, ty, .. } => {
                self.fail_all(value.ty);
                self.fail_all(*ty);
            }
            ExprKind::Assign { op: None, value, .. } => self.store(value, e.ty),
            ExprKind::Assign { value, .. } => self.store(value, Type::Void),
            ExprKind::Binary(BinOp::Eq | BinOp::Ne, l, r)
                if (param(l) && r.ty == Type::Null) || (param(r) && l.ty == Type::Null) => {}
            ExprKind::Binary(_, l, r) => {
                self.store(l, Type::Void);
                self.store(r, Type::Void);
            }
            ExprKind::Conditional(c, a, b) => {
                self.store(c, Type::Boolean);
                self.store(a, e.ty);
                self.store(b, e.ty);
            }
            ExprKind::Call { args, resolved, .. }
            | ExprKind::New { args, ctor: resolved, .. }
            | ExprKind::ThisCall { args, resolved }
            | ExprKind::SuperInit { args, resolved } => self.args(args, *resolved),
            ExprKind::MethodCall { receiver, args, resolved, .. } => {
                self.store(receiver, Type::Void);
                // A method that a real generic class inherits (`Object`'s `equals`, say) sees its
                // object as a value of the method's class.
                if let (Type::Class(name, _), Some(r)) = (receiver.ty, resolved)
                    && let Some(t) = self.lowering.type_index(name)
                    && self.lowering.is_real(Owner::Class(t))
                    && self.methods.get(*r).is_some_and(|m| m.class != t)
                {
                    self.lowering.fail(Owner::Class(t));
                }
                self.args(args, *resolved);
            }
            ExprKind::Unary(_, x) | ExprKind::Length(x) => self.store(x, Type::Void),
            ExprKind::Field { object, .. } => self.store(object, Type::Void),
            ExprKind::Index(a, i) => {
                self.store(a, Type::Void);
                self.store(i, Type::Int);
            }
            ExprKind::Builtin(_, args) | ExprKind::NewArray(args) | ExprKind::ArrayLit(args) => {
                args.iter().for_each(|a| self.store(a, Type::Void));
            }
            ExprKind::Switch { selector, .. } => self.store(selector, Type::Int),
            _ => {}
        }
    }

    /// The arguments of a call of method `resolved`: a value of a type variable goes only to a
    /// parameter of a type variable.
    fn args(&self, args: &[Expr], resolved: Option<usize>) {
        for (i, a) in args.iter().enumerate() {
            let to = resolved
                .and_then(|m| self.methods.get(m))
                .and_then(|m| m.params.get(i))
                .map_or(Type::Void, |p| p.ty);
            self.store(a, to);
        }
    }
}
