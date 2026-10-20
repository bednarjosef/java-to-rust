//! Array locals that no other name can see. Such an array is a plain `Vec`, not a shared
//! `java::Array`. A local qualifies when it is declared once in its method, is a one-dimensional
//! array, always holds a fresh array (from `new` or an initializer), and is used only through its
//! own elements and length: `a[i]`, `a[i] = v`, `a.length` and for-each. Passing it, returning it,
//! copying it to another variable or giving it to `java.util.Arrays` makes it shared.
//!
//! A counting loop `for (int i = 0; i < a.length; i++)` over a plain array, whose body neither
//! assigns `i` nor `a`, keeps `i` in bounds, so `a[i]` in its body needs no check.

use super::Facts;
use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub(super) fn analyze(class: &Class, facts: &mut Facts) {
    for (m, method) in class.methods.iter().enumerate() {
        let plain = plain_locals(method);
        for stmt in &method.body {
            visit_stmt(stmt, &mut |_| {}, &mut |s| {
                if let Some((array, index, body)) = counting_over(s, &plain) {
                    in_bounds(array, index, body, facts);
                }
            });
        }
        for name in plain {
            facts.plain_arrays.insert((m, name));
        }
    }
}

/// The array locals of `method` that are never aliased.
fn plain_locals(method: &Method) -> HashSet<String> {
    let mut declared: HashMap<String, usize> = HashMap::new();
    for p in &method.params {
        *declared.entry(p.name.clone()).or_default() += 1;
    }
    let mut arrays = HashSet::new();
    let mut escaped = HashSet::new();
    let mut good = HashSet::new();
    let mut vars = Vec::new();
    for stmt in &method.body {
        visit_stmt(
            stmt,
            &mut |e| visit_expr(e, &mut |x| use_of(x, &mut vars, &mut good, &mut escaped)),
            &mut |_| {},
        );
    }
    for stmt in &method.body {
        visit_stmt(stmt, &mut |_| {}, &mut |s| match s {
            Stmt::Local { ty, name, init, .. } => {
                *declared.entry(name.clone()).or_default() += 1;
                let Type::Array(elem) = ty else { return };
                if matches!(**elem, Type::Array(_)) || init.as_ref().is_some_and(|e| !is_fresh(e)) {
                    escaped.insert(name.clone());
                }
                arrays.insert(name.clone());
            }
            Stmt::ForEach { name, array, .. } => {
                *declared.entry(name.clone()).or_default() += 1;
                good.insert(array.id);
            }
            _ => {}
        });
    }
    let mut bad = escaped;
    for (id, name) in vars {
        if !good.contains(&id) {
            bad.insert(name);
        }
    }
    arrays.into_iter().filter(|name| declared.get(name) == Some(&1) && !bad.contains(name)).collect()
}

/// Records one expression's use of a variable: the array of an index, length, or element
/// assignment is a good use; any other mention of a variable is a use that may alias it.
fn use_of(x: &Expr, vars: &mut Vec<(ExprId, String)>, good: &mut HashSet<ExprId>, escaped: &mut HashSet<String>) {
    match &x.kind {
        ExprKind::Var(name) => vars.push((x.id, name.clone())),
        ExprKind::Index(array, _) | ExprKind::Length(array) => {
            good.insert(array.id);
        }
        ExprKind::Assign { target, value, .. } => match target {
            Place::Index(array, _) => {
                good.insert(array.id);
            }
            Place::Var(name) if !is_fresh(value) => {
                escaped.insert(name.clone());
            }
            Place::Var(_) => {}
        },
        ExprKind::IncDec { target: Place::Index(array, _), .. } => {
            good.insert(array.id);
        }
        _ => {}
    }
}

/// Whether `e` makes a new array, which no other name can hold yet.
fn is_fresh(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::NewArray(_) | ExprKind::ArrayLit(_))
}

/// For `for (int i = 0; i < a.length; i++) body` over a plain array `a`, the names `a` and `i` and
/// the body, when `i` and `a` cannot change in the body.
fn counting_over<'s>(s: &'s Stmt, plain: &HashSet<String>) -> Option<(&'s str, &'s str, &'s Stmt)> {
    let Stmt::For { init, cond: Some(cond), update, body } = s else { return None };
    let [Stmt::Local { name: index, init: Some(start), .. }] = init.as_slice() else { return None };
    let ExprKind::Binary(BinOp::Lt, var, end) = &cond.kind else { return None };
    let ExprKind::Length(array) = &end.kind else { return None };
    let ExprKind::Var(array) = &array.kind else { return None };
    let counts_up = matches!(update.as_slice(), [Expr { kind: ExprKind::IncDec { target: Place::Var(t), increment: true, .. }, .. }] if t == index);
    let starts_at_zero = matches!(start.kind, ExprKind::Int(0));
    let index_is_var = matches!(&var.kind, ExprKind::Var(v) if v == index);
    if !(counts_up && starts_at_zero && index_is_var && plain.contains(array)) {
        return None;
    }
    if assigns_stmt(body, index) || assigns_stmt(body, array) {
        return None;
    }
    Some((array, index, body))
}

/// Marks every `a[i]` in `body` as in bounds, where `a` is the array and `i` the loop index.
fn in_bounds(array: &str, index: &str, body: &Stmt, facts: &mut Facts) {
    visit_stmt(
        body,
        &mut |e| {
            visit_expr(e, &mut |x| {
                if let ExprKind::Index(a, i) = &x.kind {
                    let is = |e: &Expr, name: &str| matches!(&e.kind, ExprKind::Var(v) if v == name);
                    if is(a, array) && is(i, index) {
                        facts.in_bounds.insert(x.id);
                    }
                }
            })
        },
        &mut |_| {},
    );
}
