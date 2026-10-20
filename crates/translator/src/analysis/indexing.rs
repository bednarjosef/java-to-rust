//! Finds `s.charAt(i)` calls whose index is always in bounds: inside a counting loop
//! `for (int i = k; i < s.length(); i++)` with `k >= 0`, where neither `i` nor `s` is assigned
//! in the body (`counting_loop` checks both), `0 <= i < s.length()` holds throughout the body.

use super::Facts;
use crate::ast::*;

pub fn analyze(class: &Class, facts: &mut Facts) {
    for method in &class.methods {
        for stmt in &method.body {
            visit_stmt(stmt, &mut |_| {}, &mut |s| {
                if let Stmt::For { init, cond, update, body } = s {
                    in_bounds(init, cond.as_ref(), update, body, facts);
                }
            });
        }
    }
}

fn in_bounds(init: &[Stmt], cond: Option<&Expr>, update: &[Expr], body: &Stmt, facts: &mut Facts) {
    let Some((index, start, end)) = counting_loop(init, cond, update, body) else { return };
    let ExprKind::Builtin(Builtin::StringLength, args) = &end.kind else { return };
    let ExprKind::Var(string) = &args[0].kind else { return };
    if !matches!(start.kind, ExprKind::Int(k) if k >= 0) {
        return;
    }
    visit_stmt(
        body,
        &mut |e| {
            visit_expr(e, &mut |x| {
                if let ExprKind::Builtin(Builtin::StringCharAt, args) = &x.kind
                    && matches!(&args[0].kind, ExprKind::Var(s) if s == string)
                    && matches!(&args[1].kind, ExprKind::Var(i) if i == index)
                {
                    facts.index_in_bounds.insert(x.id);
                }
            })
        },
        &mut |_| {},
    );
}
