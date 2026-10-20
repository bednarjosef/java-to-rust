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
                            if x.ty == Type::String && maybe_null(facts, m, value) =>
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
                    Stmt::Local { ty: Type::String, name, init: Some(init), .. }
                        if maybe_null(facts, m, init) =>
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
    // Only strings, and the null literal, can be null.
    if !matches!(e.ty, Type::String | Type::Null) {
        return false;
    }
    match &e.kind {
        ExprKind::Null => true,
        ExprKind::Var(name) => nullable_var(facts, m, name),
        ExprKind::Assign { op: None, target: Place::Var(_), value } => maybe_null(facts, m, value),
        ExprKind::Call { resolved: Some(callee), .. } => facts.nullable_returns.contains(callee),
        ExprKind::Conditional(_, a, b) => maybe_null(facts, m, a) || maybe_null(facts, m, b),
        ExprKind::Cast(_, value) => maybe_null(facts, m, value),
        _ => false,
    }
}
