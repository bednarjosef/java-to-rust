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
        // A variable narrowed by a null check is a `&str` there, so it is not null there. Only
        // strings that may be null are narrowed, and those are never borrowed parameters.
        if let ExprKind::Var(name) = &e.kind
            && self.borrowed.contains(name)
        {
            return false;
        }
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

/// Whether `stmt` reads or writes the variable `name` anywhere inside it.
fn mentions_stmt(stmt: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(stmt, &mut |e| found |= mentions(e, name), &mut |_| {});
    found
}

/// Whether `stmt` always leaves the block it is in.
fn exits(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(..) | Stmt::Break | Stmt::Continue => true,
        Stmt::Block(stmts) => stmts.last().is_some_and(exits),
        _ => false,
    }
}

impl<'a> Gen<'a> {
    /// The string variable of `s == null` or `s != null`, when `s` may be null here, and whether
    /// the then-branch is the one where it is not null.
    fn null_test_var(&self, cond: &'a Expr) -> Option<(&'a str, bool)> {
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
        if !self.nullable(var) {
            return None;
        }
        Some((name.as_str(), matches!(op, BinOp::Ne)))
    }

    /// `if (s != null) { .. }` or `if (s == null) .. else { .. }`: the branch where `s` is not
    /// null binds it as `if let Some(s) = s.as_deref()`, so `s` is a `&str` there. Returns
    /// false, emitting nothing, when the `if` is not of that form, or the branch does not use `s`
    /// (a binding that is never read would only warn), or the branch assigns `s`.
    pub(super) fn narrow_if(&mut self, cond: &'a Expr, then: &'a Stmt, otherwise: Option<&'a Stmt>) -> bool {
        let Some((name, then_non_null)) = self.null_test_var(cond) else { return false };
        let (non_null, null) = if then_non_null { (Some(then), otherwise) } else { (otherwise, Some(then)) };
        let Some(non_null) = non_null else { return false };
        if assigns_stmt(non_null, name) || !mentions_stmt(non_null, name) {
            return false;
        }
        let rust = self.local(name);
        self.line(&format!("if let Some({rust}) = {rust}.as_deref() {{"));
        let added = self.borrowed.insert(name.to_string());
        self.nested(non_null);
        if added {
            self.borrowed.remove(name);
        }
        if let Some(null) = null {
            self.line("} else {");
            self.nested(null);
        }
        self.line("}");
        true
    }

    /// `if (s == null) return ..;` (or `break`, `continue`): the rest of the block sees `s` as
    /// non-null, so it is bound with `let Some(s) = s.as_deref() else { .. };`. Returns the name
    /// of `s`, or `None`, emitting nothing, when `stmt` is not of that form or `rest` assigns `s`.
    pub(super) fn early_null_exit(&mut self, stmt: &'a Stmt, rest: &[Stmt]) -> Option<&'a str> {
        let Stmt::If { cond, then, otherwise: None } = stmt else { return None };
        let (name, then_non_null) = self.null_test_var(cond)?;
        if then_non_null || !exits(then) || assigns_any(rest, name) || !rest.iter().any(|s| mentions_stmt(s, name)) {
            return None;
        }
        let rust = self.local(name);
        self.line(&format!("let Some({rust}) = {rust}.as_deref() else {{"));
        self.nested(then);
        self.line("};");
        Some(name)
    }
}
