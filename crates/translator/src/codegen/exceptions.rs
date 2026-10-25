//! Exceptions as values (see `analysis::exceptions` for which methods throw).
//!
//! A method that throws returns `Result<T, Throwable>`, where `Throwable` is the enum of the
//! program's exception classes: `return v` is `return Ok(v)`, `throw e` is `return Err(e)`, and a
//! call of a method that throws is `call?`. A method that does not throw calls one that does with
//! `call.unwrap_or_else(java::uncaught)`: nothing that escapes there is caught, so the exception
//! ends the program at once, as Java's would.
//!
//! A live `try` (one whose `catch` clauses catch something its body may throw) is a labeled block
//! that evaluates to `Result<(), Throwable>`: inside it, a failure leaves the block with
//! `java::or_break!(call, 'attempt)` and `throw e` is `break 'attempt Err(e)`. The `catch` clauses
//! are the arms of a `match` on the result, in Java's order, each matching the variants of the
//! classes it catches that no earlier clause caught; what no clause catches goes on as from any
//! other point of the method. A `try` that catches nothing its body throws is a plain block.
//!
//! A `try` with a `finally` runs its body (and the `catch` clause that matches what the body
//! threw) as labeled blocks that evaluate to how they ended: `Result<(), Throwable>` when only an
//! exception can leave them early, else `java::Completion`, which also records a `return` (with
//! its value) and a `break` or `continue` out of the `try` (numbered). The `finally`'s statements
//! follow the blocks, and then what ended them happens: the exception goes on, the method returns,
//! the jump is made. A `try` that nothing leaves early is its body followed by the `finally`.

use super::*;

/// Whether a statement can complete normally, so that code after it runs: Java's rules,
/// conservatively (a statement not known to end abruptly completes), which is also when the Rust
/// it becomes can.
pub(super) fn completes(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(..)
        | Stmt::Throw(_)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::BreakLabel(_)
        | Stmt::ContinueLabel(_)
        | Stmt::Yield(_) => false,
        Stmt::Expr(Expr {
            kind:
                ExprKind::Builtin(
                    Builtin::Exit
                    | Builtin::NoObject { .. }
                    | Builtin::UnknownClass
                    | Builtin::UpdaterRefused
                    | Builtin::MissingResource,
                    _,
                ),
            ..
        }) => false,
        Stmt::Block(stmts) => completes_all(stmts),
        Stmt::If { then, otherwise: Some(otherwise), .. } => completes(then) || completes(otherwise),
        Stmt::While { cond, body } if matches!(cond.kind, ExprKind::Bool(true)) => breaks_out_of(body, None),
        Stmt::For { cond: None, body, .. } => breaks_out_of(body, None),
        Stmt::Labeled { label, body } => match &**body {
            Stmt::While { cond, body } if matches!(cond.kind, ExprKind::Bool(true)) => {
                breaks_out_of(body, Some(label))
            }
            Stmt::For { cond: None, body, .. } => breaks_out_of(body, Some(label)),
            other => completes(other) || breaks_out_of(other, Some(label)),
        },
        Stmt::Try { body, catches, finally } => {
            (completes_all(body) || catches.iter().any(|c| completes_all(&c.body)))
                && finally.as_deref().is_none_or(completes_all)
        }
        // A switch with a `default` whose last group ends abruptly, and that no `break` leaves,
        // as javac's Lower ends its code of an operator with a `return` in each case.
        Stmt::Switch { cases, .. } => {
            let default = cases.iter().any(|c| c.labels.iter().any(|l| matches!(l, CaseLabel::Default)));
            !default
                || cases.last().is_none_or(|c| completes_all(&c.body))
                || cases.iter().any(|c| c.body.iter().any(|s| breaks_out_of(s, None)))
        }
        _ => true,
    }
}

/// Whether a statement list can complete normally.
pub(super) fn completes_all(stmts: &[Stmt]) -> bool {
    stmts.last().is_none_or(completes)
}

/// Whether `stmt`, the body of a loop, has a `break` that leaves the loop: an unlabeled one outside
/// nested loops and switches, or one naming the loop's `label`.
fn breaks_out_of(stmt: &Stmt, label: Option<&str>) -> bool {
    fn walk(s: &Stmt, label: Option<&str>, nested: bool) -> bool {
        match s {
            Stmt::Break => !nested,
            Stmt::BreakLabel(l) => Some(l.as_str()) == label,
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::For { body, .. } => walk(body, label, true),
            Stmt::Switch { cases, .. } => cases.iter().flat_map(|c| &c.body).any(|b| walk(b, label, true)),
            Stmt::If { then, otherwise, .. } => {
                walk(then, label, nested) || otherwise.as_deref().is_some_and(|o| walk(o, label, nested))
            }
            Stmt::Block(stmts) => stmts.iter().any(|b| walk(b, label, nested)),
            Stmt::Labeled { body, .. } => walk(body, label, nested),
            Stmt::Try { body, catches, finally } => body
                .iter()
                .chain(catches.iter().flat_map(|c| &c.body))
                .chain(finally.iter().flatten())
                .any(|b| walk(b, label, nested)),
            _ => false,
        }
    }
    walk(stmt, label, false)
}

/// The locals that a live `try` block assigns and that are read outside it: Rust cannot tell that
/// one declared without a value is assigned on every path to a read after the `try`, as Java can,
/// so it is given one. A local declared inside the block (`for (Object e; ..)` in
/// `ArrayList.batchRemove`), or read only there (`script` in `CharPredicates.forUnicodeScript`),
/// is read where Rust follows its assignments as Java does.
pub(super) fn try_assigned(facts: &Facts, body: &[Stmt]) -> HashSet<String> {
    // `all`: the method's body, for the reads of a local outside the block.
    fn assigned(all: &[Stmt], stmts: &[Stmt], out: &mut HashSet<String>) {
        let mut found = HashSet::new();
        let mut declared = HashSet::new();
        for s in stmts {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Assign { target: Place::Var(name), .. }
                        | ExprKind::IncDec { target: Place::Var(name), .. } = &x.kind
                        {
                            found.insert(name.clone());
                        }
                    })
                },
                &mut |s| {
                    if let Stmt::Local { name, .. } | Stmt::ForEach { name, .. } = s {
                        declared.insert(name.clone());
                    }
                },
            );
        }
        out.extend(
            found
                .into_iter()
                .filter(|name| !declared.contains(name) && reads(all, name) > reads(stmts, name)),
        );
    }
    // The reads of local `name` in `stmts` (and of other locals of that name).
    fn reads(stmts: &[Stmt], name: &str) -> usize {
        let mut count = 0;
        for s in stmts {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Var(v)
                        | ExprKind::Assign { target: Place::Var(v), op: Some(_), .. }
                        | ExprKind::IncDec { target: Place::Var(v), .. }
                            if v == name =>
                        {
                            count += 1
                        }
                        _ => {}
                    })
                },
                &mut |_| {},
            );
        }
        count
    }
    // Finds the `try` statements of `stmts`, including those in the cases of a switch expression,
    // which `visit_stmt` reaches only through an expression.
    fn scan(facts: &Facts, all: &[Stmt], stmts: &[Stmt], out: &mut HashSet<String>) {
        for s in stmts {
            let mut nested = HashSet::new();
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Switch { cases, .. } = &x.kind {
                            for case in cases {
                                scan(facts, all, &case.body, &mut nested);
                            }
                        }
                    })
                },
                &mut |s| match s {
                    Stmt::Try { body, finally: None, .. }
                        if facts.live_tries.contains(&(s as *const Stmt as usize)) =>
                    {
                        assigned(all, body, out)
                    }
                    // The body and the `catch` clauses of a `try` with a `finally` are blocks whose
                    // ends Rust does not follow into the code after the `finally`, unless nothing
                    // leaves them early.
                    Stmt::Try { body, catches, finally: Some(_) }
                        if facts.raising_tries.contains(&(s as *const Stmt as usize))
                            || leaves_early(body, catches) != (false, false) =>
                    {
                        assigned(all, body, out);
                        for catch in catches {
                            assigned(all, &catch.body, out);
                        }
                    }
                    _ => {}
                },
            );
            out.extend(nested);
        }
    }
    let mut out = HashSet::new();
    scan(facts, body, body, &mut out);
    out
}

/// The value Java gives a field of type `ty`, for a local that `try_assigned` names; `None` for a
/// type whose locals are not given one yet.
pub(super) fn default_value(ty: Type) -> Option<&'static str> {
    Some(match ty {
        Type::Int | Type::Long | Type::Byte | Type::Short | Type::Char => "0",
        Type::Double | Type::Float => "0.0",
        Type::Boolean => "false",
        Type::String if super::shared_strings() => "java::JStr::default()",
        Type::String => "java::JString::new()",
        _ => return None,
    })
}

/// Whether a statement of `body` itself, not one nested in it, sets local `name`: once `body`
/// completes normally, the local holds a value.
fn sets_at_top(body: &[Stmt], name: &str) -> bool {
    body.iter().any(|s| {
        matches!(s, Stmt::Expr(e)
            if matches!(&e.kind, ExprKind::Assign { op: None, target: Place::Var(v), .. } if v == name))
    })
}

/// Whether some code of `stmts` assigns local `name`.
fn assigns(stmts: &[Stmt], name: &str) -> bool {
    let mut found = false;
    for s in stmts {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::Assign { target: Place::Var(v), .. }
                    | ExprKind::IncDec { target: Place::Var(v), .. } = &x.kind
                        && v == name
                    {
                        found = true;
                    }
                })
            },
            &mut |_| {},
        );
    }
    found
}

/// The locals that the block `body` of a `try` without `finally` carries out in its `Ok` value
/// (`Ok((a, b))`), for the `match` on it to set them: those of `unset` that a statement of the
/// block itself sets, which then hold a value wherever the block completes normally, as Java's
/// definite assignment has them when the code after the `try` reads them.
pub(super) fn carried_out(body: &[Stmt], unset: &HashMap<String, Type>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in body {
        if let Stmt::Expr(e) = s
            && let ExprKind::Assign { op: None, target: Place::Var(v), .. } = &e.kind
            && unset.contains_key(v)
            && !out.contains(v)
        {
            out.push(v.clone());
        }
    }
    if completes_all(body) { out } else { Vec::new() }
}

/// A Rust tuple of `items`: `(a,)` of one.
fn tuple(items: &[String]) -> String {
    match items {
        [one] => format!("({one},)"),
        _ => format!("({})", items.join(", ")),
    }
}

/// Whether every `try` of `body` that assigns local `name` carries its value out where Rust needs
/// it (`carried_out`): it has no `finally`, and its block sets the local with a statement of its
/// own or never completes normally. The `try` statements in the cases of a switch expression are
/// looked at too.
fn carriable(body: &[Stmt], name: &str) -> bool {
    let ok = std::cell::Cell::new(true);
    for s in body {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::Switch { cases, .. } = &x.kind
                        && !cases.iter().all(|case| carriable(&case.body, name))
                    {
                        ok.set(false);
                    }
                })
            },
            &mut |s| {
                if let Stmt::Try { body, catches, finally } = s
                    && (assigns(body, name) || catches.iter().any(|c| assigns(&c.body, name)))
                    && !(finally.is_none()
                        && (!assigns(body, name) || sets_at_top(body, name) || !completes_all(body)))
                {
                    ok.set(false);
                }
            },
        );
    }
    ok.get()
}

/// A local of the method that `try_assigned` names (`assigned`) whose type has no value to start
/// as (`starts`, see `Gen::try_start`), that is not an `Option` (`optional`), which starts as
/// `None`, and that not every `try` that assigns it can carry out (`carriable`): not supported
/// yet.
pub(super) fn check_try_assigned(
    body: &[Stmt],
    assigned: &HashSet<String>,
    starts: impl Fn(Type) -> bool,
    optional: impl Fn(Type, &str) -> bool,
) -> Option<Error> {
    let mut found = None;
    for s in body {
        visit_stmt(s, &mut |_| {}, &mut |s| {
            if let Stmt::Local { ty, name, init: None, pos } = s
                && assigned.contains(name)
                && !starts(*ty)
                && !optional(*ty, name)
                && !carriable(body, name)
                && found.is_none()
            {
                found = Some(Error {
                    pos: *pos,
                    message: format!(
                        "a local of type `{ty}` declared without a value and assigned in a `try` block not supported yet"
                    ),
                });
            }
        });
    }
    found
}

/// Checks that no `?` or `java::or_break!` of the generated code is inside a closure, where it
/// would not leave the method or the `try` block. The generator writes some expressions as
/// closures (an update of an array element or a static field), and a call that throws inside one
/// is not supported yet.
pub(super) fn check_closures(code: &str) -> Result<(), Error> {
    let bytes = code.as_bytes();
    // The closures being read, each by the bracket depth of its body and whether the body is a
    // block (which ends with its bracket) or an expression (which ends at a comma or a bracket).
    let mut closures: Vec<(usize, bool)> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    let fail =
        || error(Pos::default(), "a call that throws inside a closure of the translation not supported yet");
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'\'' if bytes.get(i + 1) == Some(&b'\\') || bytes.get(i + 2) == Some(&b'\'') => {
                // A char literal such as '?' or '\n' (a label or a lifetime has no closing quote).
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                // A block body ends with its bracket; an expression body with the one around it.
                closures.retain(|&(d, _)| d < depth);
                depth = depth.saturating_sub(1);
            }
            b',' | b';' => closures.retain(|&(d, block)| block || d < depth),
            b'|' => {
                let before = code[..i].trim_end();
                let head = before.ends_with(['(', ',', '=', '{']) || before.ends_with(" move");
                if head && let Some(end) = code[i + 1..].find('|') {
                    i += end + 2;
                    // A closure that returns a `Result` (as `caught` writes) is left by its `?`.
                    if code[i..].trim_start().starts_with("-> Result<") {
                        continue;
                    }
                    let block = code[i..].trim_start().starts_with('{');
                    closures.push((if block { depth + 1 } else { depth }, block));
                    continue;
                }
            }
            b'?' if !closures.is_empty() => return fail(),
            b'o' if !closures.is_empty() && code[i..].starts_with("or_break!") => return fail(),
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

/// A `try` block being written, which an exception (and, with a `finally`, a `return` or a jump)
/// leaves by its label.
pub(super) struct Attempt {
    /// The label it gets when something leaves it.
    label: String,
    /// The frame (index into `Gen::frames`) of the block, which writes the label when it is used.
    frame: usize,
    /// Whether the block evaluates to a `java::Completion` (the body or a `catch` clause of a
    /// `try` with a `finally` that a `return` or a jump leaves) rather than a `Result`.
    completion: bool,
}

/// A `try` with a `finally` whose body or `catch` clauses are being written: the returns and
/// jumps out of it run the `finally` first, and then happen.
pub(super) struct Finally {
    /// The frame of its body and of its `catch` clauses (they have the same index).
    frame: usize,
    /// The jumps out of it, as the frames they go to and whether they `continue`; a jump is
    /// `java::Completion::Jump(n)` with its index here.
    jumps: Vec<(usize, bool)>,
}

/// Whether a `return`, or a `break`, `continue` or `yield` to a statement outside them, leaves the
/// statements.
fn jumps_out(stmts: &[Stmt]) -> (bool, bool) {
    // `labels`: the labels of the statements around the one being walked, inside `stmts`.
    fn walk(s: &Stmt, in_loop: bool, in_switch: bool, labels: &mut Vec<String>, out: &mut (bool, bool)) {
        match s {
            Stmt::Return(..) => out.0 = true,
            Stmt::Break if !in_loop && !in_switch => out.1 = true,
            Stmt::Continue if !in_loop => out.1 = true,
            Stmt::BreakLabel(l) | Stmt::ContinueLabel(l) if !labels.contains(l) => out.1 = true,
            Stmt::Yield(_) => out.1 = true,
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::For { body, .. } => walk(body, true, in_switch, labels, out),
            Stmt::Switch { cases, .. } => {
                for b in cases.iter().flat_map(|c| &c.body) {
                    walk(b, in_loop, true, labels, out);
                }
            }
            Stmt::If { then, otherwise, .. } => {
                walk(then, in_loop, in_switch, labels, out);
                if let Some(o) = otherwise {
                    walk(o, in_loop, in_switch, labels, out);
                }
            }
            Stmt::Block(stmts) => stmts.iter().for_each(|b| walk(b, in_loop, in_switch, labels, out)),
            Stmt::Labeled { label, body } => {
                labels.push(label.clone());
                walk(body, in_loop, in_switch, labels, out);
                labels.pop();
            }
            Stmt::Try { body, catches, finally } => {
                for b in
                    body.iter().chain(catches.iter().flat_map(|c| &c.body)).chain(finally.iter().flatten())
                {
                    walk(b, in_loop, in_switch, labels, out);
                }
            }
            _ => {}
        }
    }
    let mut out = (false, false);
    for s in stmts {
        walk(s, false, false, &mut Vec::new(), &mut out);
    }
    out
}

/// Whether a `return`, and whether a jump, leaves the body or a `catch` clause of a `try`.
fn leaves_early(body: &[Stmt], catches: &[Catch]) -> (bool, bool) {
    let (mut returns, mut jumps) = jumps_out(body);
    for catch in catches {
        let (r, j) = jumps_out(&catch.body);
        returns |= r;
        jumps |= j;
    }
    (returns, jumps)
}

impl<'a> Gen<'a> {
    /// The Rust type of an exception: the enum of `Throwable`.
    pub(super) fn throwable_type(&mut self) -> String {
        self.ty_name(Type::class("java.lang.Throwable"))
    }

    /// Whether the program has exceptions: `Throwable` is among its classes.
    pub(super) fn has_throwable(&self) -> bool {
        self.class.type_index("java.lang.Throwable").is_some()
    }

    /// The runtime's function `name` that ends the program with an uncaught exception, which
    /// shows the exception through its `Display`.
    pub(super) fn uncaught(&mut self, name: &str) -> String {
        let t = self.class.type_index("java.lang.Throwable").expect("a program that throws has `Throwable`");
        if let Some(t) = self.enum_for(t) {
            self.need(Need::Display(t));
        }
        // A static initializer's exception ends the program as `ExceptionInInitializerError`.
        if self.class.is_initializer(self.method) {
            let initializer =
                if name == "throw_uncaught" { "throw_initializer" } else { "initializer_uncaught" };
            return self.runtime(initializer);
        }
        self.runtime(name)
    }

    /// The label of the innermost `try` block, which writing it uses; and whether the block is
    /// a `java::Completion`.
    fn innermost_attempt(&mut self) -> Option<(String, bool)> {
        let a = self.attempts.last()?;
        let (label, frame, completion) = (a.label.clone(), a.frame, a.completion);
        self.name_frame(frame, &label);
        Some((label, completion))
    }

    /// The label of the innermost `try` block that is a `java::Completion`, which writing it uses.
    fn completion_attempt(&mut self) -> String {
        let a = self.attempts.iter().rev().find(|a| a.completion).expect("a `try` with a `finally` is open");
        let (label, frame) = (a.label.clone(), a.frame);
        self.name_frame(frame, &label);
        label
    }

    /// A label for a `try` block or a `catch` clause of the current method: `attempt`, `attempt2`.
    fn attempt_label(&mut self, base: &str) -> String {
        self.attempt_count += 1;
        match self.attempt_count {
            1 => base.to_string(),
            n => format!("{base}{n}"),
        }
    }

    /// The value with which exception `e` leaves a `try` block: `Err(e)`, or
    /// `java::Completion::Throw(e)`.
    fn thrown(&mut self, completion: bool, e: &str) -> String {
        if completion { format!("{}({e})", self.runtime("Completion::Throw")) } else { format!("Err({e})") }
    }

    /// The call `code` of method `callee`: as it is when the method does not throw, else with
    /// what its failure does here (see the module's documentation).
    /// The value that a local that `try_assigned` names starts as, which Java's definite
    /// assignment never lets the code read: the value Java would give a field, an empty array,
    /// the first constant of an enum, or the zero of a plain box; `None` for a type without one.
    pub(super) fn try_start(&self, ty: Type) -> Option<String> {
        match ty {
            Type::Array(_) => Some(self.placeholder(ty, false)),
            Type::Enum(name) => {
                let t = self.class.type_index(name)?;
                let constants = self.class.types[t].enum_constants.as_deref().unwrap_or_default();
                (!constants.is_empty()).then(|| self.placeholder(ty, false))
            }
            Type::Boxed(_) if !self.faithful_box(ty) => default_value(ty.unboxed()).map(str::to_string),
            _ => default_value(ty).map(str::to_string),
        }
    }

    pub(super) fn raising(&mut self, code: Code, callee: usize) -> Code {
        if !self.facts.throwing.contains(&callee) {
            return code;
        }
        Code::atom(self.passed_on(code))
    }

    /// A `Result` whose error is a `Throwable` (`code`), whose `Ok` value is used and whose error
    /// goes on from here: it leaves the enclosing `try` block or the method, or ends the program.
    pub(super) fn passed_on(&mut self, code: Code) -> String {
        // Inside `java::catching` (see `caught`), the closure returns it.
        if !self.catching_now.is_empty() {
            return format!("{}?", code.at(ATOM));
        }
        match self.innermost_attempt() {
            Some((label, _)) => format!("{}({}, '{label})", self.runtime("or_break!"), code.text),
            None if self.throwing => format!("{}?", code.at(ATOM)),
            None => format!("{}.unwrap_or_else({})", code.at(ATOM), self.uncaught("uncaught")),
        }
    }

    /// A call of the runtime's function `name` (`generics` after its name, such as `::<i32>`)
    /// with the arguments `args`, for the operation `id`, which may fail: where a `catch` or a
    /// `finally` may see the failure, its checked form `try_name`, whose failure goes on from
    /// here as a thrown exception does.
    pub(super) fn may_fail(&mut self, id: ExprId, name: &str, generics: &str, args: &str) -> Code {
        if self.facts.raising.contains(&id) {
            let call = format!("{}{generics}({args})", self.runtime(&format!("try_{name}")));
            return Code::atom(self.passed_on(Code::atom(call)));
        }
        Code::atom(format!("{}{generics}({args})", self.runtime(name)))
    }

    /// `class_cast` of the cast `id`: where a `catch` or a `finally` may see its
    /// `ClassCastException` (`Facts::raising`), in the checked form, whose failure goes on from
    /// here as a thrown exception does.
    pub(super) fn checked_class_cast(&mut self, id: ExprId, to: Type, value: &'a Expr) -> Code {
        let outer = self.checked_cast;
        self.checked_cast = self.facts.raising.contains(&id).then_some(id);
        let code = self.class_cast(to, value);
        self.checked_cast = outer;
        code
    }

    /// The checked form of a cast whose text is `text`, a `Result` (an `Option` of one when the
    /// value may be null), when `checked` is the cast; else `text` as it is.
    pub(super) fn cast_result(&mut self, text: String, checked: Option<ExprId>, optional: bool) -> Code {
        let Some(id) = checked else { return Code::atom(text) };
        self.dereferenced.insert(id);
        let result = if optional { format!("{text}.transpose()") } else { text };
        Code::atom(self.passed_on(Code::atom(result)))
    }

    /// `code`, the operation `id` of `Facts::catching`, run inside `java::catching`, whose failure
    /// goes on from here as a thrown exception does. An operation whose own code passes a
    /// failure on (`?`, `or_break!`) cannot be in its closure, and is rejected.
    pub(super) fn caught(&mut self, id: ExprId, code: Code, mark: usize) -> Code {
        // Lines written before the operation (from `mark`) run outside the closure.
        let before = &self.out[mark..];
        if code.text.contains("or_break!") || before.contains('?') || before.contains("or_break!") {
            let pos = self.facts.catching[&id];
            let message = "catching the exception of an operation that holds another one that is caught not supported yet";
            self.rejected.push(Error { pos, message: message.to_string() });
            return code;
        }
        self.dereferenced.insert(id);
        let call = if code.text.contains('?') {
            // An operand that throws (`passed_on` writes `?` inside) leaves the closure with its
            // exception, which goes on as the operation's own would.
            let throwable = self.throwable_type();
            format!(
                "{}(|| -> Result<_, {throwable}> {{ Ok({}) }}).map_err({throwable}::from).and_then(|r| r)",
                self.runtime("catching"),
                code.text
            )
        } else {
            format!("{}(|| {})", self.runtime("catching"), code.text)
        };
        Code::atom(self.passed_on(Code::atom(call)))
    }

    /// Rejects the program when an operation of `Facts::catching` that is written does not run
    /// inside `java::catching`.
    pub(super) fn check_catching(&mut self) {
        let mut missed: Vec<Pos> = Vec::new();
        for (_, method) in self.class.methods.iter().enumerate().filter(|(i, _)| self.written.contains(i)) {
            for stmt in &method.body {
                visit_stmt(
                    stmt,
                    &mut |e| {
                        visit_expr(e, &mut |x| {
                            if let Some(&pos) = self.facts.catching.get(&x.id)
                                && !self.dereferenced.contains(&x.id)
                            {
                                missed.push(pos);
                            }
                        })
                    },
                    &mut |_| {},
                );
            }
        }
        if let Some(pos) = missed.into_iter().min_by_key(|p| (p.line, p.column)) {
            let message = "catching the exception of this operation not supported yet";
            self.rejected.push(Error { pos, message: message.to_string() });
        }
    }

    /// Rejects the program when a cast of `Facts::raising` that is written is not in the checked
    /// form, where its `ClassCastException` would end the program instead of reaching the `catch`
    /// or the `finally`.
    pub(super) fn check_casts(&mut self) {
        let mut missed: Vec<Pos> = Vec::new();
        for (_, method) in self.class.methods.iter().enumerate().filter(|(i, _)| self.written.contains(i)) {
            for stmt in &method.body {
                visit_stmt(
                    stmt,
                    &mut |e| {
                        visit_expr(e, &mut |x| {
                            if matches!(x.kind, ExprKind::Cast(..))
                                && self.facts.raising.contains(&x.id)
                                && !self.dereferenced.contains(&x.id)
                            {
                                missed.push(x.pos);
                            }
                        })
                    },
                    &mut |_| {},
                );
            }
        }
        if let Some(pos) = missed.into_iter().min_by_key(|p| (p.line, p.column)) {
            let message = "catching the `ClassCastException` of this cast not supported yet";
            self.rejected.push(Error { pos, message: message.to_string() });
        }
    }

    /// `impl From<java::Exception> for Throwable`, for the enum of `Throwable` (`t`) of a program
    /// in which the runtime raises exceptions that the program may see: the failure becomes an
    /// object of its standard class, built from its message. (`?` and `java::or_break!` convert
    /// with it.)
    pub(super) fn raised_impl(&mut self, t: usize) {
        let Some(throwable) = self.class.type_index("java.lang.Throwable") else { return };
        if t != throwable || self.facts.raised_classes.is_empty() {
            return;
        }
        let name = self.throwable_type();
        self.gap();
        self.line(&format!("impl From<java::Exception> for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn from(e: java::Exception) -> {name} {{"));
        self.indent += 1;
        // A message the runtime made is a new string.
        match super::shared_strings() {
            true => self.line("let raised_message = java::JStr::new(e.message.clone().unwrap_or_default());"),
            false => {
                self.line("let raised_message = java::JString::from(e.message.clone().unwrap_or_default());")
            }
        }
        self.line("match e.class {");
        self.indent += 1;
        let outer = self.method;
        for new in &self.class.raised {
            let ExprKind::New { class: c, args, ctor: Some(ctor) } = &new.kind else { continue };
            if !self.facts.raised_classes.contains(c) {
                continue;
            }
            self.method = *ctor;
            let object = self.new_object_as(new.id, *ctor, args, Some(throwable)).text;
            // The form without a message comes first (`Lowering::raised_objects`).
            let bare = if args.is_empty() { " if e.message.is_none()" } else { "" };
            self.line(&format!("{}{bare} => {object},", rust_string(&self.class.types[*c].name)));
        }
        self.method = outer;
        let uncaught = self.uncaught("uncaught");
        self.line(&format!("_ => {uncaught}(e),"));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
    }

    /// `throw value;`.
    pub(super) fn throw_stmt(&mut self, value: &'a Expr) {
        let text = self.thrown_text(value);
        self.throw_text(&text);
    }

    /// The `Throwable` that `throw value` throws. A value that may be null throws a
    /// `NullPointerException` instead (`Facts::null_checked` for the checked form, see
    /// `dereference`).
    pub(super) fn thrown_text(&mut self, value: &'a Expr) -> String {
        // The value is converted to `Throwable`; what it is comes from below the conversion.
        let thrown = match &value.kind {
            ExprKind::Cast(_, inner) => inner,
            _ => value,
        };
        if matches!(thrown.kind, ExprKind::Null) {
            let none = format!("None::<{}>", self.throwable_type());
            return self.dereference(&[value.id, thrown.id], &none);
        }
        if self.nullable(value) {
            let option = self.expr(value).text;
            return self.dereference(&[value.id, thrown.id], &option);
        }
        self.expr(value).text
    }

    /// A statement that throws the exception `text`: it leaves the enclosing `try` block or the
    /// method, or ends the program.
    fn throw_text(&mut self, text: &str) {
        match self.innermost_attempt() {
            Some((label, completion)) => {
                let thrown = self.thrown(completion, text);
                self.line(&format!("break '{label} {thrown};"));
            }
            None if self.throwing => self.line(&format!("return Err({text});")),
            None => {
                let end = self.uncaught("throw_uncaught");
                self.line(&format!("{end}({text});"));
            }
        }
    }

    /// What an exception `e` that no `catch` clause of a `try` matches does: it leaves the
    /// enclosing `try` block or the method, or ends the program.
    fn propagate(&mut self, e: &str) -> String {
        match self.innermost_attempt() {
            Some((label, completion)) => format!("break '{label} {}", self.thrown(completion, e)),
            None if self.throwing => format!("return Err({e})"),
            // `throw_uncaught` returns `!`, so that Rust sees the arm end the code (a local that
            // the `Ok` arm sets is set after the `match`).
            None => format!("{}({e})", self.uncaught("throw_uncaught")),
        }
    }

    /// Whether a jump to the frame at `index` leaves a `try` with a `finally`.
    pub(super) fn leaves_finally(&self, index: usize) -> bool {
        self.finallies.last().is_some_and(|f| f.frame > index)
    }

    /// `return value;` (`text` is the value, `None` for a `return` without one), which first runs
    /// the `finally` of every `try` that it leaves.
    pub(super) fn return_stmt(&mut self, text: Option<String>) {
        if !self.finallies.is_empty() {
            let label = self.completion_attempt();
            let value = text.unwrap_or_else(|| "()".to_string());
            let completion = self.runtime("Completion::Return");
            self.line(&format!("break '{label} {completion}({value});"));
            return;
        }
        match (text, self.ctor_result) {
            (_, Some(result)) if self.throwing => self.line(&format!("return Ok({result});")),
            (_, Some(result)) => self.line(&format!("return {result};")),
            (None, None) if self.throwing => self.line("return Ok(());"),
            (None, None) => self.line("return;"),
            (Some(value), None) if self.throwing => self.line(&format!("return Ok({value});")),
            (Some(value), None) => self.line(&format!("return {value};")),
        }
    }

    /// A jump to the frame at `index` that leaves a `try` with a `finally`, which then runs the
    /// `finally` first; `false` when the jump leaves none. (`yield_value` rejects a `yield`
    /// through a `finally`.)
    pub(super) fn jump_through_finally(&mut self, index: usize, cont: bool) -> bool {
        if !self.leaves_finally(index) {
            return false;
        }
        let f = self.finallies.last_mut().expect("the jump leaves a `try`");
        let n = match f.jumps.iter().position(|&j| j == (index, cont)) {
            Some(n) => n,
            None => {
                f.jumps.push((index, cont));
                f.jumps.len() - 1
            }
        };
        let label = self.completion_attempt();
        let jump = self.runtime("Completion::Jump");
        self.line(&format!("break '{label} {jump}({n});"));
        true
    }

    /// The statements of a `try` block or `catch` clause, as a block that evaluates to the
    /// `Result` or `java::Completion` (`completion`) its header (written) declares: what leaves it
    /// leaves by `label`, and its normal end is `Ok(())` or `java::Completion::Normal`.
    fn attempt_block(
        &mut self,
        label: String,
        completion: bool,
        stmts: &'a [Stmt],
        before: &[String],
        normal: Option<String>,
    ) {
        self.indent += 1;
        self.open_block(Kind::Try);
        self.attempts.push(Attempt { label, frame: self.frames.len() - 1, completion });
        for line in before {
            self.line(line);
        }
        self.stmts(stmts, false);
        if completes_all(stmts) {
            let normal = match normal {
                Some(normal) => normal,
                None if completion => self.runtime("Completion::Normal"),
                None => "Ok(())".to_string(),
            };
            self.line(&normal);
        }
        self.attempts.pop();
        self.close();
        self.indent -= 1;
    }

    /// A `try` statement.
    pub(super) fn try_stmt(
        &mut self,
        stmt: &'a Stmt,
        body: &'a [Stmt],
        catches: &'a [Catch],
        finally: Option<&'a [Stmt]>,
    ) {
        let id = stmt as *const Stmt as usize;
        if let Some(finally) = finally {
            return self.try_finally(id, body, catches, finally);
        }
        if !self.facts.live_tries.contains(&id) {
            // Nothing that the body throws is caught here.
            for catch in catches {
                self.not_written(&catch.body);
            }
            self.line("{");
            self.indent += 1;
            self.stmts(body, false);
            self.indent -= 1;
            self.line("}");
            return;
        }
        let label = self.attempt_label("attempt");
        let result = self.result_name(&label);
        let throwable = self.throwable_type();
        // A body that cannot complete normally ends only by an exception (or leaves the method):
        // its block is never `Ok`, which its type says, so that the `match` needs no `Ok` arm.
        let completes = completes_all(body);
        // The locals that the block sets for the code after the `try`, which Rust does not see
        // set on the way through the `match`, go out in its `Ok` value.
        let carried = carried_out(body, &self.unset_locals);
        let ok = match () {
            _ if !completes => "std::convert::Infallible".to_string(),
            _ if carried.is_empty() => "()".to_string(),
            _ => {
                let types: Vec<String> = carried.iter().map(|n| self.ty_name(self.unset_locals[n])).collect();
                tuple(&types)
            }
        };
        let rust: Vec<String> = carried.iter().map(|n| self.local(n)).collect();
        let normal = (!carried.is_empty()).then(|| format!("Ok({})", tuple(&rust)));
        self.line(&format!("let {result}: Result<{ok}, {throwable}> = {{"));
        self.attempt_block(label, false, body, &[], normal);
        self.line("};");
        self.line(&format!("match {result} {{"));
        self.indent += 1;
        if completes && carried.is_empty() {
            self.line("Ok(()) => {}");
        } else if completes {
            let values: Vec<String> = rust.iter().map(|r| self.temp(&format!("{r}_value"))).collect();
            let sets: Vec<String> = rust.iter().zip(&values).map(|(r, v)| format!("{r} = {v};")).collect();
            self.line(&format!("Ok({}) => {{ {} }}", tuple(&values), sets.join(" ")));
        }
        let (arms, all) = self.catch_arms(catches);
        let count = all.len();
        let mut every = false;
        for (catch, matched, reaching) in &arms {
            let (pattern, before) = self.catch_pattern(catch, matched, reaching, count);
            every |= !pattern.contains(") if matches!(");
            self.line(&format!("Err({pattern}) => {{"));
            self.indent += 1;
            for line in before {
                self.line(&line);
            }
            self.stmts(&catch.body, false);
            self.indent -= 1;
            self.line("}");
        }
        if arms.iter().map(|(_, m, _)| m.len()).sum::<usize>() < count {
            let text = self.propagate("e");
            self.line(&format!("Err(e) => {text},"));
        } else if !every {
            // The arms test the class of each exception, and together they catch all.
            self.line("Err(_) => unreachable!(\"every class of exception is caught\"),");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// Notes the statements of a `catch` clause that is not written: their dereferences and
    /// operations need no checked form (see `check_dereferences`).
    fn not_written(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| _ = self.dereferenced.insert(x.id)), &mut |_| {});
        }
    }

    /// A local for the result of a `try` block, distinct from the method's locals and from the
    /// free functions, which share their namespace.
    fn result_name(&self, label: &str) -> String {
        let mut used: HashSet<String> = self.locals.values().cloned().collect();
        super::names::unique_beside(label.to_string(), &mut used, &self.static_fn_names)
    }

    /// The `catch` clauses that match something, each with the variants of the enum of
    /// `Throwable` it matches that no earlier clause matched and the variants that no earlier
    /// clause matched (which reach its arm); and all the variants.
    #[allow(clippy::type_complexity)]
    fn catch_arms(&mut self, catches: &'a [Catch]) -> (Vec<(&'a Catch, Vec<usize>, Vec<usize>)>, Vec<usize>) {
        if catches.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let t = self.class.type_index("java.lang.Throwable").expect("a program with `try` has `Throwable`");
        let all = self.enum_variants(t);
        let mut covered: Vec<usize> = Vec::new();
        let mut arms = Vec::new();
        for catch in catches {
            let reaching: Vec<usize> = all.iter().copied().filter(|c| !covered.contains(c)).collect();
            let matched: Vec<usize> = all
                .iter()
                .copied()
                .filter(|&c| {
                    !covered.contains(&c)
                        && catch.types.iter().any(|&ty| self.class.hierarchy.is_subtype(c, ty))
                })
                .collect();
            // A clause that nothing the body throws reaches is not written.
            if matched.is_empty() {
                self.not_written(&catch.body);
            } else {
                covered.extend(&matched);
                arms.push((catch, matched, reaching));
            }
        }
        (arms, all)
    }

    /// The pattern of the arm of a `catch` clause that matches the variants `matched` of the enum
    /// of `Throwable`, which has `count` variants, with the lines that bind its parameter. Unless
    /// the clause matches every class, the pattern binds the exception and ends in a guard that
    /// tests its class (`java::Thin`): `caught) if matches!(caught.0.tag(), 3..=5 | 9`, which the
    /// arm's `Err(..)` closes. An exception that reaches the arm is of a class of `reaching`, as
    /// the arms before it took the others, so the guard's ranges may span those.
    fn catch_pattern(
        &mut self,
        catch: &'a Catch,
        matched: &[usize],
        reaching: &[usize],
        count: usize,
    ) -> (String, Vec<String>) {
        let t = self.class.type_index("java.lang.Throwable").expect("a program with `try` has `Throwable`");
        let mut used = false;
        for s in &catch.body {
            visit_stmt(s, &mut |e| used |= mentions(e, &catch.name), &mut |_| {});
        }
        let name = self.local(&catch.name);
        let mutable = if assigns_any(&catch.body, &catch.name) { "mut " } else { "" };
        let ty = self.class.class_index(catch.ty).expect("a catch parameter has a class type");
        let mut before = Vec::new();
        let throwable = self.rep(ty) == hierarchy::Rep::Enum(t);
        let pattern = if matched.len() == count && throwable {
            // The parameter is a `Throwable` that every class matches.
            if used { format!("{mutable}{name}") } else { "_".to_string() }
        } else {
            // The exception, under the parameter's name or one that no local of the method has.
            let caught =
                if throwable && used { format!("{mutable}{name}") } else { self.result_name("caught") };
            let tested = caught.trim_start_matches("mut ").to_string();
            match self.rep(ty) {
                // The parameter is the object of the one class it catches.
                hierarchy::Rep::Object(c) if used => {
                    let view = self.view_pattern(t, c);
                    before.push(format!(
                        "let {mutable}{name} = match {tested}.view() {{ {view}(o) => o.clone(), _ => unreachable!(\"the arm matches its class\") }};"
                    ));
                }
                // The parameter is a value of the enum of a class of the program, which has the
                // same bits.
                hierarchy::Rep::Enum(u) if used && u != t => {
                    let target = struct_name(&self.class.types[u].name);
                    before.push(format!("let {mutable}{name} = {target}({tested}.0);"));
                }
                _ => {}
            }
            if matched.len() == count {
                caught
            } else {
                format!("{caught}) if matches!({tested}.0.tag(), {}", self.tags(matched, reaching))
            }
        };
        if used && self.optional_var(self.method, catch.ty, &catch.name) {
            before.push(format!("let {mutable}{name} = Some({name});"));
        }
        (pattern, before)
    }

    /// A `try` statement with a `finally`. Its body and its `catch` clauses are blocks that
    /// evaluate to how they ended; the `finally` runs after them, and then what ended them
    /// happens: an exception goes on, and a `return` or a jump out of the `try` is made.
    fn try_finally(&mut self, id: usize, body: &'a [Stmt], catches: &'a [Catch], finally: &'a [Stmt]) {
        let (has_return, has_jump) = leaves_early(body, catches);
        let completion = has_return || has_jump;
        if !completion && !self.facts.raising_tries.contains(&id) {
            // Only the body's end leaves it: the `catch` clauses never run.
            self.line("{");
            self.indent += 1;
            self.stmts(body, false);
            self.indent -= 1;
            self.line("}");
            self.finally_ends.push(None);
            self.finally_block(finally);
            self.finally_ends.pop();
            return;
        }
        // Whether the body or a `catch` clause completes normally, as Java tells: when none does,
        // the block is never `Ok` or `Normal`, and the code after the `finally` diverges.
        let ends = completes_all(body) || catches.iter().any(|c| completes_all(&c.body));
        let label = self.attempt_label("attempt");
        let result = self.result_name(&label);
        let never = "std::convert::Infallible";
        // In a program without exceptions (no `Throwable`, or one without objects), nothing is
        // thrown.
        let exceptions =
            self.class.type_index("java.lang.Throwable").is_some_and(|t| !self.enum_variants(t).is_empty());
        let throwable = if exceptions { self.throwable_type() } else { never.to_string() };
        let ty = if completion {
            let value = if has_return { self.completion_value() } else { never.to_string() };
            let jump = if has_jump { "usize" } else { never };
            format!("{}<{value}, {throwable}, {jump}>", self.runtime("Completion"))
        } else {
            format!("Result<{}, {throwable}>", if ends { "()" } else { never })
        };
        let (arms, all) = self.catch_arms(catches);
        let finally_completes = completes_all(finally);
        // When the `finally` does not complete, what ended the `try` is not used after it.
        let last = if finally_completes { result.clone() } else { format!("_{result}") };
        let first = if arms.is_empty() { &last } else { &result };
        self.line(&format!("let {first}: {ty} = {{"));
        self.finallies.push(Finally { frame: self.frames.len(), jumps: Vec::new() });
        self.attempt_block(label, completion, body, &[], None);
        self.line("};");
        if !arms.is_empty() {
            self.line(&format!("let {last} = match {result} {{"));
            self.indent += 1;
            let count = all.len();
            for (catch, matched, reaching) in &arms {
                let (pattern, before) = self.catch_pattern(catch, matched, reaching, count);
                let thrown = self.thrown(completion, &pattern);
                self.line(&format!("{thrown} => {{"));
                let handler = self.attempt_label("handler");
                self.attempt_block(handler, completion, &catch.body, &before, None);
                self.line("}");
            }
            self.line("other => other,");
            self.indent -= 1;
            self.line("};");
        }
        let Finally { jumps, .. } = self.finallies.pop().expect("the `try` is open");
        let thrown = match completion {
            true => format!("matches!({last}, {}(_))", self.runtime("Completion::Throw")),
            false => format!("{last}.is_err()"),
        };
        self.finally_ends.push(Some(thrown));
        self.finally_block(finally);
        self.finally_ends.pop();
        if !finally_completes {
            return;
        }
        if !completion && ends {
            let text = self.passed_on(Code::atom(result));
            self.line(&format!("{text};"));
            return;
        }
        if !completion {
            self.line(&format!("let Err(e) = {result};"));
            self.throw_text("e");
            return;
        }
        self.line(&format!("match {result} {{"));
        self.indent += 1;
        let normal = self.runtime("Completion::Normal");
        if ends {
            self.line(&format!("{normal} => {{}}"));
        } else {
            // Nothing writes it: the arm makes the `match` diverge, as the Java statement does.
            self.line(&format!("{normal} => unreachable!(\"the `try` does not complete normally\"),"));
        }
        if has_return {
            let ret = self.runtime("Completion::Return");
            let void = self.ret == Type::Void;
            self.line(&format!("{ret}({}) => {{", if void { "_" } else { "value" }));
            self.indent += 1;
            self.return_stmt((!void).then(|| "value".to_string()));
            self.indent -= 1;
            self.line("}");
        }
        if has_jump && jumps.is_empty() {
            let jump = self.runtime("Completion::Jump");
            self.line(&format!("{jump}(_) => unreachable!(\"no jump leaves the `try`\"),"));
        }
        let count = jumps.len();
        for (n, (index, cont)) in jumps.into_iter().enumerate() {
            let jump = self.runtime("Completion::Jump");
            let pattern = if n + 1 == count { "_".to_string() } else { n.to_string() };
            self.line(&format!("{jump}({pattern}) => {{"));
            self.indent += 1;
            // A `yield` that reaches its switch expression takes the value that waits for it
            // (see `yield_value`); through another `finally`, the value stays where it is.
            let value = (self.is_value_frame(index) && !self.leaves_finally(index))
                .then(|| format!("yielded{index}.take().unwrap()"));
            self.jump(index, cont, value);
            self.indent -= 1;
            self.line("}");
        }
        if exceptions {
            let throw = self.runtime("Completion::Throw");
            let text = self.propagate("e");
            self.line(&format!("{throw}(e) => {text},"));
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The type of the value that a `return` inside a `try` with a `finally` gives back.
    fn completion_value(&mut self) -> String {
        // A constructor's `return` carries nothing: the object is returned after the `finally`.
        if self.ctor_result.is_some() {
            return "()".to_string();
        }
        let ret = self.plain_ret_text(self.method);
        ret.strip_prefix(" -> ").unwrap_or("()").to_string()
    }

    /// The statements of a `finally`, in a block of their own when they declare locals.
    fn finally_block(&mut self, finally: &'a [Stmt]) {
        if finally.iter().any(|s| matches!(s, Stmt::Local { .. })) {
            self.line("{");
            self.indent += 1;
            self.stmts(finally, false);
            self.indent -= 1;
            self.line("}");
        } else {
            self.stmts(finally, false);
        }
    }

    /// `o.getClass().getName()`: the binary name of the object's class; with `simple`,
    /// `o.getClass().getSimpleName()`, the name of the class in its source.
    pub(super) fn class_name(&mut self, object: &'a Expr, simple: bool) -> Code {
        let method = if simple { "get_simple_name" } else { "get_name" };
        match self.enum_of(object) {
            // A proxy class is named when it is defined (`Class::proxy_classes`).
            None if self.struct_class(object).is_some_and(|c| self.class.proxy_classes.contains(&c)) => {
                let c = self.struct_class(object).expect("an object has a class");
                let class = self.type_class(Type::class(&self.class.types[c].name));
                self.after_object(object, &format!("{class}.{method}()"))
            }
            None => {
                let c = self.struct_class(object).expect("an object has a class");
                let name = rust_string(&self.java_class_name(c, simple));
                self.after_object(object, &format!("java::JString::from({name})"))
            }
            Some(t) => {
                let obj = self.object(object);
                // An `Object[]` that may carry its class (`Class::element_classes`) is named by it.
                let object_array = Type::array(Type::class(OBJECT));
                // So is a value that may be of a proxy class (`Class::proxy_classes`).
                let variants = self.enum_variants(t);
                if self.carries_class(object_array)
                    && variants.iter().any(|&c| self.class.array_of(c) == Some(object_array))
                    || variants.iter().any(|c| self.class.proxy_classes.contains(c))
                {
                    self.need(Need::RuntimeClass(t));
                    let class = super::class_values::method_on(&obj, "runtime_class");
                    return Code::atom(format!("{class}.{method}()"));
                }
                let mut arms: Vec<String> = self
                    .enum_variants(t)
                    .into_iter()
                    .map(|c| {
                        format!("{} => {}", self.class_tag(c), rust_string(&self.java_class_name(c, simple)))
                    })
                    .collect();
                arms.push("tag => java::bad_tag(tag)".to_string());
                let tag = format!("{}.0.tag()", super::dispatch::operand(&obj));
                Code::atom(format!("java::JString::from(match {tag} {{ {} }})", arms.join(", ")))
            }
        }
    }

    /// The name that `Class.getName` gives class `c`, its binary name; with `simple`, the
    /// name that `getSimpleName` gives it, its name in its source.
    pub(super) fn java_class_name(&self, c: usize, simple: bool) -> String {
        let decl = &self.class.types[c];
        // An array held as an `Object`: `[C`, `[Ljava.lang.String;`.
        if let (false, Some(Type::Array(element))) = (simple, self.class.array_of(c)) {
            return super::class_values::array_name(self, *element);
        }
        if let Some((binary, simple_name)) = decl.origin.java_names() {
            (if simple { simple_name } else { binary }).to_string()
        } else if simple {
            decl.name.rsplit('.').next().unwrap_or_default().to_string()
        } else if decl.kind == TypeKind::Runtime || crate::lower::is_standard(&decl.name) {
            decl.name.clone()
        } else if decl.package.is_empty() {
            decl.name.replace('.', "$")
        } else {
            format!("{}.{}", decl.package, decl.name[decl.package.len() + 1..].replace('.', "$"))
        }
    }
}
