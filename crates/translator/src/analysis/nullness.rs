//! Which strings can be `null`. A local, parameter or return value that never holds null stays a
//! plain `String` (or `&str`); only the others become `Option`. The analysis is flow-insensitive,
//! so it over-approximates: the code generator narrows a variable inside a null check and may then
//! treat it as non-null, but never the other way round.

use super::Facts;
use crate::ast::*;

/// Records the string variables, parameters and return values that can hold null, until nothing
/// changes. A parameter is recorded under its name, as the names of a method's variables and
/// parameters are distinct.
pub fn analyze(class: &Class, facts: &mut Facts) {
    loop {
        let before = (facts.nullable_vars.len(), facts.nullable_returns.len());
        let mut vars = Vec::new();
        let mut params = Vec::new();
        let mut declared = Vec::new();
        let mut returns = Vec::new();
        for (m, method) in class.methods.iter().enumerate() {
            for stmt in &method.body {
                let mut on_expr = |e: &Expr| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Assign { op: None, target: Place::Var(target), value }
                            if x.ty.is_reference() && maybe_null(facts, m, value) =>
                        {
                            vars.push((m, target.clone()));
                        }
                        ExprKind::Call { args, resolved: Some(callee), .. } => {
                            for (i, arg) in args.iter().enumerate() {
                                if maybe_null(facts, m, arg) {
                                    params.push((*callee, i));
                                }
                            }
                        }
                        _ => {}
                    })
                };
                let mut on_stmt = |s: &Stmt| match s {
                    Stmt::Local { ty, name, init: Some(init), .. }
                        if ty.is_reference() && maybe_null(facts, m, init) =>
                    {
                        declared.push((m, name.clone()));
                    }
                    Stmt::Return(Some(value), _) if maybe_null(facts, m, value) => returns.push(m),
                    _ => {}
                };
                visit_stmt(stmt, &mut on_expr, &mut on_stmt);
            }
        }
        facts.nullable_vars.extend(vars);
        facts.nullable_vars.extend(declared);
        facts
            .nullable_vars
            .extend(params.into_iter().map(|(m, i)| (m, class.methods[m].params[i].name.clone())));
        facts.nullable_returns.extend(returns);
        if before == (facts.nullable_vars.len(), facts.nullable_returns.len()) {
            break;
        }
    }
}

/// Whether the string variable or parameter `name` of method `m` can hold null.
pub fn nullable_var(facts: &Facts, m: usize, name: &str) -> bool {
    facts.nullable_vars.contains(&(m, name.to_string()))
}

/// Whether `e`, evaluated in method `m`, can produce null.
pub fn maybe_null(facts: &Facts, m: usize, e: &Expr) -> bool {
    // Only references, and the null literal, can be null.
    if !e.ty.is_reference() {
        return false;
    }
    match &e.kind {
        ExprKind::Null => true,
        ExprKind::Var(name) => nullable_var(facts, m, name),
        ExprKind::Assign { op: None, target: Place::Var(_), value } => maybe_null(facts, m, value),
        ExprKind::Call { resolved: Some(callee), .. } => facts.nullable_returns.contains(callee),
        // In `s != null ? s : t` the arm that is just `s` is not null.
        ExprKind::Conditional(cond, a, b) => {
            let narrowed = |arm: &Expr, when: bool| matches!(&arm.kind, ExprKind::Var(v) if non_null_names(cond, when).contains(&v.as_str()));
            (!narrowed(a, true) && maybe_null(facts, m, a))
                || (!narrowed(b, false) && maybe_null(facts, m, b))
        }
        ExprKind::Cast(_, value) => maybe_null(facts, m, value),
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
