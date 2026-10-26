//! Switch statements and expressions, labeled statements and the jumps between them.
//!
//! Java's `break` and `continue` name the statement they leave; Rust's unlabeled ones name the
//! innermost loop. So every Java statement that a jump can leave is a frame on `Gen.frames`. A jump
//! is unlabeled only when its target is the innermost frame and a Rust loop; otherwise it names the
//! target's label, which is written in front of the target's header when the frame closes, and only
//! if a jump named it.
//!
//! A switch takes the plainest form that is equivalent: a `match` when no case falls through and no
//! `break` leaves it early; a labeled block when a `break` leaves it early; a state machine when a
//! case falls into the next one.

use super::exceptions::completes_all;
use super::*;

/// What a frame is in Java, and how Rust runs it.
#[derive(Clone, Copy)]
pub(super) enum Kind<'a> {
    /// A Java loop, which is a Rust loop.
    Loop(Loop<'a>),
    /// A switch statement: `break` leaves it. A state machine is a Rust loop, a block otherwise.
    Switch { machine: bool },
    /// A switch expression: `yield` leaves it with a value of type `ty`.
    Value { machine: bool, ty: Type },
    /// A labeled statement that is not a loop: a labeled Rust block, left only by its label.
    Block,
    /// The body of a `try` or a `catch` clause written as a labeled Rust block, which an
    /// exception (and, with a `finally`, a jump or a `return`) leaves by its label: a jump out of
    /// it to a loop must name the loop.
    Try,
}

impl Kind<'_> {
    /// Whether Rust's unlabeled `break` and `continue` reach a frame of this kind.
    fn is_loop(self) -> bool {
        match self {
            Kind::Loop(_) => true,
            Kind::Switch { machine } | Kind::Value { machine, .. } => machine,
            Kind::Block | Kind::Try => false,
        }
    }
}

pub(super) struct Frame<'a> {
    kind: Kind<'a>,
    /// The Java label, if the statement has one.
    java: Option<String>,
    /// The Rust label, given when a jump first names the frame.
    name: Option<String>,
    /// Where the frame's header starts, after its indentation.
    at: usize,
}

/// Whether a case's statements always leave the switch (so they do not fall into the next case).
fn leaves(body: &[Stmt]) -> bool {
    match body.last() {
        Some(Stmt::Block(inner)) => leaves(inner),
        last => matches!(
            last,
            Some(
                Stmt::Break
                    | Stmt::Continue
                    | Stmt::Return(..)
                    | Stmt::BreakLabel(_)
                    | Stmt::ContinueLabel(_)
                    | Stmt::Yield(_)
                    | Stmt::Throw(_)
            )
        ),
    }
}

/// A case's statements without the `break` that ends an arrow case or a case that leaves anyway.
fn strip(body: &[Stmt]) -> &[Stmt] {
    match body.split_last() {
        Some((Stmt::Break, rest)) => rest,
        // `case x: { ...; break; }` is the statements of its braces.
        Some((Stmt::Block(inner), [])) => strip(inner),
        _ => body,
    }
}

/// Whether a statement has a `break` that leaves the switch around it (not one inside a loop or a
/// nested switch, and not one with a label).
fn breaks_out(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Break => true,
        Stmt::Block(stmts) => stmts.iter().any(breaks_out),
        Stmt::If { then, otherwise, .. } => breaks_out(then) || otherwise.as_deref().is_some_and(breaks_out),
        Stmt::Labeled { body, .. } => !is_breakable(body) && breaks_out(body),
        Stmt::Try { body, catches, finally } => {
            body.iter().any(breaks_out)
                || catches.iter().any(|c| c.body.iter().any(breaks_out))
                || finally.iter().flatten().any(breaks_out)
        }
        _ => false,
    }
}

/// Statements that an unlabeled `break` inside them leaves rather than the enclosing switch.
fn is_breakable(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::While { .. }
            | Stmt::DoWhile { .. }
            | Stmt::For { .. }
            | Stmt::ForEach { .. }
            | Stmt::Switch { .. }
    )
}

fn is_default(case: &Case) -> bool {
    case.labels.iter().any(|l| matches!(l, CaseLabel::Default))
}

/// The cases in the order of Rust match arms: the default last, since `_` matches everything.
fn arms(cases: &[Case]) -> Vec<&Case> {
    cases.iter().filter(|c| !is_default(c)).chain(cases.iter().filter(|c| is_default(c))).collect()
}

/// Whether a switch on a `char` matches Rust chars: every label is a char that is a Unicode
/// scalar value, so `char::from_u32` of the selector is `Some` of the label exactly when the
/// selector equals it, and a lone surrogate, `None`, matches only the default.
fn char_cases(selector: &Expr, cases: &[Case]) -> bool {
    selector.ty == Type::Char
        && cases.iter().flat_map(|c| &c.labels).all(|l| match l {
            CaseLabel::Value(Expr { kind: ExprKind::Char(c), .. }) => char::from_u32(u32::from(*c)).is_some(),
            CaseLabel::Value(_) => false,
            CaseLabel::Default => true,
        })
}

/// A case label's value: lowering keeps only literals, which are numbers or strings.
/// Whether the cases match every value of the selector without a default: a switch on an enum
/// that names each constant. Rust's `match` then needs no `_` arm (and warns about one).
fn exhaustive(class: &Class, selector: &Expr, cases: &[Case]) -> bool {
    let Type::Enum(name) = selector.ty else { return false };
    let Some(decl) = class.types.iter().find(|t| t.name == name) else { return false };
    let count = decl.enum_constants.as_ref().map_or(0, Vec::len);
    let named: HashSet<usize> = cases
        .iter()
        .flat_map(|c| &c.labels)
        .filter_map(|l| match l {
            CaseLabel::Value(Expr { kind: ExprKind::EnumConstant { ordinal, .. }, .. }) => Some(*ordinal),
            _ => None,
        })
        .collect();
    named.len() == count
}

/// The number of `yield`s in the cases, including those in nested statements.
fn yield_count(cases: &[Case]) -> usize {
    let mut n = 0;
    for case in cases {
        for stmt in &case.body {
            visit_stmt_outside_switches(stmt, &mut |s| n += usize::from(matches!(s, Stmt::Yield(_))));
        }
    }
    n
}

impl<'a> Gen<'a> {
    /// The Rust pattern of a case's labels; a case with a `default` label matches everything
    /// else. With `chars`, the selector is an `Option<char>` and the labels are char literals.
    fn pattern(&self, case: &Case, chars: bool) -> String {
        if is_default(case) {
            return "_".to_string();
        }
        let values: Vec<String> = case
            .labels
            .iter()
            .filter_map(|l| match l {
                CaseLabel::Value(Expr { kind: ExprKind::Char(c), .. }) if chars => {
                    Some(super::primitives::rust_char(*c).expect("char cases are scalar values"))
                }
                CaseLabel::Value(e) => Some(self.literal(e)),
                CaseLabel::Default => None,
            })
            .collect();
        if chars { format!("Some({})", values.join(" | ")) } else { values.join(" | ") }
    }

    fn literal(&self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(i) => i.to_string(),
            ExprKind::Char(c) => c.to_string(),
            ExprKind::Str(s) => byte_string(s.as_bytes()),
            ExprKind::Units(units) => byte_string(&super::strings::wtf8_of(units)),
            ExprKind::EnumConstant { class, ordinal } => self.constant_path(*class, *ordinal),
            ExprKind::Static { class, name } => self.cell_name(*class, name),
            _ => unreachable!("lowering keeps only constant case labels"),
        }
    }

    /// Opens a frame for the statement whose header was just written at the current indent.
    pub(super) fn open(&mut self, kind: Kind<'a>) {
        let line_start = self.out[..self.out.len() - 1].rfind('\n').map_or(0, |i| i + 1);
        let java = self.pending.take();
        self.frames.push(Frame { kind, java, name: None, at: line_start + 4 * self.indent });
    }

    /// Opens a frame for a block whose header, just written, ends with its `{`, where its label
    /// goes: `let attempt: T = {`.
    pub(super) fn open_block(&mut self, kind: Kind<'a>) {
        let at = self.out.len() - 2;
        debug_assert_eq!(&self.out[at..], "{\n");
        self.frames.push(Frame { kind, java: None, name: None, at });
    }

    /// Gives the frame at `index` the Rust label `name`, which a `break` then names.
    pub(super) fn name_frame(&mut self, index: usize, name: &str) {
        self.frames[index].name = Some(name.to_string());
    }

    /// Closes the innermost frame, writing its label in front of its header if a jump named it.
    pub(super) fn close(&mut self) {
        let frame = self.frames.pop().expect("a frame to close");
        if let Some(name) = frame.name {
            self.out.insert_str(frame.at, &format!("'{name}: "));
        }
    }

    /// The Rust label of the frame at `index`, which the jump then names.
    fn label(&mut self, index: usize) -> String {
        if let Some(name) = &self.frames[index].name {
            return name.clone();
        }
        let name = match &self.frames[index].java {
            Some(java) => snake_case(java),
            None => {
                self.labels += 1;
                let prefix = if matches!(self.frames[index].kind, Kind::Loop(_)) { "loop" } else { "sw" };
                format!("{prefix}{}", self.labels)
            }
        };
        self.frames[index].name = Some(name.clone());
        name
    }

    /// A jump to the frame at `index`: a `break`, or a `continue` after the loop's own steps, or a
    /// `break` that gives the switch expression `value`.
    pub(super) fn jump(&mut self, index: usize, cont: bool, value: Option<String>) {
        if self.jump_through_finally(index, cont) {
            return;
        }
        if cont {
            match self.frames[index].kind {
                Kind::Loop(Loop::For(update)) => {
                    for u in update {
                        self.update_effect(u);
                    }
                }
                Kind::Loop(Loop::DoWhile(cond)) => {
                    // The condition is tested before the next round, as the loop's own end does.
                    let negated = self.negate(cond);
                    let name = self.label(index);
                    self.line(&format!("if {negated} {{"));
                    self.indent += 1;
                    self.line(&format!("break '{name};"));
                    self.indent -= 1;
                    self.line("}");
                }
                _ => {}
            }
        }
        let kind = self.frames[index].kind;
        let mut text = if cont { "continue" } else { "break" }.to_string();
        if index + 1 != self.frames.len() || !kind.is_loop() {
            text.push_str(&format!(" '{}", self.label(index)));
        }
        if let Some(value) = value {
            text.push(' ');
            // `break 'label { .. } as T` reads as a block after the label: Rust wants it wrapped.
            if value.starts_with('{') && text.contains('\'') {
                text.push_str(&format!("({value})"));
            } else {
                text.push_str(&value);
            }
        }
        text.push(';');
        self.line(&text);
    }

    /// An unlabeled Java `break` or `continue`: the innermost loop, or switch for `break`.
    pub(super) fn plain_jump(&mut self, cont: bool) {
        let target = self.frames.iter().rposition(|f| match f.kind {
            Kind::Loop(_) => true,
            Kind::Switch { .. } => !cont,
            _ => false,
        });
        match target {
            Some(index) => self.jump(index, cont, None),
            None => self.line(if cont { "continue;" } else { "break;" }),
        }
    }

    /// A `break` or `continue` with a label.
    pub(super) fn labeled_jump(&mut self, label: &str, cont: bool) {
        let target = self.frames.iter().rposition(|f| f.java.as_deref() == Some(label));
        match target {
            Some(index) => self.jump(index, cont, None),
            None => {
                self.line(&format!("{} '{};", if cont { "continue" } else { "break" }, snake_case(label)))
            }
        }
    }

    /// Whether the frame at `index` is a switch expression, which a `yield` leaves with a value.
    pub(super) fn is_value_frame(&self, index: usize) -> bool {
        matches!(self.frames[index].kind, Kind::Value { .. })
    }

    /// A `yield`: leaves the innermost switch expression with the value.
    pub(super) fn yield_value(&mut self, value: &'a Expr) {
        let target = self.frames.iter().rposition(|f| matches!(f.kind, Kind::Value { .. }));
        let Some(index) = target else { return self.line("break;") };
        let Kind::Value { ty, .. } = self.frames[index].kind else { unreachable!() };
        let code = if self.optional_yield { self.optional(value) } else { self.value(value, ty) };
        if self.leaves_finally(index) {
            // The `finally` runs before the switch expression ends, so the value waits in a local
            // that `switch_expr` declares; the `try` passes it on when it reaches the switch.
            if !self.yield_slots.contains(&index) {
                self.yield_slots.push(index);
            }
            self.line(&format!("yielded{index} = Some({});", code.text));
            self.jump(index, false, None);
            return;
        }
        self.jump(index, false, Some(code.text));
    }

    /// A labeled statement. Loops and switches take the label as their own; any other statement
    /// becomes a labeled block, which only a `break` with its label leaves.
    pub(super) fn labeled(&mut self, label: &str, body: &'a Stmt) {
        match body {
            Stmt::While { .. }
            | Stmt::DoWhile { .. }
            | Stmt::For { .. }
            | Stmt::ForEach { .. }
            | Stmt::Switch { .. } => {
                self.pending = Some(label.to_string());
                self.stmt(body, &[]);
            }
            _ => {
                self.line("{");
                self.pending = Some(label.to_string());
                self.open(Kind::Block);
                self.nested(body);
                self.line("}");
                self.close();
            }
        }
    }

    /// Whether a switch, as the last statement of a function body, is the function's value: no
    /// case falls through or leaves early, every case ends in a value, and there is a default (or
    /// every constant of the enum), so the `match` has no `_ => {}` arm.
    pub(super) fn tail_switch(&self, selector: &Expr, cases: &[Case]) -> bool {
        let falls = cases.split_last().is_some_and(|(_, init)| init.iter().any(|c| !leaves(&c.body)));
        !falls
            && !cases.iter().any(|c| strip(&c.body).iter().any(breaks_out))
            && arms(cases).iter().all(|c| strip(&c.body).last().is_some_and(|s| self.tail_value(s)))
            && (cases.iter().any(is_default) || exhaustive(self.class, selector, cases))
    }

    /// A switch statement, or a switch expression when `value` is its type. `tail`: the switch is
    /// the function's value (see `tail_switch`), and its arms end in tail expressions.
    pub(super) fn switch_form(
        &mut self,
        selector: &'a Expr,
        cases: &'a [Case],
        value: Option<Type>,
        tail: bool,
    ) {
        if let [case] = cases
            && is_default(case)
            && matches!(selector.kind, ExprKind::Int(_))
        {
            return self.default_only(&case.body, value);
        }
        let labeled = self.pending.is_some();
        self.chars = char_cases(selector, cases);
        self.exhaustive = exhaustive(self.class, selector, cases);
        let falls = cases.split_last().is_some_and(|(_, init)| init.iter().any(|c| !leaves(&c.body)));
        // A `Ref` made by the selector would live through the whole `match`, while the cases may
        // borrow the same object mutably: end each borrow inside the selector.
        let outer = self.guarded;
        self.guarded |= self.reads_through_borrow(selector);
        let text = if self.chars {
            let c = self.value(selector, Type::Char).text;
            format!("char::from_u32(u32::from({c}))")
        } else if matches!(selector.ty, Type::Enum(_)) {
            // An enum that may be null throws `NullPointerException` there, as Java's does.
            self.enum_value(selector).text
        } else if selector.ty != Type::String {
            self.value(selector, selector.ty).typed()
        } else if self.nullable(selector) && super::shared_strings() {
            // The bytes of the `&JStr` that throws on null.
            let option = format!("{}.as_ref()", self.expr(selector).at(ATOM));
            format!("{}.as_bytes()", self.dereference(&[selector.id], &option))
        } else if self.nullable(selector) {
            // `deref` is a `&Wtf8` that throws on null.
            format!("{}.as_bytes()", self.deref(selector))
        } else if let ExprKind::Var(name) = &selector.kind {
            // A string's text is matched as bytes, which a byte string literal is.
            format!("{}.as_bytes()", self.local(name))
        } else {
            format!("{}.as_bytes()", self.expr(selector).at(ATOM))
        };
        self.guarded = outer;
        let yields_last = cases.iter().all(|c| matches!(c.body.last(), Some(Stmt::Yield(_))));
        if falls {
            self.machine(&text, cases, value);
        } else if let Some(ty) = value
            && yields_last
            && yield_count(cases) == cases.len()
        {
            self.value_match(&text, cases, ty);
        } else if labeled || value.is_some() || cases.iter().any(|c| strip(&c.body).iter().any(breaks_out)) {
            self.block(&text, cases, value);
        } else {
            self.matched(&text, cases, tail);
        }
    }

    /// A switch on a constant with only a `default`, which holds the `if` chain of a pattern
    /// switch (see `lower::patterns`): a block, which a `break` (or, for a switch expression, a
    /// `yield`) leaves by its label.
    fn default_only(&mut self, body: &'a [Stmt], value: Option<Type>) {
        self.line("{");
        self.open(match value {
            Some(ty) => Kind::Value { machine: false, ty },
            None => Kind::Switch { machine: false },
        });
        self.indent += 1;
        self.stmts(body, false);
        self.indent -= 1;
        self.line("}");
        self.close();
    }

    /// A switch expression, as a block that ends with its value: an `Option` when `optional`.
    pub(super) fn switch_expr(
        &mut self,
        ty: Type,
        optional: bool,
        selector: &'a Expr,
        cases: &'a [Case],
    ) -> Code {
        let outer = std::mem::replace(&mut self.optional_yield, optional);
        let code = self.switch_value(ty, selector, cases);
        self.optional_yield = outer;
        code
    }

    fn switch_value(&mut self, ty: Type, selector: &'a Expr, cases: &'a [Case]) -> Code {
        let mark = self.out.len();
        let indent = self.indent;
        let pending = self.pending.take();
        // The switch's own frame is the first one that `switch_form` opens.
        let index = self.frames.len();
        self.indent += 1;
        self.switch_form(selector, cases, Some(ty), false);
        self.indent = indent;
        self.pending = pending;
        let mut body = self.out.split_off(mark);
        if let Some(slot) = self.yield_slots.iter().position(|&i| i == index) {
            self.yield_slots.swap_remove(slot);
            // The initial `None` is overwritten before any read on every path that reads the slot.
            let pad = "    ".repeat(indent + 1);
            let declare =
                format!("#[allow(unused_assignments)]\n{pad}let mut yielded{index}: Option<_> = None;");
            body = format!("{pad}{declare}\n{body}");
        }
        let text = format!("{{\n{body}{}}}", "    ".repeat(indent));
        // A switch that yields nothing, as one that only throws, has the type `!`.
        if yield_count(cases) > 0 { Code::atom(text) } else { self.never_of(text, ty) }
    }

    /// No case falls through and none leaves early: `match`, with `_ => {}` when there is no default.
    fn matched(&mut self, text: &str, cases: &'a [Case], tail: bool) {
        let chars = std::mem::take(&mut self.chars);
        let exhaustive = std::mem::take(&mut self.exhaustive);
        // One label, and nothing for any other value: `if let`, as clippy's `single_match` asks.
        let nothing_else = match cases.iter().find(|c| is_default(c)) {
            Some(default) => strip(&default.body).is_empty(),
            None => !exhaustive,
        };
        let labeled: Vec<&Case> = cases.iter().filter(|c| !is_default(c)).collect();
        if let [case] = labeled.as_slice()
            && nothing_else
        {
            self.line(&format!("if let {} = {text} {{", self.pattern(case, chars)));
            self.indent += 1;
            self.stmts(strip(&case.body), tail);
            self.indent -= 1;
            self.line("}");
            return;
        }
        self.line(&format!("match {text} {{"));
        self.indent += 1;
        for case in arms(cases) {
            self.line(&format!("{} => {{", self.pattern(case, chars)));
            self.indent += 1;
            self.stmts(strip(&case.body), tail);
            self.indent -= 1;
            self.line("}");
        }
        if !cases.iter().any(is_default) && !exhaustive {
            self.line("_ => {}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// Some case leaves early: a labeled block around the `match`, which the `break`s name.
    fn block(&mut self, text: &str, cases: &'a [Case], value: Option<Type>) {
        let chars = std::mem::take(&mut self.chars);
        let exhaustive = std::mem::take(&mut self.exhaustive);
        self.line("{");
        let kind = match value {
            Some(ty) => Kind::Value { machine: false, ty },
            None => Kind::Switch { machine: false },
        };
        self.open(kind);
        self.indent += 1;
        self.line(&format!("match {text} {{"));
        self.indent += 1;
        for case in arms(cases) {
            self.line(&format!("{} => {{", self.pattern(case, chars)));
            self.indent += 1;
            let body = if value.is_some() { &case.body[..] } else { strip(&case.body) };
            self.stmts(body, false);
            self.indent -= 1;
            self.line("}");
        }
        if value.is_none() && !cases.iter().any(is_default) && !exhaustive {
            self.line("_ => {}");
        }
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.close();
        self.line("}");
    }

    /// A switch expression whose every case ends with its value: a `match` whose arms are those
    /// values.
    fn value_match(&mut self, text: &str, cases: &'a [Case], ty: Type) {
        let chars = std::mem::take(&mut self.chars);
        self.line(&format!("match {text} {{"));
        self.indent += 1;
        for case in arms(cases) {
            let Some((Stmt::Yield(value), init)) = case.body.split_last() else { unreachable!() };
            self.line(&format!("{} => {{", self.pattern(case, chars)));
            self.indent += 1;
            self.yielded(init, value, ty);
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// Some case falls into the next: a state machine. `case` is the index of the case to run; a
    /// case that does not leave sets it to the next one, and the loop runs the cases in order.
    fn machine(&mut self, text: &str, cases: &'a [Case], value: Option<Type>) {
        let chars = std::mem::take(&mut self.chars);
        let exhaustive = std::mem::take(&mut self.exhaustive);
        self.line("{");
        self.indent += 1;
        // `case` is assigned only by a case that falls on; without one it need not be `mut`.
        let falls = cases.iter().enumerate().any(|(i, c)| i + 1 < cases.len() && completes_all(&c.body));
        let mutable = if falls { "mut " } else { "" };
        self.line(&format!("let {mutable}case: usize = match {text} {{"));
        self.indent += 1;
        for case in arms(cases) {
            let index = cases.iter().position(|c| std::ptr::eq(c, case)).unwrap_or(0);
            self.line(&format!("{} => {index},", self.pattern(case, chars)));
        }
        let matches_all = cases.iter().any(is_default) || exhaustive;
        if !matches_all {
            self.line(&format!("_ => {},", cases.len()));
        }
        self.indent -= 1;
        self.line("};");
        self.line("loop {");
        // A value machine ends only by a `yield`, which breaks with the value: any other way out
        // of its cases is one that javac rejects, so the loop's fall-out is unreachable.
        let kind = match value {
            Some(ty) => Kind::Value { machine: true, ty },
            None => Kind::Switch { machine: true },
        };
        let fallout = if value.is_some() { "unreachable!()" } else { "break" };
        self.open(kind);
        self.indent += 1;
        self.line("match case {");
        self.indent += 1;
        for (i, case) in cases.iter().enumerate() {
            self.line(&format!("{i} => {{"));
            self.indent += 1;
            self.stmts(&case.body, false);
            // A case that cannot complete, such as a `try` whose every path yields, does not fall on.
            if completes_all(&case.body) {
                if i + 1 < cases.len() {
                    self.line(&format!("case = {};", i + 1));
                } else {
                    self.line(&format!("{fallout};"));
                }
            }
            self.indent -= 1;
            self.line("}");
        }
        // `case` is the index of a case, or one past the last where no case matched: with a
        // `default` or every constant covered, never that, so Rust sees that every way out of the
        // loop ran a case (and what it assigned).
        let unmatched = if matches_all { "unreachable!()" } else { fallout };
        self.line(&format!("_ => {unmatched},"));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.close();
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }
}

/// A string's WTF-8 bytes as a Rust byte string literal, for matching a string's bytes.
pub(super) fn byte_string(bytes: &[u8]) -> String {
    let mut out = String::from("b\"");
    for &b in bytes {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b' '..=b'~' => out.push(b as char),
            b'\n' => out.push_str("\\n"),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out.push('"');
    out
}
