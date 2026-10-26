//! Tail position: when the last statement of a function body ends in `return x;` on every path,
//! it is the function's value, so it is written as the tail expression `x`, with no `return`.
//! That covers a final `return`, an `if` whose branches all end in one, and a `switch` whose
//! cases all do. Anything else keeps its `return`s.

use super::*;

impl<'a> Gen<'a> {
    /// Whether `stmt`, as the last statement of a function body, is the function's value: every
    /// path through it ends in `return x;`.
    pub(super) fn tail_value(&self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Return(Some(_), _) => true,
            Stmt::Block(stmts) => stmts.last().is_some_and(|s| self.tail_value(s)),
            Stmt::If { then, otherwise: Some(otherwise), .. } => {
                self.tail_value(then) && self.tail_value(otherwise)
            }
            Stmt::Switch { selector, cases } => self.tail_switch(selector, cases),
            _ => false,
        }
    }

    /// The statement that is the function's value (see `tail_value`), written as its tail
    /// expression. The guard sets are those of the whole statement, as in `stmt`.
    pub(super) fn tail_stmt(&mut self, stmt: &'a Stmt) {
        let conflicts = self.stmt_conflicts(stmt);
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        match stmt {
            Stmt::Return(Some(value), _) => {
                let text = match self.returned_closure(value) {
                    // The captures of a returned closure are bound on lines of their own.
                    Some((prelude, closure)) => {
                        for line in prelude {
                            self.line(&line);
                        }
                        self.ok(closure)
                    }
                    None => {
                        let code = self.returned(value);
                        self.ok(last_expression(value, code.text))
                    }
                };
                self.line(&text);
            }
            Stmt::Block(stmts) => self.stmts(stmts, true),
            Stmt::If { cond, then, otherwise } => self.if_stmt(cond, then, otherwise.as_deref(), true),
            Stmt::Switch { selector, cases } => self.switch_form(selector, cases, None, true),
            _ => self.stmt(stmt, &[]),
        }
        self.guarded = outer;
    }

    /// Whether the last two statements of a function body are `T x = e;` and `return x;`, with
    /// `T` the return type and a primitive. Then `e` is the function's value. Primitives only: a
    /// reference local may be a shared or optional one, which the declaration handles itself.
    pub(super) fn folds_into_return(&self, stmts: &[Stmt]) -> bool {
        let [.., Stmt::Local { ty, name, init: Some(_), .. }, Stmt::Return(Some(value), _)] = stmts else {
            return false;
        };
        *ty == self.ret && !ty.is_reference() && matches!(&value.kind, ExprKind::Var(v) if v == name)
    }

    /// The initializer `e` of `T x = e;`, which `folds_into_return` found before `return x;`,
    /// written as the function's value.
    pub(super) fn tail_local(&mut self, stmt: &'a Stmt) {
        let Stmt::Local { init: Some(value), .. } = stmt else {
            unreachable!("folds_into_return only finds a local with an initializer")
        };
        let conflicts = self.stmt_conflicts(stmt);
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        let code = self.returned(value);
        let text = self.ok(last_expression(value, code.text));
        self.line(&text);
        self.guarded = outer;
    }

    /// The value `text` as the result of the current method: `Ok(text)` in a method that throws.
    pub(super) fn ok(&self, text: String) -> String {
        if self.throwing { format!("Ok({text})") } else { text }
    }

    /// The statements of a case of a switch expression, then its value. `T x = e; yield x;`, with
    /// `T` the switch's type and a primitive, is the value `e`, as `tail_local` makes it for a
    /// function.
    pub(super) fn yielded(&mut self, init: &'a [Stmt], value: &'a Expr, ty: Type) {
        if let Some((last @ Stmt::Local { ty: local, name, init: Some(folded), .. }, rest)) =
            init.split_last()
            && *local == ty
            && !ty.is_reference()
            && matches!(&value.kind, ExprKind::Var(v) if v == name)
        {
            self.stmts(rest, false);
            let conflicts = self.stmt_conflicts(last);
            let outer = std::mem::replace(&mut self.guarded, conflicts);
            let code = self.value(folded, ty);
            self.line(&last_expression(folded, code.text));
            self.guarded = outer;
            return;
        }
        self.stmts(init, false);
        let code = if self.optional_yield { self.optional(value) } else { self.value(value, ty) };
        self.line(&last_expression(value, code.text));
    }

    /// The body of an `if` or a loop. In tail position (`tail`), a branch that is the value of
    /// the function is written as its tail expression.
    pub(super) fn nested_in(&mut self, stmt: &'a Stmt, tail: bool) {
        self.indent += 1;
        match stmt {
            Stmt::Block(stmts) => self.stmts(stmts, tail),
            _ if tail => self.tail_stmt(stmt),
            _ => self.stmt(stmt, &[]),
        }
        self.indent -= 1;
    }

    /// An `if` and its `else if` chain. In tail position every branch is a value, so the `if` is
    /// the value of the function, also in `narrow_if`'s forms.
    pub(super) fn if_stmt(
        &mut self,
        cond: &'a Expr,
        then: &'a Stmt,
        otherwise: Option<&'a Stmt>,
        tail: bool,
    ) {
        if self.narrow_if(cond, then, otherwise, tail) {
            return;
        }
        let (cond_text, narrowed) = self.guard(cond, then);
        self.line(&format!("if {cond_text} {{"));
        self.nested_in(then, tail);
        self.unnarrow(narrowed);
        let mut otherwise = otherwise;
        while let Some(stmt) = otherwise {
            // `else { if .. }` is `else if ..`: the block holds nothing else.
            let stmt = match stmt {
                Stmt::Block(inner) => match inner.as_slice() {
                    [only @ Stmt::If { .. }] => only,
                    _ => stmt,
                },
                _ => stmt,
            };
            // An `if` that `narrow_if` writes is nested in the `else`.
            if let Stmt::If { cond, then, otherwise: next } = stmt
                && !self.narrows_else(cond, next.as_deref())
            {
                // Its condition is a statement's own: the borrows in it may conflict where those
                // of the first condition do not (`else if (!c.type.isErroneous())` completes `c`
                // while `c.base()` would hold it).
                let conflicts = self.stmt_conflicts(stmt);
                let outer = std::mem::replace(&mut self.guarded, conflicts);
                let (cond_text, narrowed) = self.guard(cond, then);
                self.guarded = outer;
                self.line(&format!("}} else if {cond_text} {{"));
                self.nested_in(then, tail);
                self.unnarrow(narrowed);
                otherwise = next.as_deref();
            } else {
                self.line("} else {");
                self.nested_in(stmt, tail);
                otherwise = None;
            }
        }
        self.line("}");
    }
}

/// The text of `value` as the last expression of a block: in parentheses where it starts with
/// a `match`, an `if` or a block and goes on past it (`match .. { .. } || b`), which Rust would
/// otherwise take for a statement followed by a closure or an operator's operand.
fn last_expression(value: &Expr, text: String) -> String {
    let block_first = ["match ", "if ", "loop ", "{", "unsafe {", "'"].iter().any(|k| text.starts_with(k));
    let goes_on = matches!(
        value.kind,
        ExprKind::Binary(..)
            | ExprKind::Cast(..)
            | ExprKind::MethodCall { .. }
            | ExprKind::Field { .. }
            | ExprKind::Index(..)
            | ExprKind::InstanceOf { .. }
    );
    if block_first && goes_on { format!("({text})") } else { text }
}
