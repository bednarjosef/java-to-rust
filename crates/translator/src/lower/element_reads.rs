//! Repeated reads of one array element. `bodies[j].x - ...; bodies[j].vx += ...` reads the
//! element `bodies[j]` again for every use, which in Rust clones its handle out of the shared
//! array each time (a borrow of the array, a count increment and decrement), where Java only
//! loads it. Within a run of statements that cannot change the array, the index or the element,
//! the element is read once into a new local at its first read, which becomes
//! `(element = bodies[j])`, so the order of evaluation and of any exception stays Java's; the
//! later reads of the run read the local.
//!
//! A run is a sequence of statements of one block (declarations with initializers and
//! expression statements) without a call, an object or array creation, a pattern, a switch or a
//! library method other than `Math`'s `abs`, `min`, `max`, `sqrt` and `pow`: nothing in it runs
//! code that could change the array through another reference. An element is shared when the
//! array is a local, a field of `this` or a static field, the index is a local or a constant,
//! nothing in the run assigns the array (a field of that name of any object, for a field), the
//! index or an element of an array of references, and its first read is not inside a
//! conditional part (`&&`, `||`, `?:`). Only elements that are objects or strings are shared: an
//! element that is itself an array stays a read, as the array analyses decide how each array is
//! held from the names that reach it.

use super::*;
use std::collections::{HashMap, HashSet};

/// Where the array of a shared read is.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Array {
    Local(String),
    /// A field of `this`, by name.
    Field(String),
    Static(usize, String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Index {
    Local(String),
    Constant(i32),
}

/// The element that `e` reads, if it is one this pass can share.
fn key_of(e: &Expr) -> Option<(Array, Index)> {
    let ExprKind::Index(array, index) = &e.kind else { return None };
    if !matches!(e.ty, Type::Class(..) | Type::String) {
        return None;
    }
    let array = match &array.kind {
        ExprKind::Var(name) => Array::Local(name.clone()),
        ExprKind::Static { class, name } => Array::Static(*class, name.clone()),
        ExprKind::Field { object, name } if matches!(object.kind, ExprKind::This) => {
            Array::Field(name.clone())
        }
        _ => return None,
    };
    let index = match &index.kind {
        ExprKind::Var(name) => Index::Local(name.clone()),
        ExprKind::Int(n) => Index::Constant(*n),
        _ => return None,
    };
    Some((array, index))
}

/// What a run of statements does, in Java's order of evaluation.
#[derive(Default)]
struct Walk {
    /// Every element read that may be shared: its key, its id, and whether it is evaluated only
    /// under a condition.
    reads: Vec<((Array, Index), ExprId, bool)>,
    /// The locals, fields (by name, of any object) and statics that the run assigns.
    assigned: HashSet<Array>,
    /// Whether the run assigns an element of an array of references.
    writes_references: bool,
}

impl Walk {
    /// Walks `e` in evaluation order; false when `e` has something that ends a run.
    fn expr(&mut self, e: &Expr, conditional: bool) -> bool {
        match &e.kind {
            ExprKind::Int(_)
            | ExprKind::Long(_)
            | ExprKind::Double(_)
            | ExprKind::Char(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Null
            | ExprKind::Var(_)
            | ExprKind::This
            | ExprKind::Static { .. } => true,
            ExprKind::Unary(_, x)
            | ExprKind::Cast(_, x)
            | ExprKind::Length(x)
            | ExprKind::Field { object: x, .. }
            | ExprKind::InstanceOf { value: x, binding: None, .. } => self.expr(x, conditional),
            ExprKind::Binary(op, l, r) => {
                let short = matches!(op, BinOp::And | BinOp::Or);
                self.expr(l, conditional) && self.expr(r, conditional || short)
            }
            ExprKind::Conditional(c, a, b) => {
                self.expr(c, conditional) && self.expr(a, true) && self.expr(b, true)
            }
            ExprKind::Index(array, index) => {
                if !(self.expr(array, conditional) && self.expr(index, conditional)) {
                    return false;
                }
                if let Some(key) = key_of(e) {
                    self.reads.push((key, e.id, conditional));
                }
                true
            }
            ExprKind::Assign { target, value, .. } => {
                self.place(target, conditional) && self.expr(value, conditional)
            }
            ExprKind::IncDec { target, .. } => self.place(target, conditional),
            ExprKind::Builtin(
                Builtin::MathAbs | Builtin::MathMax | Builtin::MathMin | Builtin::MathSqrt | Builtin::MathPow,
                args,
            ) => args.iter().all(|a| self.expr(a, conditional)),
            _ => false,
        }
    }

    /// Walks the parts of an assigned place, and records what it assigns.
    fn place(&mut self, target: &Place, conditional: bool) -> bool {
        match target {
            Place::Var(name) => {
                self.assigned.insert(Array::Local(name.clone()));
                true
            }
            Place::Static { class, name } => {
                self.assigned.insert(Array::Static(*class, name.clone()));
                true
            }
            Place::Field { object, name } => {
                self.assigned.insert(Array::Field(name.clone()));
                self.expr(object, conditional)
            }
            Place::Index(array, index) => {
                let element = match array.ty {
                    Type::Array(_) => array.ty.element(),
                    _ => Type::Null,
                };
                self.writes_references |= element.is_reference() || element == Type::Null;
                self.expr(array, conditional) && self.expr(index, conditional)
            }
        }
    }

    /// Whether an element's array and index stay the same through the run.
    fn steady(&self, (array, index): &(Array, Index)) -> bool {
        let index_assigned =
            matches!(index, Index::Local(v) if self.assigned.contains(&Array::Local(v.clone())));
        !self.writes_references && !self.assigned.contains(array) && !index_assigned
    }
}

/// The expression of a statement that can be in a run, or `None` for one that ends a run.
fn run_expr(stmt: &Stmt) -> Option<&Expr> {
    match stmt {
        Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) => Some(e),
        _ => None,
    }
}

fn in_run(stmt: &Stmt) -> bool {
    run_expr(stmt).is_some_and(|e| Walk::default().expr(e, false))
}

impl<'t> Lowering<'t> {
    /// Shares the repeated element reads of the runs of every block of `method`.
    pub(super) fn share_element_reads(&self, method: &mut Method) {
        let mut taken: HashSet<String> = method.params.iter().map(|p| p.name.clone()).collect();
        let mut declared = HashSet::new();
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Var(v) | ExprKind::InstanceOf { binding: Some(v), .. } => {
                            taken.insert(v.clone());
                        }
                        _ => {}
                    })
                },
                &mut |s| match s {
                    Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => {
                        declared.insert(name.clone());
                    }
                    Stmt::Try { catches, .. } => declared.extend(catches.iter().map(|c| c.name.clone())),
                    _ => {}
                },
            );
        }
        taken.extend(declared);
        self.block_reads(&mut method.body, &mut taken);
    }

    fn block_reads(&self, stmts: &mut Vec<Stmt>, taken: &mut HashSet<String>) {
        for stmt in stmts.iter_mut() {
            self.nested_reads(stmt, taken);
        }
        let mut i = 0;
        while i < stmts.len() {
            if !in_run(&stmts[i]) {
                i += 1;
                continue;
            }
            let end = (i..stmts.len()).find(|&k| !in_run(&stmts[k])).unwrap_or(stmts.len());
            let added = self.run_reads(stmts, i, end, taken);
            i = end + added;
        }
    }

    fn nested_reads(&self, stmt: &mut Stmt, taken: &mut HashSet<String>) {
        match stmt {
            Stmt::Block(stmts) => self.block_reads(stmts, taken),
            Stmt::If { then, otherwise, .. } => {
                self.nested_reads(then, taken);
                if let Some(o) = otherwise {
                    self.nested_reads(o, taken);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::Labeled { body, .. } => self.nested_reads(body, taken),
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    self.block_reads(&mut case.body, taken);
                }
            }
            Stmt::Try { body, catches, finally } => {
                self.block_reads(body, taken);
                for c in catches {
                    self.block_reads(&mut c.body, taken);
                }
                if let Some(f) = finally {
                    self.block_reads(f, taken);
                }
            }
            _ => {}
        }
    }

    /// Shares the elements read more than once in `stmts[start..end]`, a run; returns how many
    /// declarations it inserted.
    fn run_reads(
        &self,
        stmts: &mut Vec<Stmt>,
        start: usize,
        end: usize,
        taken: &mut HashSet<String>,
    ) -> usize {
        let mut walk = Walk::default();
        // The statement of the run that each read is in.
        let mut in_stmt = HashMap::new();
        for (i, stmt) in stmts[start..end].iter().enumerate() {
            let before = walk.reads.len();
            walk.expr(run_expr(stmt).expect("a statement of a run"), false);
            for (_, id, _) in &walk.reads[before..] {
                in_stmt.insert(*id, i);
            }
        }
        // Per element: its first read, which is unconditional, and the reads after it.
        let mut shared: Vec<((Array, Index), ExprId, Vec<ExprId>)> = Vec::new();
        for (key, id, conditional) in &walk.reads {
            if !walk.steady(key) {
                continue;
            }
            match shared.iter_mut().find(|(k, ..)| k == key) {
                Some((_, _, later)) => later.push(*id),
                None if !conditional => shared.push((key.clone(), *id, Vec::new())),
                None => {}
            }
        }
        shared.retain(|(_, _, later)| !later.is_empty());
        let mut declarations = Vec::new();
        for ((array, index), first, later) in shared {
            let array = match array {
                Array::Local(n) | Array::Field(n) | Array::Static(_, n) => n,
            };
            let index = match index {
                Index::Local(v) => v,
                Index::Constant(n) => n.to_string(),
            };
            let mut name = format!("{array}_{index}");
            while taken.contains(&name) {
                name.push('_');
            }
            taken.insert(name.clone());
            let mut found = None;
            for stmt in &mut stmts[start..end] {
                if let Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) = stmt {
                    self.share_in(e, first, &later, &name, &mut found);
                }
            }
            let (ty, pos) = found.expect("the first read is in the run");
            declarations.push((in_stmt[&first], Stmt::Local { ty, name, init: None, pos }));
        }
        // Each declaration goes right before the statement of its first read.
        declarations.sort_by_key(|(i, _)| std::cmp::Reverse(*i));
        let added = declarations.len();
        for (i, declaration) in declarations {
            stmts.insert(start + i, declaration);
        }
        added
    }

    /// Rewrites, in `e`, the read `first` into `(name = element)` and the reads `later` into
    /// `name`; `found` is set to the element's type and position.
    fn share_in(
        &self,
        e: &mut Expr,
        first: ExprId,
        later: &[ExprId],
        name: &str,
        found: &mut Option<(Type, Pos)>,
    ) {
        rewrite_expr(e, &mut |x| {
            if x.id == first {
                *found = Some((x.ty, x.pos));
                let read = std::mem::replace(&mut x.kind, ExprKind::Null);
                let element = self.synth(read, x.pos, x.ty);
                x.kind = ExprKind::Assign {
                    op: None,
                    target: Place::Var(name.to_string()),
                    value: Box::new(element),
                };
            } else if later.contains(&x.id) {
                x.kind = ExprKind::Var(name.to_string());
            }
        });
    }
}
