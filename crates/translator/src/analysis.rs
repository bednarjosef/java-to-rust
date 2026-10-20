//! Analyses that prove facts about the program, so that the code generator can write the Rust a
//! person would write wherever that behaves exactly like the Java:
//!
//! - The value ranges of `int` and `long` variables show which arithmetic cannot overflow and
//!   which divisions can neither divide by zero nor overflow; those use Rust's plain operators
//!   instead of `wrapping_add` and `java::div` (see `ranges`).
//! - Strings that are always ASCII have a `len()` equal to Java's UTF-16 `length()`.
//! - String parameters that a method never assigns are borrowed as `&str`.
//! - Array locals that no other name can see are plain `Vec`s (see `arrays`).
//!
//! The facts form a table keyed by expression id. An expression without a fact gets the faithful
//! default translation, so a weaker analysis only makes the output plainer, never wrong.

mod arrays;
mod ranges;

use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Facts {
    /// Integer `+`, `-`, `*`, negation, `++`, `--`, compound assignments and `Math.abs` that
    /// never overflow.
    pub no_overflow: HashSet<ExprId>,
    /// Integer `/`, `%` and their compound assignments whose divisor is never zero and that never
    /// compute `MIN / -1`.
    pub safe_division: HashSet<ExprId>,
    /// `String.length()` calls on strings that are always ASCII.
    pub ascii_length: HashSet<ExprId>,
    /// String parameters, as (method, parameter) indexes, that are never assigned.
    pub borrowed_params: HashSet<(usize, usize)>,
    /// Array locals, as (method, name), that no other name can see: plain `Vec`s. The flag says
    /// whether an element of the array is ever assigned.
    pub plain_arrays: HashMap<(usize, String), bool>,
    /// `a[i]` expressions that a counting loop over the plain array `a` proves in bounds.
    pub in_bounds: HashSet<ExprId>,
    /// Array parameters, as (method, parameter) indexes, that are plain at every call: `&[T]`, or
    /// `&mut [T]` when the flag says the elements are written.
    pub borrowed_arrays: HashMap<(usize, usize), bool>,
    /// Counting loops over a plain array whose body only reads `a[i]`, keyed by the loop
    /// condition: the reads that can become elements of `for x in a.iter()`.
    pub element_loops: HashMap<ExprId, Vec<ExprId>>,
}

pub fn analyze(class: &Class) -> Facts {
    let mut facts = Facts::default();
    ranges::analyze(class, &mut facts);
    borrowed_params(class, &mut facts);
    ascii_strings(class, &mut facts);
    arrays::analyze(class, &mut facts);
    facts
}

fn borrowed_params(class: &Class, facts: &mut Facts) {
    for (m, method) in class.methods.iter().enumerate() {
        for (i, p) in method.params.iter().enumerate() {
            if p.ty == Type::String && !assigns_any(&method.body, &p.name) {
                facts.borrowed_params.insert((m, i));
            }
        }
    }
}

/// Which string variables, parameters and return values can hold a non-ASCII string. Starts from
/// "all ASCII" and marks what a non-ASCII value can reach, until nothing changes.
#[derive(Default)]
struct NonAscii {
    vars: HashSet<(usize, String)>,
    params: HashSet<(usize, usize)>,
    returns: HashSet<usize>,
}

impl NonAscii {
    fn var(&self, class: &Class, m: usize, name: &str) -> bool {
        let param = class.methods[m].params.iter().position(|p| p.name == name);
        self.vars.contains(&(m, name.to_string())) || param.is_some_and(|i| self.params.contains(&(m, i)))
    }

    /// Whether `e`, evaluated in method `m`, is ASCII text (or not a string at all).
    fn ascii(&self, class: &Class, m: usize, e: &Expr) -> bool {
        if e.ty != Type::String {
            return true;
        }
        match &e.kind {
            ExprKind::Str(s) => s.is_ascii(),
            ExprKind::Var(name) => !self.var(class, m, name),
            ExprKind::Binary(_, l, r) => self.ascii(class, m, l) && self.ascii(class, m, r),
            ExprKind::Assign { op: None, value, .. } => self.ascii(class, m, value),
            ExprKind::Assign { target: Place::Var(target), value, .. } => {
                !self.var(class, m, target) && self.ascii(class, m, value)
            }
            ExprKind::Call { resolved: Some(callee), .. } => !self.returns.contains(callee),
            ExprKind::Conditional(_, a, b) => self.ascii(class, m, a) && self.ascii(class, m, b),
            _ => false,
        }
    }
}

fn ascii_strings(class: &Class, facts: &mut Facts) {
    let mut non_ascii = NonAscii::default();
    loop {
        let mut vars = Vec::new();
        let mut declared = Vec::new();
        let mut params = Vec::new();
        let mut returns = Vec::new();
        for (m, method) in class.methods.iter().enumerate() {
            for stmt in &method.body {
                let mut on_expr = |e: &Expr| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Assign { target: Place::Var(target), .. } if x.ty == Type::String => {
                            if !non_ascii.ascii(class, m, x) {
                                vars.push((m, target.clone()));
                            }
                        }
                        ExprKind::Call { args, resolved: Some(callee), .. } => {
                            for (i, arg) in args.iter().enumerate() {
                                if !non_ascii.ascii(class, m, arg) {
                                    params.push((*callee, i));
                                }
                            }
                        }
                        _ => {}
                    })
                };
                let mut on_stmt = |s: &Stmt| match s {
                    Stmt::Local { ty: Type::String, name, init: Some(init), .. }
                        if !non_ascii.ascii(class, m, init) =>
                    {
                        declared.push((m, name.clone()))
                    }
                    Stmt::Return(Some(value), _) if !non_ascii.ascii(class, m, value) => returns.push(m),
                    _ => {}
                };
                visit_stmt(stmt, &mut on_expr, &mut on_stmt);
            }
        }
        let before = (non_ascii.vars.len(), non_ascii.params.len(), non_ascii.returns.len());
        non_ascii.vars.extend(vars);
        non_ascii.vars.extend(declared);
        non_ascii.params.extend(params);
        non_ascii.returns.extend(returns);
        if before == (non_ascii.vars.len(), non_ascii.params.len(), non_ascii.returns.len()) {
            break;
        }
    }
    for (m, method) in class.methods.iter().enumerate() {
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Builtin(Builtin::StringLength, args) = &x.kind
                            && non_ascii.ascii(class, m, &args[0])
                        {
                            facts.ascii_length.insert(x.id);
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
}
