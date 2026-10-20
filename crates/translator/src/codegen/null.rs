//! Strings that may be null, which are `Option<String>` in Rust. Which values are optional comes
//! from `analysis::maybe_null` and `analysis::nullable_var`; here is how each use is written.

use super::*;
use crate::analysis::{conjuncts, maybe_null, nullable_type, nullable_var};

/// Whether `l == r` or `l != r` compares a string, object or enum with the `null` literal.
pub(super) fn is_null_test(op: BinOp, l: &Expr, r: &Expr) -> bool {
    matches!(op, BinOp::Eq | BinOp::Ne)
        && (matches!(l.kind, ExprKind::Null) || matches!(r.kind, ExprKind::Null))
        && nullable_type(l.ty)
        && nullable_type(r.ty)
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
        nullable_type(ty) && nullable_var(self.facts, method, name)
    }

    /// Whether the current method returns an `Option<String>`.
    pub(super) fn optional_return(&self) -> bool {
        self.ret.is_reference() && self.facts.nullable_returns.contains(&self.method)
    }

    /// `e` as an `Option<String>`: a possibly-null value as it is, any other string in `Some`.
    pub(super) fn optional(&mut self, e: &'a Expr) -> Code {
        self.optional_to(e, e.ty)
    }

    /// `e` as an `Option` of the type `ty`: a possibly-null value as it is, any other value in
    /// `Some`, converted to `ty` (boxing it when `ty` is boxed).
    pub(super) fn optional_to(&mut self, e: &'a Expr, ty: Type) -> Code {
        if let ExprKind::Var(name) = &e.kind
            && self.borrowed_optional.contains(name)
            && self.nullable(e)
        {
            return Code::atom(format!("{}.map(String::from)", self.local(name)));
        }
        if self.nullable(e) {
            return self.expr(e);
        }
        let value = self.value(e, ty);
        Code::atom(format!("Some({})", value.text))
    }

    /// `let name: Option<T> = init;`, where `T` is the Rust type of `ty`.
    pub(super) fn optional_local(&mut self, name: &str, ty: Type, init: Option<&'a Expr>, mutable: bool) {
        let mutable = if mutable { "mut " } else { "" };
        let rust = self.local(name);
        let rust_ty = self.ty_name(ty);
        match init {
            None => self.line(&format!("let {mutable}{rust}: Option<{rust_ty}>;")),
            Some(init) => {
                let code = self.optional_to(init, ty);
                self.line(&format!("let {mutable}{rust}: Option<{rust_ty}> = {};", code.text));
            }
        }
    }

    /// `e` as an argument for an `Option<&str>` parameter.
    pub(super) fn option_arg(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "None".to_string(),
            _ if self.nullable(e) => self.option_ref(e),
            _ => format!("Some({})", self.str_ref(e)),
        }
    }

    /// The possibly-null variable `name` of type `ty` as an `Option<&str>`, or an `Option<&Rc<..>>`
    /// for an object.
    fn option_source(&self, name: &str, ty: Type) -> String {
        let rust = self.local(name);
        if matches!(ty, Type::Class(_) | Type::StringBuilder | Type::Enum(_)) {
            return format!("{rust}.as_ref()");
        }
        // An `Option<i32>` is `Copy` and matches as it is; a faithful box is `as_deref`'d.
        let plain_box = self.boxed.get(name).is_some_and(|&ty| !self.faithful_box(ty));
        if self.borrowed_optional.contains(name) || plain_box { rust } else { format!("{rust}.as_deref()") }
    }

    /// A possibly-null string as an `Option<&str>`.
    pub(super) fn option_ref(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            _ if matches!(e.ty, Type::Boxed(_)) => self.expr(e).text,
            _ if e.ty == Type::StringBuilder => self.builder_option(e),
            ExprKind::Null => "None::<&str>".to_string(),
            ExprKind::Var(name) => self.option_source(name, e.ty),
            ExprKind::Cast(_, inner) => self.option_ref(inner),
            // In a format, a field is borrowed until the text is formatted, after every operand,
            // so it is borrowed only when no operand has effects.
            ExprKind::Field { .. } | ExprKind::Index(..) if !self.formatting || self.lazy_text => {
                format!("{}.as_deref()", self.object_place(e))
            }
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
            let test = if matches!(op, BinOp::Ne) { "is_some" } else { "is_none" };
            let text = match &other.kind {
                // A variable or field is tested where it is, without a copy.
                ExprKind::Var(_) | ExprKind::Field { .. } | ExprKind::Index(..) => {
                    format!("{}.{test}()", self.object_place(other))
                }
                ExprKind::Static { class, name } if self.init_call(*class).is_none() => {
                    let cell = cell_name(&self.class.types[*class].name, name);
                    format!("{cell}.with_borrow(Option::{test})")
                }
                _ if matches!(other.ty, Type::Class(_) | Type::Array(_)) => {
                    format!("{}.{test}()", self.expr(other).at(ATOM))
                }
                _ => format!("{}.{test}()", self.option_ref(other)),
            };
            return Code::atom(text);
        } else if self.nullable(other) {
            let option = self.option_ref(other);
            let test = if matches!(op, BinOp::Ne) { "is_some" } else { "is_none" };
            return Code::atom(format!("{option}.{test}()"));
        } else if matches!(other.kind, ExprKind::Var(_) | ExprKind::Str(_)) {
            Code::atom("false")
        } else {
            let code = self.expr(other);
            Code::new(format!("{{ {}; false }}", code.text), ATOM)
        };
        if matches!(op, BinOp::Ne) { Code::new(format!("!{}", is_null.at(UNARY)), UNARY) } else { is_null }
    }

    /// `a.equals(b)` where one of the strings may be null: a null receiver throws, and a null
    /// argument is simply not equal. The operands are bound by a `match`, which keeps a
    /// temporary they borrow (an array element read by `java::at`) alive to the end of the test.
    pub(super) fn equals_optional(&mut self, args: &'a [Expr]) -> Code {
        // Java evaluates the argument before the call throws on a null receiver.
        if self.nullable(&args[0]) && has_effects(&args[1]) {
            let receiver = self.option_ref(&args[0]);
            let non_null = self.runtime("non_null");
            let (other, test) = if self.nullable(&args[1]) {
                (self.option_ref(&args[1]), format!("other_ == Some({non_null}(receiver_))"))
            } else {
                (self.value(&args[1], Type::String).text, format!("{non_null}(receiver_) == other_"))
            };
            return Code::new(
                format!("match ({receiver}, {other}) {{ (receiver_, other_) => {test} }}"),
                LOWEST,
            );
        }
        let receiver = if self.nullable(&args[0]) { self.deref(&args[0]) } else { self.str_value(&args[0]) };
        if self.nullable(&args[1]) {
            let option = self.option_ref(&args[1]);
            // A `match` is parenthesised where it is used as an operand, not in an `if` condition.
            let text =
                format!("match {receiver} {{ receiver_ => {option}.is_some_and(|o| o == receiver_) }}");
            return Code::new(text, LOWEST);
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
        if !nullable_type(e.ty) || !self.nullable(e) {
            return None;
        }
        if let Type::Enum(_) = e.ty {
            // A nullable enum local is an `Option` of the enum, shown as its name or as `null`.
            let option = match &e.kind {
                ExprKind::Var(name) if assigned.contains(name) => format!("{}.clone()", self.local(name)),
                _ => self.option_ref(e),
            };
            return Some(Part::Arg(Code::atom(format!("{}({option})", self.runtime("or_null")))));
        }
        if matches!(e.ty, Type::Boxed(_)) {
            return Some(Part::Arg(Code::atom(self.boxed_text(e, e.ty.unboxed()))));
        }
        if e.ty == Type::StringBuilder {
            let option = self.builder_option(e);
            let text = format!("{}({option})", self.runtime("or_null"));
            return Some(Part::Arg(Code::atom(if self.lazy_text {
                text
            } else {
                format!("{text}.to_string()")
            })));
        }
        if matches!(e.ty, Type::Class(_)) {
            // `java::or_null` shows `null`, or the object as it shows when it is not null.
            let option = format!("{}.as_ref()", self.object_place(e));
            let text = self.handle_text("o", e);
            let option = match text.strip_suffix("(o)") {
                _ if text == "o" => option,
                Some(function) if !function.contains(['.', '(']) => format!("{option}.map({function})"),
                _ => format!("{option}.map(|o| {text})"),
            };
            return Some(Part::Arg(Code::atom(format!("{}({option})", self.runtime("or_null")))));
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
    fn null_test_var(&self, cond: &'a Expr) -> Option<(&'a str, bool, Type)> {
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
        // An enum stays an `Option` of the enum: its null test does not bind it as a reference.
        if matches!(var.ty, Type::Enum(_)) || !self.nullable(var) {
            return None;
        }
        Some((name.as_str(), matches!(op, BinOp::Ne), var.ty))
    }

    /// `if (s != null && ..) { .. }`: see `let_chain`. And `if (s == null) .. else { .. }`: the
    /// else branch binds `s` as `if let Some(s) = s.as_deref()`, so `s` is a `&str` there. Returns
    /// false, emitting nothing, when the `if` is of neither form, or the branch where `s` is not
    /// null does not use it (a binding that is never read would only warn), or assigns it.
    pub(super) fn narrow_if(&mut self, cond: &'a Expr, then: &'a Stmt, otherwise: Option<&'a Stmt>) -> bool {
        if let Some((name, false, ty)) = self.null_test_var(cond)
            && let Some(non_null) = otherwise
        {
            if assigns_stmt(non_null, name) || !mentions_stmt(non_null, name) {
                return false;
            }
            self.line(&format!("if let Some({}) = {} {{", self.local(name), self.option_source(name, ty)));
            let narrowed = self.narrow(name);
            self.nested(non_null);
            self.unnarrow(narrowed);
            self.line("} else {");
            self.nested(then);
            self.line("}");
            return true;
        }
        let Some((cond_text, narrowed)) = self.let_chain(cond, &[then], &[]) else { return false };
        self.line(&format!("if {cond_text} {{"));
        self.nested(then);
        self.unnarrow(narrowed);
        if let Some(otherwise) = otherwise {
            self.line("} else {");
            self.nested(otherwise);
        }
        self.line("}");
        true
    }

    /// `cond` as a let chain that binds, as a `&str`, each possibly-null string tested with
    /// `s != null` in its top-level `&&` chain that is read later in the chain or in the guarded
    /// code (`stmts`, `exprs`) and assigned in neither:
    /// `n > 0 && let Some(s) = s.as_deref() && s.len() > 3`. The bound names stay narrowed for
    /// the guarded code; the caller ends that with `unnarrow`. `None`, emitting nothing, when no
    /// test binds.
    pub(super) fn let_chain(
        &mut self,
        cond: &'a Expr,
        stmts: &[&'a Stmt],
        exprs: &[&'a Expr],
    ) -> Option<(String, Vec<String>)> {
        let mut parts = Vec::new();
        conjuncts(cond, &mut parts);
        let binds: Vec<Option<(&str, Type)>> = parts
            .iter()
            .enumerate()
            .map(|(k, part)| {
                let (name, true, ty) = self.null_test_var(part)? else { return None };
                let later = parts[k + 1..].iter().chain(exprs);
                let assigned =
                    later.clone().any(|e| assigns(e, name)) || stmts.iter().any(|s| assigns_stmt(s, name));
                let used =
                    later.clone().any(|e| mentions(e, name)) || stmts.iter().any(|s| mentions_stmt(s, name));
                (used && !assigned).then_some((name, ty))
            })
            .collect();
        if binds.iter().all(Option::is_none) {
            return None;
        }
        let mut texts = Vec::new();
        let mut narrowed = Vec::new();
        for (part, bind) in parts.iter().zip(binds) {
            match bind {
                Some((name, ty)) => {
                    texts.push(format!("let Some({}) = {}", self.local(name), self.option_source(name, ty)));
                    narrowed.extend(self.narrow(name));
                }
                None => {
                    let text = self.expr(part).at(AND + 1);
                    texts.push(if text.starts_with('{') { format!("({text})") } else { text });
                }
            }
        }
        Some((texts.join(" && "), narrowed))
    }

    /// Treats the possibly-null `name` as a non-null `&str` from here on; returns it if it was not
    /// already, for `unnarrow`.
    fn narrow(&mut self, name: &str) -> Option<String> {
        self.borrowed.insert(name.to_string()).then(|| name.to_string())
    }

    pub(super) fn unnarrow(&mut self, names: impl IntoIterator<Item = String>) {
        for name in names {
            self.borrowed.remove(&name);
        }
    }

    /// `s != null && .. ? a : b` with a let chain, and `s == null ? a : b` as
    /// `if let Some(s) = s.as_deref() { b } else { a }`; `s == null ? "text" : s` as
    /// `s.as_deref().unwrap_or("text")`. `None` when `cond` narrows nothing.
    pub(super) fn narrowed_conditional(
        &mut self,
        e: &'a Expr,
        cond: &'a Expr,
        a: &'a Expr,
        b: &'a Expr,
    ) -> Option<Code> {
        let arm =
            |g: &mut Self, arm: &'a Expr| if g.nullable(e) { g.optional(arm) } else { g.value(arm, e.ty) };
        let (head, non_null, other, narrowed) = match self.null_test_var(cond) {
            Some((name, then_non_null, ty)) => {
                let (non_null, other) = if then_non_null { (a, b) } else { (b, a) };
                if assigns(non_null, name) || !mentions(non_null, name) {
                    return None;
                }
                // `s == null ? "text" : s`
                if matches!(&non_null.kind, ExprKind::Var(v) if v == name)
                    && let ExprKind::Str(text) = &other.kind
                    && !self.nullable(e)
                {
                    let source = self.option_source(name, ty);
                    return Some(Code::atom(format!(
                        "{source}.unwrap_or({}).to_string()",
                        rust_string(text)
                    )));
                }
                let head = format!("let Some({}) = {}", self.local(name), self.option_source(name, ty));
                (head, non_null, other, self.narrow(name).into_iter().collect())
            }
            None => {
                let (head, narrowed) = self.let_chain(cond, &[], &[a])?;
                (head, a, b, narrowed)
            }
        };
        let non_null = arm(self, non_null);
        self.unnarrow(narrowed);
        let other = arm(self, other);
        let text = format!("if {head} {{ {} }} else {{ {} }}", non_null.text, other.text);
        let mut code = Code::new(text, LOWEST);
        code.untyped = non_null.untyped && other.untyped;
        Some(code)
    }

    /// `if (s == null) return ..;` (or `break`, `continue`): the rest of the block sees `s` as
    /// non-null, so it is bound with `let Some(s) = s.as_deref() else { .. };`. Returns the name
    /// of `s`, or `None`, emitting nothing, when `stmt` is not of that form or `rest` assigns `s`.
    pub(super) fn early_null_exit(&mut self, stmt: &'a Stmt, rest: &[Stmt]) -> Option<&'a str> {
        let Stmt::If { cond, then, otherwise: None } = stmt else { return None };
        let (name, then_non_null, ty) = self.null_test_var(cond)?;
        if then_non_null
            || !exits(then)
            || assigns_any(rest, name)
            || !rest.iter().any(|s| mentions_stmt(s, name))
        {
            return None;
        }
        self.line(&format!("let Some({}) = {} else {{", self.local(name), self.option_source(name, ty)));
        self.nested(then);
        self.line("};");
        Some(name)
    }
}
