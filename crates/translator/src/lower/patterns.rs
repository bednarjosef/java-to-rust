//! Pattern matching in `switch`: type patterns, record patterns (nested too), guards (`when`),
//! `case null`, and constants among them. A pattern switch lowers to a chain of `if`s in the
//! order of its cases, each testing its labels with `instanceof` and then its guard:
//!
//! ```text
//! switch (shape) {                          if shape instanceof Circle c && c.r > 1 { .. }
//!     case Circle c when c.r() > 1 -> ..    else if shape instanceof Square square { .. }
//!     case Square(double s) -> ..           else { <null check of shape>; .. }
//!     default -> ..
//! }
//! ```
//!
//! A switch expression whose cases are all expressions is a chain of `?:` instead. A switch that
//! a `break` leaves early, or an expression whose cases are blocks, keeps the chain as the one
//! case of a switch with only a `default` (on the constant `0`), which the `break` or `yield`
//! leaves.
//!
//! A record pattern binds the record to a variable of its own (an `instanceof`, which also tests
//! for null: a record pattern never matches null) and tests its components as fields of that
//! variable. A pattern that always matches its value (`var x`, or a type the value's type is
//! under) binds nothing: its variable is an alias, which lowering reads as the value wherever
//! the case uses it (see `alias`). A pattern variable is never assigned, so the two agree. (A
//! record's own accessor runs at each use of its alias, where Java runs it once.)
//!
//! Without `case null`, a null selector throws `NullPointerException` before any case is tried.
//! Every test is false on null, so the null check comes where the chain ends: before the case
//! that takes every value left (a `default` or a total pattern), or, in an exhaustive switch
//! without one, at its end (`Unmatched`), which nothing else reaches.

use super::*;
use std::cell::RefCell;
use std::collections::HashSet;

/// A pattern variable that names a value without testing it.
pub(super) struct Alias {
    name: String,
    /// The type of the pattern variable.
    ty: Type,
    target: Target,
}

#[derive(Clone)]
enum Target {
    /// The selector, a variable.
    Var(String, Type),
    /// The component `field` of the record bound to the variable `record`, read through the
    /// record's own accessor `accessor` if it declares one.
    Component { record: String, record_ty: Type, field: String, field_ty: Type, accessor: Option<usize> },
    /// No value: a later pattern variable of the name hides the earlier alias (see
    /// `forget_alias`).
    Hidden,
}

/// One case of a pattern switch.
struct Arm<'t> {
    /// What the labels test; `None` when they take every value that reaches them.
    test: Option<Expr>,
    aliases: Vec<Alias>,
    guard: Option<Node<'t>>,
    /// Whether the case is `case null`, alone or with others.
    takes_null: bool,
    case: Node<'t>,
}

/// Whether a switch needs patterns: a case with a pattern, a guard or `null`.
pub(super) fn is_pattern_switch(node: Node) -> bool {
    node.list("cases").any(|case| {
        case.get("guard").is_some()
            || case.list("labels").any(|l| l.kind() == "PATTERN_CASE_LABEL" || is_null_label(l))
    })
}

fn is_null_label(label: Node) -> bool {
    label.kind() == "CONSTANT_CASE_LABEL"
        && label.get("constantExpression").is_some_and(|c| c.kind() == "NULL_LITERAL")
}

/// Every name that the code in `v` mentions, which a variable of the translation must not take.
fn names(v: &serde_json::Value, out: &mut HashSet<String>) {
    match v {
        serde_json::Value::Object(fields) => {
            if let Some(name) = fields.get("name").and_then(serde_json::Value::as_str) {
                out.insert(name.to_string());
            }
            fields.values().for_each(|x| names(x, out));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|x| names(x, out)),
        _ => {}
    }
}

/// Whether the last statement of a case leaves it, so that it does not fall into the next case.
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

/// Whether a statement has a `break` that leaves the switch around it.
fn breaks_out(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Break => true,
        Stmt::Block(stmts) => stmts.iter().any(breaks_out),
        Stmt::If { then, otherwise, .. } => breaks_out(then) || otherwise.as_deref().is_some_and(breaks_out),
        Stmt::Labeled { body, .. } => breaks_out(body),
        Stmt::Try { body, catches, finally } => {
            body.iter().any(breaks_out)
                || catches.iter().any(|c| c.body.iter().any(breaks_out))
                || finally.iter().flatten().any(breaks_out)
        }
        _ => false,
    }
}

/// Whether a test binds a variable.
fn binds(test: &Expr) -> bool {
    let mut found = false;
    visit_expr(test, &mut |x| found |= matches!(x.kind, ExprKind::InstanceOf { binding: Some(_), .. }));
    found
}

/// The names a pattern switch gives its own variables, none of them a name its code mentions.
struct Fresh(RefCell<HashSet<String>>);

impl Fresh {
    fn name(&self, base: &str) -> String {
        let mut taken = self.0.borrow_mut();
        let name = (1..)
            .map(|i| if i == 1 { base.to_string() } else { format!("{base}{i}") })
            .find(|n| !taken.contains(n))
            .expect("a free name");
        taken.insert(name.clone());
        name
    }
}

/// `circle` for the record `Circle`.
fn variable_name(simple: &str) -> String {
    let mut chars = simple.chars();
    chars.next().map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
}

impl<'t> Lowering<'t> {
    /// A pattern switch statement.
    pub(super) fn pattern_switch_stmt(&self, node: Node<'t>) -> Result<Stmt, Error> {
        let (prelude, selector, arms) = self.arms(node)?;
        let mut bodies = Vec::new();
        for (i, arm) in arms.iter().enumerate() {
            let case = arm.case;
            let rule = case.str("caseKind") == "RULE";
            let mut body = self.with_aliases(&arm.aliases, || {
                if rule { self.arrow_body(case.child("body")?, false) } else { self.block(case) }
            })?;
            if !rule && !leaves(&body) && i + 1 < arms.len() {
                return unsupported(case, "a case that falls into the next one in a pattern switch");
            }
            if matches!(body.last(), Some(Stmt::Break)) {
                body.pop();
            }
            bodies.push(body);
        }
        let early = bodies.iter().flatten().any(breaks_out);
        let mut stmts = prelude;
        stmts.push(self.chain(node, &selector, arms, bodies)?);
        if early {
            // A `break` leaves the switch, which holds the chain as its one case.
            let zero = self.synth(ExprKind::Int(0), node.pos(), Type::Int);
            let cases = vec![Case { labels: vec![CaseLabel::Default], body: stmts }];
            return Ok(Stmt::Switch { selector: zero, cases });
        }
        Ok(if stmts.len() == 1 { stmts.pop().expect("one statement") } else { Stmt::Block(stmts) })
    }

    /// A pattern switch expression.
    pub(super) fn pattern_switch_expr(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let ty = self.ty(node, node.java_type())?;
        let (prelude, selector, arms) = self.arms(node)?;
        let plain = |case: Node| {
            case.str("caseKind") == "RULE"
                && case
                    .get("body")
                    .is_some_and(|b| !matches!(b.kind(), "BLOCK" | "THROW" | "EXPRESSION_STATEMENT"))
        };
        if prelude.is_empty() && arms.iter().all(|a| plain(a.case)) {
            return Ok(self.conditional_chain(node, &selector, arms, ty)?.kind);
        }
        let mut bodies = Vec::new();
        for (i, arm) in arms.iter().enumerate() {
            let case = arm.case;
            let rule = case.str("caseKind") == "RULE";
            let body = self.with_aliases(&arm.aliases, || {
                if rule { self.arrow_body(case.child("body")?, true) } else { self.block(case) }
            })?;
            if !leaves(&body) && i + 1 < arms.len() {
                return unsupported(case, "a case that falls into the next one in a pattern switch");
            }
            bodies.push(body);
        }
        let mut stmts = prelude;
        stmts.push(self.chain(node, &selector, arms, bodies)?);
        let zero = self.synth(ExprKind::Int(0), node.pos(), Type::Int);
        let cases = vec![Case { labels: vec![CaseLabel::Default], body: stmts }];
        Ok(ExprKind::Switch { selector: Box::new(zero), cases })
    }

    /// The selector, a variable (with the statement that stores it in one, when it is not), and
    /// the arms of a pattern switch: `case null` first, then the cases in order, the default
    /// last.
    fn arms(&self, node: Node<'t>) -> Result<(Vec<Stmt>, Expr, Vec<Arm<'t>>), Error> {
        let mut taken = HashSet::new();
        names(node.0, &mut taken);
        let fresh = Fresh(RefCell::new(taken));
        let value = self.expr(node.child("expression")?)?;
        if !value.ty.is_reference() && !matches!(value.ty, Type::Enum(_)) {
            return unsupported(node, "a pattern switch on a primitive value");
        }
        let (ty, pos) = (value.ty, value.pos);
        let (prelude, name) = match &value.kind {
            ExprKind::Var(name) => (Vec::new(), name.clone()),
            _ => {
                let name = fresh.name("selector");
                (vec![Stmt::Local { ty, name: name.clone(), init: Some(value), pos }], name)
            }
        };
        let selector = || self.synth(ExprKind::Var(name.clone()), pos, ty);

        let mut arms = Vec::new();
        let mut default = None;
        let mut joining: Vec<Node<'t>> = Vec::new();
        let cases: Vec<Node<'t>> = node.list("cases").collect();
        for (i, &case) in cases.iter().enumerate() {
            let mut labels = std::mem::take(&mut joining);
            labels.extend(case.list("labels"));
            let empty = case.str("caseKind") != "RULE" && case.list("statements").next().is_none();
            if empty && i + 1 < cases.len() && case.get("guard").is_none() {
                joining = labels;
                continue;
            }
            let mut tests = Vec::new();
            let mut aliases = Vec::new();
            let (mut takes_null, mut takes_all) = (false, false);
            for &label in &labels {
                match label.kind() {
                    "DEFAULT_CASE_LABEL" => takes_all = true,
                    "CONSTANT_CASE_LABEL" if is_null_label(label) => {
                        takes_null = true;
                        let null = self.synth(ExprKind::Null, label.pos(), Type::Null);
                        let test = ExprKind::Binary(BinOp::Eq, Box::new(selector()), Box::new(null));
                        tests.push(Some(self.synth(test, label.pos(), Type::Boolean)));
                    }
                    "CONSTANT_CASE_LABEL" => tests.push(Some(self.constant_test(label, selector())?)),
                    "PATTERN_CASE_LABEL" => {
                        let (test, mut found) =
                            self.pattern_test(label.child("pattern")?, selector(), &fresh)?;
                        if labels.len() > 1 && (test.as_ref().is_some_and(binds) || !found.is_empty()) {
                            return unsupported(label, "several patterns with variables in one case");
                        }
                        aliases.append(&mut found);
                        tests.push(test);
                    }
                    kind => return unsupported(label, &kind_words(kind)),
                }
            }
            let guard = case.get("guard");
            let test = if takes_all || tests.iter().any(Option::is_none) {
                None
            } else {
                tests.into_iter().flatten().reduce(|a, b| self.both(BinOp::Or, a, b))
            };
            let arm = Arm { test, aliases, guard, takes_null, case };
            if takes_all {
                default = Some(arm);
            } else if takes_null && arm.test.is_some() && labels.len() == 1 {
                // Only null matches `case null`, so it is tested first, where nothing else takes
                // the null it would.
                arms.insert(0, arm);
            } else {
                arms.push(arm);
            }
        }
        arms.extend(default);
        Ok((prelude, selector(), arms))
    }

    /// `a op b`, for `&&` and `||`.
    fn both(&self, op: BinOp, a: Expr, b: Expr) -> Expr {
        let pos = a.pos;
        self.synth(ExprKind::Binary(op, Box::new(a), Box::new(b)), pos, Type::Boolean)
    }

    /// The test of a constant label: `equals` for a string selector, `==` for an enum.
    fn constant_test(&self, label: Node<'t>, selector: Expr) -> Result<Expr, Error> {
        let CaseLabel::Value(constant) = self.case_label(label, selector.ty)? else {
            return unsupported(label, "this case label");
        };
        let kind = match selector.ty {
            Type::String => ExprKind::Builtin(Builtin::StringEquals, vec![constant, selector]),
            Type::Enum(_) => ExprKind::Binary(BinOp::Eq, Box::new(selector), Box::new(constant)),
            _ => return unsupported(label, "a constant label in a pattern switch on this type"),
        };
        Ok(self.synth(kind, label.pos(), Type::Boolean))
    }

    /// The test of `pattern` on `value`, `None` when it always matches, with the aliases of the
    /// variables it binds without a test.
    fn pattern_test(
        &self,
        pattern: Node<'t>,
        value: Expr,
        fresh: &Fresh,
    ) -> Result<(Option<Expr>, Vec<Alias>), Error> {
        match pattern.kind() {
            "BINDING_PATTERN" => {
                let variable = pattern.child("variable")?;
                let name = variable.str("name").to_string();
                let ty = self.ty(variable, variable.symbol().map_or("", |s| s.ty))?;
                if self.always_is(value.ty, ty) {
                    return Ok((None, vec![Alias { name, ty, target: target(value) }]));
                }
                Ok((Some(self.instance_test(pattern, value, ty, name)?), Vec::new()))
            }
            "DECONSTRUCTION_PATTERN" => self.record_test(pattern, value, fresh),
            kind => unsupported(pattern, &kind_words(kind)),
        }
    }

    /// The test of a record pattern: the record bound to a variable of its own, then the tests of
    /// its components.
    fn record_test(
        &self,
        pattern: Node<'t>,
        value: Expr,
        fresh: &Fresh,
    ) -> Result<(Option<Expr>, Vec<Alias>), Error> {
        let deconstructor = pattern.child("deconstructor")?;
        let ty = self.ty(deconstructor, deconstructor.java_type())?;
        let record = match ty {
            Type::Class(name, []) => self.type_index(name).filter(|&t| self.types[t].node.kind() == "RECORD"),
            _ => None,
        };
        let Some(record) = record else { return unsupported(pattern, "this record pattern") };
        if self.types[record].node.list("typeParameters").next().is_some() {
            return unsupported(pattern, "record patterns of generic records");
        }
        let simple = self.types[record].node.str("simpleName");
        let name = fresh.name(&variable_name(simple));
        let mut test = self.instance_test(pattern, value, ty, name.clone())?;
        let mut aliases = Vec::new();
        let components = self.components(record)?;
        let nested: Vec<Node<'t>> = pattern.list("nestedPatterns").collect();
        if nested.len() != components.len() {
            return unsupported(pattern, "this record pattern");
        }
        for (p, (field, field_ty, accessor)) in nested.into_iter().zip(components) {
            let at = Target::Component { record: name.clone(), record_ty: ty, field, field_ty, accessor };
            let read = self.read(&at, p.pos());
            let (more, mut found) = self.pattern_test(p, read, fresh)?;
            if let Some(more) = more {
                test = self.both(BinOp::And, test, more);
            }
            aliases.append(&mut found);
        }
        Ok((Some(test), aliases))
    }

    /// Whether every value of type `value` but null is a `ty`.
    fn always_is(&self, value: Type, ty: Type) -> bool {
        if value == ty || (Self::is_object(ty) && value.is_reference()) {
            return true;
        }
        match (value, ty) {
            (Type::Class(a, _), Type::Class(b, _)) => match (self.type_index(a), self.type_index(b)) {
                (Some(a), Some(b)) => self.is_subclass(a, b),
                _ => false,
            },
            _ => false,
        }
    }

    /// `value instanceof ty name`.
    fn instance_test(&self, pattern: Node<'t>, value: Expr, ty: Type, name: String) -> Result<Expr, Error> {
        let target = matches!(ty, Type::Class(..) | Type::Enum(_))
            || (Self::is_object(value.ty) && matches!(ty, Type::String | Type::Boxed(_)))
            || Self::is_numeric_box_of(value.ty, ty);
        if !matches!(value.ty, Type::Class(..) | Type::Enum(_)) || !target {
            return unsupported(pattern, "this pattern on a value of this type");
        }
        self.forget_alias(&name);
        let kind = ExprKind::InstanceOf { value: Box::new(value), ty, binding: Some(name) };
        Ok(self.synth(kind, pattern.pos(), Type::Boolean))
    }

    /// The components of the record `index`: each field's name and type, with the record's own
    /// accessor of it if it declares one (`javac::records` marks the ones it writes synthetic).
    fn components(&self, index: usize) -> Result<Vec<(String, Type, Option<usize>)>, Error> {
        let node = self.types[index].node;
        let mut out = Vec::new();
        for member in node.list("members").filter(|m| m.kind() == "VARIABLE" && !m.has_flag("STATIC")) {
            let name = member.str("name").to_string();
            let ty = self.ty(member, member.symbol().map_or("", |s| s.ty))?;
            let accessor = self.methods.iter().position(|m| {
                m.owner == index
                    && m.name == name
                    && m.sig.starts_with("()")
                    && m.node.0.get("synthetic").is_none()
            });
            out.push((name, ty, accessor));
        }
        Ok(out)
    }

    /// The value at `target`.
    fn read(&self, target: &Target, pos: Pos) -> Expr {
        match target {
            Target::Var(name, ty) => self.synth(ExprKind::Var(name.clone()), pos, *ty),
            Target::Component { record, record_ty, field, field_ty, accessor } => {
                let record = Box::new(self.synth(ExprKind::Var(record.clone()), pos, *record_ty));
                let kind = match accessor {
                    Some(m) => ExprKind::MethodCall {
                        receiver: record,
                        name: field.clone(),
                        args: Vec::new(),
                        resolved: Some(*m),
                        is_super: false,
                    },
                    None => ExprKind::Field { object: record, name: field.clone() },
                };
                self.synth(kind, pos, *field_ty)
            }
            Target::Hidden => unreachable!("a hidden alias names nothing"),
        }
    }

    /// Runs `f` with these aliases in scope.
    fn with_aliases<R>(&self, aliases: &[Alias], f: impl FnOnce() -> Result<R, Error>) -> Result<R, Error> {
        let depth = self.aliases.borrow().len();
        self.aliases.borrow_mut().extend(aliases.iter().map(|a| Alias {
            name: a.name.clone(),
            ty: a.ty,
            target: a.target.clone(),
        }));
        let result = f();
        self.aliases.borrow_mut().truncate(depth);
        result
    }

    /// The value that the pattern variable `node` names, if it is an alias.
    pub(super) fn alias(&self, node: Node<'t>) -> Option<ExprKind> {
        let symbol = node.symbol().filter(|s| s.kind == "BINDING_VARIABLE")?;
        let aliases = self.aliases.borrow();
        let alias = aliases.iter().rev().find(|a| a.name == symbol.name)?;
        if matches!(alias.target, Target::Hidden) {
            return None;
        }
        let value = self.read(&alias.target, node.pos());
        Some(self.convert(value, alias.ty).kind)
    }

    /// `value instanceof Segment(Point(int x, int y), Point to)`: the test of the record
    /// pattern. Its variables are aliases until a later pattern variable of the same name hides
    /// them: javac has checked that they are read only where they are in scope, and Java lets no
    /// variable in scope have the name of another, so a read names the latest pattern variable of
    /// its name in the method.
    pub(super) fn record_instance_of(&self, node: Node<'t>, pattern: Node<'t>) -> Result<ExprKind, Error> {
        let Some(method) = self.method_ctx.get() else {
            return unsupported(pattern, "a record pattern outside a method");
        };
        // The variables of the method, and those of the records that earlier patterns bound,
        // which the record variables of this one must not hide.
        let mut taken = HashSet::new();
        names(self.methods[method].node.0, &mut taken);
        for alias in self.aliases.borrow().iter() {
            if let Target::Component { record, .. } = &alias.target {
                taken.insert(record.clone());
            }
        }
        let fresh = Fresh(RefCell::new(taken));
        let value = self.expr(node.child("expression")?)?;
        let (test, aliases) = self.record_test(pattern, value, &fresh)?;
        for alias in &aliases {
            self.forget_alias(&alias.name);
        }
        self.aliases.borrow_mut().extend(aliases);
        Ok(test.expect("a record pattern tests its value").kind)
    }

    /// Hides the alias of `name`, which a pattern variable now names.
    pub(super) fn forget_alias(&self, name: &str) {
        let mut aliases = self.aliases.borrow_mut();
        if aliases.iter().any(|a| a.name == name) {
            aliases.push(Alias { name: name.to_string(), ty: Type::Void, target: Target::Hidden });
        }
    }

    /// The test of an arm with its guard, which reads the arm's aliases.
    fn arm_test(&self, arm: &mut Arm<'t>) -> Result<Option<Expr>, Error> {
        let guard = match arm.guard {
            Some(g) => Some(self.with_aliases(&arm.aliases, || self.expr(g))?),
            None => None,
        };
        Ok(match (arm.test.take(), guard) {
            (Some(t), Some(g)) => Some(self.both(BinOp::And, t, g)),
            (t, g) => t.or(g),
        })
    }

    /// Whether the selector must be checked for null where the chain ends: no `case null`.
    fn checks_null(arms: &[Arm]) -> bool {
        !arms.iter().any(|a| a.takes_null)
    }

    /// The `if` chain of the arms, each running its body. It ends with the arm that takes every
    /// value left, or with `Unmatched`; either checks the selector for null first.
    fn chain(
        &self,
        node: Node<'t>,
        selector: &Expr,
        mut arms: Vec<Arm<'t>>,
        bodies: Vec<Vec<Stmt>>,
    ) -> Result<Stmt, Error> {
        let checks_null = Self::checks_null(&arms);
        let mut tests = Vec::new();
        for arm in &mut arms {
            tests.push(self.arm_test(arm)?);
        }
        let mut rest: Option<Stmt> = None;
        for (test, mut body) in tests.into_iter().zip(bodies).rev() {
            match test {
                None => {
                    if checks_null {
                        body.insert(0, Stmt::Expr(self.null_check(selector)));
                    }
                    rest = Some(Stmt::Block(body));
                }
                Some(cond) => {
                    let otherwise =
                        rest.take().unwrap_or_else(|| Stmt::Expr(self.unmatched(node, selector, Type::Void)));
                    rest = Some(Stmt::If {
                        cond,
                        then: Box::new(Stmt::Block(body)),
                        otherwise: Some(Box::new(otherwise)),
                    });
                }
            }
        }
        Ok(rest.unwrap_or(Stmt::Block(Vec::new())))
    }

    /// The `?:` chain of a switch expression whose cases are all expressions.
    fn conditional_chain(
        &self,
        node: Node<'t>,
        selector: &Expr,
        mut arms: Vec<Arm<'t>>,
        ty: Type,
    ) -> Result<Expr, Error> {
        let checks_null = Self::checks_null(&arms);
        let mut parts = Vec::new();
        for arm in &mut arms {
            let body = arm.case.child("body")?;
            let value = self.with_aliases(&arm.aliases, || self.expr(body))?;
            parts.push((self.arm_test(arm)?, self.convert(value, ty)));
        }
        let mut rest: Option<Expr> = None;
        for (test, value) in parts.into_iter().rev() {
            rest = Some(match test {
                None if checks_null => {
                    let pos = value.pos;
                    self.synth(
                        ExprKind::Builtin(Builtin::NullChecked, vec![self.copy(selector), value]),
                        pos,
                        ty,
                    )
                }
                None => value,
                Some(cond) => {
                    let otherwise = rest.take().unwrap_or_else(|| self.unmatched(node, selector, ty));
                    let pos = cond.pos;
                    self.synth(
                        ExprKind::Conditional(Box::new(cond), Box::new(value), Box::new(otherwise)),
                        pos,
                        ty,
                    )
                }
            });
        }
        Ok(rest.expect("a pattern switch has a case"))
    }

    /// Another expression of the selector, a variable.
    fn copy(&self, selector: &Expr) -> Expr {
        let ExprKind::Var(name) = &selector.kind else { unreachable!("the selector is a variable") };
        self.synth(ExprKind::Var(name.clone()), selector.pos, selector.ty)
    }

    /// The selector checked for null: `NullPointerException` when it is null.
    fn null_check(&self, selector: &Expr) -> Expr {
        self.synth(
            ExprKind::Builtin(Builtin::NullChecked, vec![self.copy(selector)]),
            selector.pos,
            selector.ty,
        )
    }

    /// The end of an exhaustive switch that no case matched, which only null reaches.
    fn unmatched(&self, node: Node<'t>, selector: &Expr, ty: Type) -> Expr {
        self.synth(ExprKind::Builtin(Builtin::Unmatched, vec![self.copy(selector)]), node.pos(), ty)
    }
}

/// Where a value that a pattern names without testing it is.
fn target(value: Expr) -> Target {
    match value.kind {
        ExprKind::Var(name) => Target::Var(name, value.ty),
        ExprKind::Field { object, name } => component(*object, name, value.ty, None),
        ExprKind::MethodCall { receiver, name, resolved, .. } => {
            component(*receiver, name, value.ty, resolved)
        }
        _ => unreachable!("a pattern tests the selector or a component"),
    }
}

fn component(record: Expr, field: String, field_ty: Type, accessor: Option<usize>) -> Target {
    let ExprKind::Var(name) = record.kind else { unreachable!("a component of a bound record") };
    Target::Component { record: name, record_ty: record.ty, field, field_ty, accessor }
}
