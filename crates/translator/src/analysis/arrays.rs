//! Arrays that no other name can see. Such an array is a plain `Vec`, not a shared `java::Array`.
//! A local or a parameter qualifies when it is a one-dimensional array declared once in its
//! method, never assigned again, and used only through its own elements and length: `a[i]`,
//! `a[i] = v`, `a.length` and for-each. A local must also be initialised with a fresh array (from
//! `new` or an initializer). Returning it, copying it to another variable or giving it to
//! `java.util.Arrays` makes it shared.
//!
//! A parameter is borrowed, as `&[T]` or `&mut [T]` when its elements are written, when every
//! call of its method passes a plain array, and no array is passed twice in one call. Passing a
//! plain array to a borrowed parameter keeps it plain; passing it to any other parameter makes it
//! shared.
//!
//! A counting loop `for (int i = 0; i < a.length; i++)` over a plain array, whose body neither
//! assigns `i` nor `a`, keeps `i` in bounds, so `a[i]` in its body needs no check. When the body
//! reads `a[i]` and uses `i` and `a` nowhere else, the loop can be `for x in a.iter()`.

use super::Facts;
use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub(super) fn analyze(class: &Class, facts: &mut Facts) {
    let scans: Vec<Scan> = class.methods.iter().map(scan).collect();
    let (plain, borrowed) = solve(&scans);
    let written = written_names(class, &scans, &borrowed);
    for (m, method) in class.methods.iter().enumerate() {
        let names: HashMap<String, bool> =
            plain[m].iter().map(|name| (name.clone(), written[m].contains(name))).collect();
        for stmt in &method.body {
            visit_stmt(stmt, &mut |_| {}, &mut |s| {
                if let Some(counting) = counting_over(s, &names) {
                    in_bounds(counting.array, counting.index, counting.body, facts);
                    if let Some(reads) = element_reads(&counting) {
                        facts.element_loops.insert(counting.cond.id, reads);
                    }
                }
            });
        }
        for (name, written) in names {
            facts.plain_arrays.insert((m, name), written);
        }
        for (i, p) in method.params.iter().enumerate() {
            if borrowed.contains(&(m, i)) {
                facts.borrowed_arrays.insert((m, i), written[m].contains(&p.name));
            }
        }
    }
}

/// What one method does with its arrays, before the analysis decides which are plain.
#[derive(Default)]
struct Scan {
    /// Array locals and parameters that qualify unless a use makes them shared.
    candidates: HashSet<String>,
    /// Array parameters, as (index, name).
    params: Vec<(usize, String)>,
    /// Arrays with an element assigned in this method.
    written: HashSet<String>,
    /// The calls made in this method.
    calls: Vec<Call>,
}

/// A call and the variable each argument names (`None` for an argument that names no variable).
struct Call {
    callee: usize,
    args: Vec<Option<String>>,
}

fn scan(method: &Method) -> Scan {
    let mut uses = Uses::default();
    for stmt in &method.body {
        visit_stmt(stmt, &mut |e| visit_expr(e, &mut |x| uses.record(x)), &mut |_| {});
    }
    let mut declared: HashMap<String, usize> = HashMap::new();
    for p in &method.params {
        *declared.entry(p.name.clone()).or_default() += 1;
    }
    let mut arrays = HashSet::new();
    for stmt in &method.body {
        visit_stmt(stmt, &mut |_| {}, &mut |s| match s {
            Stmt::Local { ty, name, init, .. } => {
                *declared.entry(name.clone()).or_default() += 1;
                let Type::Array(elem) = ty else { return };
                if matches!(**elem, Type::Array(_)) || init.as_ref().is_none_or(|e| !is_fresh(e)) {
                    uses.escaped.insert(name.clone());
                }
                arrays.insert(name.clone());
            }
            Stmt::ForEach { name, array, .. } => {
                *declared.entry(name.clone()).or_default() += 1;
                uses.good.insert(array.id);
            }
            _ => {}
        });
    }
    let mut params = Vec::new();
    for (i, p) in method.params.iter().enumerate() {
        if let Type::Array(elem) = &p.ty && !matches!(**elem, Type::Array(_)) {
            params.push((i, p.name.clone()));
            arrays.insert(p.name.clone());
        }
    }
    let bad: HashSet<String> = uses
        .vars
        .iter()
        .filter(|(id, _)| !uses.good.contains(id))
        .map(|(_, name)| name.clone())
        .chain(uses.escaped.iter().cloned())
        .collect();
    let candidates =
        arrays.into_iter().filter(|name| declared.get(name) == Some(&1) && !bad.contains(name)).collect();
    Scan { candidates, params, written: uses.written, calls: uses.calls }
}

/// What the uses of variables in one method are, collected before any of them is judged.
#[derive(Default)]
struct Uses {
    /// Every mention of a variable, to be judged once all the good uses are known.
    vars: Vec<(ExprId, String)>,
    /// Expressions that are good uses of a variable: the array of an index, length, element
    /// assignment or for-each, and a variable passed as an argument (judged at the call).
    good: HashSet<ExprId>,
    /// Variables whose array is replaced by another, so that they cannot be plain.
    escaped: HashSet<String>,
    written: HashSet<String>,
    calls: Vec<Call>,
}

impl Uses {
    fn record(&mut self, x: &Expr) {
        match &x.kind {
            ExprKind::Var(name) => self.vars.push((x.id, name.clone())),
            ExprKind::Index(array, _) | ExprKind::Length(array) => {
                self.good.insert(array.id);
            }
            ExprKind::Assign { target, .. } => match target {
                Place::Index(array, _) => self.element_write(array),
                Place::Var(name) => {
                    self.escaped.insert(name.clone());
                }
            },
            ExprKind::IncDec { target: Place::Index(array, _), .. } => self.element_write(array),
            ExprKind::Call { args, resolved: Some(callee), .. } => {
                let names = args
                    .iter()
                    .map(|a| match &a.kind {
                        ExprKind::Var(name) => {
                            self.good.insert(a.id);
                            Some(name.clone())
                        }
                        _ => None,
                    })
                    .collect();
                self.calls.push(Call { callee: *callee, args: names });
            }
            _ => {}
        }
    }

    /// An assignment to an element of `array`: a good use that also writes the array.
    fn element_write(&mut self, array: &Expr) {
        self.good.insert(array.id);
        if let ExprKind::Var(name) = &array.kind {
            self.written.insert(name.clone());
        }
    }
}

/// Whether `e` makes a new array, which no other name can hold yet.
fn is_fresh(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::NewArray(_) | ExprKind::ArrayLit(_))
}

/// Decides which arrays are plain and which parameters are borrowed. Both only shrink: a plain
/// array passed to a parameter that is not borrowed escapes, and a parameter is borrowed only
/// while every call of its method passes a distinct plain array. A parameter that is not borrowed
/// is shared. Returns the plain names of each method and the borrowed (method, parameter) pairs.
fn solve(scans: &[Scan]) -> (Vec<HashSet<String>>, HashSet<(usize, usize)>) {
    let mut plain: Vec<HashSet<String>> = scans.iter().map(|s| s.candidates.clone()).collect();
    let called: HashSet<usize> = scans.iter().flat_map(|s| s.calls.iter().map(|c| c.callee)).collect();
    let mut borrowed: HashSet<(usize, usize)> = scans
        .iter()
        .enumerate()
        .filter(|(m, _)| called.contains(m))
        .flat_map(|(m, s)| s.params.iter().map(move |(i, _)| (m, *i)))
        .collect();
    loop {
        let mut changed = false;
        for (m, scan) in scans.iter().enumerate() {
            for call in &scan.calls {
                for (i, arg) in call.args.iter().enumerate() {
                    let distinct = call.args.iter().filter(|a| *a == arg).count() == 1;
                    let plain_arg = matches!(arg, Some(name) if distinct && plain[m].contains(name));
                    if !plain_arg && borrowed.remove(&(call.callee, i)) {
                        changed = true;
                    }
                }
            }
        }
        for (m, scan) in scans.iter().enumerate() {
            for call in &scan.calls {
                for (i, arg) in call.args.iter().enumerate() {
                    if let Some(name) = arg && !borrowed.contains(&(call.callee, i)) && plain[m].remove(name) {
                        changed = true;
                    }
                }
            }
        }
        for (m, scan) in scans.iter().enumerate() {
            for (i, name) in &scan.params {
                let key = (m, *i);
                if !plain[m].contains(name) && borrowed.remove(&key) {
                    changed = true;
                }
                if !borrowed.contains(&key) && plain[m].remove(name) {
                    changed = true;
                }
            }
        }
        if !changed {
            return (plain, borrowed);
        }
    }
}

/// The arrays whose elements are written: the ones written in their method, and any argument
/// passed to a borrowed parameter that is written, which must then be borrowed mutably.
fn written_names(class: &Class, scans: &[Scan], borrowed: &HashSet<(usize, usize)>) -> Vec<HashSet<String>> {
    let mut written: Vec<HashSet<String>> = scans.iter().map(|s| s.written.clone()).collect();
    loop {
        let mut changed = false;
        for (m, scan) in scans.iter().enumerate() {
            for call in &scan.calls {
                for (i, arg) in call.args.iter().enumerate() {
                    let Some(name) = arg else { continue };
                    let param = &class.methods[call.callee].params[i].name;
                    let param_written = written[call.callee].contains(param);
                    if param_written && borrowed.contains(&(call.callee, i)) {
                        changed |= written[m].insert(name.clone());
                    }
                }
            }
        }
        if !changed {
            return written;
        }
    }
}

/// A counting loop `for (int i = 0; i < a.length; i++)` over a plain array `a`, which does not
/// assign `i` or `a` in its body.
struct Counting<'s> {
    cond: &'s Expr,
    array: &'s str,
    index: &'s str,
    body: &'s Stmt,
}

/// The counting loop `s`, if `s` is one over an array in `plain`.
fn counting_over<'s>(s: &'s Stmt, plain: &HashMap<String, bool>) -> Option<Counting<'s>> {
    let Stmt::For { init, cond: Some(cond), update, body } = s else { return None };
    let [Stmt::Local { name: index, init: Some(start), .. }] = init.as_slice() else { return None };
    let ExprKind::Binary(BinOp::Lt, var, end) = &cond.kind else { return None };
    let ExprKind::Length(array) = &end.kind else { return None };
    let ExprKind::Var(array) = &array.kind else { return None };
    let counts_up = matches!(update.as_slice(), [Expr { kind: ExprKind::IncDec { target: Place::Var(t), increment: true, .. }, .. }] if t == index);
    let starts_at_zero = matches!(start.kind, ExprKind::Int(0));
    let index_is_var = matches!(&var.kind, ExprKind::Var(v) if v == index);
    if !(counts_up && starts_at_zero && index_is_var && plain.contains_key(array)) {
        return None;
    }
    if assigns_stmt(body, index) || assigns_stmt(body, array) {
        return None;
    }
    Some(Counting { cond, array, index, body })
}

/// Marks every `a[i]` in `body` as in bounds, where `a` is the array and `i` the loop index: a read
/// or a plain element assignment.
fn in_bounds(array: &str, index: &str, body: &Stmt, facts: &mut Facts) {
    visit_stmt(
        body,
        &mut |e| {
            visit_expr(e, &mut |x| {
                let is = |e: &Expr, name: &str| matches!(&e.kind, ExprKind::Var(v) if v == name);
                match &x.kind {
                    ExprKind::Index(a, i) | ExprKind::Assign { op: None, target: Place::Index(a, i), .. }
                        if is(a, array) && is(i, index) =>
                    {
                        facts.in_bounds.insert(x.id);
                    }
                    _ => {}
                }
            })
        },
        &mut |_| {},
    );
}

/// The reads `a[i]` in the body of a counting loop, when the body uses `a` only to read its
/// elements and `i` only as their index, so that `for x in a.iter()` gives the same values.
fn element_reads(c: &Counting) -> Option<Vec<ExprId>> {
    let is = |e: &Expr, name: &str| matches!(&e.kind, ExprKind::Var(v) if v == name);
    let (mut array_mentions, mut array_uses, mut index_mentions) = (0, 0, 0);
    let mut reads = Vec::new();
    visit_stmt(
        c.body,
        &mut |e| {
            visit_expr(e, &mut |x| match &x.kind {
                ExprKind::Var(v) => {
                    array_mentions += usize::from(v == c.array);
                    index_mentions += usize::from(v == c.index);
                }
                ExprKind::Index(a, i) if is(a, c.array) => {
                    array_uses += 1;
                    if is(i, c.index) {
                        reads.push(x.id);
                    }
                }
                ExprKind::Length(a) if is(a, c.array) => array_uses += 1,
                _ => {}
            })
        },
        &mut |_| {},
    );
    let only_reads = array_mentions == array_uses && index_mentions == reads.len();
    (only_reads && !reads.is_empty()).then_some(reads)
}
