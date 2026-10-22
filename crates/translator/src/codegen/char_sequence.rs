//! The text that `append(s, start, end)` takes a range of: Java checks the range on `s.length()`
//! and appends `s.subSequence(start, end)`, so the text must be what those two give for `s`.

use super::*;

impl<'a> Gen<'a> {
    /// `sb.append_range(text, start, end)` for the operation `id`: where a `catch` or a `finally`
    /// may see its `IndexOutOfBoundsException` (`Facts::raising`), the checked form, whose failure
    /// goes on from here as a thrown exception does.
    pub(super) fn append_range_call(&mut self, id: ExprId, sb: &str, args: &str) -> Code {
        if self.facts.raising.contains(&id) {
            return Code::atom(self.passed_on(Code::atom(format!("{sb}.try_append_range({args})"))));
        }
        Code::atom(format!("{sb}.append_range({args})"))
    }

    /// The text of `s` in `StringBuilder.append(s, start, end)`, as a `&str`, for a `s` that holds
    /// only strings and builders (see `plain_sequence`). A null `s` is `"null"`, as Java's
    /// `append` makes it.
    pub(super) fn range_text(&mut self, e: &'a Expr) -> String {
        match e.ty {
            Type::String | Type::StringBuilder if !self.nullable(e) => self.text_ref(e),
            _ => format!("&{}", self.text_of(e)),
        }
    }

    /// `sb.append(s, start, end)` for the operation `id`, where `s` is a `CharSequence` with a
    /// program class among its variants. Java evaluates `s` before the range, and then asks `s`
    /// for its `length` and its `charAt` of each index, which are the enum's own methods; the
    /// runtime does the rest (`append_seq_range`). Those two must not throw (see
    /// `quiet_sequence`), so the only failure is the range's, which the checked form carries where
    /// a `catch` or a `finally` may see it.
    pub(super) fn program_range_call(&mut self, id: ExprId, sb: &str, s: &'a Expr, range: &str) -> Code {
        if self.nullable(s) {
            let message = "appending a range of a nullable `CharSequence` that a program class implements";
            self.rejected.push(Error { pos: s.pos, message: message.to_string() });
            return Code::atom("unreachable!()".to_string());
        }
        let t = self.type_of(s.ty).expect("a CharSequence has an enum");
        let sequence =
            self.class.type_index(CHAR_SEQUENCE).expect("a program with a CharSequence has its type");
        let length = self.char_sequence_method(sequence, "length", 0);
        let char_at = self.char_sequence_method(sequence, "charAt", 1);
        if !self.quiet_sequence(t, &[length, char_at]) {
            let message = "appending a range of a `CharSequence` whose `charAt` or `length` may throw not supported yet";
            self.rejected.push(Error { pos: s.pos, message: message.to_string() });
            return Code::atom("unreachable!()".to_string());
        }
        let (seq, index) = (self.temp("seq"), self.temp("index"));
        let length_call = format!("Ok({seq}.{}())", self.dispatch_name(t, length));
        let char_call = format!("Ok({seq}.{}({index}))", self.dispatch_name(t, char_at));
        let args = format!("{range}, {length_call}, |{index}| {char_call}");
        let call = if self.facts.raising.contains(&id) {
            let call = format!("{sb}.try_append_seq_range::<java::Exception>({args})");
            Code::atom(self.passed_on(Code::atom(call)))
        } else {
            Code::atom(format!("{sb}.append_seq_range::<java::Exception>({args})"))
        };
        let obj = self.object(s);
        Code::atom(format!("{{ let {seq} = &{obj}; {} }}", call.text))
    }

    /// Whether the methods `methods` of the `CharSequence` enum of `t` cannot throw in any variant
    /// of the program: each program class among the variants implements them with a body that
    /// only computes (`quiet_stmts`), and that is not `synchronized` or in `facts.throwing`. The
    /// variants that are strings and builders do not throw for an index in range, which the range
    /// check has made sure of.
    fn quiet_sequence(&self, t: usize, methods: &[usize]) -> bool {
        self.enum_variants(t).into_iter().filter(|&c| self.runtime_of(c).is_none()).all(|c| {
            methods.iter().all(|&m| match self.class.hierarchy.implementation(m, c) {
                Some(i) => {
                    let method = &self.class.methods[i];
                    !method.is_synchronized && !self.facts.throwing.contains(&i) && quiet_stmts(&method.body)
                }
                None => false,
            })
        })
    }

    /// The index of the method `name` with `params` parameters of the `CharSequence` at `sequence`.
    fn char_sequence_method(&self, sequence: usize, name: &str, params: usize) -> usize {
        self.class
            .methods
            .iter()
            .position(|m| m.class == sequence && m.name == name && !m.is_static && m.params.len() == params)
            .expect("`CharSequence` declares the method")
    }

    /// The text that `new StringBuilder(seq)` copies, as a `&str`. Java's constructor asks `seq`
    /// for its length, so a null `seq` throws `NullPointerException` rather than giving "null".
    /// A `CharSequence` with a program class among its variants is rejected, as `range_text` does.
    pub(super) fn copied_text(&mut self, e: &'a Expr) -> String {
        if !self.plain_sequence(e) {
            let message = "creating a builder from a `CharSequence` that a program class implements";
            self.rejected.push(Error { pos: e.pos, message: message.to_string() });
            return "\"\"".to_string();
        }
        let t = self.type_of(e.ty).expect("a CharSequence has an enum");
        self.need(Need::Display(t));
        let obj = self.object(e);
        format!("&{obj}.to_string()")
    }

    /// Whether the `CharSequence` `e` holds only strings and builders: no program class is one of
    /// its variants, so its text is the text of the string or builder it holds.
    pub(super) fn plain_sequence(&self, e: &Expr) -> bool {
        self.type_of(e.ty).is_some_and(|t| {
            self.enum_variants(t)
                .into_iter()
                .all(|c| matches!(self.runtime_of(c), Some(Type::String | Type::StringBuilder)))
        })
    }
}

/// Whether the statements of a method cannot throw: they only compute, with no call, index,
/// division, unboxing or text. This is conservative: whatever it does not know may throw.
fn quiet_stmts(stmts: &[Stmt]) -> bool {
    stmts.iter().all(quiet_stmt)
}

fn quiet_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Local { ty, init, .. } => !matches!(ty, Type::Boxed(_)) && init.as_ref().is_none_or(quiet_expr),
        Stmt::Expr(e) => quiet_expr(e),
        Stmt::Return(value, _) => value.as_ref().is_none_or(quiet_expr),
        Stmt::If { cond, then, otherwise } => {
            quiet_expr(cond) && quiet_stmt(then) && otherwise.as_deref().is_none_or(quiet_stmt)
        }
        Stmt::Block(body) => quiet_stmts(body),
        _ => false,
    }
}

fn quiet_expr(e: &Expr) -> bool {
    if matches!(e.ty, Type::Boxed(_)) {
        return false;
    }
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Long(_)
        | ExprKind::Double(_)
        | ExprKind::Char(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Var(_)
        | ExprKind::This => true,
        ExprKind::Unary(_, v) => quiet_expr(v),
        ExprKind::Binary(op, l, r) => {
            !matches!(op, BinOp::Div | BinOp::Rem)
                && primitive(l.ty)
                && primitive(r.ty)
                && quiet_expr(l)
                && quiet_expr(r)
        }
        ExprKind::Cast(to, v) => primitive(*to) && primitive(v.ty) && quiet_expr(v),
        ExprKind::Conditional(c, a, b) => quiet_expr(c) && quiet_expr(a) && quiet_expr(b),
        ExprKind::Field { object, .. } => matches!(object.kind, ExprKind::This),
        ExprKind::Assign { op, target: Place::Var(_), value } => {
            !matches!(op, Some(BinOp::Div | BinOp::Rem)) && primitive(value.ty) && quiet_expr(value)
        }
        _ => false,
    }
}

/// Whether `t` is a primitive type, whose operations cannot throw (except division).
fn primitive(t: Type) -> bool {
    matches!(
        t,
        Type::Int
            | Type::Long
            | Type::Double
            | Type::Byte
            | Type::Short
            | Type::Char
            | Type::Float
            | Type::Boolean
    )
}
