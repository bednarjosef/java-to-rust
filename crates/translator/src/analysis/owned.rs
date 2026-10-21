//! Owned objects: a class whose objects are never shared. An object of such a class is a plain
//! value held by the one local that created it, and its methods take `&self` or `&mut self`.
//! Nothing else may hold one: it is never a field, parameter or return value, and `this` only
//! receives a method call or a field access, so no alias can outlive the call that uses it.
//!
//! What decides it is gathered in one pass over the program for every class at once (`Uses`).

use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// Finds the owned classes among those that are not immutable (immutable objects are shared), by
/// qualified name.
pub fn owned_classes(
    class: &Class,
    immutable: &HashSet<String>,
    field_level: &HashSet<usize>,
) -> HashSet<String> {
    let uses = Uses::of(class);
    (0..class.types.len())
        .filter(|&t| {
            !immutable.contains(&class.types[t].name)
                && !class.hierarchy.has_subtypes(t)
                && uses.is_owned(t, class.types[t].name.as_str())
                && !uses.runs_inherited_object_code(class, t, field_level)
        })
        .map(|t| class.types[t].name.clone())
        .collect()
}

/// What the program does with the objects of each class, by the class's name.
#[derive(Default)]
struct Uses {
    /// The classes that are the type of a field, a parameter or a return value.
    in_signatures: HashSet<&'static str>,
    /// The classes that are a type argument, or one of a type argument, in the type of a field,
    /// a parameter, a return value, an expression or a local of a method: a generic class holds
    /// its type arguments' values as `T: Clone`, which a plain value is not.
    in_args: HashSet<&'static str>,
    /// The classes with a constructor that calls `this(...)`.
    this_calls: HashSet<usize>,
    /// Every expression of each class's type.
    typed: HashMap<&'static str, Vec<ExprId>>,
    /// The `new C(...)` expressions that initialize a local of type C.
    news: HashMap<&'static str, HashSet<ExprId>>,
    /// The names of the locals of each class's type.
    locals: HashMap<&'static str, HashSet<String>>,
    /// The classes with a local of their type that is not initialized by `new` of the class.
    bad: HashSet<&'static str>,
    /// The receivers of calls and field accesses that are a variable or `this`.
    receivers: HashSet<ExprId>,
    /// Every name that an assignment writes.
    assigned: HashSet<String>,
    /// The methods called (not with `super`) on a receiver of each class's type.
    calls_on: HashMap<&'static str, Vec<usize>>,
    /// The methods called with `super` in the methods of each class.
    super_calls: HashMap<usize, Vec<usize>>,
    /// The classes whose objects are made text: printed or concatenated.
    shown: HashSet<&'static str>,
}

impl Uses {
    fn of(class: &Class) -> Uses {
        let mut uses = Uses::default();
        let index: HashMap<&str, usize> =
            class.types.iter().enumerate().map(|(t, d)| (d.name.as_str(), t)).collect();
        for d in &class.types {
            for f in d.fields.iter().chain(&d.statics) {
                uses.signature(f.ty);
            }
        }
        for m in &class.methods {
            uses.signature(m.ret);
            m.params.iter().for_each(|p| uses.signature(p.ty));
            if m.is_ctor && has_this_call(&m.body) {
                uses.this_calls.insert(m.class);
            }
            for s in &m.body {
                let expr = &mut |e: &Expr| {
                    visit_expr(e, &mut |x| {
                        type_args(x.ty, &mut uses.in_args);
                        uses.expr(x, Some(m.class));
                    })
                };
                visit_stmt(s, expr, &mut |_| {});
                visit_stmt(s, &mut |_| {}, &mut |s| uses.decl(s, &index));
            }
        }
        for d in &class.types {
            for f in d.fields.iter().chain(&d.statics) {
                if let Some(init) = &f.init {
                    visit_expr(init, &mut |x| uses.expr(x, None));
                }
            }
        }
        uses
    }

    /// The type of a field, a parameter or a return value.
    fn signature(&mut self, ty: Type) {
        if let Type::Class(name, _) = ty {
            self.in_signatures.insert(name);
        }
        type_args(ty, &mut self.in_args);
    }

    /// A local of type C: it must be created by `new` of C.
    fn decl(&mut self, stmt: &Stmt, index: &HashMap<&str, usize>) {
        let Stmt::Local { ty, name: local, init, .. } = stmt else { return };
        type_args(*ty, &mut self.in_args);
        let Type::Class(name, _) = *ty else { return };
        self.locals.entry(name).or_default().insert(local.clone());
        match init {
            Some(Expr { kind: ExprKind::New { class, .. }, id, .. }) if index.get(name) == Some(class) => {
                self.news.entry(name).or_default().insert(*id);
            }
            _ => {
                self.bad.insert(name);
            }
        }
    }

    /// An expression, in a method of class `in_class` (`None` in a field initializer).
    fn expr(&mut self, x: &Expr, in_class: Option<usize>) {
        if let Type::Class(name, _) = x.ty {
            self.typed.entry(name).or_default().push(x.id);
        }
        let shown = |e: &Expr| match e.ty {
            Type::Class(name, _) => Some(name),
            _ => None,
        };
        match &x.kind {
            ExprKind::MethodCall { receiver, resolved, is_super, .. } => {
                match (resolved, is_super, in_class) {
                    (Some(r), true, Some(c)) => self.super_calls.entry(c).or_default().push(*r),
                    (Some(r), false, _) => {
                        if let Some(name) = shown(receiver) {
                            self.calls_on.entry(name).or_default().push(*r);
                        }
                    }
                    _ => {}
                }
                self.receiver(receiver);
            }
            ExprKind::Field { object, .. } => self.receiver(object),
            // Printing and concatenation borrow an object to read its `toString`.
            ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                for a in args {
                    self.shown.extend(shown(a));
                    self.receiver(a);
                }
            }
            ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => {
                for a in [l, r] {
                    self.shown.extend(shown(a));
                    self.receiver(a);
                }
            }
            ExprKind::Assign { target, .. } | ExprKind::IncDec { target, .. } => match target {
                Place::Var(n) => {
                    self.assigned.insert(n.clone());
                }
                Place::Field { object, .. } => self.receiver(object),
                Place::Static { .. } | Place::Index(..) => {}
            },
            _ => {}
        }
    }

    fn receiver(&mut self, e: &Expr) {
        if matches!(e.kind, ExprKind::Var(_) | ExprKind::This) {
            self.receivers.insert(e.id);
        }
    }

    /// Whether the objects of class `t`, named `name`, are never shared: it is no field's,
    /// parameter's or return value's type, nor a type argument; its constructors do not call
    /// `this(...)`; each local of its type is created by `new` and never assigned again; and each
    /// expression of its type is such a `new` or the receiver of a call or a field access.
    fn is_owned(&self, t: usize, name: &str) -> bool {
        let none = HashSet::new();
        let news = self.news.get(name).unwrap_or(&none);
        !self.in_signatures.contains(name)
            && !self.in_args.contains(name)
            && !self.this_calls.contains(&t)
            && !self.bad.contains(name)
            && self.locals.get(name).is_none_or(|locals| locals.is_disjoint(&self.assigned))
            && self
                .typed
                .get(name)
                .is_none_or(|ids| ids.iter().all(|id| news.contains(id) || self.receivers.contains(id)))
    }

    /// Whether an object of class `t` runs a method of a superclass that needs the whole object
    /// (one that does not live on the superclass's fields struct, see `hierarchy::field_level`):
    /// such a method takes the object as a value of the superclass's type, which a plain value
    /// cannot be.
    fn runs_inherited_object_code(&self, class: &Class, t: usize, field_level: &HashSet<usize>) -> bool {
        let h = &class.hierarchy;
        let name = class.types[t].name.as_str();
        let inherited = |m: usize| {
            class.methods[m].class != t && !class.methods[m].is_static && !field_level.contains(&m)
        };
        // Turning the object into text runs its `toString`.
        let text_inherited = super::hierarchy::to_string_impl(class, t).is_some_and(inherited);
        text_inherited && self.shown.contains(name)
            || self
                .calls_on
                .get(name)
                .is_some_and(|rs| rs.iter().any(|&r| h.implementation(r, t).is_some_and(inherited)))
            || self.super_calls.get(&t).is_some_and(|rs| rs.iter().any(|&r| inherited(r)))
    }
}

/// Adds to `out` the classes that are a type argument in `ty`, or in one of its type arguments.
fn type_args(ty: Type, out: &mut HashSet<&'static str>) {
    match ty {
        Type::Class(_, args) => {
            for &a in args {
                if let Type::Class(name, _) = a {
                    out.insert(name);
                }
                type_args(a, out);
            }
        }
        Type::Array(elem) => type_args(*elem, out),
        _ => {}
    }
}

fn has_this_call(body: &[Stmt]) -> bool {
    let mut found = false;
    for s in body {
        visit_stmt(
            s,
            &mut |e| visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::ThisCall { .. })),
            &mut |_| {},
        );
    }
    found
}
