//! Strings that may be null, which are `Option<String>` in Rust. Which values are optional comes
//! from `analysis::maybe_null` and `analysis::nullable_var`; here is how each use is written.
//!
//! Narrowing conditions are here too: a null test that binds a string as a `&str`, and a pattern
//! `x instanceof T name`, which binds `name` with `if let Some(name) = <x as a T>`, in a let
//! chain, a `let .. else` or `is_some_and`.

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
        // A cast of a narrowed variable is not null either.
        if let ExprKind::Cast(_, value) = &e.kind
            && nullable_type(e.ty)
        {
            return self.nullable(value);
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
        if matches!(ty, Type::Class(..) | Type::StringBuilder | Type::Enum(_)) {
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
            ExprKind::Cast(_, inner) if !matches!(inner.ty, Type::Class(..)) => self.option_ref(inner),
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
                _ if matches!(other.ty, Type::Class(..) | Type::Array(_)) => {
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
        if matches!(e.ty, Type::Class(..)) {
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

/// `value instanceof T name`: its parts, when `e` is a pattern.
pub(super) fn pattern(e: &Expr) -> Option<(&Expr, Type, &str)> {
    match &e.kind {
        ExprKind::InstanceOf { value, ty, binding: Some(name) } => Some((value, *ty, name)),
        _ => None,
    }
}

/// `!(value instanceof T name)`: the pattern, when `e` is a negated one.
pub(super) fn negated_pattern(e: &Expr) -> Option<(&Expr, Type, &str)> {
    match &e.kind {
        ExprKind::Unary(UnOp::Not, inner) => pattern(inner),
        _ => None,
    }
}

/// Whether `stmt` always leaves the block it is in.
pub(super) fn exits(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(..) | Stmt::Break | Stmt::Continue | Stmt::Throw(_) => true,
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

    /// `if (s == null) .. else { .. }`: the else branch binds `s` as
    /// `if let Some(s) = s.as_deref()`, so `s` is a `&str` there; and `if (!(x instanceof T name))
    /// .. else { .. }`, whose else branch binds `name`. Returns false, emitting nothing, when the
    /// `if` is of neither form, or the else branch does not use the name (a binding that is never
    /// read would only warn), or assigns it. (`if (s != null && ..)` is a let chain, `let_chain`.)
    /// In tail position (`tail`), the branches are the function's value (see `tail.rs`).
    pub(super) fn narrow_if(
        &mut self,
        cond: &'a Expr,
        then: &'a Stmt,
        otherwise: Option<&'a Stmt>,
        tail: bool,
    ) -> bool {
        if !self.narrows_else(cond, otherwise) {
            return false;
        }
        let passed = otherwise.expect("an else branch");
        let narrowed = match negated_pattern(cond) {
            // `if (!(x instanceof T name)) .. else { .. }`
            Some((value, ty, name)) => {
                let source = self.pattern_source(value, ty);
                self.line(&format!("if let Some({}) = {source} {{", self.local(name)));
                None
            }
            None => {
                let (name, _, ty) = self.null_test_var(cond).expect("a null test");
                self.line(&format!(
                    "if let Some({}) = {} {{",
                    self.local(name),
                    self.option_source(name, ty)
                ));
                self.narrow(name)
            }
        };
        self.nested_in(passed, tail);
        self.unnarrow(narrowed);
        self.line("} else {");
        self.nested_in(then, tail);
        self.line("}");
        true
    }

    /// Whether `narrow_if` writes an `if` with this condition and else branch.
    pub(super) fn narrows_else(&self, cond: &'a Expr, otherwise: Option<&'a Stmt>) -> bool {
        let Some(passed) = otherwise else { return false };
        if let Some((_, _, name)) = negated_pattern(cond) {
            return mentions_stmt(passed, name);
        }
        matches!(self.null_test_var(cond), Some((name, false, _))
            if !assigns_stmt(passed, name) && mentions_stmt(passed, name))
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
        let used = |k: usize, name: &str| {
            parts[k + 1..].iter().chain(exprs).any(|e| mentions(e, name))
                || stmts.iter().any(|s| mentions_stmt(s, name))
        };
        let binds: Vec<Option<(&str, Type)>> = parts
            .iter()
            .enumerate()
            .map(|(k, part)| {
                let (name, true, ty) = self.null_test_var(part)? else { return None };
                let later = parts[k + 1..].iter().chain(exprs);
                let assigned =
                    later.clone().any(|e| assigns(e, name)) || stmts.iter().any(|s| assigns_stmt(s, name));
                (used(k, name) && !assigned).then_some((name, ty))
            })
            .collect();
        let patterns: Vec<bool> = parts
            .iter()
            .enumerate()
            .map(|(k, part)| pattern(part).is_some_and(|(_, _, name)| used(k, name)))
            .collect();
        if binds.iter().all(Option::is_none) && !patterns.contains(&true) {
            return None;
        }
        let mut texts = Vec::new();
        let mut narrowed = Vec::new();
        for ((part, bind), bound) in parts.iter().zip(binds).zip(patterns) {
            match bind {
                _ if bound => {
                    let (value, ty, name) = pattern(part).expect("a pattern");
                    let source = self.pattern_source(value, ty);
                    texts.push(format!("let Some({}) = {source}", self.local(name)));
                }
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
        let chain = std::mem::replace(&mut self.chain, false);
        let non_null = arm(self, non_null);
        self.unnarrow(narrowed);
        self.chain = matches!(other.kind, ExprKind::Conditional(..));
        let lines = chain || self.chain;
        let other = arm(self, other);
        self.chain = chain;
        let text = self.if_else(&head, &non_null.text, &other.text, lines);
        let mut code = Code::new(text, LOWEST);
        code.untyped = non_null.untyped && other.untyped;
        Some(code)
    }

    /// `a && x instanceof T name && b`, where a later operand reads `name`, as an expression:
    /// `a && <x as a T>.is_some_and(|name| b)`. `None` when no pattern of the chain binds.
    pub(super) fn bound_and(&mut self, e: &'a Expr) -> Option<Code> {
        let mut parts = Vec::new();
        conjuncts(e, &mut parts);
        let binds = |k: usize| {
            pattern(parts[k]).is_some_and(|(_, _, name)| parts[k + 1..].iter().any(|l| mentions(l, name)))
        };
        if !(0..parts.len()).any(binds) {
            return None;
        }
        Some(Code::new(self.and_chain(&parts), AND))
    }

    fn and_chain(&mut self, parts: &[&'a Expr]) -> String {
        let mut texts = Vec::new();
        for (k, part) in parts.iter().enumerate() {
            if let Some((value, ty, name)) = pattern(part)
                && parts[k + 1..].iter().any(|l| mentions(l, name))
            {
                let source = self.pattern_source(value, ty);
                let rest = self.and_chain(&parts[k + 1..]);
                texts.push(format!("{source}.is_some_and(|{}| {rest})", self.local(name)));
                break;
            }
            texts.push(self.expr(part).at(AND + 1));
        }
        texts.join(" && ")
    }

    /// `if (!(x instanceof T name)) return ..;` (or `break`, `continue`): the rest of the block
    /// sees `name`, bound with `let Some(name) = <x as a T> else { .. };`. Returns false, emitting
    /// nothing, when `stmt` is not of that form or the rest does not read `name`.
    pub(super) fn early_pattern_exit(&mut self, stmt: &'a Stmt, rest: &[Stmt]) -> bool {
        let Stmt::If { cond, then, otherwise: None } = stmt else { return false };
        let Some((value, ty, name)) = negated_pattern(cond) else { return false };
        if !exits(then) || !rest.iter().any(|s| mentions_stmt(s, name)) {
            return false;
        }
        let source = self.pattern_source(value, ty);
        self.line(&format!("let Some({}) = {source} else {{", self.local(name)));
        self.nested(then);
        self.line("};");
        true
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
