//! Which functional interfaces are closure types (`docs/closures-and-nested-classes.md`, 4.2):
//! their values are `Rc<dyn Fn(A) -> R>`, each lambda a Rust closure, and a call of the
//! interface's method a call of the closure.
//!
//! An interface is one when every value of it comes from a lambda or a method reference converted
//! to it, and nothing does with a value of it anything but call its method, store it and pass it
//! on: no conversion to another type (to `Object`, for printing, `equals` or a collection), no
//! `==`, no `instanceof`. The interface declares one abstract method and no default method, and
//! extends no other interface, nor does any interface extend it; each lambda class implements it
//! alone, and its method reads its object only through the fields that hold its captures. A
//! generic interface stays an enum for now. Everything else keeps the object model's form, an
//! enum with a variant per lambda class.

use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// The interfaces (indexes into `Class::types`) whose values are closures.
pub fn closure_types(class: &Class) -> HashSet<usize> {
    let h = &class.hierarchy;
    let object = class.type_index(OBJECT);
    let mut candidates: HashSet<usize> = HashSet::new();
    for (t, decl) in class.types.iter().enumerate() {
        if decl.kind != TypeKind::Interface
            || decl.name == OBJECT
            || decl.name == COMPARABLE
            || !decl.type_params.is_empty()
            || decl.implements.iter().any(|&i| Some(i) != object)
        {
            continue;
        }
        let own: Vec<&Method> = class.methods.iter().filter(|m| m.class == t && !m.is_static).collect();
        if own.len() != 1 || !own[0].is_abstract {
            continue;
        }
        // No interface extends it, and every class below it is a lambda's that implements it alone.
        let below: Vec<usize> = (0..class.types.len()).filter(|&s| s != t && h.is_subtype(s, t)).collect();
        let lambdas = !below.is_empty()
            && below.iter().all(|&s| {
                let d = &class.types[s];
                matches!(d.origin, Origin::Lambda { .. })
                    && d.extends.is_none()
                    && d.implements.iter().all(|&i| i == t || Some(i) == object)
            });
        if lambdas {
            candidates.insert(t);
        }
    }
    if candidates.is_empty() {
        return candidates;
    }
    let abstract_method =
        |t: usize| class.methods.iter().position(|m| m.class == t && !m.is_static).expect("one method");
    let of = |ty: Type| class.class_index(ty).filter(|t| candidates.contains(t));
    let lambda_of = |c: usize| candidates.iter().copied().find(|&t| h.is_subtype(c, t));
    // The interfaces some use rules out, and how many `new`s of each lambda class are converted
    // to its interface directly, of how many in all.
    let mut ruled_out: HashSet<usize> = HashSet::new();
    let mut news: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut on_expr = |x: &Expr| match &x.kind {
        ExprKind::Cast(to, value) => {
            if let ExprKind::New { class: c, .. } = value.kind
                && let Some(t) = lambda_of(c)
                && of(*to) == Some(t)
            {
                news.entry(c).or_default().0 += 1;
            } else if let Some(t) = of(value.ty)
                && value.ty != *to
            {
                ruled_out.insert(t);
            }
        }
        ExprKind::New { class: c, .. } => {
            if lambda_of(*c).is_some() {
                news.entry(*c).or_default().1 += 1;
            }
        }
        ExprKind::Binary(_, l, r) => {
            for operand in [l, r] {
                if let Some(t) = of(operand.ty) {
                    ruled_out.insert(t);
                }
            }
        }
        ExprKind::InstanceOf { value, .. } => {
            if let Some(t) = of(value.ty) {
                ruled_out.insert(t);
            }
        }
        ExprKind::MethodCall { receiver, resolved, .. } => {
            if let Some(t) = of(receiver.ty)
                && *resolved != Some(abstract_method(t))
            {
                ruled_out.insert(t);
            }
        }
        ExprKind::Builtin(_, args) => {
            for a in args {
                if let Some(t) = of(a.ty) {
                    ruled_out.insert(t);
                }
            }
        }
        _ => {}
    };
    let mut visit = |e: &Expr| visit_expr(e, &mut on_expr);
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(s, &mut visit, &mut |_| {});
        }
    }
    for decl in &class.types {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init {
                visit(init);
            }
        }
        for constant in decl.enum_constants.iter().flatten() {
            constant.values.iter().for_each(&mut visit);
        }
    }
    for (&c, &(converted, all)) in &news {
        if converted != all
            && let Some(t) = lambda_of(c)
        {
            ruled_out.insert(t);
        }
    }
    // A lambda's method reads its object only through its fields.
    for &t in &candidates {
        for c in h.concretes(t) {
            let Some(m) = h.implementation(abstract_method(t), c) else {
                ruled_out.insert(t);
                continue;
            };
            if class.methods[m].body.iter().any(uses_this) {
                ruled_out.insert(t);
            }
        }
    }
    candidates.retain(|t| !ruled_out.contains(t));
    candidates
}

/// Whether the statement uses `this` other than to read one of its fields.
fn uses_this(s: &Stmt) -> bool {
    let mut found = false;
    visit_stmt(s, &mut |e| found |= this_outside_fields(e), &mut |_| {});
    found
}

fn this_outside_fields(e: &Expr) -> bool {
    let mut found = false;
    let mut fields = 0usize;
    let mut this = 0usize;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This) => fields += 1,
        ExprKind::This => this += 1,
        ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
            if matches!(object.kind, ExprKind::This) =>
        {
            found = true
        }
        _ => {}
    });
    found || this > fields
}
