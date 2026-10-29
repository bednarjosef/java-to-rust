//! Counting loops whose accesses are checked once, before the loop. In
//! `for (int j = 0; j < n; j++) c[i][j] += a * b[k][j];` the arrays `c[i]` and `b[k]` stay the
//! same arrays for the whole loop and are reached only at `j`, so when `0 <= 0`, `k` is a row of
//! `b`, and `n` is at most the length of `c[i]` and of `b[k]`, every access is in bounds. The
//! loop then runs over slices taken before it (`let b_row = &b[k][..n];`), whose accesses need no
//! check, and Rust can vectorize it. When the condition fails the loop runs as it is, with every
//! check where Java makes it, so it throws at the same iteration.
//!
//! Only innermost loops are taken, so the loop is written twice at most. An array qualifies when
//! it is plain, its every mention in the body is an element access `P[j]` of one path `P` whose
//! row indexes are constants or variables the body does not assign, and the loop's bounds do not
//! mention it. A loop is taken when one of its accesses is not already known to be in bounds.

use super::super::Facts;
use super::super::{FastMap, FastSet};
use crate::ast::*;

/// An index of a row on the way from an array variable to the slice: a constant or a variable.
#[derive(Clone, Debug, PartialEq)]
pub enum RowIndex {
    Int(i32),
    Var(String),
}

/// A plain array that a counting loop reaches through a slice taken before the loop.
#[derive(Clone, Debug)]
pub struct Slice {
    /// The array variable.
    pub root: String,
    /// The rows from it to the array the loop indexes: empty for the variable itself.
    pub rows: Vec<RowIndex>,
    /// Whether the loop writes elements through the slice.
    pub written: bool,
}

/// The arrays a sliced loop reaches through slices, and its accesses, which are in bounds where
/// the slices are taken.
#[derive(Clone, Debug, Default)]
pub struct SlicedLoop {
    pub slices: Vec<Slice>,
    pub accesses: Vec<ExprId>,
}

/// The sliced loops of `method`, keyed by the id of the loop's condition.
pub(super) fn sliced_loops(
    method: &Method,
    plain: &FastSet<String>,
    facts: &Facts,
) -> Vec<(ExprId, SlicedLoop)> {
    let mut out = Vec::new();
    for stmt in &method.body {
        visit_stmt(stmt, &mut |_| {}, &mut |s| {
            if let Some(found) = sliced(s, plain, facts) {
                out.push(found);
            }
        });
    }
    out
}

/// One element access of an array variable at the loop variable.
struct Access {
    root: String,
    rows: Vec<RowIndex>,
    id: ExprId,
    write: bool,
}

fn sliced(s: &Stmt, plain: &FastSet<String>, facts: &Facts) -> Option<(ExprId, SlicedLoop)> {
    let Stmt::For { init, cond: Some(cond), update, body } = s else { return None };
    let (var, from, to) = counting_loop(init, Some(cond), update, body)?;
    if from.ty != Type::Int || has_effects(from) || has_effects(to) || has_loop(body) {
        return None;
    }
    let mut mentions_of: FastMap<String, usize> = FastMap::default();
    let mut accesses = Vec::new();
    visit_stmt(
        body,
        &mut |e| {
            visit_expr(e, &mut |x| {
                if let ExprKind::Var(v) | ExprKind::Assign { target: Place::Var(v), .. } = &x.kind {
                    *mentions_of.entry(v.clone()).or_default() += 1;
                }
                if let Some(access) = access(x, var, body) {
                    accesses.push(access);
                }
            })
        },
        &mut |_| {},
    );
    let mut slices: Vec<Slice> = Vec::new();
    let mut ids = Vec::new();
    let mut unproven = false;
    for root in plain {
        let of_root: Vec<&Access> = accesses.iter().filter(|a| &a.root == root).collect();
        let Some(first) = of_root.first() else { continue };
        let only_accesses = mentions_of.get(root) == Some(&of_root.len());
        let one_path = of_root.iter().all(|a| a.rows == first.rows);
        if !only_accesses || !one_path || mentions(from, root) || mentions(to, root) {
            continue;
        }
        unproven |= of_root.iter().any(|a| !facts.in_bounds.contains(&a.id));
        ids.extend(of_root.iter().map(|a| a.id));
        slices.push(Slice {
            root: root.clone(),
            rows: first.rows.clone(),
            written: of_root.iter().any(|a| a.write),
        });
    }
    if !unproven {
        return None;
    }
    slices.sort_by(|a, b| a.root.cmp(&b.root));
    Some((cond.id, SlicedLoop { slices, accesses: ids }))
}

/// `x` as an element access `P[var]` (a read, an assignment or a `++`), where `P` is an array
/// variable or a row of one reached through constants and variables that `body` does not assign.
fn access(x: &Expr, var: &str, body: &Stmt) -> Option<Access> {
    let (array, index, write) = match &x.kind {
        ExprKind::Index(array, index) => (array, index, false),
        ExprKind::Assign { target: Place::Index(array, index), .. }
        | ExprKind::IncDec { target: Place::Index(array, index), .. } => (array, index, true),
        _ => return None,
    };
    if !matches!(&index.kind, ExprKind::Var(v) if v == var) || matches!(array.ty.element(), Type::Array(_)) {
        return None;
    }
    let mut rows = Vec::new();
    let mut path = &**array;
    loop {
        match &path.kind {
            ExprKind::Var(root) => {
                rows.reverse();
                return Some(Access { root: root.clone(), rows, id: x.id, write });
            }
            ExprKind::Index(inner, row) => {
                rows.push(match &row.kind {
                    ExprKind::Int(k) => RowIndex::Int(*k),
                    ExprKind::Var(v) if v != var && row.ty == Type::Int && !assigns_stmt(body, v) => {
                        RowIndex::Var(v.clone())
                    }
                    _ => return None,
                });
                path = inner;
            }
            _ => return None,
        }
    }
}

/// Whether `s` contains a loop.
fn has_loop(s: &Stmt) -> bool {
    let mut found = false;
    visit_stmt(s, &mut |_| {}, &mut |s| {
        found |=
            matches!(s, Stmt::For { .. } | Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::ForEach { .. })
    });
    found
}
