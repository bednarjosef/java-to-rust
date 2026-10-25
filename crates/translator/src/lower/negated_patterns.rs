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
//!
//! An `if` whose condition negates an `&&` chain with a pattern has it negated operand by operand
//! first, which runs the same: `!(a && x instanceof T name)` is `!a || !(x instanceof T name)`.
//! An `||` chain inside the condition's `&&` chains (`a && (!(x instanceof T name) || c(name))`)
//! whose negated pattern a later operand reads becomes a conditional, which binds the pattern
//! for the operands it guards:
//!
//! ```text
//! !(x instanceof T name) || c(name)            x instanceof T name ? c(name) : true
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
        let cond = self.inner_ors(self.negated_and(cond));
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

    /// `cond`, an `&&` chain with a pattern under a negation, as the `||` chain of its operands
    /// negated.
    fn negated_and(&self, cond: Expr) -> Expr {
        match cond.kind {
            ExprKind::Unary(UnOp::Not, inner)
                if matches!(inner.kind, ExprKind::Binary(BinOp::And, ..)) && binds_pattern(&inner) =>
            {
                let pos = cond.pos;
                into_chain(*inner, BinOp::And)
                    .into_iter()
                    .map(|operand| self.negated(operand))
                    .reduce(|left, right| {
                        self.synth(
                            ExprKind::Binary(BinOp::Or, Box::new(left), Box::new(right)),
                            pos,
                            Type::Boolean,
                        )
                    })
                    .expect("an `&&` chain has operands")
            }
            kind => Expr { kind, ..cond },
        }
    }

    /// `cond` with each `||` chain among the operands of its `&&` chains (those of its own `||`
    /// chain) that has a negated pattern which a later operand reads as a conditional.
    fn inner_ors(&self, cond: Expr) -> Expr {
        let mut disjuncts = Vec::new();
        chain(&cond, BinOp::Or, &mut disjuncts);
        let needed = disjuncts.iter().any(|d| {
            let mut conjuncts = Vec::new();
            chain(d, BinOp::And, &mut conjuncts);
            conjuncts.len() > 1
                && conjuncts.iter().any(|c| {
                    let mut operands = Vec::new();
                    chain(c, BinOp::Or, &mut operands);
                    operands.iter().enumerate().any(|(i, o)| {
                        negated_pattern(o)
                            .is_some_and(|name| operands[i + 1..].iter().any(|l| mentions(l, name)))
                    })
                })
        });
        if !needed {
            return cond;
        }
        let pos = cond.pos;
        let join = |op: BinOp, operands: Vec<Expr>| {
            operands
                .into_iter()
                .reduce(|left, right| {
                    self.synth(ExprKind::Binary(op, Box::new(left), Box::new(right)), pos, Type::Boolean)
                })
                .expect("a chain has operands")
        };
        let disjuncts = into_chain(cond, BinOp::Or)
            .into_iter()
            .map(|disjunct| {
                let conjuncts = into_chain(disjunct, BinOp::And);
                if conjuncts.len() < 2 {
                    return join(BinOp::And, conjuncts);
                }
                let conjuncts =
                    conjuncts.into_iter().map(|c| self.or_as_conditional(into_chain(c, BinOp::Or))).collect();
                join(BinOp::And, conjuncts)
            })
            .collect();
        join(BinOp::Or, disjuncts)
    }

    /// The `||` chain of `operands`, from its first negated pattern that a later operand reads on
    /// as `x instanceof T name ? (later operands) : true`.
    fn or_as_conditional(&self, mut operands: Vec<Expr>) -> Expr {
        let first = operands.remove(0);
        let pos = first.pos;
        if operands.is_empty() {
            return first;
        }
        let rest = self.or_as_conditional(operands);
        match negated_pattern(&first) {
            Some(name) if mentions(&rest, name) => {
                let ExprKind::Unary(UnOp::Not, pattern) = first.kind else {
                    unreachable!("a negated pattern")
                };
                let yes = Box::new(rest);
                let no = Box::new(self.synth(ExprKind::Bool(true), pos, Type::Boolean));
                self.synth(ExprKind::Conditional(pattern, yes, no), pos, Type::Boolean)
            }
            _ => self.synth(ExprKind::Binary(BinOp::Or, Box::new(first), Box::new(rest)), pos, Type::Boolean),
        }
    }

    /// `!operand`, without a double negation.
    fn negated(&self, operand: Expr) -> Expr {
        match operand.kind {
            ExprKind::Unary(UnOp::Not, inner) => *inner,
            kind => {
                let operand = Expr { kind, ..operand };
                let pos = operand.pos;
                self.synth(ExprKind::Unary(UnOp::Not, Box::new(operand)), pos, Type::Boolean)
            }
        }
    }

    /// `if (cond) then else otherwise` with `cond` an `||` chain, as `if (!a && ..) otherwise
    /// else then`.
    fn turned(&self, cond: Expr, then: Stmt, otherwise: Option<Stmt>) -> Stmt {
        let pos = cond.pos;
        let mut chain: Option<Expr> = None;
        for operand in into_chain(cond, BinOp::Or) {
            let negated = self.negated(operand);
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

/// The operands of the chain of `op` that `e` is, left to right, taken out of it.
fn into_chain(e: Expr, op: BinOp) -> Vec<Expr> {
    match e.kind {
        ExprKind::Binary(o, l, r) if o == op => {
            let mut out = into_chain(*l, op);
            out.extend(into_chain(*r, op));
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
