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
}

impl Kind<'_> {
    /// Whether Rust's unlabeled `break` and `continue` reach a frame of this kind.
    fn is_loop(self) -> bool {
        match self {
            Kind::Loop(_) => true,
            Kind::Switch { machine } | Kind::Value { machine, .. } => machine,
            Kind::Block => false,
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
    matches!(
        body.last(),
        Some(Stmt::Break | Stmt::Continue | Stmt::Return(..) | Stmt::BreakLabel(_) | Stmt::ContinueLabel(_) | Stmt::Yield(_))
    )
}

/// A case's statements without the `break` that ends an arrow case or a case that leaves anyway.
fn strip(body: &[Stmt]) -> &[Stmt] {
    match body.split_last() {
        Some((Stmt::Break, rest)) => rest,
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
        _ => false,
    }
}

/// Statements that an unlabeled `break` inside them leaves rather than the enclosing switch.
fn is_breakable(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::For { .. } | Stmt::ForEach { .. } | Stmt::Switch { .. }
    )
}

fn is_default(case: &Case) -> bool {
    case.labels.iter().any(|l| matches!(l, CaseLabel::Default))
}

/// The cases in the order of Rust match arms: the default last, since `_` matches everything.
fn arms(cases: &[Case]) -> Vec<&Case> {
    cases.iter().filter(|c| !is_default(c)).chain(cases.iter().filter(|c| is_default(c))).collect()
}

/// The Rust pattern of a case's labels; a case with a `default` label matches everything else.
fn pattern(case: &Case) -> String {
    if is_default(case) {
        return "_".to_string();
    }
    let values: Vec<String> = case
        .labels
        .iter()
        .filter_map(|l| match l {
            CaseLabel::Value(e) => Some(literal(e)),
            CaseLabel::Default => None,
        })
        .collect();
    values.join(" | ")
}

/// A case label's value: lowering keeps only literals, which are numbers or strings.
fn literal(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Int(i) => i.to_string(),
        ExprKind::Char(c) => c.to_string(),
        ExprKind::Str(s) => rust_string(s),
        _ => unreachable!("lowering keeps only literal case labels"),
    }
}

/// The number of `yield`s in the cases, including those in nested statements.
fn yield_count(cases: &[Case]) -> usize {
    let mut n = 0;
    for case in cases {
        for stmt in &case.body {
            visit_stmt(stmt, &mut |_| {}, &mut |s| n += usize::from(matches!(s, Stmt::Yield(_))));
        }
    }
    n
}

impl<'a> Gen<'a> {
    /// Opens a frame for the statement whose header was just written at the current indent.
    pub(super) fn open(&mut self, kind: Kind<'a>) {
        let line_start = self.out[..self.out.len() - 1].rfind('\n').map_or(0, |i| i + 1);
        let java = self.pending.take();
        self.frames.push(Frame { kind, java, name: None, at: line_start + 4 * self.indent });
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
        if cont {
            match self.frames[index].kind {
                Kind::Loop(Loop::For(update)) => {
                    for u in update {
                        self.effect(u);
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
            text.push_str(&value);
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
            None => self.line(&format!("{} '{};", if cont { "continue" } else { "break" }, snake_case(label))),
        }
    }

    /// A `yield`: leaves the innermost switch expression with the value.
    pub(super) fn yield_value(&mut self, value: &'a Expr) {
        let target = self.frames.iter().rposition(|f| matches!(f.kind, Kind::Value { .. }));
        let Some(index) = target else { return self.line("break;") };
        let Kind::Value { ty, .. } = self.frames[index].kind else { unreachable!() };
        let code = self.value(value, ty);
        self.jump(index, false, Some(code.text));
    }

    /// A labeled statement. Loops and switches take the label as their own; any other statement
    /// becomes a labeled block, which only a `break` with its label leaves.
    pub(super) fn labeled(&mut self, label: &str, body: &'a Stmt) {
        match body {
            Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::For { .. } | Stmt::ForEach { .. } | Stmt::Switch { .. } => {
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

    /// A switch statement, or a switch expression when `value` is its type.
    pub(super) fn switch_form(&mut self, selector: &'a Expr, cases: &'a [Case], value: Option<Type>) {
        let labeled = self.pending.is_some();
        let falls = cases.split_last().is_some_and(|(_, init)| init.iter().any(|c| !leaves(&c.body)));
        let text = if selector.ty != Type::String {
            self.value(selector, selector.ty).typed()
        } else if self.nullable(selector) {
            // `deref` is a `&str` that throws on null.
            self.deref(selector)
        } else if let ExprKind::Var(name) = &selector.kind
            && self.borrowed.contains(name)
        {
            self.local(name)
        } else {
            format!("&*{}", self.expr(selector).at(UNARY))
        };
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
            self.matched(&text, cases);
        }
    }

    /// A switch expression, as a block that ends with its value.
    pub(super) fn switch_expr(&mut self, ty: Type, selector: &'a Expr, cases: &'a [Case]) -> Code {
        let mark = self.out.len();
        let indent = self.indent;
        let pending = self.pending.take();
        self.indent += 1;
        self.switch_form(selector, cases, Some(ty));
        self.indent = indent;
        self.pending = pending;
        let body = self.out.split_off(mark);
        Code::atom(format!("{{\n{body}{}}}", "    ".repeat(indent)))
    }

    /// No case falls through and none leaves early: `match`, with `_ => {}` when there is no default.
    fn matched(&mut self, text: &str, cases: &'a [Case]) {
        self.line(&format!("match {text} {{"));
        self.indent += 1;
        for case in arms(cases) {
            self.line(&format!("{} => {{", pattern(case)));
            self.indent += 1;
            self.stmts(strip(&case.body), false);
            self.indent -= 1;
            self.line("}");
        }
        if !cases.iter().any(is_default) {
            self.line("_ => {}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// Some case leaves early: a labeled block around the `match`, which the `break`s name.
    fn block(&mut self, text: &str, cases: &'a [Case], value: Option<Type>) {
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
            self.line(&format!("{} => {{", pattern(case)));
            self.indent += 1;
            let body = if value.is_some() { &case.body[..] } else { strip(&case.body) };
            self.stmts(body, false);
            self.indent -= 1;
            self.line("}");
        }
        if value.is_none() && !cases.iter().any(is_default) {
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
        self.line(&format!("match {text} {{"));
        self.indent += 1;
        for case in arms(cases) {
            let Some((Stmt::Yield(value), init)) = case.body.split_last() else { unreachable!() };
            self.line(&format!("{} => {{", pattern(case)));
            self.indent += 1;
            self.stmts(init, false);
            let code = self.value(value, ty);
            self.line(&code.text);
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// Some case falls into the next: a state machine. `case` is the index of the case to run; a
    /// case that does not leave sets it to the next one, and the loop runs the cases in order.
    fn machine(&mut self, text: &str, cases: &'a [Case], value: Option<Type>) {
        self.line("{");
        self.indent += 1;
        self.line(&format!("let mut case: usize = match {text} {{"));
        self.indent += 1;
        for case in arms(cases) {
            let index = cases.iter().position(|c| std::ptr::eq(c, case)).unwrap_or(0);
            self.line(&format!("{} => {index},", pattern(case)));
        }
        if !cases.iter().any(is_default) {
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
            if !leaves(&case.body) {
                if i + 1 < cases.len() {
                    self.line(&format!("case = {};", i + 1));
                } else {
                    self.line(&format!("{fallout};"));
                }
            }
            self.indent -= 1;
            self.line("}");
        }
        self.line(&format!("_ => {fallout},"));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.close();
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }
}
