//! What a pattern switch needs beyond `if` chains and `instanceof` (see `lower::patterns`): the
//! null check of its selector where its chain ends.

use super::*;

/// What the end of an exhaustive switch is, which no value but null reaches.
const UNMATCHED: &str = "unreachable!(\"a case of the exhaustive switch matches\")";

impl<'a> Gen<'a> {
    /// `NullChecked(selector, value)`: the value, once a null selector has thrown, as the
    /// switch's type `ty` (an `int` constant in a `long` switch); without a value, the check
    /// alone (which a statement of a selector that is never null leaves out).
    pub(super) fn null_checked(&mut self, args: &'a [Expr], ty: Type) -> Code {
        let check = self.null_check(&args[0]);
        match args.get(1) {
            None => check,
            Some(value) if !self.nullable(&args[0]) => self.value(value, ty),
            // `NullChecked(v, v)`: the variable itself, not null: the enclosing instance of
            // `v.new Inner()` (`lower::inner`), which javac checks with `Objects.requireNonNull`.
            Some(value)
                if matches!((&args[0].kind, &value.kind), (ExprKind::Var(a), ExprKind::Var(b)) if a == b) =>
            {
                let option = self.expr(value).text;
                Code::atom(self.dereference(&[args[0].id], &option))
            }
            Some(value) => {
                let value = self.value(value, ty);
                Code::atom(format!("{{ {}; {} }}", check.text, value.text))
            }
        }
    }

    /// `Unmatched(selector)`: a null selector throws, and nothing else gets here.
    pub(super) fn unmatched(&mut self, args: &'a [Expr]) -> Code {
        if !self.nullable(&args[0]) {
            return Code::atom(UNMATCHED);
        }
        let check = self.null_check(&args[0]);
        Code::atom(format!("{{ {}; {UNMATCHED} }}", check.text))
    }
}
