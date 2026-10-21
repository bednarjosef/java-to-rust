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
use crate::program::FieldUpdate;
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
                    // The expressions of the statements of a switch expression: `visit_stmt` reports
                    // those statements to `on_stmt`, not their expressions to `on_expr`.
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

/// Rejects what the `Object` enum cannot hold or compare: an array as an `Object`, and `==` or
/// `!=` between two values of `Object` that may both be strings or boxes, whose identity Java
/// compares and the translation does not keep (a string or a box in an `Object` is a plain value).
pub(super) fn check_object_values(class: &Class, facts: &super::Facts) -> Result<(), Error> {
    let Some(object) = class.types.iter().position(|d| d.kind == TypeKind::Interface && d.name == OBJECT)
    else {
        return Ok(());
    };
    // An array in an `Object` keeps its identity: it is shared.
    let holds_values = super::hierarchy::variants(class, facts, object)
        .into_iter()
        .any(|c| class.types[c].kind == TypeKind::Runtime && class.shared_of(c).is_none());
    // A static `Object` field that is only ever set to objects of the program's classes, such as
    // the sentinel `ImmutableCollections.EMPTY = new Object()`, is never a string or a box.
    let object_of_class = |e: &Expr| {
        matches!(&e.kind, ExprKind::Cast(Type::Class(name, _), inner)
            if *name == OBJECT && matches!(inner.ty, Type::Class(..) | Type::Enum(_) | Type::Array(_) | Type::StringBuilder))
    };
    let mut sentinels: HashSet<(usize, &str)> = HashSet::new();
    for (t, decl) in class.types.iter().enumerate() {
        for f in &decl.statics {
            if matches!(f.ty, Type::Class(name, _) if name == OBJECT)
                && f.init.as_ref().is_none_or(object_of_class)
            {
                sentinels.insert((t, f.name.as_str()));
            }
        }
    }
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Assign { op, target: Place::Static { class, name }, value } = &x.kind
                            && (op.is_some() || !object_of_class(value))
                        {
                            sentinels.retain(|&(t, f)| (t, f) != (*class, name.as_str()));
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    let may_be_value = |e: &Expr| match &e.kind {
        ExprKind::Null => false,
        ExprKind::Static { class, name } if sentinels.contains(&(*class, name.as_str())) => false,
        ExprKind::Cast(Type::Class(name, _), inner) if *name == OBJECT => {
            !matches!(inner.ty, Type::Class(..) | Type::Enum(_) | Type::Array(_) | Type::StringBuilder)
        }
        _ => matches!(e.ty, Type::Class(name, _) if name == OBJECT),
    };
    let mut shortcuts = HashSet::new();
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| shortcuts.extend(shortcut(x))), &mut |_| {});
        }
    }
    let sentinel = |e: &Expr| matches!(&e.kind, ExprKind::Static { class, name } if sentinels.contains(&(*class, name.as_str())));
    let fences = fence_keys(class, &sentinel);
    // Which places may hold a string or a box, computed when a comparison needs it.
    let values = std::cell::OnceCell::new();
    let may_both_hold_values = |m: usize, l: &Expr, r: &Expr| {
        values.get_or_init(|| super::object_values::analyze(class, facts)).may_both_be_values(class, m, l, r)
    };
    let mut found = None;
    for (m, method) in class.methods.iter().enumerate() {
        let in_sorted_map = in_sorted_map(class, method.class);
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        // A builder is an `Rc`, which an `Object` that threads may share cannot hold.
                        ExprKind::Cast(Type::Class(name, _), inner)
                            if *name == OBJECT && inner.ty == Type::StringBuilder && facts.threaded =>
                        {
                            found.get_or_insert((
                                x.pos,
                                "a `StringBuilder` as an `Object` in a program with threads",
                            ));
                        }
                        ExprKind::Binary(BinOp::Eq | BinOp::Ne, l, r)
                            if holds_values
                                && may_be_value(l)
                                && may_be_value(r)
                                && !shortcuts.contains(&x.id)
                                && !(in_sorted_map && sorted_key(l, &fences) && sorted_key(r, &fences))
                                && may_both_hold_values(m, l, r) =>
                        {
                            found.get_or_insert((
                                x.pos,
                                "`==` on `Object` values that may be strings or boxes",
                            ));
                        }
                        // An atomic compare of an element compares it with the expected value
                        // by identity, which a string or a box does not keep.
                        ExprKind::Builtin(
                            Builtin::ElementUpdate(
                                FieldUpdate::CompareAndSet | FieldUpdate::CompareAndExchange,
                            ),
                            args,
                        ) if holds_values
                            && may_be_value(&args[0])
                            && may_both_hold_values(m, &args[0], &args[0]) =>
                        {
                            found.get_or_insert((
                                x.pos,
                                "an atomic compare of `Object` elements that may be strings or boxes",
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

/// `TreeMap`'s entries, whose keys `TreeMap`'s own code compares by identity.
const SORTED_ENTRY: &str = "java.util.TreeMap.Entry";

/// Whether type `t` is `TreeMap` or a class nested in it.
fn in_sorted_map(class: &Class, t: usize) -> bool {
    let name = &class.types[t].name;
    name.strip_prefix("java.util.TreeMap").is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

/// The `Object` fields of `TreeMap`'s classes that only ever hold the key of an entry, `null` or
/// a sentinel, as the iterators of a sub-map hold its end in `fenceKey = fence == null ?
/// UNBOUNDED : fence.key`, by the class's name and the field's.
fn fence_keys<'c>(class: &'c Class, sentinel: &dyn Fn(&Expr) -> bool) -> HashSet<(&'c str, &'c str)> {
    fn key_or_end(e: &Expr, sentinel: &dyn Fn(&Expr) -> bool) -> bool {
        match &peel(e).kind {
            ExprKind::Null => true,
            ExprKind::Conditional(_, a, b) => key_or_end(a, sentinel) && key_or_end(b, sentinel),
            _ => sentinel(peel(e)) || entry_key(e),
        }
    }
    let mut fences: HashSet<(&str, &str)> = HashSet::new();
    for (t, decl) in class.types.iter().enumerate() {
        if !in_sorted_map(class, t) {
            continue;
        }
        for f in &decl.fields {
            if matches!(f.ty, Type::Class(name, _) if name == OBJECT)
                && f.init.as_ref().is_none_or(|e| key_or_end(e, sentinel))
            {
                fences.insert((decl.name.as_str(), f.name.as_str()));
            }
        }
    }
    // Any assignment of something else, anywhere in the program, disqualifies the field.
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Assign { op, target: Place::Field { object, name }, value } = &x.kind
                            && let Type::Class(owner, _) = object.ty
                            && (op.is_some() || !key_or_end(value, sentinel))
                        {
                            fences.retain(|&(t, f)| (t, f) != (owner, name.as_str()));
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    fences
}

/// Whether `e` reads the key of an entry of a `TreeMap`.
fn entry_key(e: &Expr) -> bool {
    matches!(&peel(e).kind, ExprKind::Field { object, name }
        if name == "key" && matches!(object.ty, Type::Class(entry, _) if entry == SORTED_ENTRY))
}

/// Whether `e`, in `TreeMap`'s code, is the key of one of the map's entries, or a field of
/// `fence_keys`, the key of one or an end that is not a string or a box. Compared by value, such
/// keys give what Java's identity gives: the keys of a sorted map are distinct under its
/// comparator, which, in the translation, sees strings and boxes only by value, so two keys that
/// are equal values compare as equal and cannot both be in the map. (`TreeMap`'s code compares
/// only keys of one map with each other.)
fn sorted_key(e: &Expr, fences: &HashSet<(&str, &str)>) -> bool {
    entry_key(e)
        || matches!(&peel(e).kind, ExprKind::Field { object, name }
            if matches!(object.ty, Type::Class(owner, _) if fences.contains(&(owner, name.as_str()))))
}

/// The `a == b` of `a == b || .. a.equals(b) ..`, the shortcut before an `equals` that `Objects.equals`
/// takes: whether the identities compare equal or not, the result is what `equals` says when the
/// values are strings or boxes. The `==` may be any operand of a chain of `||` with the `equals`
/// in a later one, as `HashMap.removeNode` writes `!matchValue || (v = node.value) == value ||
/// (value != null && value.equals(v))`.
fn shortcut(e: &Expr) -> Vec<ExprId> {
    // Its negation too, `a != b && .. !a.equals(b) ..`, as `EnumMap.equals` writes it.
    let (op, test) = match &e.kind {
        ExprKind::Binary(BinOp::Or, ..) => (BinOp::Or, BinOp::Eq),
        ExprKind::Binary(BinOp::And, ..) => (BinOp::And, BinOp::Ne),
        _ => return Vec::new(),
    };
    let mut operands = Vec::new();
    chain(e, op, &mut operands);
    let mut found = Vec::new();
    for (i, l) in operands.iter().enumerate() {
        let ExprKind::Binary(o, a, b) = &l.kind else { continue };
        // An operand may assign its variable first, as `HashMap.getNode` writes `(k = first.key) == key`.
        let (true, Some(a), Some(b)) = (*o == test, compared(a), compared(b)) else { continue };
        let mut equals = false;
        for r in &operands[i + 1..] {
            visit_expr(r, &mut |x| {
                if let ExprKind::MethodCall { receiver, name, args, .. } = &x.kind
                    && name == "equals"
                    && let (Some(x), [arg]) = (compared(receiver), &args[..])
                    && let Some(y) = compared(arg)
                {
                    equals |= (x == a && y == b) || (x == b && y == a);
                }
            });
        }
        if equals {
            found.push(l.id);
        }
    }
    found
}

/// The operands of the chain of `op` that `e` is, left to right.
fn chain<'e>(e: &'e Expr, op: BinOp, operands: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Binary(o, l, r) if *o == op => {
            chain(l, op, operands);
            chain(r, op, operands);
        }
        _ => operands.push(e),
    }
}

/// What an operand of the shortcut's `==` reads: a variable (or an assignment to one), or a field
/// of `this` (`this.name`), as `ConcurrentHashMap`'s entries compare `k == key || k.equals(key)`.
fn compared(e: &Expr) -> Option<String> {
    match &peel(e).kind {
        ExprKind::Var(name) | ExprKind::Assign { op: None, target: Place::Var(name), .. } => {
            Some(name.clone())
        }
        ExprKind::Field { object, name } if matches!(object.kind, ExprKind::This) => {
            Some(format!("this.{name}"))
        }
        _ => None,
    }
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
