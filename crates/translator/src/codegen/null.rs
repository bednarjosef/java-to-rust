//! Strings that may be null, which are `Option<String>` in Rust. Which values are optional comes
//! from `analysis::maybe_null` and `analysis::nullable_var`; here is how each use is written.

use super::*;
use crate::analysis::{maybe_null, nullable_var};

/// Whether `l == r` or `l != r` compares a string with the `null` literal.
pub(super) fn is_null_test(op: BinOp, l: &Expr, r: &Expr) -> bool {
    let string_or_null = |e: &Expr| matches!(e.ty, Type::String | Type::Null);
    matches!(op, BinOp::Eq | BinOp::Ne)
        && (matches!(l.kind, ExprKind::Null) || matches!(r.kind, ExprKind::Null))
        && string_or_null(l)
        && string_or_null(r)
}

impl<'a> Gen<'a> {
    /// Whether `e`, in the current method, may be null, so that its Rust text is an `Option`.
    pub(super) fn nullable(&self, e: &Expr) -> bool {
        maybe_null(self.facts, self.method, e)
    }

    /// Whether the variable or parameter `name` of method `method`, of type `ty`, is an `Option`.
    pub(super) fn optional_var(&self, method: usize, ty: Type, name: &str) -> bool {
        ty == Type::String && nullable_var(self.facts, method, name)
    }

    /// Whether the current method returns an `Option<String>`.
    pub(super) fn optional_return(&self) -> bool {
        self.ret == Type::String && self.facts.nullable_returns.contains(&self.method)
    }

    /// `e` as an `Option<String>`: a possibly-null value as it is, any other string in `Some`.
    pub(super) fn optional(&mut self, e: &'a Expr) -> Code {
        if self.nullable(e) {
            return self.expr(e);
        }
        let value = self.value(e, Type::String);
        Code::atom(format!("Some({})", value.text))
    }

    /// `let name: Option<String> = init;`
    pub(super) fn optional_local(&mut self, name: &str, init: Option<&'a Expr>, mutable: bool) {
        let mutable = if mutable { "mut " } else { "" };
        let rust = self.local(name);
        match init {
            None => self.line(&format!("let {mutable}{rust}: Option<String>;")),
            Some(init) => {
                let code = self.optional(init);
                self.line(&format!("let {mutable}{rust}: Option<String> = {};", code.text));
            }
        }
    }

    /// A possibly-null string as an `Option<&str>`.
    pub(super) fn option_ref(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "None::<&str>".to_string(),
            ExprKind::Var(name) => format!("{}.as_deref()", self.local(name)),
            ExprKind::Cast(_, inner) => self.option_ref(inner),
            _ => format!("{}.as_deref()", self.expr(e).at(ATOM)),
        }
    }

    /// A possibly-null string as a `&str` that a method is called on: null throws
    /// `NullPointerException`.
    pub(super) fn deref(&mut self, e: &'a Expr) -> String {
        let option = self.option_ref(e);
        format!("{}({option})", self.runtime("non_null"))
    }

    /// `x == null` and `x != null` for a string.
    pub(super) fn null_test(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let other = if matches!(l.kind, ExprKind::Null) { r } else { l };
        let is_null = if matches!(other.kind, ExprKind::Null) {
            Code::atom("true")
        } else if self.nullable(other) {
            let option = self.option_ref(other);
            Code::atom(format!("{option}.is_none()"))
        } else if matches!(other.kind, ExprKind::Var(_) | ExprKind::Str(_)) {
            Code::atom("false")
        } else {
            let code = self.expr(other);
            Code::new(format!("{{ {}; false }}", code.text), ATOM)
        };
        if matches!(op, BinOp::Ne) { Code::new(format!("!{}", is_null.at(UNARY)), UNARY) } else { is_null }
    }

    /// `a.equals(b)` where one of the strings may be null: a null receiver throws, and a null
    /// argument is simply not equal.
    pub(super) fn equals_optional(&mut self, args: &'a [Expr]) -> Code {
        let receiver = if self.nullable(&args[0]) { self.deref(&args[0]) } else { self.str_value(&args[0]) };
        if self.nullable(&args[1]) {
            let option = self.option_ref(&args[1]);
            let text =
                format!("({{ let receiver_ = {receiver}; {option}.is_some_and(|o| o == receiver_) }})");
            return Code::atom(text);
        }
        let other = self.str_value(&args[1]);
        infix("==", CMP, &Code::atom(receiver), &Code::atom(other))
    }

    /// `s += value` where `s` may be null: a null `s` prints as `null`.
    pub(super) fn append_optional(&mut self, name: &str, value: &'a Expr) -> String {
        let mut parts = vec![Part::Arg(Code::atom(format!("{name}.as_deref().unwrap_or(\"null\")")))];
        self.parts(value, &mut parts);
        let text = self.format("format!", parts).text;
        format!("{name} = Some({text})")
    }

    /// A null or possibly-null string as a piece of a format string, or `None` if `e` is an
    /// ordinary string.
    pub(super) fn null_part(&mut self, e: &'a Expr, assigned: &HashSet<String>) -> Option<Part> {
        if matches!(e.kind, ExprKind::Null) {
            return Some(Part::Lit("null".to_string()));
        }
        if e.ty != Type::String || !self.nullable(e) {
            return None;
        }
        let text = match &e.kind {
            // An operand that assigns the variable is read before the assignment, as a copy.
            ExprKind::Var(name) if assigned.contains(name) => {
                format!("{}.clone().unwrap_or_else(|| \"null\".into())", self.local(name))
            }
            _ => format!("{}.unwrap_or(\"null\")", self.option_ref(e)),
        };
        Some(Part::Arg(Code::atom(text)))
    }
}
