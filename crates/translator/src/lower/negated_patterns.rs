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

use super::*;

impl<'t> Lowering<'t> {
    /// `if (cond) then else otherwise`, turned around as above when `cond` is an `||` chain with
    /// a negated pattern whose variable a later operand or the else branch reads.
    pub(super) fn if_stmt(&self, cond: Expr, then: Stmt, otherwise: Option<Stmt>) -> Stmt {
        let mut operands = Vec::new();
        disjuncts(&cond, &mut operands);
        let read_later = operands.iter().enumerate().any(|(i, operand)| {
            let Some(name) = negated_pattern(operand) else { return false };
            operands[i + 1..].iter().any(|later| mentions(later, name))
                || otherwise.as_ref().is_some_and(|o| mentions_stmt(o, name))
        });
        if operands.len() < 2 || !read_later {
            return Stmt::If { cond, then: Box::new(then), otherwise: otherwise.map(Box::new) };
        }
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

/// The operands of the `||` chain that `e` is, left to right.
fn disjuncts<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Binary(BinOp::Or, l, r) => {
            disjuncts(l, out);
            disjuncts(r, out);
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

fn mentions_stmt(stmt: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(stmt, &mut |e| found |= mentions(e, name), &mut |_| {});
    found
}
