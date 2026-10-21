//! Which strings and objects can be `null`. A local, parameter, return value or field that never
//! holds null stays a plain value (or a borrowed `&str`); only the others become `Option`. The
//! analysis is flow-insensitive, so it over-approximates: the code generator narrows a variable
//! inside a null check and may then treat it as non-null, but never the other way round.
//!
//! A field starts as Java's `null` unless something sets it first. An object field is not null
//! when every constructor sets it, among the assignments that become the constructor's struct
//! literal (see `leading_assignments`) or by its initializer, to a value that is not null, and no
//! other assignment stores a value that may be null. A static field needs a non-null initializer
//! that runs on first use (see `statics`).

use super::{Facts, leading_assignments};
use crate::ast::*;

/// Records the variables, parameters, return values and fields that can hold null, until nothing
/// changes. A parameter is recorded under its name, as the names of a method's variables and
/// parameters are distinct.
pub fn analyze(class: &Class, facts: &mut Facts) {
    loop {
        let before = (
            facts.nullable_vars.len(),
            facts.nullable_returns.len(),
            facts.nullable_fields.len(),
            facts.nullable_statics.len(),
            facts.nullable_elements.len(),
        );
        let mut elements = Vec::new();
        let mut vars = Vec::new();
        let mut params = Vec::new();
        let mut declared = Vec::new();
        let mut returns = Vec::new();
        let mut fields = Vec::new();
        let mut statics = Vec::new();
        let mut placeholders = Vec::new();
        let mut on_expr = |m: usize, e: &Expr| {
            visit_expr(e, &mut |x| match &x.kind {
                ExprKind::Assign { op: None, target, value }
                    if nullable_type(x.ty) && maybe_null(facts, m, value) =>
                {
                    match target {
                        Place::Var(name) => vars.push((m, name.clone())),
                        Place::Field { object, name } => {
                            if let Some(owner) = class.field_owner_name(object.ty, name) {
                                fields.push((owner.to_string(), name.clone()));
                            }
                        }
                        Place::Static { class, name } => statics.push((*class, name.clone())),
                        Place::Index(array, _) => elements.push(array.ty.element()),
                    }
                }
                // `new T[n]` leaves its strings and objects null, and `new T[n][]` its
                // arrays.
                ExprKind::NewArray(sizes) => {
                    let elem = x.ty.element_at(sizes.len());
                    if elem.is_reference() {
                        elements.push(elem);
                    }
                }
                // `Arrays.copyOf` and `copyOfRange` pad a longer copy with null.
                ExprKind::Builtin(Builtin::ArraysCopyOf | Builtin::Arrays(ArraysOp::CopyOfRange), _)
                    if x.ty.element().is_reference() =>
                {
                    elements.push(x.ty.element());
                }
                ExprKind::ArrayLit(items) if items.iter().any(|item| maybe_null(facts, m, item)) => {
                    elements.push(x.ty.element());
                }
                ExprKind::Call { args, resolved: Some(callee), .. }
                | ExprKind::MethodCall { args, resolved: Some(callee), .. }
                | ExprKind::New { args, ctor: Some(callee), .. }
                | ExprKind::ThisCall { args, resolved: Some(callee) }
                | ExprKind::SuperInit { args, resolved: Some(callee) } => {
                    // The argument reaches every method the call may run.
                    for (i, arg) in args.iter().enumerate() {
                        if maybe_null(facts, m, arg) {
                            params.push((*callee, i));
                        }
                    }
                }
                _ => {}
            })
        };
        for (m, method) in class.methods.iter().enumerate() {
            for stmt in &method.body {
                let mut on_stmt = |s: &Stmt| match s {
                    Stmt::Local { ty, name, init: Some(init), .. }
                        if nullable_type(*ty) && maybe_null(facts, m, init) =>
                    {
                        declared.push((m, name.clone()));
                    }
                    Stmt::Return(Some(value), _) if maybe_null(facts, m, value) => returns.push(m),
                    Stmt::ForEach { name, array, .. }
                        if facts.nullable_elements.contains(&array.ty.element()) =>
                    {
                        declared.push((m, name.clone()));
                    }
                    _ => {}
                };
                visit_stmt(stmt, &mut |e| on_expr(m, e), &mut on_stmt);
            }
        }
        // Field initializers run in every object's constructor: a `new String[2]` there leaves
        // its strings null too.
        for f in class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics)) {
            if let Some(init) = &f.init {
                on_expr(usize::MAX, init);
            }
        }
        for (t, decl) in class.types.iter().enumerate() {
            for f in decl.fields.iter().filter(|f| may_be_unset(f.ty)) {
                if !set_by_construction(class, facts, t, &f.name) {
                    fields.push((decl.name.clone(), f.name.clone()));
                }
            }
            for f in decl.statics.iter().filter(|f| may_be_unset(f.ty)) {
                // An eagerly initialized static starts at a placeholder, which an object has not,
                // so its cell starts empty; once the class is initialized, it holds the value.
                if !f.init.as_ref().is_some_and(|e| !maybe_null(facts, usize::MAX, e)) {
                    statics.push((t, f.name.clone()));
                } else if facts.eager_init.contains(&t) && matches!(f.ty, Type::Class(..)) {
                    placeholders.push((t, f.name.clone()));
                }
            }
        }
        facts.nullable_vars.extend(vars);
        facts.nullable_vars.extend(declared);
        // The methods of an override group share their parameters' and return value's nullness,
        // so that a call may run any of them.
        let params: Vec<(usize, usize)> = params
            .into_iter()
            .flat_map(|(m, i)| class.hierarchy.group(m).into_iter().map(move |g| (g, i)))
            .collect();
        facts
            .nullable_vars
            .extend(params.into_iter().map(|(m, i)| (m, class.methods[m].params[i].name.clone())));
        let returns: Vec<usize> = returns.into_iter().flat_map(|m| class.hierarchy.group(m)).collect();
        facts.nullable_returns.extend(returns);
        facts.nullable_fields.extend(fields);
        facts.nullable_statics.extend(statics);
        facts.placeholder_statics.extend(placeholders);
        facts.nullable_elements.extend(elements);
        let after = (
            facts.nullable_vars.len(),
            facts.nullable_returns.len(),
            facts.nullable_fields.len(),
            facts.nullable_statics.len(),
            facts.nullable_elements.len(),
        );
        if before == after {
            break;
        }
    }
    // A static that the program may set to null holds null like any other, whatever its
    // initializer: its cell is an `Option` that is read as it is.
    let nullable = facts.nullable_statics.clone();
    facts.placeholder_statics.retain(|key| !nullable.contains(key));
}

/// The fields whose nullness is analyzed: strings, arrays and objects. A boxed field is always an
/// `Option`.
fn may_be_unset(ty: Type) -> bool {
    matches!(ty, Type::String | Type::StringBuilder | Type::Array(_) | Type::Class(..))
}

/// Whether every constructor of class `t` sets the field `name` to a value that is not null
/// before anything can read it.
fn set_by_construction(class: &Class, facts: &Facts, t: usize, name: &str) -> bool {
    let decl = &class.types[t];
    let i = decl.fields.iter().position(|f| f.name == name).expect("a field of the class");
    let init = decl.fields[i].init.as_ref();
    if decl.fields[i].ty == Type::String && init.is_some() {
        // A string field's initializer, or the empty string that stands in for it, is not null.
        return init.is_some_and(|e| !maybe_null(facts, usize::MAX, e));
    }
    class.methods.iter().enumerate().filter(|(_, m)| m.is_ctor && m.class == t).all(|(index, m)| {
        let delegates =
            matches!(m.body.first(), Some(Stmt::Expr(Expr { kind: ExprKind::ThisCall { .. }, .. })));
        if delegates {
            return true;
        }
        let leading = leading_assignments(class, m, facts);
        match leading.iter().find(|&&(j, _)| j == i) {
            Some((_, value)) => !maybe_null(facts, index, value),
            None => {
                super::inline_initializers(class, t, facts)
                    && init.is_some_and(|e| !maybe_null(facts, index, e))
            }
        }
    })
}

/// Whether the string variable or parameter `name` of method `m` can hold null.
pub fn nullable_var(facts: &Facts, m: usize, name: &str) -> bool {
    facts.nullable_vars.contains(&(m, name.to_string()))
}

/// The types whose values can be null: references, and enums, which are only ever null in a local
/// (see `lower::enum_nulls`).
pub fn nullable_type(ty: Type) -> bool {
    ty.is_reference() || matches!(ty, Type::Enum(_))
}

/// Whether `e`, evaluated in method `m`, can produce null.
pub fn maybe_null(facts: &Facts, m: usize, e: &Expr) -> bool {
    // Only references, enums and the null literal can be null.
    if !nullable_type(e.ty) {
        return false;
    }
    match &e.kind {
        ExprKind::Null => true,
        ExprKind::Var(name) => nullable_var(facts, m, name),
        ExprKind::Assign { op: None, target: Place::Var(_), value } => maybe_null(facts, m, value),
        ExprKind::Call { resolved: Some(callee), .. }
        | ExprKind::MethodCall { resolved: Some(callee), .. } => facts.nullable_returns.contains(callee),
        // In `s != null ? s : t` the arm that is just `s` is not null.
        ExprKind::Conditional(cond, a, b) => {
            let narrowed = |arm: &Expr, when: bool| matches!(&arm.kind, ExprKind::Var(v) if non_null_names(cond, when).contains(&v.as_str()));
            (!narrowed(a, true) && maybe_null(facts, m, a))
                || (!narrowed(b, false) && maybe_null(facts, m, b))
        }
        ExprKind::Cast(_, value) => maybe_null(facts, m, value),
        // A boxed field is always an `Option`: its default is `null`, and so is an assignment of
        // `null` to it.
        _ if matches!(e.ty, Type::Boxed(_))
            && matches!(
                e.kind,
                ExprKind::Field { .. }
                    | ExprKind::Static { .. }
                    | ExprKind::Assign { op: None, target: Place::Field { .. } | Place::Static { .. }, .. }
            ) =>
        {
            true
        }
        ExprKind::Field { object, name } => facts.field_nullable(object.ty, name),
        ExprKind::Static { class, name } => facts.nullable_statics.contains(&(*class, name.clone())),
        ExprKind::Index(array, _) => facts.nullable_elements.contains(&array.ty.element()),
        ExprKind::Assign {
            op: None,
            target: Place::Field { .. } | Place::Static { .. } | Place::Index(..),
            value,
        } => maybe_null(facts, m, value),
        _ => false,
    }
}

/// The variables that `cond` proves not null when it evaluates to `when`: those tested with
/// `s != null` in its top-level `&&` chain (and not assigned later in the chain) when true, the
/// one tested by a whole `s == null` when false.
pub fn non_null_names(cond: &Expr, when: bool) -> Vec<&str> {
    let mut parts = Vec::new();
    conjuncts(cond, &mut parts);
    if !when {
        return match &parts[..] {
            [test] => {
                null_test(test).filter(|&(_, non_null)| !non_null).map(|(v, _)| v).into_iter().collect()
            }
            _ => Vec::new(),
        };
    }
    let mut out = Vec::new();
    for (k, part) in parts.iter().enumerate() {
        if let Some((v, true)) = null_test(part)
            && !parts[k + 1..].iter().any(|later| assigns(later, v))
        {
            out.push(v);
        }
    }
    out
}

/// The operands of a chain of `&&`, left to right.
pub fn conjuncts<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Binary(BinOp::And, l, r) => {
            conjuncts(l, out);
            conjuncts(r, out);
        }
        _ => out.push(e),
    }
}

/// `v == null` or `v != null` (either way round) on a variable: the variable and whether the test
/// is true when it is not null.
pub fn null_test(e: &Expr) -> Option<(&str, bool)> {
    let ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) = &e.kind else { return None };
    let name = match (&l.kind, &r.kind) {
        (ExprKind::Var(v), ExprKind::Null) | (ExprKind::Null, ExprKind::Var(v)) => v,
        _ => return None,
    };
    Some((name.as_str(), *op == BinOp::Ne))
}
