//! Pattern variables, `x instanceof T name`, where the translation binds them. Java scopes a
//! pattern variable by flow: it is in scope wherever the test is known to have passed. The
//! translation binds one with a let chain, an `if let`, a `let .. else` or `is_some_and`, which
//! covers the places it is in scope in:
//!
//! - an operand of an `&&` chain: the later operands, and when the chain is the condition of an
//!   `if`, a `while` or a `?:`, the branch or body it guards;
//! - `if (!(x instanceof T name)) .. else ..`: the else branch, and with no else branch and a
//!   then branch that always leaves the block (`return`, `break`, `continue`), the rest of the
//!   block.
//!
//! The statements of a switch expression are checked as a block of their own.
//!
//! A program that reads a pattern variable anywhere else is rejected. Java has already checked
//! that each read is in the variable's scope, which lets this check be simple: a read of `name`
//! in code that does not declare a `name` of its own is a read of the pattern's variable.

use crate::ast::*;
use std::collections::HashSet;

pub(super) fn check(class: &Class) -> Result<(), Error> {
    for m in &class.methods {
        let declared = declarations(&m.body);
        // The patterns that `if (!(x instanceof T name))` tests, which `statement` checks.
        let mut negated = Vec::new();
        for s in &m.body {
            visit_stmt(s, &mut |_| {}, &mut |s| {
                if let Stmt::If { cond, .. } = s
                    && let ExprKind::Unary(UnOp::Not, inner) = &cond.kind
                {
                    negated.push(inner.id);
                }
            });
        }
        for s in &m.body {
            let mut found = Ok(());
            visit_stmt(
                s,
                &mut |e| {
                    if found.is_ok() {
                        found = expression(e, &m.body, &declared, &negated);
                    }
                    // The statements of a switch expression, which `visit_stmt` does not reach.
                    switch_bodies(e, &mut |body| {
                        for inner in body {
                            visit_stmt(
                                inner,
                                &mut |x| {
                                    if found.is_ok() {
                                        found = expression(x, &m.body, &declared, &negated);
                                    }
                                },
                                &mut |_| {},
                            );
                        }
                    });
                },
                &mut |_| {},
            );
            found?;
        }
        block(&m.body)?;
    }
    Ok(())
}

/// Rejects a cast to an enum of a value that may be null: an enum is null only in a local that is
/// assigned `null` (see `lower::enum_nulls`), and a cast would give one a `None` anywhere.
pub(super) fn check_enum_casts(class: &Class, facts: &super::Facts) -> Result<(), Error> {
    for (m, method) in class.methods.iter().enumerate() {
        let mut found = None;
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Cast(Type::Enum(_), value) = &x.kind
                            && matches!(value.ty, Type::Class(..))
                            && super::maybe_null(facts, m, value)
                        {
                            found.get_or_insert(x.pos);
                        }
                    })
                },
                &mut |_| {},
            );
        }
        if let Some(pos) = found {
            return Err(Error {
                pos,
                message: "casting a value that may be null to an enum not supported yet".to_string(),
            });
        }
    }
    Ok(())
}

/// Rejects what the `Object` enum cannot hold or compare: an array as an `Object`, and `==` or
/// `!=` between two values of `Object` that may both be strings or boxes, whose identity Java
/// compares and the translation does not keep (a string or a box in an `Object` is a plain value).
pub(super) fn check_object_values(class: &Class, facts: &super::Facts) -> Result<(), Error> {
    let Some(object) = class.types.iter().position(|d| d.kind == TypeKind::Interface && d.name == OBJECT)
    else {
        return Ok(());
    };
    let holds_values = super::hierarchy::variants(class, facts, object)
        .into_iter()
        .any(|c| class.types[c].kind == TypeKind::Runtime);
    let may_be_value = |e: &Expr| match &e.kind {
        ExprKind::Null => false,
        ExprKind::Cast(Type::Class(name, _), inner) if *name == OBJECT => {
            !matches!(inner.ty, Type::Class(..) | Type::Enum(_))
        }
        _ => matches!(e.ty, Type::Class(name, _) if name == OBJECT),
    };
    let mut shortcuts = HashSet::new();
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| shortcuts.extend(shortcut(x))), &mut |_| {});
        }
    }
    let mut found = None;
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Cast(Type::Class(name, _), inner)
                            if *name == OBJECT && matches!(inner.ty, Type::Array(_)) =>
                        {
                            found.get_or_insert((x.pos, "an array as an `Object` value"));
                        }
                        ExprKind::Cast(Type::Class(name, _), inner)
                            if *name == OBJECT && inner.ty == Type::StringBuilder =>
                        {
                            found.get_or_insert((x.pos, "a `StringBuilder` as an `Object` value"));
                        }
                        ExprKind::Binary(BinOp::Eq | BinOp::Ne, l, r)
                            if holds_values
                                && may_be_value(l)
                                && may_be_value(r)
                                && !shortcuts.contains(&x.id) =>
                        {
                            found.get_or_insert((
                                x.pos,
                                "`==` on `Object` values that may be strings or boxes",
                            ));
                        }
                        _ => {}
                    })
                },
                &mut |_| {},
            );
        }
    }
    match found {
        Some((pos, what)) => Err(Error { pos, message: format!("{what} not supported yet") }),
        None => Ok(()),
    }
}

/// The `a == b` of `a == b || .. a.equals(b) ..`, the shortcut before an `equals` that `Objects.equals`
/// takes: whether the identities compare equal or not, the result is what `equals` says when the
/// values are strings or boxes.
fn shortcut(e: &Expr) -> Option<ExprId> {
    let ExprKind::Binary(BinOp::Or, l, r) = &e.kind else { return None };
    let ExprKind::Binary(BinOp::Eq, a, b) = &l.kind else { return None };
    let (ExprKind::Var(a), ExprKind::Var(b)) = (&a.kind, &b.kind) else { return None };
    let mut equals = false;
    visit_expr(r, &mut |x| {
        if let ExprKind::MethodCall { receiver, name, args, .. } = &x.kind
            && name == "equals"
            && let (ExprKind::Var(x), [arg]) = (&receiver.kind, &args[..])
            && let ExprKind::Var(y) = &peel(arg).kind
        {
            equals |= (x == a && y == b) || (x == b && y == a);
        }
    });
    equals.then_some(l.id)
}

/// `e` without the conversions to `Object` that lowering adds.
fn peel(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Cast(Type::Class(name, _), inner) if *name == OBJECT => peel(inner),
        _ => e,
    }
}

fn unsupported(pos: Pos) -> Error {
    Error { pos, message: "a pattern variable used here not supported yet".to_string() }
}

/// A pattern: its id, where it is and its variable.
struct Pattern {
    id: ExprId,
    pos: Pos,
    name: String,
}

/// The patterns in `e`, but those in the statements of a switch expression in it.
fn patterns(e: &Expr) -> Vec<Pattern> {
    let mut inner = HashSet::new();
    switch_bodies(e, &mut |body| {
        for s in body {
            visit_stmt(s, &mut |x| visit_expr(x, &mut |y| _ = inner.insert(y.id)), &mut |_| {});
        }
    });
    let mut out = Vec::new();
    visit_expr(e, &mut |x| {
        if let ExprKind::InstanceOf { binding: Some(name), .. } = &x.kind
            && !inner.contains(&x.id)
        {
            out.push(Pattern { id: x.id, pos: x.pos, name: name.clone() });
        }
    });
    out
}

/// Calls `f` on the statements of each case of the switch expressions in `e`.
fn switch_bodies(e: &Expr, f: &mut dyn FnMut(&[Stmt])) {
    visit_expr(e, &mut |x| {
        if let ExprKind::Switch { cases, .. } = &x.kind {
            cases.iter().for_each(|c| f(&c.body));
        }
    });
}

/// The ids of the patterns that are operands of an `&&` chain in `e` (or all of `e`, or of the
/// condition of a `?:`), whose variables the translation binds for the later operands.
fn chained(e: &Expr, out: &mut Vec<ExprId>) {
    let operands = |e: &Expr, out: &mut Vec<ExprId>| {
        let mut parts = Vec::new();
        super::conjuncts(e, &mut parts);
        out.extend(parts.iter().filter(|p| matches!(p.kind, ExprKind::InstanceOf { .. })).map(|p| p.id));
    };
    operands(e, out);
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Binary(BinOp::And, ..) => operands(x, out),
        ExprKind::Conditional(cond, ..) => operands(cond, out),
        _ => {}
    });
}

/// Every name that the statements declare: locals, loop variables and pattern variables, with
/// how many times.
fn declarations(stmts: &[Stmt]) -> Vec<String> {
    let mut out = Vec::new();
    for s in stmts {
        visit_stmt(s, &mut |e| out.extend(patterns(e).into_iter().map(|p| p.name)), &mut |_| {});
    }
    for s in stmts {
        visit_stmt(s, &mut |_| {}, &mut |s| {
            if let Stmt::Local { name, .. } | Stmt::ForEach { name, .. } = s {
                out.push(name.clone());
            }
        });
    }
    out
}

/// A pattern in `e` that is not an operand of a chain, nor what `if (!(..))` tests: its variable must not be read in `e` (if
/// the method declares the name only there, anywhere in the method).
fn expression(e: &Expr, body: &[Stmt], declared: &[String], negated: &[ExprId]) -> Result<(), Error> {
    let mut ok = negated.to_vec();
    chained(e, &mut ok);
    for p in patterns(e) {
        if ok.contains(&p.id) {
            continue;
        }
        let name = p.name.as_str();
        let only = declared.iter().filter(|d| *d == name).count() == 1;
        let read = if only { body.iter().any(|s| mentions_stmt(s, name)) } else { mentions(e, name) };
        if read {
            return Err(unsupported(p.pos));
        }
    }
    Ok(())
}

/// Whether `stmts` read or write `name` without declaring a `name` of their own.
fn reads(stmts: &[&Stmt], name: &str) -> bool {
    let declares = |s: &Stmt| declarations(std::slice::from_ref(s)).iter().any(|d| d == name);
    stmts.iter().any(|s| mentions_stmt(s, name)) && !stmts.iter().any(|s| declares(s))
}

fn mentions_stmt(stmt: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(stmt, &mut |e| found |= mentions(e, name), &mut |_| {});
    found
}

/// Whether `stmt` always leaves the block it is in.
fn exits(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(..) | Stmt::Break | Stmt::Continue => true,
        Stmt::Block(stmts) => stmts.last().is_some_and(exits),
        _ => false,
    }
}

fn block(stmts: &[Stmt]) -> Result<(), Error> {
    let all: Vec<&Stmt> = stmts.iter().collect();
    for (i, s) in stmts.iter().enumerate() {
        statement(s, &all[i + 1..])?;
    }
    Ok(())
}

/// The statement `s`, followed by `rest` in its block: a pattern variable of its own expressions
/// may be read only where the translation binds it.
fn statement(s: &Stmt, rest: &[&Stmt]) -> Result<(), Error> {
    let rest = rest.to_vec();
    let nested = |s: &Stmt| match s {
        Stmt::Block(stmts) => block(stmts),
        _ => statement(s, &[]),
    };
    // The patterns of `e`, whose variables `outside` may not read.
    let check = |e: &Expr, outside: &[&Stmt]| -> Result<(), Error> {
        for p in patterns(e) {
            if reads(outside, &p.name) {
                return Err(unsupported(p.pos));
            }
        }
        Ok(())
    };
    match s {
        Stmt::If { cond, then, otherwise } => {
            let mut parts = Vec::new();
            super::conjuncts(cond, &mut parts);
            for p in patterns(cond) {
                let negated = matches!(&cond.kind, ExprKind::Unary(UnOp::Not, inner) if inner.id == p.id);
                let positive = parts.iter().any(|part| part.id == p.id);
                let mut outside: Vec<&Stmt> = Vec::new();
                if !positive {
                    outside.push(then);
                }
                if !negated && let Some(o) = otherwise {
                    outside.push(o);
                }
                if !(negated && otherwise.is_none() && exits(then)) {
                    outside.extend(&rest);
                }
                if reads(&outside, &p.name) {
                    return Err(unsupported(p.pos));
                }
            }
            nested(then)?;
            if let Some(o) = otherwise {
                nested(o)?;
            }
        }
        Stmt::While { cond, body } => {
            let mut parts = Vec::new();
            super::conjuncts(cond, &mut parts);
            for p in patterns(cond) {
                let mut outside = rest.clone();
                if !parts.iter().any(|part| part.id == p.id) {
                    outside.push(body);
                }
                if reads(&outside, &p.name) {
                    return Err(unsupported(p.pos));
                }
            }
            nested(body)?;
        }
        Stmt::DoWhile { body, cond } => {
            check(cond, &rest)?;
            nested(body)?;
        }
        Stmt::For { init, cond, update, body } => {
            let mut outside: Vec<&Stmt> = rest.clone();
            outside.push(body);
            outside.extend(init);
            if let Some(cond) = cond {
                check(cond, &outside)?;
            }
            for u in update {
                check(u, &outside)?;
            }
            block(init)?;
            nested(body)?;
        }
        Stmt::ForEach { array, body, .. } => {
            let mut outside = rest.clone();
            outside.push(body);
            check(array, &outside)?;
            nested(body)?;
        }
        Stmt::Local { init: Some(e), .. }
        | Stmt::Expr(e)
        | Stmt::Return(Some(e), _)
        | Stmt::Yield(e)
        | Stmt::Throw(e) => {
            check(e, &rest)?;
            let mut found = Ok(());
            switch_bodies(e, &mut |body| {
                if found.is_ok() {
                    found = block(body);
                }
            });
            found?;
        }
        Stmt::Try { body, catches, finally } => {
            block(body)?;
            for catch in catches {
                block(&catch.body)?;
            }
            if let Some(finally) = finally {
                block(finally)?;
            }
        }
        Stmt::Switch { selector, cases } => {
            let mut outside = rest.clone();
            outside.extend(cases.iter().flat_map(|c| &c.body));
            check(selector, &outside)?;
            for case in cases {
                block(&case.body)?;
            }
        }
        Stmt::Labeled { body, .. } => statement(body, &rest)?,
        Stmt::Block(stmts) => block(stmts)?,
        Stmt::Local { init: None, .. }
        | Stmt::Return(None, _)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_) => {}
    }
    Ok(())
}
