//! Reads of a static that may run while its class's initialization is under way, before the
//! static initializer has set it. Java's `static final Node accept = new Node();` in `Pattern`,
//! with `Node() { next = Pattern.accept; }`: the first `Node` is built while `accept` is still
//! null, so that read gives null, and `next` holds it. A static that starts empty
//! (`Facts::placeholder_statics`) is then null wherever such a read flows (see `nullness`).

use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// What a method does that matters here: the statics it reads, by expression, and the methods
/// that running it may run, its callees and the static initializers of the classes it uses.
#[derive(Default)]
struct Summary {
    reads: Vec<(usize, String, ExprId)>,
    runs: Vec<usize>,
}

/// The reads of a static of an object type of a class with an eager static initializer that may
/// run, within that initializer or code it runs, before the initializer's own assignment of the
/// static: the statements of the `<clinit>` run in order, and a static counts as set after the
/// statement that assigns it, not during it.
pub fn early_reads(class: &Class, eager: &HashSet<usize>) -> HashSet<ExprId> {
    let clinits: HashMap<usize, usize> = (class.methods.iter().enumerate())
        .filter(|(_, m)| m.name == "<clinit>")
        .map(|(i, m)| (m.class, i))
        .collect();
    let mut summaries: Vec<Option<Summary>> = (0..class.methods.len()).map(|_| None).collect();
    let mut early = HashSet::new();
    // The methods seen for the initializer of each class, as the number of that class's turn:
    // a set for each class without clearing one.
    let mut seen_in: Vec<u32> = vec![0; class.methods.len()];
    let mut turn = 0;
    for &t in eager {
        let Some(&clinit) = clinits.get(&t) else { continue };
        let mut unset: HashSet<&str> = (class.types[t].statics.iter())
            .filter(|f| matches!(f.ty, Type::Class(..)) && f.init.is_some())
            .map(|f| f.name.as_str())
            .collect();
        if unset.is_empty() {
            continue;
        }
        let mut statements = Vec::new();
        flatten(&class.methods[clinit].body, &mut statements);
        // The initializer under way returns at once when code it runs uses its class again.
        turn += 1;
        seen_in[clinit] = turn;
        for s in statements {
            if unset.is_empty() {
                break;
            }
            let mut first = Summary::default();
            visit_stmt(s, &mut |e| summarize(class, &clinits, e, &mut first), &mut |_| {});
            let mut mark = |reads: &[(usize, String, ExprId)]| {
                for (c, name, id) in reads {
                    if *c == t && unset.contains(name.as_str()) {
                        early.insert(*id);
                    }
                }
            };
            mark(&first.reads);
            let mut work = first.runs;
            while let Some(m) = work.pop() {
                if seen_in[m] == turn {
                    continue;
                }
                seen_in[m] = turn;
                let known = summaries[m].get_or_insert_with(|| summary(class, &clinits, m));
                mark(&known.reads);
                work.extend(known.runs.iter().filter(|&&r| seen_in[r] != turn));
            }
            if let Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Static { class: c, name }, .. },
                ..
            }) = s
                && *c == t
            {
                unset.remove(name.as_str());
            }
        }
    }
    early
}

/// The statements of a static initializer in the order they run, with those of blocks and of
/// `try` blocks (a rewritten `<clinit>` runs its own statements in one, see
/// `lower::clinit_errors`) in place of them.
fn flatten<'s>(stmts: &'s [Stmt], out: &mut Vec<&'s Stmt>) {
    for s in stmts {
        match s {
            Stmt::Block(inner) | Stmt::Try { body: inner, .. } => flatten(inner, out),
            _ => out.push(s),
        }
    }
}

/// The summary of method `m`. A constructor runs its class's field initializers too.
fn summary(class: &Class, clinits: &HashMap<usize, usize>, m: usize) -> Summary {
    let method = &class.methods[m];
    let mut out = Summary::default();
    for s in &method.body {
        visit_stmt(s, &mut |e| summarize(class, clinits, e, &mut out), &mut |_| {});
    }
    if method.is_ctor {
        for f in &class.types[method.class].fields {
            if let Some(init) = &f.init {
                summarize(class, clinits, init, &mut out);
            }
        }
    }
    out
}

/// Adds what evaluating `e` reads and runs to `out`.
fn summarize(class: &Class, clinits: &HashMap<usize, usize>, e: &Expr, out: &mut Summary) {
    visit_expr(e, &mut |x| {
        if let ExprKind::Static { class: c, name } = &x.kind {
            out.reads.push((*c, name.clone(), x.id));
        }
        let callees = class.callees(x);
        // The first use of a class runs its static initializer, and its superclasses'.
        let mut used = match &x.kind {
            ExprKind::Static { class: c, .. }
            | ExprKind::Assign { target: Place::Static { class: c, .. }, .. }
            | ExprKind::IncDec { target: Place::Static { class: c, .. }, .. }
            | ExprKind::EnumConstant { class: c, .. }
            | ExprKind::Builtin(Builtin::InitializeClass { class: c }, _) => Some(*c),
            _ => callees
                .iter()
                .find(|&&c| class.methods[c].is_static || class.methods[c].is_ctor)
                .map(|&c| class.methods[c].class),
        };
        while let Some(c) = used {
            out.runs.extend(clinits.get(&c));
            used = class.types[c].extends;
        }
        out.runs.extend(callees);
    })
}
