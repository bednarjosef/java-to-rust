//! Plainer Rust for expressions that Rust writes shorter than Java: a comparison with a `true`
//! or `false` literal, and `x = x op v` on a local, which Rust writes `x op= v`.

use super::*;

/// The operand compared with a `true` or `false` literal, and that literal.
pub(super) fn bool_test<'e>(l: &'e Expr, r: &'e Expr) -> Option<(&'e Expr, bool)> {
    match (&l.kind, &r.kind) {
        (_, ExprKind::Bool(b)) if l.ty == Type::Boolean && r.ty == Type::Boolean => Some((l, *b)),
        (ExprKind::Bool(b), _) if l.ty == Type::Boolean && r.ty == Type::Boolean => Some((r, *b)),
        _ => None,
    }
}

impl<'a> Gen<'a> {
    /// `x == true` is `x`, and `x == false` is `!x`; `!=` is the other way round. The negation
    /// of a comparison is written as the flipped comparison, as in `negate`.
    pub(super) fn bool_comparison(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let Some((other, literal)) = bool_test(l, r) else { unreachable!("checked by the caller") };
        if (op == BinOp::Eq) == literal {
            return self.value(other, Type::Boolean);
        }
        Code::new(self.negate(other), LOWEST)
    }

    /// `x = x op v` on a local `name` of type `ty`, written `x op= v` where Rust's compound
    /// assignment does what Java's does: the value is only read before it is stored.
    pub(super) fn self_update(&mut self, name: &str, target: &str, ty: Type, value: &'a Expr) -> Option<String> {
        let ExprKind::Binary(op, l, r) = &value.kind else { return None };
        let reads_target = matches!(&l.kind, ExprKind::Var(v) if v == target);
        if !reads_target || value.ty != ty {
            return None;
        }
        if !matches!(ty, Type::Int | Type::Long | Type::Double | Type::Float | Type::Boolean) {
            return None;
        }
        let symbol = self.in_place_op(*op, ty, r, value.id)?;
        let v = self.value(r, if op.is_shift() { r.ty } else { ty });
        Some(format!("{name} {symbol} {}", v.text))
    }
}
