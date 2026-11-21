//! Statements: blocks, `if`, loops (counting loops become `for` over a range), locals, returns.

use super::*;

/// Whether a loop body contains a `continue`, labeled or not. This is conservative: a `continue`
/// of a nested loop does not need the loop's steps, but counting it costs only a plainer frame.
pub(super) fn has_continue(body: &Stmt) -> bool {
    let mut found = false;
    visit_stmt_outside_switches(body, &mut |s| found |= matches!(s, Stmt::Continue | Stmt::ContinueLabel(_)));
    found
}

impl<'a> Gen<'a> {
    /// The statements of a block. In a function body (`tail`), a final `return x;` becomes the
    /// tail expression `x`, and `T x = e; return x;` becomes `e`.
    pub(super) fn stmts(&mut self, stmts: &'a [Stmt], tail: bool) {
        let folded = tail && self.folds_into_return(stmts);
        let end = if folded { stmts.len() - 2 } else { stmts.len() };
        // A loop's updates, written after this block's statements in their scope.
        let after = std::mem::take(&mut self.after_body);
        let mut narrowed = Vec::new();
        let mut synchronized = false;
        let outer_steady = self.steady.take();
        let known = self.known_init.len();
        let mut slot_reads = Vec::new();
        let mut unnamed: Vec<&'a str> = Vec::new();
        for (i, stmt) in stmts[..end].iter().enumerate() {
            let rest = &stmts[i + 1..];
            // The block of a `synchronized` that `synchronized_block` wrote with its lock.
            if std::mem::take(&mut synchronized) {
                continue;
            }
            self.steady = Some(strings::SteadyOf::new(stmt, rest.first()));
            if self.synchronized_block(stmt, rest.first(), tail && rest.len() == 1) {
                synchronized = true;
                continue;
            }
            self.steady = Some(strings::SteadyOf::new(stmt, None));
            if let Some(name) = self.early_null_exit(stmt, rest, after) {
                if self.borrowed.insert(name.to_string()) {
                    narrowed.push(name);
                }
                continue;
            }
            if self.early_pattern_exit(stmt, rest) {
                continue;
            }
            if let Some(name) = self.early_length_read(stmt, rest, after)
                && self.borrowed.insert(name.to_string())
            {
                narrowed.push(name);
            }
            match stmt {
                // A `throw` that ends a method that throws is its `Err` result.
                Stmt::Throw(value) if tail && rest.is_empty() && self.throwing => {
                    let text = self.thrown_text(value);
                    let text = self.cold_thrown(value, text);
                    self.line(&format!("Err({text})"));
                }
                _ if tail && rest.is_empty() && self.tail_value(stmt) => self.tail_stmt(stmt),
                _ => {
                    let site = self.declared_site(stmt);
                    self.stmt(stmt, rest);
                    if let Some(site) = site {
                        let reads = self.bind_slot_fields(stmt, site, rest);
                        unnamed.extend(self.unname_slot_object(stmt, site, rest, &reads));
                        slot_reads.extend(reads);
                    }
                    // The statements after it run only once it completes normally.
                    let method = self.method;
                    let initialized = self.initialized_by(stmt);
                    self.known_init.extend(initialized.into_iter().map(|c| (method, c)));
                }
            }
        }
        self.known_init.truncate(known);
        // The reads that read a stack object's fields from locals of this block read them no
        // more: the same code may be written again, in another function, with no such locals.
        for id in slot_reads {
            self.loop_fields.remove(&id);
        }
        // The locals of this block that name no object until a move are out of scope.
        for name in unnamed {
            self.unnamed_locals.remove(name);
        }
        if folded {
            self.steady = Some(strings::SteadyOf::new(&stmts[end], Some(&stmts[end + 1])));
            self.tail_local(&stmts[end]);
        }
        self.steady = outer_steady;
        for name in narrowed {
            self.borrowed.remove(name);
        }
    }

    /// The body of an `if` or a loop.
    pub(super) fn nested(&mut self, stmt: &'a Stmt) {
        self.nested_in(stmt, false);
    }

    /// One statement; `rest` holds the statements after it in the same block, where a local
    /// declared here is in scope.
    pub(super) fn stmt(&mut self, stmt: &'a Stmt, rest: &'a [Stmt]) {
        let conflicts = self.stmt_conflicts(stmt);
        let outer = std::mem::replace(&mut self.guarded, conflicts);
        let outer_steady = self.steady.replace(strings::SteadyOf::new(stmt, None));
        self.statement(stmt, rest);
        self.steady = outer_steady;
        self.guarded = outer;
    }

    pub(super) fn statement(&mut self, stmt: &'a Stmt, rest: &'a [Stmt]) {
        let named = self.move_to_heap(stmt);
        match stmt {
            Stmt::Local { ty, name, init: None, pos } => {
                let mutable = self.moved_local(*pos) || self.declared_local_is_mut(*ty, name, rest);
                self.local_decl(*ty, name, None, mutable);
            }
            Stmt::Local { ty, name, init, pos } => {
                let mutable = self.moved_local(*pos) || self.local_is_mut(*ty, name, rest);
                self.local_decl(*ty, name, init.as_ref(), mutable);
            }
            Stmt::Expr(e) => self.effect(e),
            Stmt::ForEach { name, array, body, .. } => self.for_each(name, array, body),
            Stmt::If { cond, then, otherwise } => self.if_stmt(cond, then, otherwise.as_deref(), false),
            Stmt::While { cond, body } => {
                let fields = self.bind_loop_fields(Some(cond), &[], body);
                self.while_loop(cond, body);
                self.unbind_loop_fields(fields);
            }
            Stmt::DoWhile { body, cond } => {
                let fields = self.bind_loop_fields(Some(cond), &[], body);
                self.do_while_loop(body, cond);
                self.unbind_loop_fields(fields);
            }
            Stmt::For { init, cond, update, body } => self.for_loop(init, cond.as_ref(), update, body),
            Stmt::Return(None, _) => self.return_stmt(None),
            Stmt::Return(Some(value), _) => {
                let code = self.returned(value);
                self.return_stmt(Some(code.text));
            }
            Stmt::Throw(value) => self.throw_stmt(value),
            Stmt::Try { body, catches, finally } => self.try_stmt(stmt, body, catches, finally.as_deref()),
            Stmt::Break => self.plain_jump(false),
            Stmt::Continue => self.plain_jump(true),
            Stmt::Switch { selector, cases } => self.switch_form(selector, cases, None, false),
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
        self.named.truncate(named);
    }

    fn while_loop(&mut self, cond: &'a Expr, body: &'a Stmt) {
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

    fn do_while_loop(&mut self, body: &'a Stmt, cond: &'a Expr) {
        let kind = if has_continue(body) { Loop::DoWhile(cond) } else { Loop::Plain };
        // The body, when the condition reads a local, is a block of its own: a name that the
        // body narrows (`let Some(node) = .. else`, `let mags = ..`) is the body's to its end, and
        // the condition, written after it, reads the variable itself. Java's condition sees none
        // of the body's own locals. The block opens on the loop's line, where the loop's label
        // goes.
        let fenced = !matches!(cond.kind, ExprKind::Bool(true)) && reads_a_local(cond);
        self.line(if fenced { "loop { {" } else { "loop {" });
        self.loop_body(kind, body, &[]);
        if fenced {
            self.indent += 1;
            self.line("}");
            self.indent -= 1;
        }
        // `while (true)` never leaves, so the loop is an expression of any type, as
        // Rust's `loop` without a `break` is.
        if !matches!(cond.kind, ExprKind::Bool(true)) {
            self.indent += 1;
            self.exit_unless(cond);
            self.indent -= 1;
        }
        self.line("}");
    }

    /// The value of a `return`: a string variable is moved out rather than copied, since the
    /// method ends.
    pub(super) fn returned(&mut self, value: &'a Expr) -> Code {
        if self.optional_return() {
            return self.optional_to(value, self.ret);
        }
        // A result that is never null, though the analysis cannot tell of the value
        // (`analysis::nullness::never_null_result`): an `Option` that is always `Some`.
        if crate::analysis::nullable_type(self.ret) && self.nullable(value) {
            let code = self.expr(value);
            return Code::atom(format!("{}.expect(\"a result that is never null\")", code.at(ATOM)));
        }
        if self.facts.owned_returns.contains(&self.method) {
            return Code::atom(self.owned(value));
        }
        // (A `finally` that the `return` runs may still read the variable.)
        if let ExprKind::Var(name) = &value.kind
            && value.ty == Type::String
            && !self.borrowed.contains(name)
            && self.finallies.is_empty()
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
        self.after_body = update;
        self.nested(body);
        self.after_body = &[];
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
        if self.search_loop(init, cond, update, body) {
            return;
        }
        if let Some(cond) = cond
            && self.element_loop(cond, body)
        {
            return;
        }
        if let Some(counted) = counting(init, cond, update, body) {
            if let Some(cond) = cond
                && self.sliced_loop(cond, &counted, body)
            {
                return;
            }
            return self.range_loop(&counted, body);
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
        let fields = self.bind_loop_fields(cond, update, body);
        match cond {
            // `for (..; true; ..)` never leaves but by `break`, as `loop` does, so the loop is an
            // expression of any type, as a method's last statement.
            None => self.line("loop {"),
            Some(Expr { kind: ExprKind::Bool(true), .. }) => self.line("loop {"),
            Some(cond) => {
                let cond_text = self.condition(cond);
                self.line(&format!("while {cond_text} {{"));
            }
        }
        let kind = if has_continue(body) && !update.is_empty() { Loop::For(update) } else { Loop::Plain };
        self.loop_body(kind, body, update);
        self.line("}");
        self.unbind_loop_fields(fields);
    }

    /// A counting loop as `for i in a..b`, `(a..b).rev()` or `(a..b).step_by(k)`.
    pub(super) fn range_loop(&mut self, counted: &Counting<'a>, body: &'a Stmt) {
        let ty = counted.ty;
        let start = self.value(counted.from, ty);
        let start = self.range_bound(start, "start");
        let end = self.value(counted.to, ty);
        let end = self.range_bound(end, "end");
        let mut used = false;
        visit_stmt(body, &mut |e| used |= mentions(e, counted.var), &mut |_| {});
        let var = if used { self.local(counted.var) } else { "_".to_string() };
        let start_text = if used && start.untyped && end.untyped {
            let suffix = self.annotation(ty, rust_type(ty).to_string());
            if suffix == rust_type(ty) { start.typed() } else { format!("{}{suffix}", start.text) }
        } else {
            start.at(ADD + 1)
        };
        let range =
            format!("{start_text}{}{}", if counted.inclusive { "..=" } else { ".." }, end.at(ADD + 1));
        let range = if counted.descending {
            format!("({range}).rev()")
        } else if counted.step > 1 {
            format!("({range}).step_by({})", counted.step)
        } else {
            range
        };
        let viewed = self.bind_views(body);
        let fields = self.bind_loop_fields(None, &[], body);
        self.line(&format!("for {var} in {range} {{"));
        self.loop_body(Loop::Plain, body, &[]);
        self.line("}");
        self.unbind_loop_fields(fields);
        for id in viewed {
            self.views.remove(&id);
        }
    }

    pub(super) fn local_decl(&mut self, ty: Type, name: &str, init: Option<&'a Expr>, mutable: bool) {
        if self.optional_var(self.method, ty, name) {
            return self.optional_local(name, ty, init, mutable);
        }
        if let Some(written) = self.plain(name) {
            self.plain_local(ty, name, init, written || mutable);
            return;
        }
        let mutable = if mutable { "mut " } else { "" };
        let rust = self.local(name);
        if let Some(closure) = init.and_then(|init| self.local_closure(name, init)) {
            self.line(&format!("let {rust} = {closure};"));
            return;
        }
        match init {
            // A `try` block carries its value out (see `exceptions::carried_out`).
            None if self.try_assigned.contains(name) && self.try_start(ty).is_none() => {
                let ty_text = self.ty_name(ty);
                self.unset_locals.insert(name.to_string(), ty);
                self.line(&format!("let mut {rust}: {ty_text};"));
            }
            None if self.try_assigned.contains(name) => {
                // Rust cannot tell that a `try` block or its `catch` clauses assign it before it is
                // read, as Java can: it starts as the value Java would give a field.
                // The `try` assigns it after that value, so it is `mut` though Java assigns it once.
                let ty_text = self.ty_name(ty);
                let value = self.try_start(ty).unwrap_or_else(|| "todo!()".to_string());
                self.line("#[allow(unused_assignments)]");
                self.line(&format!("let mut {rust}: {ty_text} = {value};"));
            }
            None => {
                let ty_text = self.ty_name(ty);
                self.line(&format!("let {mutable}{rust}: {ty_text};"));
            }
            Some(init) => {
                let code = self.value(init, ty);
                // An array's element type cannot be inferred from `from_vec(vec![])`, so it is always written.
                // A frozen field held for a store into it (`analysis::frozen_fields`) has its own type.
                let annotation =
                    if code.untyped || matches!(ty, Type::Array(_)) && !self.frozen_field_read(init) {
                        let ty_text = self.ty_name(ty);
                        self.annotation(ty, format!(": {ty_text}"))
                    } else {
                        String::new()
                    };
                self.line(&format!("let {mutable}{rust}{annotation} = {};", code.text));
            }
        }
    }

    /// The condition of an `if` that guards `then`: a let chain where it binds (see `let_chain`),
    /// and the names it narrows for `then`, which the caller unnarrows after it.
    pub(super) fn guard(&mut self, cond: &'a Expr, then: &'a Stmt) -> (String, Vec<String>) {
        match self.let_chain(cond, &[then], &[]) {
            Some(chain) => chain,
            None => (self.condition(cond), Vec::new()),
        }
    }

    /// The condition of an `if` or `while`, which may not start with a block in Rust.
    pub(super) fn condition(&mut self, cond: &'a Expr) -> String {
        // A `Boolean` is unboxed, which throws on a null (`boxing_value`).
        let code = self.value(cond, cond.ty.unboxed());
        if code.text.starts_with('{') { format!("({})", code.text) } else { code.text }
    }

    /// The negation of a condition, flipping the comparison where that is exact.
    pub(super) fn negate(&mut self, cond: &'a Expr) -> String {
        match &cond.kind {
            ExprKind::Unary(UnOp::Not, inner) => return self.condition(inner),
            // An enum or box that may be null is an `Option`, which the flip must keep `Some`.
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) if self.optional_equality(l, r) => {
                let flipped = if *op == BinOp::Eq { BinOp::Ne } else { BinOp::Eq };
                return self.optional_equality_code(flipped, l, r).text;
            }
            ExprKind::Binary(op, l, r)
                if op.is_comparison()
                    && !l.ty.is_floating()
                    && !r.ty.is_floating()
                    && !matches!(l.ty, Type::Class(..))
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
        format!("!{}", self.value(cond, cond.ty.unboxed()).at(UNARY))
    }

    /// An expression evaluated for its effect, as a statement.
    pub(super) fn effect(&mut self, e: &'a Expr) {
        match &e.kind {
            // A pattern switch's null check of a selector that is never null does nothing.
            ExprKind::Builtin(Builtin::NullChecked(_), args)
                if args.len() == 1 && !self.nullable(&args[0]) => {}
            // So does that of the receiver of a regular expression call that the JDK runs.
            ExprKind::Builtin(Builtin::String(StringOp::Receiver(..)), args) if !self.nullable(&args[0]) => {}
            // So does the initialization of a class that has nothing to run, or that has run.
            ExprKind::Builtin(Builtin::InitializeClass { class }, _) if self.init_call(*class).is_none() => {}
            // A plain array takes over the array it is given.
            ExprKind::Assign { op: None, target: Place::Var(target), value }
                if self.plain(target).is_some() =>
            {
                let value = self.owned(value);
                self.line(&format!("{} = {value};", self.local(target)));
            }
            ExprKind::Assign { op, target: Place::Var(target), value } => {
                let text = self.assignment(*op, target, e.ty, value, e.id);
                self.line(&format!("{text};"));
            }
            ExprKind::IncDec { target: Place::Var(target), increment, .. } => {
                let text = self.step(target, e.ty, *increment, e.id);
                self.line(&format!("{text};"));
            }
            ExprKind::Assign { op: None, target: Place::Index(array, index), value } => {
                // The element type, which is `Object` for a covariant array.
                let text = self.index_set(e.id, array, index, arrays::element(array.ty), value, false);
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
            // No statement is a cast, so this one to an enum is erasure's, of a call whose result
            // is not used (`map.put(k, v)` of a `Map<K, V>`), which javac does not check: a
            // previous value of null is no null enum value.
            ExprKind::Cast(Type::Enum(_), value)
                if !self.facts.raising.contains(&e.id) && !self.facts.catching.contains_key(&e.id) =>
            {
                self.effect(value)
            }
            // A chained builder call is made for its effect on the builder (one whose failure a
            // `catch` sees runs in `java::catching`, as an expression).
            ExprKind::Builtin(Builtin::Builder(op), args)
                if op.chains() && !self.facts.catching.contains_key(&e.id) =>
            {
                let code = self.builder_call(e.id, *op, args);
                self.line(&format!("{};", code.text));
            }
            _ => {
                let discarded = self.discarded.replace(e.id);
                let code = self.expr(e);
                self.discarded = discarded;
                self.line(&format!("{};", code.text));
            }
        }
    }
}

/// Whether expression `e` reads a local.
fn reads_a_local(e: &Expr) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| found |= matches!(x.kind, ExprKind::Var(_)));
    found
}
