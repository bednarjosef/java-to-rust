//! The classes that a dominating test already proved a local holds. After `x instanceof T`
//! passes, or after `(T) x` has run, `x` holds a `T` (or null, after the cast) until it is
//! assigned again; a later cast of `x` to a type above `T`, a test of it, or a call on it need
//! not test its class again (`codegen::dispatch`).
//!
//! The facts are about the reads of locals and parameters: each read of `x` that such a test
//! dominates, and where `x` is not assigned between the test and the read, gets the type that
//! the test proved (`Facts::proven_types`), as does a cast of such a read (the cast's value is
//! the same object). The regions are found on the code's structure:
//! - the operand after `&&` sees what its left operand proves true, the operand after `||` what
//!   its left operand proves false, and the branches of `?:` and `if` what the condition proves
//!   true or false;
//! - the body of a `while` or a `for` sees what its condition proves true;
//! - the statements after an `if` one of whose branches always leaves the block (`return`,
//!   `throw`, `break`, `continue`, `yield`) see what the condition proves on the other branch:
//!   `if (!(x instanceof T)) return;`;
//! - the statements after one that always runs a cast `(T) x` see `x` as a `T`.
//!
//! What a test proves of `x` is forgotten at the first statement that assigns `x` (or declares
//! a variable of its name); a loop, a `switch`, a `try` and a labeled statement forget what
//! comes before them of every variable they assign anywhere, as their code may run again or be
//! entered past the assignment, and nothing proved inside one of them is kept after it. The
//! bodies of lambdas and local classes are methods of their own, which start with nothing
//! proved: a variable they capture is effectively final anyway.

use super::{FastMap, FastSet};
use crate::ast::*;

/// The proven type of each read of a local, by the read's expression id: an index into
/// `Class::types`, narrower than the read's own type.
pub fn proven_types(class: &Class) -> FastMap<ExprId, usize> {
    let mut walk = Walk { class, found: FastMap::default() };
    for method in &class.methods {
        walk.block(&method.body, Known::default());
    }
    walk.found
}

/// What is proven at a point: the type each variable holds.
type Known = FastMap<String, usize>;

struct Walk<'c> {
    class: &'c Class,
    found: FastMap<ExprId, usize>,
}

/// Whether `stmt` never completes normally: it leaves the block it is in.
fn leaves(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(..)
        | Stmt::Throw(_)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_)
        | Stmt::Yield(_) => true,
        Stmt::Block(stmts) => stmts.iter().any(leaves),
        Stmt::If { then, otherwise: Some(otherwise), .. } => leaves(then) && leaves(otherwise),
        _ => false,
    }
}

/// The variables that `stmt` assigns or declares anywhere inside it.
fn assigned_in_stmt(stmt: &Stmt, out: &mut FastSet<String>) {
    let mut declared = Vec::new();
    visit_stmt(stmt, &mut |e| assigned_in_expr(e, out), &mut |s| match s {
        Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => declared.push(name),
        Stmt::Try { catches, .. } => declared.extend(catches.iter().map(|c| &c.name)),
        _ => {}
    });
    out.extend(declared.into_iter().cloned());
}

/// The variables that `e` assigns, or binds with a pattern, anywhere inside it.
fn assigned_in_expr(e: &Expr, out: &mut FastSet<String>) {
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Assign { target: Place::Var(v), .. } | ExprKind::IncDec { target: Place::Var(v), .. } => {
            out.insert(v.clone());
        }
        ExprKind::InstanceOf { binding: Some(v), .. } => {
            out.insert(v.clone());
        }
        _ => {}
    });
}

/// `known` without the variables that `stmt` assigns.
fn forget_stmt(mut known: Known, stmt: &Stmt) -> Known {
    if !known.is_empty() {
        let mut assigned = FastSet::default();
        assigned_in_stmt(stmt, &mut assigned);
        known.retain(|v, _| !assigned.contains(v));
    }
    known
}

/// `known` without the variables that `e` assigns.
fn forget_expr(mut known: Known, e: &Expr) -> Known {
    if !known.is_empty() {
        let mut assigned = FastSet::default();
        assigned_in_expr(e, &mut assigned);
        known.retain(|v, _| !assigned.contains(v));
    }
    known
}

impl Walk<'_> {
    /// The type of the program that `ty` names, for a class type.
    fn type_of(&self, ty: Type) -> Option<usize> {
        self.class.class_index(ty)
    }

    /// Adds to `known` that `name` holds a `t`, keeping a narrower type it already has.
    fn learn(&self, known: &mut Known, name: &str, t: usize) {
        let h = &self.class.hierarchy;
        match known.get(name) {
            Some(&k) if h.is_subtype(k, t) => {}
            _ => {
                known.insert(name.to_string(), t);
            }
        }
    }

    /// `e` without the conversions to a supertype that lowering adds, which keep the object.
    fn peel<'e>(&self, e: &'e Expr) -> &'e Expr {
        match &e.kind {
            ExprKind::Cast(to, inner) => match (self.type_of(*to), self.type_of(inner.ty)) {
                (Some(t), Some(s)) if self.class.hierarchy.is_subtype(s, t) => self.peel(inner),
                _ => e,
            },
            _ => e,
        }
    }

    /// The variable `e` reads, through conversions to a supertype, with its type.
    fn var<'e>(&self, e: &'e Expr) -> Option<(&'e str, usize)> {
        let e = self.peel(e);
        match &e.kind {
            ExprKind::Var(name) => Some((name, self.type_of(e.ty)?)),
            _ => None,
        }
    }

    /// What `cond` proves when it is `when`: the variables it tests with `instanceof`.
    fn proves(&self, cond: &Expr, when: bool) -> Known {
        let mut known = Known::default();
        match &cond.kind {
            ExprKind::InstanceOf { value, ty, .. } if when => {
                if let (Some((name, _)), Some(t)) = (self.var(value), self.type_of(*ty)) {
                    self.learn(&mut known, name, t);
                }
            }
            ExprKind::Unary(UnOp::Not, inner) => return self.proves(inner, !when),
            // `a && b` true, or `a || b` false: both operands were, and `b` ran after `a`.
            ExprKind::Binary(op @ (BinOp::And | BinOp::Or), a, b) if when == (*op == BinOp::And) => {
                known = forget_expr(self.proves(a, when), b);
                for (name, t) in self.proves(b, when) {
                    self.learn(&mut known, &name, t);
                }
            }
            _ => {}
        }
        known
    }

    /// `known` with what `cond` proves when it is `when`.
    fn with(&self, mut known: Known, cond: &Expr, when: bool) -> Known {
        for (name, t) in self.proves(cond, when) {
            self.learn(&mut known, &name, t);
        }
        known
    }

    /// Records what is known at each read of a variable in `e`, which assigns none of the
    /// variables of `known`.
    fn expr(&mut self, e: &Expr, known: &Known) {
        if !known.is_empty() || has_test(e) {
            self.within(e, known);
        }
    }

    /// `expr`, past the check that something may be proven.
    fn within(&mut self, e: &Expr, known: &Known) {
        match &e.kind {
            ExprKind::Var(name) => {
                if let Some(&t) = known.get(name)
                    && self.type_of(e.ty).is_some_and(|s| !self.class.hierarchy.is_subtype(s, t))
                {
                    self.found.insert(e.id, t);
                }
            }
            ExprKind::Binary(op @ (BinOp::And | BinOp::Or), a, b) => {
                self.within(a, known);
                let then = forget_expr(self.with(known.clone(), a, *op == BinOp::And), b);
                self.within(b, &then);
            }
            ExprKind::Conditional(c, a, b) => {
                self.within(c, known);
                let then = forget_expr(self.with(known.clone(), c, true), a);
                self.within(a, &then);
                let otherwise = forget_expr(self.with(known.clone(), c, false), b);
                self.within(b, &otherwise);
            }
            ExprKind::Cast(to, inner) => {
                self.within(inner, known);
                // The cast's value is the object that the variable holds.
                if let Some(&t) = self.found.get(&self.peel(inner).id)
                    && self.type_of(*to).is_some_and(|s| !self.class.hierarchy.is_subtype(s, t))
                {
                    self.found.insert(e.id, t);
                }
            }
            ExprKind::Switch { selector, cases } => {
                self.within(selector, known);
                self.cases(cases, known.clone());
            }
            _ => {
                let mut parts: Vec<&Expr> = Vec::new();
                children(e, &mut parts);
                for part in parts {
                    self.within(part, known);
                }
            }
        }
    }

    /// The cases of a `switch`, where one falls through into the next: each starts with what is
    /// known before the `switch` of the variables that no case assigns.
    fn cases(&mut self, cases: &[Case], known: Known) {
        let mut known = known;
        if !known.is_empty() {
            let mut assigned = FastSet::default();
            for case in cases {
                case.body.iter().for_each(|s| assigned_in_stmt(s, &mut assigned));
            }
            known.retain(|v, _| !assigned.contains(v));
        }
        for case in cases {
            self.block(&case.body, known.clone());
        }
    }

    /// The statements of a block, in order, each seeing what the ones before it proved.
    fn block(&mut self, stmts: &[Stmt], mut known: Known) {
        for stmt in stmts {
            known = self.stmt(stmt, known);
        }
    }

    /// Records the facts of the reads in `stmt`, which runs with `known`, and returns what is
    /// known after it.
    fn stmt(&mut self, stmt: &Stmt, known: Known) -> Known {
        match stmt {
            Stmt::Local { init, .. } => {
                let known = forget_stmt(known, stmt);
                match init {
                    Some(init) => self.simple(init, known),
                    None => known,
                }
            }
            Stmt::Expr(e) => {
                let known = forget_stmt(known, stmt);
                self.simple(e, known)
            }
            Stmt::Return(Some(e), _) | Stmt::Throw(e) | Stmt::Yield(e) => {
                self.expr(e, &forget_expr(known.clone(), e));
                known
            }
            Stmt::If { cond, then, otherwise } => {
                let before = forget_expr(known.clone(), cond);
                self.expr(cond, &before);
                let on_then = self.with(before.clone(), cond, true);
                self.stmt(then, on_then);
                let on_otherwise = self.with(before.clone(), cond, false);
                if let Some(otherwise) = otherwise {
                    self.stmt(otherwise, on_otherwise);
                }
                let mut after = forget_stmt(known, stmt);
                // The code after the `if` runs only after the branch that does not leave.
                let kept = match otherwise {
                    _ if leaves(then) => Some((otherwise.as_deref(), false)),
                    Some(o) if leaves(o) => Some((Some(&**then), true)),
                    _ => None,
                };
                if let Some((branch, when)) = kept {
                    let mut proved = self.proves(cond, when);
                    if let Some(branch) = branch {
                        proved = forget_stmt(proved, branch);
                    }
                    for (name, t) in proved {
                        self.learn(&mut after, &name, t);
                    }
                }
                after
            }
            Stmt::While { cond, body } => {
                let looping = forget_stmt(known, stmt);
                self.expr(cond, &looping);
                self.stmt(body, self.with(looping.clone(), cond, true));
                looping
            }
            Stmt::DoWhile { body, cond } => {
                let looping = forget_stmt(known, stmt);
                self.stmt(body, looping.clone());
                self.expr(cond, &looping);
                looping
            }
            Stmt::For { init, cond, update, body } => {
                let mut known = known;
                for s in init {
                    known = self.stmt(s, known);
                }
                let looping = forget_stmt(known, stmt);
                let mut on_body = looping.clone();
                if let Some(cond) = cond {
                    self.expr(cond, &looping);
                    on_body = self.with(on_body, cond, true);
                }
                self.stmt(body, on_body);
                for u in update {
                    self.expr(u, &looping);
                }
                looping
            }
            Stmt::ForEach { array, body, .. } => {
                self.expr(array, &forget_expr(known.clone(), array));
                let looping = forget_stmt(known, stmt);
                self.stmt(body, looping.clone());
                looping
            }
            Stmt::Block(stmts) => {
                self.block(stmts, known.clone());
                forget_stmt(known, stmt)
            }
            Stmt::Switch { selector, cases } => {
                self.expr(selector, &forget_expr(known.clone(), selector));
                let after = forget_stmt(known, stmt);
                self.cases(cases, after.clone());
                after
            }
            Stmt::Labeled { body, .. } => {
                let after = forget_stmt(known, stmt);
                self.stmt(body, after.clone());
                after
            }
            Stmt::Try { body, catches, finally } => {
                self.block(body, known.clone());
                let mut assigned = FastSet::default();
                body.iter().for_each(|s| assigned_in_stmt(s, &mut assigned));
                let mut caught = known.clone();
                caught.retain(|v, _| !assigned.contains(v));
                for catch in catches {
                    caught.remove(&catch.name);
                    self.block(&catch.body, caught.clone());
                }
                if let Some(finally) = finally {
                    let finished = forget_stmt(known.clone(), stmt);
                    self.block(finally, finished);
                }
                forget_stmt(known, stmt)
            }
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => known,
        }
    }

    /// An expression statement or a local's initializer `e`, run with `known`, which holds no
    /// variable that `e` assigns: what is known after it, with the variables it always casts.
    fn simple(&mut self, e: &Expr, mut known: Known) -> Known {
        self.expr(e, &known);
        let mut assigned = FastSet::default();
        assigned_in_expr(e, &mut assigned);
        let mut casts = Vec::new();
        self.always_cast(e, &mut casts);
        for (name, t) in casts {
            if !assigned.contains(name) {
                self.learn(&mut known, name, t);
            }
        }
        known
    }

    /// The casts of a variable to a narrower class type that evaluating `e` always runs, when
    /// it completes: not those in the right operand of `&&` or `||`, a branch of `?:` or a case
    /// of a `switch`.
    fn always_cast<'e>(&self, e: &'e Expr, out: &mut Vec<(&'e str, usize)>) {
        match &e.kind {
            ExprKind::Cast(to, inner) => {
                if let (Some((name, s)), Some(t)) = (self.var(inner), self.type_of(*to))
                    && !self.class.hierarchy.is_subtype(s, t)
                {
                    out.push((name, t));
                }
                self.always_cast(inner, out);
            }
            ExprKind::Binary(BinOp::And | BinOp::Or, a, _) => self.always_cast(a, out),
            ExprKind::Conditional(c, _, _) => self.always_cast(c, out),
            ExprKind::Switch { selector, .. } => self.always_cast(selector, out),
            _ => {
                let mut parts: Vec<&Expr> = Vec::new();
                children(e, &mut parts);
                for part in parts {
                    self.always_cast(part, out);
                }
            }
        }
    }
}

/// Whether `e` holds an `instanceof`, which may prove something inside it.
fn has_test(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::InstanceOf { .. }));
    found
}

/// The expressions directly inside `e`, those of a place too, in the order they are evaluated.
/// Not the statements of a `switch` expression's cases.
fn children<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    let place = |p: &'e Place, out: &mut Vec<&'e Expr>| match p {
        Place::Index(a, i) => {
            out.push(a);
            out.push(i);
        }
        Place::Field { object, .. } => out.push(object),
        Place::Var(_) | Place::Static { .. } => {}
    };
    match &e.kind {
        ExprKind::Unary(_, x)
        | ExprKind::Cast(_, x)
        | ExprKind::InstanceOf { value: x, .. }
        | ExprKind::Length(x)
        | ExprKind::Field { object: x, .. } => out.push(x),
        ExprKind::Assign { target, value, .. } => {
            place(target, out);
            out.push(value);
        }
        ExprKind::IncDec { target, .. } => place(target, out),
        ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => {
            out.push(l);
            out.push(r);
        }
        ExprKind::NewArray(items) | ExprKind::ArrayLit(items) => out.extend(items),
        ExprKind::Conditional(c, a, b) => out.extend([&**c, &**a, &**b]),
        ExprKind::MethodCall { receiver, args, .. } => {
            out.push(receiver);
            out.extend(args);
        }
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. } => out.extend(args),
        ExprKind::Switch { selector, .. } => out.push(selector),
        _ => {}
    }
}
