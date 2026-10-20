//! Last uses. A read of a variable whose value is never read again can move the value instead of
//! cloning it: a string, an array handle or an object handle. A read is a last use when no later
//! code of the method mentions the same variable, and it is not inside a loop that the variable
//! is declared outside of, since the next iteration would read it again.
//!
//! Moving instead of cloning never changes what a program does: the moved value is the same
//! string or the same handle. A wrong answer here could only make the Rust fail to compile.

use super::Facts;
use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub(super) fn analyze(class: &Class, facts: &mut Facts) {
    for method in &class.methods {
        let mut walk = Walk::default();
        for p in &method.params {
            walk.declare(&p.name);
        }
        walk.block(&method.body);
        // A variable mentioned twice in one statement is not moved there: Rust keeps temporaries
        // such as a `Ref` from a field read, or the references `format_args!` takes, alive to the
        // end of the statement, so a move next to another mention would not compile.
        let mut per_statement: HashMap<(usize, usize), usize> = HashMap::new();
        for use_ in &walk.uses {
            *per_statement.entry((use_.var, use_.statement)).or_default() += 1;
        }
        let mut later = HashSet::new();
        for use_ in walk.uses.iter().rev() {
            if later.insert(use_.var)
                && per_statement[&(use_.var, use_.statement)] == 1
                && use_.loops == walk.declared_in[use_.var]
                && let Some(id) = use_.id
            {
                facts.moves.insert(id);
            }
        }
    }
}

/// A mention of a variable: a read (`id` is the `Var` expression) or a compound assignment.
struct Use {
    id: Option<ExprId>,
    var: usize,
    /// How many loops enclose the mention.
    loops: usize,
    /// The statement (or condition) the mention is in.
    statement: usize,
}

/// The mentions of variables in evaluation order. Each declaration is a variable of its own, so
/// two locals of one name in different blocks are told apart.
#[derive(Default)]
struct Walk {
    /// The variables in scope, innermost last: (Java name, variable).
    scope: Vec<(String, usize)>,
    /// How many loops enclose each variable's declaration.
    declared_in: Vec<usize>,
    loops: usize,
    /// The number of the statement being walked.
    statement: usize,
    uses: Vec<Use>,
}

impl Walk {
    fn declare(&mut self, name: &str) {
        self.scope.push((name.to_string(), self.declared_in.len()));
        self.declared_in.push(self.loops);
    }

    fn mention(&mut self, name: &str, id: Option<ExprId>) {
        if let Some(&(_, var)) = self.scope.iter().rev().find(|(n, _)| n == name) {
            self.uses.push(Use { id, var, loops: self.loops, statement: self.statement });
        }
    }

    fn block(&mut self, stmts: &[Stmt]) {
        let mark = self.scope.len();
        for s in stmts {
            self.stmt(s);
        }
        self.scope.truncate(mark);
    }

    fn scoped(&mut self, s: &Stmt) {
        let mark = self.scope.len();
        self.stmt(s);
        self.scope.truncate(mark);
    }

    /// An expression a statement holds directly: a statement of its own for Rust's temporaries.
    fn top(&mut self, e: &Expr) {
        self.statement += 1;
        self.expr(e);
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local { name, init, .. } => {
                if let Some(e) = init {
                    self.top(e);
                }
                self.declare(name);
            }
            Stmt::Expr(e) | Stmt::Return(Some(e), _) => self.top(e),
            Stmt::If { cond, then, otherwise } => {
                self.top(cond);
                self.scoped(then);
                if let Some(o) = otherwise {
                    self.scoped(o);
                }
            }
            Stmt::While { cond, body } => {
                self.loops += 1;
                self.top(cond);
                self.scoped(body);
                self.loops -= 1;
            }
            Stmt::DoWhile { body, cond } => {
                self.loops += 1;
                self.scoped(body);
                self.top(cond);
                self.loops -= 1;
            }
            Stmt::For { init, cond, update, body } => {
                let mark = self.scope.len();
                for s in init {
                    self.stmt(s);
                }
                self.loops += 1;
                if let Some(c) = cond {
                    self.top(c);
                }
                self.scoped(body);
                for u in update {
                    self.top(u);
                }
                self.loops -= 1;
                self.scope.truncate(mark);
            }
            Stmt::ForEach { name, array, body, .. } => {
                self.top(array);
                self.loops += 1;
                let mark = self.scope.len();
                self.declare(name);
                self.scoped(body);
                self.scope.truncate(mark);
                self.loops -= 1;
            }
            Stmt::Switch { selector, cases } => {
                self.top(selector);
                // The cases share one scope. A switch with fall-through is a `loop` in Rust, which
                // would not let a value move inside it, so the cases count as a loop.
                self.loops += 1;
                let mark = self.scope.len();
                for case in cases {
                    for s in &case.body {
                        self.stmt(s);
                    }
                }
                self.scope.truncate(mark);
                self.loops -= 1;
            }
            Stmt::Labeled { body, .. } => self.scoped(body),
            Stmt::Yield(e) => self.top(e),
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
            Stmt::Block(stmts) => self.block(stmts),
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Var(name) => self.mention(name, Some(e.id)),
            // `s += t` reads `s` after `t`.
            ExprKind::Assign { op, target: Place::Var(name), value } => {
                self.expr(value);
                if op.is_some() {
                    self.mention(name, None);
                }
            }
            ExprKind::IncDec { target: Place::Var(name), .. } => self.mention(name, None),
            ExprKind::Assign { target, value, .. } => {
                self.place(target);
                self.expr(value);
            }
            ExprKind::IncDec { target, .. } => self.place(target),
            ExprKind::Unary(_, x)
            | ExprKind::Cast(_, x)
            | ExprKind::Length(x)
            | ExprKind::Field { object: x, .. } => self.expr(x),
            ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => {
                self.expr(l);
                self.expr(r);
            }
            ExprKind::Conditional(c, a, b) => {
                self.expr(c);
                self.expr(a);
                self.expr(b);
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.expr(receiver);
                args.iter().for_each(|a| self.expr(a));
            }
            ExprKind::Call { args, .. }
            | ExprKind::Builtin(_, args)
            | ExprKind::New { args, .. }
            | ExprKind::ThisCall { args, .. }
            | ExprKind::SuperInit { args, .. }
            | ExprKind::NewArray(args)
            | ExprKind::ArrayLit(args) => args.iter().for_each(|a| self.expr(a)),
            // Anything else: its variables count as mentioned, none of them as a last use.
            _ => visit_expr(e, &mut |x| {
                if let ExprKind::Var(name) = &x.kind {
                    self.mention(name, None);
                }
            }),
        }
    }

    fn place(&mut self, place: &Place) {
        match place {
            Place::Index(array, index) => {
                self.expr(array);
                self.expr(index);
            }
            Place::Field { object, .. } => self.expr(object),
            Place::Var(name) => self.mention(name, None),
            Place::Static { .. } => {}
        }
    }
}
