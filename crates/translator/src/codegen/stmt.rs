//! Statements: blocks, `if`, loops (counting loops become `for` over a range), locals, returns.

use super::*;

/// Whether a loop body contains a `continue`, labeled or not. This is conservative: a `continue`
/// of a nested loop does not need the loop's steps, but counting it costs only a plainer frame.
pub(super) fn has_continue(body: &Stmt) -> bool {
    let mut found = false;
    visit_stmt(body, &mut |_| {}, &mut |s| found |= matches!(s, Stmt::Continue | Stmt::ContinueLabel(_)));
    found
}

impl<'a> Gen<'a> {
    /// The statements of a block. In a function body (`tail`), a final `return x;` becomes the
    /// tail expression `x`.
    pub(super) fn stmts(&mut self, stmts: &'a [Stmt], tail: bool) {
        let mut narrowed = Vec::new();
        for (i, stmt) in stmts.iter().enumerate() {
            let rest = &stmts[i + 1..];
            if let Some(name) = self.early_null_exit(stmt, rest) {
                if self.borrowed.insert(name.to_string()) {
                    narrowed.push(name);
                }
                continue;
            }
            match stmt {
                Stmt::Return(Some(value), _) if tail && rest.is_empty() => {
                    let conflicts = self.stmt_conflicts(stmt);
                    let outer = std::mem::replace(&mut self.guarded, conflicts);
                    let code = self.returned(value);
                    self.line(&code.text);
                    self.guarded = outer;
                }
                _ => self.stmt(stmt, rest),
            }
        }
        for name in narrowed {
            self.borrowed.remove(name);
        }
    }

    /// The body of an `if` or a loop.
    pub(super) fn nested(&mut self, stmt: &'a Stmt) {
        self.indent += 1;
        match stmt {
            Stmt::Block(stmts) => self.stmts(stmts, false),
            _ => self.stmt(stmt, &[]),
        }
        self.indent -= 1;
    }

    /// One statement; `rest` holds the statements after it in the same block, where a local
    /// declared here is in scope.
    pub(super) fn stmt(&mut self, stmt: &'a Stmt, rest: &'a [Stmt]) {
        let conflicts = self.stmt_conflicts(stmt);
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        self.statement(stmt, rest);
        self.guarded = outer;
    }

    pub(super) fn statement(&mut self, stmt: &'a Stmt, rest: &'a [Stmt]) {
        match stmt {
            Stmt::Local { ty, name, init, .. } => {
                self.local_decl(*ty, name, init.as_ref(), self.local_is_mut(*ty, name, rest));
            }
            Stmt::Expr(e) => self.effect(e),
            Stmt::ForEach { name, array, body, .. } => self.for_each(name, array, body),
            Stmt::If { cond, then, otherwise } => {
                if self.narrow_if(cond, then, otherwise.as_deref()) {
                    return;
                }
                let cond_text = self.condition(cond);
                self.line(&format!("if {cond_text} {{"));
                self.nested(then);
                let mut otherwise = otherwise.as_deref();
                while let Some(stmt) = otherwise {
                    if let Stmt::If { cond, then, otherwise: next } = stmt {
                        let cond_text = self.condition(cond);
                        self.line(&format!("}} else if {cond_text} {{"));
                        self.nested(then);
                        otherwise = next.as_deref();
                    } else {
                        self.line("} else {");
                        self.nested(stmt);
                        otherwise = None;
                    }
                }
                self.line("}");
            }
            Stmt::While { cond, body } => {
                if matches!(cond.kind, ExprKind::Bool(true)) {
                    self.line("loop {");
                } else if let Some((cond_text, narrowed)) = self.let_chain(cond, &[body], &[]) {
                    self.line(&format!("while {cond_text} {{"));
                    self.loop_body(Loop::Plain, body, &[]);
                    self.line("}");
                    self.unnarrow(narrowed);
                    return;
                } else {
                    let cond_text = self.condition(cond);
                    self.line(&format!("while {cond_text} {{"));
                }
                self.loop_body(Loop::Plain, body, &[]);
                self.line("}");
            }
            Stmt::DoWhile { body, cond } => {
                let kind = if has_continue(body) { Loop::DoWhile(cond) } else { Loop::Plain };
                self.line("loop {");
                self.loop_body(kind, body, &[]);
                self.indent += 1;
                self.exit_unless(cond);
                self.indent -= 1;
                self.line("}");
            }
            Stmt::For { init, cond, update, body } => self.for_loop(init, cond.as_ref(), update, body),
            Stmt::Return(None, _) => match self.ctor_result {
                Some(result) => self.line(&format!("return {result};")),
                None => self.line("return;"),
            },
            Stmt::Return(Some(value), _) => {
                let code = self.returned(value);
                self.line(&format!("return {};", code.text));
            }
            Stmt::Break => self.plain_jump(false),
            Stmt::Continue => self.plain_jump(true),
            Stmt::Switch { selector, cases } => self.switch_form(selector, cases, None),
            Stmt::Labeled { label, body } => self.labeled(label, body),
            Stmt::BreakLabel(label) => self.labeled_jump(label, false),
            Stmt::ContinueLabel(label) => self.labeled_jump(label, true),
            Stmt::Yield(value) => self.yield_value(value),
            Stmt::Block(stmts) => {
                self.line("{");
                self.indent += 1;
                self.stmts(stmts, false);
                self.indent -= 1;
                self.line("}");
            }
        }
    }

    /// The value of a `return`: a string variable is moved out rather than copied, since the
    /// method ends.
    pub(super) fn returned(&mut self, value: &'a Expr) -> Code {
        if self.display {
            // `Display::fmt` writes the text that `toString` returns.
            let mut parts = Vec::new();
            self.parts(value, &mut parts);
            let text = self.format("write!", parts).text;
            return Code::atom(text.replacen("write!(", "write!(f, ", 1));
        }
        if self.optional_return() {
            return self.optional_to(value, self.ret);
        }
        if let ExprKind::Var(name) = &value.kind
            && value.ty == Type::String
            && !self.borrowed.contains(name)
        {
            return Code::atom(self.local(name));
        }
        self.value(value, self.ret)
    }

    /// `if !cond { break; }`, ending a translated `do`-`while` loop.
    pub(super) fn exit_unless(&mut self, cond: &'a Expr) {
        let negated = self.negate(cond);
        self.line(&format!("if {negated} {{"));
        self.indent += 1;
        self.line("break;");
        self.indent -= 1;
        self.line("}");
    }

    /// The body of a loop whose header was just written: the loop is a frame around it.
    pub(super) fn loop_body(&mut self, kind: Loop<'a>, body: &'a Stmt, update: &'a [Expr]) {
        self.open(Kind::Loop(kind));
        self.nested(body);
        self.indent += 1;
        for u in update {
            self.update_effect(u);
        }
        self.indent -= 1;
        self.close();
    }

    /// An update expression of a `for` loop, a statement of its own.
    pub(super) fn update_effect(&mut self, u: &'a Expr) {
        let conflicts = self.conflicts(&[u]) || self.reassigns_through_borrow(u);
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        self.effect(u);
        self.guarded = outer;
    }

    pub(super) fn for_loop(
        &mut self,
        init: &'a [Stmt],
        cond: Option<&'a Expr>,
        update: &'a [Expr],
        body: &'a Stmt,
    ) {
        if let Some(cond) = cond
            && self.element_loop(cond, body)
        {
            return;
        }
        if let Some((var, start, end)) = counting_loop(init, cond, update, body) {
            // `counting_loop` checks that the bound has the loop variable's type.
            let ty = end.ty;
            let start = self.value(start, ty);
            let end = self.value(end, ty);
            let mut used = false;
            visit_stmt(body, &mut |e| used |= mentions(e, var), &mut |_| {});
            let var = if used { self.local(var) } else { "_".to_string() };
            let start_text = if used && start.untyped && end.untyped {
                let suffix = self.annotation(ty, rust_type(ty).to_string());
                if suffix == rust_type(ty) { start.typed() } else { format!("{}{suffix}", start.text) }
            } else {
                start.at(ADD + 1)
            };
            self.line(&format!("for {var} in {start_text}..{} {{", end.at(ADD + 1)));
            self.loop_body(Loop::Plain, body, &[]);
            self.line("}");
            return;
        }
        // The loop variables stay in scope after the loop, which is harmless: Java forbids
        // using them there, and a later declaration of the same name shadows them.
        for (i, stmt) in init.iter().enumerate() {
            match stmt {
                Stmt::Local { ty, name, init: value, .. } => {
                    // The variable is in scope in the condition, the update and the body.
                    let mutable = self.local_is_mut(*ty, name, &init[i + 1..])
                        || update.iter().any(|u| assigns(u, name))
                        || cond.is_some_and(|c| assigns(c, name))
                        || assigns_stmt(body, name);
                    self.local_decl(*ty, name, value.as_ref(), mutable);
                }
                _ => self.stmt(stmt, &[]),
            }
        }
        match cond {
            None => self.line("loop {"),
            Some(cond) => {
                let cond_text = self.condition(cond);
                self.line(&format!("while {cond_text} {{"));
            }
        }
        let kind = if has_continue(body) && !update.is_empty() { Loop::For(update) } else { Loop::Plain };
        self.loop_body(kind, body, update);
        self.line("}");
    }

    pub(super) fn local_decl(&mut self, ty: Type, name: &str, init: Option<&'a Expr>, mutable: bool) {
        if self.optional_var(self.method, ty, name) {
            return self.optional_local(name, ty, init, mutable);
        }
        if let Some(written) = self.plain(name) {
            self.plain_local(ty, name, init, written);
            return;
        }
        let mutable = if mutable { "mut " } else { "" };
        let rust = self.local(name);
        match init {
            None => {
                let ty_text = self.ty_name(ty);
                self.line(&format!("let {mutable}{rust}: {ty_text};"));
            }
            Some(init) => {
                let code = self.value(init, ty);
                // An array's element type cannot be inferred from `from_vec(vec![])`, so it is always written.
                let annotation = if code.untyped || matches!(ty, Type::Array(_)) {
                    let ty_text = self.ty_name(ty);
                    self.annotation(ty, format!(": {ty_text}"))
                } else {
                    String::new()
                };
                self.line(&format!("let {mutable}{rust}{annotation} = {};", code.text));
            }
        }
    }

    /// The condition of an `if` or `while`, which may not start with a block in Rust.
    pub(super) fn condition(&mut self, cond: &'a Expr) -> String {
        let code = self.expr(cond);
        if code.text.starts_with('{') { format!("({})", code.text) } else { code.text }
    }

    /// The negation of a condition, flipping the comparison where that is exact.
    pub(super) fn negate(&mut self, cond: &'a Expr) -> String {
        match &cond.kind {
            ExprKind::Unary(UnOp::Not, inner) => return self.condition(inner),
            ExprKind::Binary(op, l, r)
                if op.is_comparison()
                    && !l.ty.is_floating()
                    && !r.ty.is_floating()
                    && !matches!(l.ty, Type::Class(_))
                    && !is_null_test(*op, l, r) =>
            {
                let flipped = match op {
                    BinOp::Eq => BinOp::Ne,
                    BinOp::Ne => BinOp::Eq,
                    BinOp::Lt => BinOp::Ge,
                    BinOp::Le => BinOp::Gt,
                    BinOp::Gt => BinOp::Le,
                    _ => BinOp::Lt,
                };
                let (l, r) = (self.operand(l), self.operand(r));
                return self.binary(flipped, l, r, Type::Boolean, cond.id).text;
            }
            _ => {}
        }
        format!("!{}", self.expr(cond).at(UNARY))
    }

    /// An expression evaluated for its effect, as a statement.
    pub(super) fn effect(&mut self, e: &'a Expr) {
        match &e.kind {
            ExprKind::Assign { op, target: Place::Var(target), value } => {
                let text = self.assignment(*op, target, e.ty, value, e.id);
                self.line(&format!("{text};"));
            }
            ExprKind::IncDec { target: Place::Var(target), increment, .. } => {
                let text = self.step(target, e.ty, *increment, e.id);
                self.line(&format!("{text};"));
            }
            ExprKind::Assign { op: None, target: Place::Index(array, index), value } => {
                let text = self.index_set(e.id, array, index, e.ty, value, false);
                self.line(&format!("{text};"));
            }
            ExprKind::Assign { op: Some(op), target: Place::Index(array, index), value } => {
                let text = self.index_update(*op, array, index, e.ty, value, e.id, false);
                self.line(&format!("{text};"));
            }
            ExprKind::IncDec { target: Place::Index(array, index), increment, prefix } => {
                let text = self.index_step(array, index, e.ty, *increment, *prefix, e.id, false);
                self.line(&format!("{text};"));
            }
            ExprKind::Assign { target: Place::Static { .. }, .. }
            | ExprKind::IncDec { target: Place::Static { .. }, .. }
                if let Some(lines) = self.static_effect(e) =>
            {
                for line in lines {
                    self.line(&line);
                }
            }
            ExprKind::Assign { target: Place::Field { .. }, .. }
            | ExprKind::IncDec { target: Place::Field { .. }, .. }
                if let Some(text) = self.field_effect(e) =>
            {
                // A field updated through a borrow of its own is a block.
                self.line(&if text.starts_with("{ let ") { text } else { format!("{text};") });
            }
            // A chained builder call is made for its effect on the builder.
            ExprKind::Builtin(Builtin::Builder(op), args) if op.chains() => {
                let code = self.builder_call(*op, args);
                self.line(&format!("{};", code.text));
            }
            _ => {
                let code = self.expr(e);
                self.line(&format!("{};", code.text));
            }
        }
    }
}
