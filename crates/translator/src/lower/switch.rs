//! Lowering of `switch` statements and expressions, labeled statements and the `break` and
//! `continue` that name them. javac has already folded the case constants (`case -4` is one
//! literal) and reports which cases are arrows.

use super::*;

/// `break`, `continue`, or the same with a label.
pub(super) fn jump(node: Node) -> Stmt {
    let label = node.str("label");
    match (node.kind() == "BREAK", label.is_empty()) {
        (true, true) => Stmt::Break,
        (false, true) => Stmt::Continue,
        (true, false) => Stmt::BreakLabel(label.to_string()),
        (false, false) => Stmt::ContinueLabel(label.to_string()),
    }
}

/// Whether `stmt` reads or assigns the local `name`.
fn uses_local(stmt: &Stmt, name: &str) -> bool {
    let mut used = false;
    visit_stmt(
        stmt,
        &mut |e| {
            visit_expr(e, &mut |x| {
                used |= match &x.kind {
                    ExprKind::Var(v)
                    | ExprKind::Assign { target: Place::Var(v), .. }
                    | ExprKind::IncDec { target: Place::Var(v), .. } => v == name,
                    _ => false,
                }
            })
        },
        &mut |_| {},
    );
    used
}

/// Whether a statement always leaves the code after it, so no `break` is needed after it.
fn ends_by_leaving(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Break
            | Stmt::Continue
            | Stmt::Return(..)
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_)
            | Stmt::Yield(_)
    )
}

impl<'t> Lowering<'t> {
    pub(super) fn switch_stmt(&self, node: Node<'t>) -> Result<Stmt, Error> {
        if patterns::is_pattern_switch(node) {
            return self.pattern_switch_stmt(node);
        }
        let selector = self.selector(node)?;
        let mut cases = self.cases(node, selector.ty, false)?;
        let mut hoisted = self.hoist_shared_locals(&mut cases);
        if hoisted.is_empty() {
            return Ok(Stmt::Switch { selector, cases });
        }
        hoisted.push(Stmt::Switch { selector, cases });
        Ok(Stmt::Block(hoisted))
    }

    /// The locals that a case declares at its top level and a later case uses (`case 3: int b
    /// = ...; ... case 4: b = ...;`): the cases of a `switch` block are one scope in Java and
    /// each an arm of its own in Rust. Each is declared before the switch, without a value, and
    /// its declaration becomes an assignment of its initializer. Java requires that it is
    /// assigned before each use, which Rust then checks the same way.
    fn hoist_shared_locals(&self, cases: &mut [Case]) -> Vec<Stmt> {
        let mut hoisted = Vec::new();
        for i in 0..cases.len() {
            let (before, after) = cases.split_at_mut(i + 1);
            for stmt in before[i].body.iter_mut() {
                let Stmt::Local { ty, name, init, pos } = stmt else { continue };
                if !after.iter().any(|c| c.body.iter().any(|s| uses_local(s, name))) {
                    continue;
                }
                let (ty, pos, name) = (*ty, *pos, name.clone());
                let assigned = init.take().map(|value| {
                    let kind = ExprKind::Assign {
                        op: None,
                        target: Place::Var(name.clone()),
                        value: Box::new(value),
                    };
                    Stmt::Expr(self.synth(kind, pos, ty))
                });
                *stmt = assigned.unwrap_or(Stmt::Block(Vec::new()));
                hoisted.push(Stmt::Local { ty, name, init: None, pos });
            }
        }
        hoisted
    }

    pub(super) fn switch_expr(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let ty = self.ty(node, node.java_type())?;
        let mut kind = if patterns::is_pattern_switch(node) {
            self.pattern_switch_expr(node)?
        } else {
            let selector = self.selector(node)?;
            let cases = self.cases(node, selector.ty, true)?;
            ExprKind::Switch { selector: Box::new(selector), cases }
        };
        if let ExprKind::Switch { cases, .. } = &mut kind {
            for case in cases {
                self.convert_yields(&mut case.body, ty);
            }
        }
        Ok(kind)
    }

    /// Converts the values that these statements of a switch expression yield to its type `ty`
    /// (a subclass's object to its superclass, an `int` to a `long`).
    fn convert_yields(&self, stmts: &mut [Stmt], ty: Type) {
        for stmt in stmts {
            match stmt {
                Stmt::Yield(value) => {
                    let placeholder = self.synth(ExprKind::Null, value.pos, Type::Null);
                    let old = std::mem::replace(value, placeholder);
                    *value = self.convert(old, ty);
                }
                Stmt::Block(body) => self.convert_yields(body, ty),
                Stmt::If { then, otherwise, .. } => {
                    self.convert_yields(std::slice::from_mut(then.as_mut()), ty);
                    if let Some(o) = otherwise {
                        self.convert_yields(std::slice::from_mut(o.as_mut()), ty);
                    }
                }
                Stmt::While { body, .. }
                | Stmt::DoWhile { body, .. }
                | Stmt::For { body, .. }
                | Stmt::ForEach { body, .. }
                | Stmt::Labeled { body, .. } => self.convert_yields(std::slice::from_mut(body.as_mut()), ty),
                Stmt::Switch { cases, .. } => {
                    cases.iter_mut().for_each(|c| self.convert_yields(&mut c.body, ty))
                }
                Stmt::Try { body, catches, finally } => {
                    self.convert_yields(body, ty);
                    catches.iter_mut().for_each(|c| self.convert_yields(&mut c.body, ty));
                    if let Some(f) = finally {
                        self.convert_yields(f, ty);
                    }
                }
                _ => {}
            }
        }
    }

    /// `label: statement`, which `break label` and `continue label` name.
    pub(super) fn labeled(&self, node: Node<'t>) -> Result<Stmt, Error> {
        let statement = node.child("statement")?;
        // A for-each over an iterable is a block around the loop, which takes the label itself.
        if statement.kind() == "ENHANCED_FOR_LOOP" {
            return self.for_each(statement, Some(node.str("label")));
        }
        let body = self.single_stmt(statement)?;
        Ok(Stmt::Labeled { label: node.str("label").to_string(), body: Box::new(body) })
    }

    /// The selector, which must be one of the types that Rust matches on directly.
    fn selector(&self, node: Node<'t>) -> Result<Expr, Error> {
        let selector = self.expr(node.child("expression")?)?;
        match selector.ty {
            Type::Int | Type::Char | Type::Byte | Type::Short | Type::String | Type::Enum(_) => Ok(selector),
            _ => unsupported(node, "a switch on this type"),
        }
    }

    /// The cases of a switch. Labels of empty `case` groups join the next group, since they run
    /// nothing; an arrow case ends with `break` (a statement) or `yield` (an expression), so it
    /// never falls through.
    fn cases(&self, node: Node<'t>, ty: Type, value: bool) -> Result<Vec<Case>, Error> {
        let nodes: Vec<Node> = node.list("cases").collect();
        let mut cases = Vec::new();
        let mut joining = Vec::new();
        for (i, case) in nodes.iter().enumerate() {
            if case.get("guard").is_some() {
                return unsupported(*case, "guards in a switch");
            }
            let mut labels = std::mem::take(&mut joining);
            for label in case.list("labels") {
                labels.push(self.case_label(label, ty)?);
            }
            let body = if case.str("caseKind") == "RULE" {
                let mut body = self.arrow_body(case.child("body")?, value)?;
                if !value && !body.last().is_some_and(ends_by_leaving) {
                    body.push(Stmt::Break);
                }
                body
            } else {
                self.block(*case)?
            };
            if body.is_empty() && i + 1 < nodes.len() {
                joining = labels;
                continue;
            }
            cases.push(Case { labels, body });
        }
        Ok(cases)
    }

    /// The statements of an arrow case. A block is flattened into the case, which is a scope of
    /// its own; an expression is a statement, or the value of a switch expression.
    pub(super) fn arrow_body(&self, node: Node<'t>, value: bool) -> Result<Vec<Stmt>, Error> {
        match node.kind() {
            "BLOCK" => self.block(node),
            "EXPRESSION_STATEMENT" | "THROW" => {
                let mut out = Vec::new();
                self.stmt_into(node, &mut out)?;
                Ok(out)
            }
            _ if value => Ok(vec![Stmt::Yield(self.expr(node)?)]),
            _ => Ok(vec![Stmt::Expr(self.expr(node)?)]),
        }
    }

    /// A case label, as a literal of the selector's type `ty`: a constant expression (`case
    /// MAX - 1`, `case 'a' + 1`, `case PREFIX + "x"`) is folded as javac folds it.
    pub(super) fn case_label(&self, node: Node<'t>, ty: Type) -> Result<CaseLabel, Error> {
        match node.kind() {
            "DEFAULT_CASE_LABEL" => Ok(CaseLabel::Default),
            "CONSTANT_CASE_LABEL" => {
                let constant = node.child("constantExpression")?;
                if let Type::Enum(_) = ty {
                    let Some(kind) = constant.symbol().and_then(|s| self.enum_constant(&s)) else {
                        return unsupported(constant, "this case label");
                    };
                    return Ok(CaseLabel::Value(self.synth(kind, constant.pos(), ty)));
                }
                // A constant field of the selector's type is matched by its name, a Rust `const`.
                if matches!(constant.kind(), "IDENTIFIER" | "MEMBER_SELECT")
                    && constant.symbol().is_some_and(|s| s.constant.is_some())
                    && let Ok(named) = self.expr(constant)
                    && matches!(named.kind, ExprKind::Static { .. })
                    && named.ty == ty
                {
                    return Ok(CaseLabel::Value(named));
                }
                let kind = match (fold(constant), ty) {
                    (Some(Folded::Text(s)), Type::String) => ExprKind::Str(s),
                    (Some(Folded::Number(v)), Type::Char) => ExprKind::Char(v as u16),
                    (Some(Folded::Number(v)), Type::Int | Type::Short | Type::Byte) => {
                        ExprKind::Int(v as i32)
                    }
                    _ => return unsupported(constant, "a case label that is not a constant"),
                };
                Ok(CaseLabel::Value(self.synth(kind, constant.pos(), ty)))
            }
            _ => unsupported(node, "this case label"),
        }
    }
}

/// The value of a constant expression.
enum Folded {
    /// An integral value (`int`, `char`, `short`, `byte` or `long`), already wrapped to its type.
    Number(i64),
    Text(String),
}

/// `v` wrapped to the integral type named `ty`, as Java's arithmetic in that type leaves it.
fn wrap(v: i64, ty: &str) -> Option<i64> {
    Some(match ty {
        "int" => v as i32 as i64,
        "char" => v as u16 as i64,
        "short" => v as i16 as i64,
        "byte" => v as i8 as i64,
        "long" => v,
        _ => return None,
    })
}

/// Whether `node` is a constant expression of type `String`, which Java interns.
/// The text of `node`, a constant expression of type `String`; `None` for any other.
pub(super) fn constant_text(node: Node) -> Option<String> {
    if node.java_type() != "java.lang.String" {
        return None;
    }
    match fold(node)? {
        Folded::Text(s) => Some(s),
        _ => None,
    }
}

pub(super) fn constant_string(node: Node) -> bool {
    node.java_type() == "java.lang.String" && matches!(fold(node), Some(Folded::Text(_)))
}

/// Folds a constant expression of integral or string type, as javac does; `None` for anything
/// else (a `double` constant, a call, a name that is no constant).
fn fold(node: Node) -> Option<Folded> {
    let ty = node.java_type();
    let number = |n: Node| match fold(n)? {
        Folded::Number(v) => Some(v),
        Folded::Text(_) => None,
    };
    let text = |f: Folded, ty: &str| match f {
        Folded::Text(s) => Some(s),
        Folded::Number(v) if ty == "char" => Some(char::from_u32(v as u32)?.to_string()),
        Folded::Number(v) if ty != "boolean" => Some(v.to_string()),
        Folded::Number(_) => None,
    };
    if ty == "java.lang.String" {
        return match node.kind() {
            "STRING_LITERAL" => node.0["value"].as_str().map(|s| Folded::Text(s.to_string())),
            "IDENTIFIER" | "MEMBER_SELECT" => Some(Folded::Text(node.symbol()?.constant?.to_string())),
            "PARENTHESIZED" => fold(node.get("expression")?),
            "PLUS" => {
                let (l, r) = (node.get("leftOperand")?, node.get("rightOperand")?);
                let left = text(fold(l)?, l.java_type())?;
                let right = text(fold(r)?, r.java_type())?;
                Some(Folded::Text(left + &right))
            }
            _ => None,
        };
    }
    let value = match node.kind() {
        "INT_LITERAL" | "LONG_LITERAL" | "CHAR_LITERAL" => node.str("value").parse().ok()?,
        "IDENTIFIER" | "MEMBER_SELECT" => node.symbol()?.constant?.parse().ok()?,
        "PARENTHESIZED" => number(node.get("expression")?)?,
        "TYPE_CAST" => number(node.get("expression")?)?,
        "UNARY_PLUS" => number(node.get("expression")?)?,
        "UNARY_MINUS" => number(node.get("expression")?)?.wrapping_neg(),
        "BITWISE_COMPLEMENT" => !number(node.get("expression")?)?,
        kind => {
            let (l, r) = (node.get("leftOperand")?, node.get("rightOperand")?);
            let (a, b) = (number(l)?, number(r)?);
            // A shift distance is masked to the width of the shifted type.
            let bits = if l.java_type() == "long" { 63 } else { 31 };
            match kind {
                "PLUS" => a.wrapping_add(b),
                "MINUS" => a.wrapping_sub(b),
                "MULTIPLY" => a.wrapping_mul(b),
                "DIVIDE" if b != 0 => a.wrapping_div(b),
                "REMAINDER" if b != 0 => a.wrapping_rem(b),
                "LEFT_SHIFT" => a.wrapping_shl((b & bits) as u32),
                "RIGHT_SHIFT" => a >> (b & bits),
                "UNSIGNED_RIGHT_SHIFT" if bits == 31 => ((a as u32) >> (b & bits)) as i64,
                "UNSIGNED_RIGHT_SHIFT" => ((a as u64) >> (b & bits)) as i64,
                "AND" => a & b,
                "OR" => a | b,
                "XOR" => a ^ b,
                _ => return None,
            }
        }
    };
    Some(Folded::Number(wrap(value, ty)?))
}
