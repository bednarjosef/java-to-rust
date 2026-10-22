//! Enums that may be null, which are `Option`s of the `Copy` enum in Rust. A `?:` between a
//! nullable enum and a constant of the enum, as javac's `Tokens.lookupKind` writes it, has the enum
//! as its type: its non-null arm reads the variable as the enum, which the null test has shown.

use super::*;

impl<'a> Gen<'a> {
    /// Whether `l == r` or `l != r` compares enums or boxes of which one may be null, which are
    /// `Option`s: `None` equals `None`, and a value equals `Some` of itself. Such a comparison is
    /// written by `expr`, not flipped by `negate`, which would leave out the `Some`.
    pub(super) fn optional_equality(&self, l: &Expr, r: &Expr) -> bool {
        (matches!((l.ty, r.ty), (Type::Enum(_), Type::Enum(_)))
            || matches!(l.ty, Type::Boxed(_)) && r.ty == l.ty)
            && (self.nullable(l) || self.nullable(r))
    }

    /// `l == r` or `l != r` for `optional_equality`, with each side as an `Option`.
    pub(super) fn optional_equality_code(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let (a, b) = (self.optional_to(l, l.ty), self.optional_to(r, r.ty));
        let symbol = if op == BinOp::Eq { "==" } else { "!=" };
        infix(symbol, CMP, &a, &b)
    }

    /// `t != null ? t : E.C` (or `t == null ? E.C : t`) for a nullable enum `t`, whose type is the
    /// enum: `if let Some(t) = t { t } else { E::C }`. `None` for any other conditional.
    pub(super) fn enum_conditional(
        &mut self,
        e: &'a Expr,
        cond: &'a Expr,
        a: &'a Expr,
        b: &'a Expr,
    ) -> Option<Code> {
        if !matches!(e.ty, Type::Enum(_)) || self.nullable(e) {
            return None;
        }
        let ExprKind::Binary(op, l, r) = &cond.kind else { return None };
        if !is_null_test(*op, l, r) {
            return None;
        }
        let var = match (&l.kind, &r.kind) {
            (ExprKind::Var(_), ExprKind::Null) => l,
            (ExprKind::Null, ExprKind::Var(_)) => r,
            _ => return None,
        };
        let ExprKind::Var(name) = &var.kind else { return None };
        if !matches!(var.ty, Type::Enum(_)) || !self.nullable(var) {
            return None;
        }
        let (non_null, other) = if matches!(op, BinOp::Ne) { (a, b) } else { (b, a) };
        if assigns(non_null, name) || assigns(other, name) || mentions(other, name) {
            return None;
        }
        if !mentions(non_null, name) {
            return None;
        }
        let rust = self.local(name);
        let narrowed = self.narrow(name);
        let non_null = self.value(non_null, e.ty);
        self.unnarrow(narrowed);
        let other = self.value(other, e.ty);
        let head = format!("let Some({rust}) = {rust}");
        let text = self.if_else(&head, &non_null.text, &other.text, false);
        Some(Code::new(text, LOWEST))
    }

    /// Whether the enum `e` may be null, including a `null` cast to the enum, as `(Target) null`.
    pub(super) fn enum_maybe_null(&self, e: &Expr) -> bool {
        let mut inner = e;
        while let ExprKind::Cast(_, value) = &inner.kind {
            inner = value;
        }
        matches!(inner.kind, ExprKind::Null) || self.nullable(e)
    }

    /// A possibly-null enum as a `&str`-like text: its name, or `null`, as Java's `append(Object)`
    /// and string conversion show it.
    pub(super) fn nullable_enum_text(&mut self, e: &'a Expr) -> String {
        let mut inner = e;
        while let ExprKind::Cast(_, value) = &inner.kind {
            inner = value;
        }
        if matches!(inner.kind, ExprKind::Null) {
            return self.wtf8_literal("null");
        }
        let option = self.option_ref(e);
        format!("&{}::from({}({option}).to_string())", self.runtime("JString"), self.runtime("or_null"))
    }
}
