//! A negated pattern in an `||` chain that an `if` tests:
//!
//! ```text
//! if (a || !(x instanceof T name) || c(name)) s1 else s2(name)
//! ```
//!
//! Java scopes `name` over the later operands of the chain and over the else branch, where every
//! operand was false and so the pattern matched. The translation binds a pattern's variable in an
//! `&&` chain (a let chain), so the `if` is turned around: its condition negated operand by
//! operand, which keeps the order in which they run, and its branches swapped:
//!
//! ```text
//! if (!a && x instanceof T name && !c(name)) s2(name) else s1
//! ```
//!
//! (Without an else branch, `s2` is an empty block.) `analysis::patterns` then checks the reads
//! of `name` as those of any pattern in an `&&` chain.
//!
//! Without an else branch and with a then branch that always leaves the block, `name` is in scope
//! in the rest of the block too, which then runs only where the pattern matched:
//!
//! ```text
//! if (a || !(x instanceof T name)) return;     if (!a && x instanceof T name) {
//! rest(name)                                       rest(name)
//!                                              } else { return; }
//! ```
//!
//! (`exits_into_rest`; one negated pattern alone is `let .. else` in the generator.)
//!
//! A conditional whose condition is a negated pattern, as decompilers write `equals`, has its
//! branches swapped, which keeps what runs: `name` is in scope in the false branch, and the
//! translation binds a pattern for the branch that its test guards.
//!
//! ```text
//! !(o instanceof T name) ? false : f(name)     o instanceof T name ? f(name) : false
//! ```

use super::*;

impl<'t> Lowering<'t> {
    /// `cond ? yes : no`, with its branches swapped when `cond` negates a test with a pattern.
    pub(super) fn conditional(&self, cond: Expr, yes: Expr, no: Expr) -> ExprKind {
        match cond.kind {
            ExprKind::Unary(UnOp::Not, inner) if binds_pattern(&inner) => {
                ExprKind::Conditional(inner, Box::new(no), Box::new(yes))
            }
            kind => ExprKind::Conditional(Box::new(Expr { kind, ..cond }), Box::new(yes), Box::new(no)),
        }
    }

    /// `if (cond) then else otherwise`, turned around as above when `cond` is an `||` chain with
    /// a negated pattern whose variable a later operand or the else branch reads.
    pub(super) fn if_stmt(&self, cond: Expr, then: Stmt, otherwise: Option<Stmt>) -> Stmt {
        let mut operands = Vec::new();
        chain(&cond, BinOp::Or, &mut operands);
        let read_later = operands.iter().enumerate().any(|(i, operand)| {
            let Some(name) = negated_pattern(operand) else { return false };
            operands[i + 1..].iter().any(|later| mentions(later, name))
                || otherwise.as_ref().is_some_and(|o| mentions_stmt(o, name))
        });
        if operands.len() < 2 || !read_later {
            return Stmt::If { cond, then: Box::new(then), otherwise: otherwise.map(Box::new) };
        }
        self.turned(cond, then, otherwise)
    }

    /// The statements of a block, each `if` with an `||` chain that has a negated pattern, no
    /// else branch and a then branch that leaves the block turned around with the rest of the
    /// block as its new then branch, where the rest reads the pattern's variable.
    pub(super) fn exits_into_rest(&self, mut stmts: Vec<Stmt>) -> Vec<Stmt> {
        let at = stmts.iter().enumerate().position(|(i, s)| {
            let Stmt::If { cond, then, otherwise } = s else { return false };
            let rest = &stmts[i + 1..];
            let read = |name: &str| rest.iter().any(|r| mentions_stmt(r, name));
            match otherwise.as_deref() {
                // As it is.
                None if exits(then) => {
                    let mut operands = Vec::new();
                    chain(cond, BinOp::Or, &mut operands);
                    operands.len() > 1 && operands.iter().filter_map(|o| negated_pattern(o)).any(read)
                }
                // As `if_stmt` turned it for a read in a later operand: an empty then branch.
                Some(o) if exits(o) && matches!(&**then, Stmt::Block(b) if b.is_empty()) => {
                    let mut operands = Vec::new();
                    chain(cond, BinOp::And, &mut operands);
                    operands.iter().any(
                        |o| matches!(&o.kind, ExprKind::InstanceOf { binding: Some(name), .. } if read(name)),
                    )
                }
                _ => false,
            }
        });
        let Some(at) = at else { return stmts };
        let rest = Stmt::Block(self.exits_into_rest(stmts.split_off(at + 1)));
        let turned = match stmts.pop() {
            Some(Stmt::If { cond, then, otherwise: None }) => self.turned(cond, *then, Some(rest)),
            Some(Stmt::If { cond, otherwise, .. }) => Stmt::If { cond, then: Box::new(rest), otherwise },
            _ => unreachable!("the `if` found above"),
        };
        stmts.push(turned);
        stmts
    }

    /// `if (cond) then else otherwise` with `cond` an `||` chain, as `if (!a && ..) otherwise
    /// else then`.
    fn turned(&self, cond: Expr, then: Stmt, otherwise: Option<Stmt>) -> Stmt {
        let pos = cond.pos;
        let mut chain: Option<Expr> = None;
        for operand in into_disjuncts(cond) {
            let negated = match operand.kind {
                ExprKind::Unary(UnOp::Not, inner) => *inner,
                kind => {
                    let operand = Expr { kind, ..operand };
                    let pos = operand.pos;
                    self.synth(ExprKind::Unary(UnOp::Not, Box::new(operand)), pos, Type::Boolean)
                }
            };
            chain = Some(match chain {
                None => negated,
                Some(left) => self.synth(
                    ExprKind::Binary(BinOp::And, Box::new(left), Box::new(negated)),
                    pos,
                    Type::Boolean,
                ),
            });
        }
        Stmt::If {
            cond: chain.expect("an `||` chain has operands"),
            then: Box::new(otherwise.unwrap_or(Stmt::Block(Vec::new()))),
            otherwise: Some(Box::new(then)),
        }
    }
}

/// The operands of the chain of `op` (`||` or `&&`) that `e` is, left to right.
fn chain<'e>(e: &'e Expr, op: BinOp, out: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Binary(o, l, r) if *o == op => {
            chain(l, op, out);
            chain(r, op, out);
        }
        _ => out.push(e),
    }
}

/// The operands of the `||` chain that `e` is, left to right, taken out of it.
fn into_disjuncts(e: Expr) -> Vec<Expr> {
    match e.kind {
        ExprKind::Binary(BinOp::Or, l, r) => {
            let mut out = into_disjuncts(*l);
            out.extend(into_disjuncts(*r));
            out
        }
        kind => vec![Expr { kind, ..e }],
    }
}

/// The variable of `!(x instanceof T name)`, when `e` is one.
fn negated_pattern(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Unary(UnOp::Not, inner) => match &inner.kind {
            ExprKind::InstanceOf { binding: Some(name), .. } => Some(name),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `stmt` always leaves the block it is in.
fn exits(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(..)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_)
        | Stmt::Throw(_) => true,
        Stmt::Block(stmts) => stmts.last().is_some_and(exits),
        _ => false,
    }
}

fn mentions_stmt(stmt: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(stmt, &mut |e| found |= mentions(e, name), &mut |_| {});
    found
}

/// Whether `e` has a pattern with a variable (`x instanceof T t`).
fn binds_pattern(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::InstanceOf { binding: Some(_), .. }));
    found
}
