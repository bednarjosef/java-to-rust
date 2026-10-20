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
        Stmt::Expr(Expr { kind: ExprKind::Builtin(Builtin::Exit, _), .. }) => false,
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

/// The locals that a live `try` block assigns: Rust cannot tell that one declared without a value
/// is assigned on every path to a read after the `try`, as Java can, so it is given one.
pub(super) fn try_assigned(facts: &Facts, body: &[Stmt]) -> HashSet<String> {
    fn assigned(stmts: &[Stmt], out: &mut HashSet<String>) {
        for s in stmts {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Assign { target: Place::Var(name), .. }
                        | ExprKind::IncDec { target: Place::Var(name), .. } = &x.kind
                        {
                            out.insert(name.clone());
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    let mut out = HashSet::new();
    for s in body {
        visit_stmt(s, &mut |_| {}, &mut |s| {
            if let Stmt::Try { body, .. } = s
                && facts.live_tries.contains(&(s as *const Stmt as usize))
            {
                assigned(body, &mut out);
            }
        });
    }
    out
}

/// The value Java gives a field of type `ty`, for a local that `try_assigned` names; `None` for a
/// type whose locals are not given one yet.
pub(super) fn default_value(ty: Type) -> Option<&'static str> {
    Some(match ty {
        Type::Int | Type::Long | Type::Byte | Type::Short | Type::Char => "0",
        Type::Double | Type::Float => "0.0",
        Type::Boolean => "false",
        Type::String => "String::new()",
        _ => return None,
    })
}

/// A local of the method that `try_assigned` names (`assigned`) whose type has no
/// `default_value`: not supported yet.
pub(super) fn check_try_assigned(body: &[Stmt], assigned: &HashSet<String>) -> Option<Error> {
    let mut found = None;
    for s in body {
        visit_stmt(s, &mut |_| {}, &mut |s| {
            if let Stmt::Local { ty, name, init: None, pos } = s
                && assigned.contains(name)
                && default_value(*ty).is_none()
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

impl<'a> Gen<'a> {
    /// The Rust type of an exception: the enum of `Throwable`.
    pub(super) fn throwable_type(&mut self) -> String {
        self.ty_name(Type::class("java.lang.Throwable"))
    }

    /// The runtime's function `name` that ends the program with an uncaught exception, which
    /// shows the exception through its `Display`.
    fn uncaught(&mut self, name: &str) -> String {
        let t = self.class.type_index("java.lang.Throwable").expect("a program that throws has `Throwable`");
        if let Some(t) = self.enum_for(t) {
            self.need(Need::Display(t));
        }
        self.runtime(name)
    }

    /// The call `code` of method `callee`: as it is when the method does not throw, else with
    /// what its failure does here (see the module's documentation).
    pub(super) fn raising(&mut self, code: Code, callee: usize) -> Code {
        if !self.facts.throwing.contains(&callee) {
            return code;
        }
        let call = code.at(ATOM);
        Code::atom(match self.attempts.last().cloned() {
            Some(label) => format!("{}({}, '{label})", self.runtime("or_break!"), code.text),
            None if self.throwing => format!("{call}?"),
            None => format!("{call}.unwrap_or_else({})", self.uncaught("uncaught")),
        })
    }

    /// `throw value;`.
    pub(super) fn throw_stmt(&mut self, value: &'a Expr) {
        let text = self.expr(value).text;
        match self.attempts.last().cloned() {
            Some(label) => self.line(&format!("break '{label} Err({text});")),
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
        match self.attempts.last().cloned() {
            Some(label) => format!("break '{label} Err({e})"),
            None if self.throwing => format!("return Err({e})"),
            None => format!("{}({e})", self.uncaught("uncaught")),
        }
    }

    /// A `try` statement without `finally`.
    pub(super) fn try_stmt(&mut self, stmt: &'a Stmt, body: &'a [Stmt], catches: &'a [Catch]) {
        if !self.facts.live_tries.contains(&(stmt as *const Stmt as usize)) {
            // Nothing that the body throws is caught here.
            self.line("{");
            self.indent += 1;
            self.stmts(body, false);
            self.indent -= 1;
            self.line("}");
            return;
        }
        self.attempt_count += 1;
        let label = match self.attempt_count {
            1 => "attempt".to_string(),
            n => format!("attempt{n}"),
        };
        // The result is a local of its own, distinct from the method's.
        let mut used: HashSet<String> = self.locals.values().cloned().collect();
        let result = unique(label.clone(), &mut used);
        let throwable = self.throwable_type();
        // A body that cannot complete normally ends only by an exception (or leaves the method):
        // its block is never `Ok`, which its type says, so that the `match` needs no `Ok` arm.
        let completes = completes_all(body);
        let ok = if completes { "()" } else { "std::convert::Infallible" };
        self.line(&format!("let {result}: Result<{ok}, {throwable}> = '{label}: {{"));
        self.indent += 1;
        self.open(Kind::Try);
        self.attempts.push(label);
        self.stmts(body, false);
        if completes {
            self.line("Ok(())");
        }
        self.attempts.pop();
        self.close();
        self.indent -= 1;
        self.line("};");
        self.line(&format!("match {result} {{"));
        self.indent += 1;
        if completes {
            self.line("Ok(()) => {}");
        }
        let t = self.class.type_index("java.lang.Throwable").expect("a program with `try` has `Throwable`");
        let all = self.enum_variants(t);
        let mut covered: Vec<usize> = Vec::new();
        for catch in catches {
            let matched: Vec<usize> = all
                .iter()
                .copied()
                .filter(|&c| {
                    !covered.contains(&c)
                        && catch.types.iter().any(|&ty| self.class.hierarchy.is_subtype(c, ty))
                })
                .collect();
            if matched.is_empty() {
                // Nothing the body throws reaches this clause.
                continue;
            }
            covered.extend(&matched);
            self.catch_arm(t, catch, &matched, all.len());
        }
        if all.iter().any(|c| !covered.contains(c)) {
            let text = self.propagate("e");
            self.line(&format!("Err(e) => {text},"));
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The arm of a `catch` clause that matches the variants `matched` of the enum of `Throwable`
    /// (`t`), which has `count` variants.
    fn catch_arm(&mut self, t: usize, catch: &'a Catch, matched: &[usize], count: usize) {
        let mut used = false;
        for s in &catch.body {
            visit_stmt(s, &mut |e| used |= mentions(e, &catch.name), &mut |_| {});
        }
        let name = self.local(&catch.name);
        let mutable = if assigns_any(&catch.body, &catch.name) { "mut " } else { "" };
        let bound = if used { format!("{mutable}{name}") } else { "_".to_string() };
        let ty = self.class.class_index(catch.ty).expect("a catch parameter has a class type");
        let variants: Vec<String> = matched.iter().map(|&c| self.variant(t, c)).collect();
        let alternatives = variants.iter().map(|v| format!("{v}(_)")).collect::<Vec<_>>().join(" | ");
        // The alternatives, bound to a name when the clause uses the value.
        let any_of = |bound: &str| match (bound, matched.len()) {
            ("_", _) => alternatives.clone(),
            (_, 1) => format!("{bound} @ {alternatives}"),
            _ => format!("{bound} @ ({alternatives})"),
        };
        let mut conversion = None;
        let pattern = match hierarchy::rep(self.class, self.facts, ty) {
            // The parameter is the object of the one class it catches.
            hierarchy::Rep::Object(_) => format!("{}({bound})", variants[0]),
            // The parameter is a `Throwable`.
            hierarchy::Rep::Enum(u) if u == t && matched.len() == count => bound,
            hierarchy::Rep::Enum(u) if u == t => any_of(&bound),
            // The parameter is a value of the enum of a class of the program.
            hierarchy::Rep::Enum(_) if !used => any_of("_"),
            hierarchy::Rep::Enum(u) => {
                // The object that the arm binds, under a name that no local of the method has.
                let mut taken: HashSet<String> = self.locals.values().cloned().collect();
                let caught = unique("caught".to_string(), &mut taken);
                if let [c] = matched {
                    conversion = Some(format!("let {mutable}{name} = {}({caught});", self.variant(u, *c)));
                    format!("{}({caught})", variants[0])
                } else {
                    let arms: Vec<String> = matched
                        .iter()
                        .zip(&variants)
                        .map(|(&c, v)| format!("{v}(o) => {}(o)", self.variant(u, c)))
                        .collect();
                    conversion = Some(format!(
                        "let {mutable}{name} = match {caught} {{ {}, _ => unreachable!(\"the arm matches these\") }};",
                        arms.join(", ")
                    ));
                    any_of(&caught)
                }
            }
        };
        self.line(&format!("Err({pattern}) => {{"));
        self.indent += 1;
        if let Some(conversion) = conversion {
            self.line(&conversion);
        }
        if used && self.optional_var(self.method, catch.ty, &catch.name) {
            self.line(&format!("let {mutable}{name} = Some({name});"));
        }
        self.stmts(&catch.body, false);
        self.indent -= 1;
        self.line("}");
    }

    /// `o.getClass().getName()`: the binary name of the object's class.
    pub(super) fn class_name(&mut self, object: &'a Expr) -> Code {
        let binary = |decl: &TypeDecl| {
            if crate::lower::is_standard(&decl.name) {
                decl.name.clone()
            } else if decl.package.is_empty() {
                decl.name.replace('.', "$")
            } else {
                format!("{}.{}", decl.package, decl.name[decl.package.len() + 1..].replace('.', "$"))
            }
        };
        match self.enum_of(object) {
            None => {
                let c = self.struct_class(object).expect("an object has a class");
                let name = rust_string(&binary(&self.class.types[c]));
                if has_effects(object) {
                    let text = self.expr(object).text;
                    return Code::atom(format!("{{ {text}; String::from({name}) }}"));
                }
                Code::atom(format!("String::from({name})"))
            }
            Some(t) => {
                let obj = self.object(object);
                let arms: Vec<String> = self
                    .enum_variants(t)
                    .into_iter()
                    .map(|c| {
                        format!("{}(_) => {}", self.variant(t, c), rust_string(&binary(&self.class.types[c])))
                    })
                    .collect();
                Code::atom(format!("String::from(match {obj} {{ {} }})", arms.join(", ")))
            }
        }
    }
}
