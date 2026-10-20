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
        let selector = self.selector(node)?;
        let cases = self.cases(node, false)?;
        Ok(Stmt::Switch { selector, cases })
    }

    pub(super) fn switch_expr(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let selector = self.selector(node)?;
        let cases = self.cases(node, true)?;
        Ok(ExprKind::Switch { selector: Box::new(selector), cases })
    }

    /// `label: statement`, which `break label` and `continue label` name.
    pub(super) fn labeled(&self, node: Node<'t>) -> Result<Stmt, Error> {
        let body = self.single_stmt(node.child("statement")?)?;
        Ok(Stmt::Labeled { label: node.str("label").to_string(), body: Box::new(body) })
    }

    /// The selector, which must be one of the types that Rust matches on directly.
    fn selector(&self, node: Node<'t>) -> Result<Expr, Error> {
        let selector = self.expr(node.child("expression")?)?;
        match selector.ty {
            Type::Int | Type::Char | Type::Byte | Type::Short | Type::String => Ok(selector),
            _ => unsupported(node, "a switch on this type"),
        }
    }

    /// The cases of a switch. Labels of empty `case` groups join the next group, since they run
    /// nothing; an arrow case ends with `break` (a statement) or `yield` (an expression), so it
    /// never falls through.
    fn cases(&self, node: Node<'t>, value: bool) -> Result<Vec<Case>, Error> {
        let nodes: Vec<Node> = node.list("cases").collect();
        let mut cases = Vec::new();
        let mut joining = Vec::new();
        for (i, case) in nodes.iter().enumerate() {
            if case.get("guard").is_some() {
                return unsupported(*case, "guards in a switch");
            }
            let mut labels = std::mem::take(&mut joining);
            for label in case.list("labels") {
                labels.push(self.case_label(label)?);
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
    fn arrow_body(&self, node: Node<'t>, value: bool) -> Result<Vec<Stmt>, Error> {
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

    fn case_label(&self, node: Node<'t>) -> Result<CaseLabel, Error> {
        match node.kind() {
            "DEFAULT_CASE_LABEL" => Ok(CaseLabel::Default),
            "CONSTANT_CASE_LABEL" => {
                let constant = node.child("constantExpression")?;
                match constant.kind() {
                    "INT_LITERAL" | "CHAR_LITERAL" | "STRING_LITERAL" => {
                        Ok(CaseLabel::Value(self.expr(constant)?))
                    }
                    _ => unsupported(constant, "a case label that is not a literal"),
                }
            }
            _ => unsupported(node, "this case label"),
        }
    }
}
