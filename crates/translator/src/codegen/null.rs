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
        // The value of an assignment to a field that may be null is the assigned value, an
        // `Option` only where that may be null (see `update`), so a narrowed variable
        // assigned there is not null either.
        if let ExprKind::Assign { op: None, target: Place::Field { .. } | Place::Static { .. }, value } =
            &e.kind
            && !matches!(e.ty, Type::Boxed(_))
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
        nullable_type(self.ret) && self.facts.nullable_returns.contains(&self.method)
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
            return Code::atom(format!("{}.map(ToOwned::to_owned)", self.local(name)));
        }
        if self.nullable(e) {
            return self.expr(e);
        }
        // A static still being initialized may be null (see `analysis::nullness`).
        if let ExprKind::Static { class, name } = &e.kind
            && ty == e.ty
            && let Some(read) = self.placeholder_read(*class, name, ty)
        {
            return read;
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
            None if self.try_assigned.contains(name) => {
                // As for a local of `exceptions::default_value`: null, as a field starts.
                self.line("#[allow(unused_assignments)]");
                self.line(&format!("let {mutable}{rust}: Option<{rust_ty}> = None;"));
            }
            None => self.line(&format!("let {mutable}{rust}: Option<{rust_ty}>;")),
            Some(init) => {
                let code = self.optional_to(init, ty);
                self.line(&format!("let {mutable}{rust}: Option<{rust_ty}> = {};", code.text));
            }
        }
    }

    /// `e` as an argument for an `Option<&str>` parameter. A borrowed `java::JString` names its
    /// type, `&java::Wtf8`, as `Some(&s)` would be an `Option<&JString>` where nothing asks for
    /// the text (`Some(&S) == key.as_deref()`).
    pub(super) fn option_arg(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "None".to_string(),
            _ if self.nullable(e) => self.option_ref(e),
            _ => {
                let text = self.str_ref(e);
                if e.ty == Type::String && text.starts_with('&') {
                    format!("Some::<&java::Wtf8>({text})")
                } else {
                    format!("Some({text})")
                }
            }
        }
    }

    /// The possibly-null variable `name` of type `ty` as an `Option<&str>`, or an `Option<&Rc<..>>`
    /// for an object.
    fn option_source(&self, name: &str, ty: Type) -> String {
        let rust = self.local(name);
        if matches!(ty, Type::Class(..) | Type::StringBuilder | Type::Enum(_) | Type::Array(_)) {
            return format!("{rust}.as_ref()");
        }
        // A charset is `Copy`, and so is an `Option` of one.
        if ty == Type::Charset {
            return rust;
        }
        // An `Option<i32>` is `Copy` and matches as it is, and so does an `Option` of a class; a
        // faithful box is `as_deref`'d.
        let plain_box = self.boxed.get(name).is_some_and(|&ty| !self.faithful_box(ty));
        if self.borrowed_optional.contains(name) || plain_box || ty == Type::JavaClass {
            rust
        } else {
            format!("{rust}.as_deref()")
        }
    }

    /// The possibly-null variable `name` as the `Option` that a null test binds: an array is its
    /// own handle (`.clone()`), which the runtime's array functions take by reference, where
    /// `as_ref()` would give a reference to a reference.
    fn narrowed_source(&self, name: &str, ty: Type) -> String {
        match ty {
            Type::Array(_) => format!("{}.clone()", self.local(name)),
            // A shared string is bound as the object, a `&java::JStr`.
            Type::String if super::shared_strings() && !self.borrowed_optional.contains(name) => {
                format!("{}.as_ref()", self.local(name))
            }
            _ => self.option_source(name, ty),
        }
    }

    /// A possibly-null string as an `Option<&str>`.
    pub(super) fn option_ref(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            _ if matches!(e.ty, Type::Boxed(_)) => self.expr(e).text,
            // A class is `Copy`, so an `Option` of one is too, and so is a charset.
            _ if matches!(e.ty, Type::JavaClass | Type::Charset) => self.expr(e).at(ATOM),
            // So is an enum, read from a field, an element or a call.
            _ if matches!(e.ty, Type::Enum(_)) && !matches!(e.kind, ExprKind::Var(_)) => {
                self.expr(e).at(ATOM)
            }
            _ if e.ty == Type::StringBuilder => self.builder_option(e),
            ExprKind::Null => "None::<&java::Wtf8>".to_string(),
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
    /// `NullPointerException` (in the checked form where a `catch` or a `finally` sees it, see
    /// `dereference`).
    pub(super) fn deref(&mut self, e: &'a Expr) -> String {
        let option = self.option_ref(e);
        self.dereference(&[e.id], &option)
    }

    /// An enum value that a call, a builtin or a switch uses: one that may be null is an `Option`
    /// of the enum, which is `Copy`, unwrapped where it is used, so that null throws
    /// `NullPointerException` (in the checked form where a `catch` or a `finally` sees it, see
    /// `dereference`).
    pub(super) fn enum_value(&mut self, e: &'a Expr) -> Code {
        if !matches!(e.ty, Type::Enum(_)) || !self.nullable(e) {
            return self.expr(e);
        }
        let option = self.expr(e).at(ATOM);
        Code::atom(self.dereference(&[e.id], &option))
    }

    /// `x == null` and `x != null` for a string.
    pub(super) fn null_test(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let other = if matches!(l.kind, ExprKind::Null) { r } else { l };
        let is_null = if matches!(other.kind, ExprKind::Null) {
            Code::atom("true")
        } else if let ExprKind::Cast(to @ Type::Enum(_), value) = &other.kind
            && matches!(value.ty, Type::Class(..))
            && self.nullable(value)
        {
            // A cast to an enum of a value that may be null, such as an element of an enum's
            // array that is an `Object[]` (`lower::covariant`): the checked cast as an `Option`,
            // which is `None` for null.
            let test = if matches!(op, BinOp::Ne) { "is_some" } else { "is_none" };
            let option = self.checked_class_cast(other.id, *to, value);
            return Code::atom(format!("{}.{test}()", option.at(ATOM)));
        } else if self.nullable(other) {
            let test = if matches!(op, BinOp::Ne) { "is_some" } else { "is_none" };
            let text = match &other.kind {
                // A variable or field is tested where it is, without a copy.
                ExprKind::Var(_) | ExprKind::Field { .. } | ExprKind::Index(..) => {
                    format!("{}.{test}()", self.object_place(other))
                }
                ExprKind::Static { class, name } if self.init_call(*class).is_none() => {
                    let cell = self.cell_name(*class, name);
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
            // Only the operand's effects are kept: an assignment is made as a statement, as its
            // value (the variable, moved out) is not needed.
            let effect = match &other.kind {
                ExprKind::Assign { op, target: Place::Var(target), value }
                    if self.plain(target).is_none() =>
                {
                    self.assignment(*op, target, other.ty, value, other.id)
                }
                _ => self.expr(other).text,
            };
            Code::new(format!("{{ {effect}; false }}"), ATOM)
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
            let checked = self.dereference(&[args[0].id], "receiver_");
            let (other, test) = if self.nullable(&args[1]) {
                (self.option_ref(&args[1]), format!("other_ == Some({checked})"))
            } else {
                (self.value(&args[1], Type::String).text, format!("{checked} == other_"))
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
            // The receiver is bound by reference, which moves no string out of the object or local
            // it is read from.
            let text = format!(
                "match {receiver} {{ ref receiver_ => {option}.is_some_and(|o| *o == **receiver_) }}"
            );
            return Code::new(text, LOWEST);
        }
        let other = self.str_value(&args[1]);
        infix("==", CMP, &Code::atom(receiver), &Code::atom(other))
    }

    /// `s += value` where `s` may be null: a null `s` prints as `null`.
    pub(super) fn append_optional(&mut self, name: &str, value: &'a Expr) -> String {
        let null = self.wtf8_literal("null");
        let mut parts = vec![Part::Text(Code::atom(format!("{name}.as_deref().unwrap_or({null})")))];
        self.parts(value, &mut parts);
        let text = self.concat(parts).text;
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
        if matches!(e.ty, Type::JavaClass | Type::Charset) {
            let option = self.expr(e).at(ATOM);
            return Some(Part::Arg(Code::atom(format!("{}({option})", self.runtime("or_null")))));
        }
        if let Type::Enum(_) = e.ty {
            // A nullable enum local is an `Option` of the enum, shown as its text or as `null`.
            let option = match &e.kind {
                ExprKind::Var(name) if assigned.contains(name) => format!("{}.clone()", self.local(name)),
                _ => self.option_ref(e),
            };
            return Some(Part::Text(Code::atom(format!("{}({option})", self.runtime("or_null")))));
        }
        if matches!(e.ty, Type::Boxed(_)) {
            return Some(Part::Arg(Code::atom(self.boxed_text(e, e.ty.unboxed()))));
        }
        if e.ty == Type::StringBuilder {
            let option = self.builder_option(e);
            let text = format!("{}({option})", self.runtime("or_null"));
            if self.lazy_text {
                return Some(Part::Text(Code::atom(text)));
            }
            let null = self.wtf8_literal("null");
            return Some(Part::Text(Code::atom(format!(
                "{option}.map_or_else(|| {null}.to_owned(), |b| b.text())"
            ))));
        }
        if matches!(e.ty, Type::Class(..)) {
            // `java::or_null` shows `null`, or the object as it shows when it is not null.
            let option = format!("{}.as_ref()", self.object_place(e));
            let (text, raises) = self.handle_text("o", e);
            if raises {
                // A `toString` that throws: its exception goes on from here, not from the closure.
                let texts = self.passed_on(Code::atom(format!("{option}.map(|o| {text}).transpose()")));
                return Some(Part::Text(Code::atom(format!("{}({texts})", self.runtime("or_null")))));
            }
            let option = match text.strip_suffix("(o)") {
                // An object shown through its `Display`.
                _ if text == "o" => {
                    return Some(Part::Arg(Code::atom(format!("{}({option})", self.runtime("or_null")))));
                }
                Some(function) if !function.contains(['.', '(']) => format!("{option}.map({function})"),
                _ => format!("{option}.map(|o| {text})"),
            };
            return Some(Part::Text(Code::atom(format!("{}({option})", self.runtime("or_null")))));
        }
        let text = match &e.kind {
            // An operand that assigns the variable is read before the assignment, as a copy.
            ExprKind::Var(name) if assigned.contains(name) => {
                format!("{}.clone().unwrap_or_else(|| \"null\".into())", self.local(name))
            }
            _ => format!("{}.unwrap_or({})", self.option_ref(e), self.wtf8_literal("null")),
        };
        Some(Part::Text(Code::atom(text)))
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
                self.line(&format!("if let {} = {source} {{", self.pattern_binding(name, &source, ty)));
                None
            }
            None => {
                let (name, _, ty) = self.null_test_var(cond).expect("a null test");
                self.line(&format!(
                    "if let Some({}) = {} {{",
                    self.local(name),
                    self.narrowed_source(name, ty)
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
                    texts.push(format!("let {} = {source}", self.pattern_binding(name, &source, ty)));
                }
                Some((name, ty)) => {
                    texts.push(format!(
                        "let Some({}) = {}",
                        self.local(name),
                        self.narrowed_source(name, ty)
                    ));
                    narrowed.extend(self.narrow(name));
                }
                // A `Boolean` is unboxed, which throws on a null, as in any condition.
                None => {
                    let text = self.value(part, Type::Boolean).at(AND + 1);
                    texts.push(if text.starts_with('{') { format!("({text})") } else { text });
                }
            }
        }
        Some((texts.join(" && "), narrowed))
    }

    /// Treats the possibly-null `name` as a non-null `&str` from here on; returns it if it was not
    /// already, for `unnarrow`.
    pub(super) fn narrow(&mut self, name: &str) -> Option<String> {
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
                    if super::shared_strings() {
                        let source = self.narrowed_source(name, ty);
                        let literal = self.string_literal(&rust_string(text));
                        return Some(Code::atom(format!("{source}.cloned().unwrap_or_else(|| {literal})")));
                    }
                    let source = self.option_source(name, ty);
                    return Some(Code::atom(format!(
                        "{source}.unwrap_or({}).to_owned()",
                        self.wtf8_literal(text)
                    )));
                }
                let head = format!("let Some({}) = {}", self.local(name), self.narrowed_source(name, ty));
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
        // The unboxing of a box that may be null is the failure of the `&&` it is an operand of,
        // which the chain does not write as one: inside the closure of `e` the whole chain is
        // caught; else such an operand runs inside `java::catching` on its own.
        let mut caught = HashMap::new();
        unboxing_ands(e, &self.facts.catching, &mut caught);
        if self.catching_now.contains(&e.id) {
            self.dereferenced.extend(caught.drain().map(|(_, and)| and));
        }
        Some(Code::new(self.and_chain(&parts, &caught), AND))
    }

    /// `parts`, joined by `&&`; `caught` maps an operand that unboxes to its `&&` of
    /// `Facts::catching`.
    fn and_chain(&mut self, parts: &[&'a Expr], caught: &HashMap<ExprId, ExprId>) -> String {
        let mut texts = Vec::new();
        for (k, part) in parts.iter().enumerate() {
            if let Some((value, ty, name)) = pattern(part)
                && parts[k + 1..].iter().any(|l| mentions(l, name))
            {
                let source = self.pattern_source(value, ty);
                let rest = self.and_chain(&parts[k + 1..], caught);
                let name = self.local(name);
                // A failure that goes on from the rest (`?`, `or_break!`) cannot leave a closure.
                texts.push(if rest.contains('?') || rest.contains("or_break!") {
                    format!("match {source} {{ Some({name}) => {rest}, None => false }}")
                } else {
                    format!("{source}.is_some_and(|{name}| {rest})")
                });
                break;
            }
            let code = match caught.get(&part.id) {
                Some(&and) => {
                    let mark = self.out.len();
                    let code = self.value(part, Type::Boolean);
                    self.caught(and, code, mark)
                }
                None => self.value(part, Type::Boolean),
            };
            // A block would end a `match` arm or a statement that it starts.
            texts.push(code.left_at(AND + 1));
        }
        texts.join(" && ")
    }

    /// `if (!(x instanceof T name)) return ..;` (or `break`, `continue`, `throw`): the rest of the
    /// block sees `name`, bound with `let Some(name) = <x as a T> else { .. };`. Returns false, emitting
    /// nothing, when `stmt` is not of that form or the rest does not read `name`.
    pub(super) fn early_pattern_exit(&mut self, stmt: &'a Stmt, rest: &[Stmt]) -> bool {
        let Stmt::If { cond, then, otherwise: None } = stmt else { return false };
        let Some((value, ty, name)) = negated_pattern(cond) else { return false };
        if !exits(then) || !rest.iter().any(|s| mentions_stmt(s, name)) {
            return false;
        }
        let source = self.pattern_source(value, ty);
        let binding = self.pattern_binding(name, &source, ty);
        self.line(&format!("let {binding} = {} else {{", let_else_source(source)));
        self.nested(then);
        self.line("};");
        true
    }

    /// `if (s == null) return ..;` (or `break`, `continue`): the rest of the block sees `s` as
    /// non-null, so it is bound with `let Some(s) = s.as_deref() else { .. };` (`s.clone()` for an
    /// array). Returns the name
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
        let source = let_else_source(self.narrowed_source(name, ty));
        self.line(&format!("let Some({}) = {source} else {{", self.local(name)));
        self.nested(then);
        self.line("};");
        Some(name)
    }
}

/// The value of a `let .. else`, in parentheses when it ends with a block, which Rust doesn't
/// take before `else`.
fn let_else_source(source: String) -> String {
    if source.ends_with('}') { format!("({source})") } else { source }
}

/// The operands of the `&&`s of a chain that unbox a box, of those `&&`s in `catching`, each
/// mapped to its `&&`.
fn unboxing_ands(e: &Expr, catching: &HashMap<ExprId, Pos>, out: &mut HashMap<ExprId, ExprId>) {
    let ExprKind::Binary(BinOp::And, l, r) = &e.kind else { return };
    for operand in [l, r] {
        if catching.contains_key(&e.id) && matches!(operand.ty, Type::Boxed(_)) {
            out.insert(operand.id, e.id);
        }
        unboxing_ands(operand, catching, out);
    }
}
