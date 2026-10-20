//! String expressions: borrowing and owning, and concatenation as `format!`.

use super::*;

impl<'a> Gen<'a> {
    /// A string expression as a `&str`.
    pub(super) fn str_ref(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => rust_string(s),
            ExprKind::Var(name) if self.borrowed.contains(name) => self.local(name),
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// A string expression for comparing, without copying variables.
    pub(super) fn str_value(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => rust_string(s),
            ExprKind::Var(name) => self.local(name),
            _ => self.expr(e).at(CMP + 1),
        }
    }

    /// Splits `e` into the pieces of a format string: string concatenation is flattened,
    /// constants become text, and everything else is formatted the way Java prints it.
    pub(super) fn parts(&mut self, e: &'a Expr, out: &mut Vec<Part>) {
        // Java converts each operand to text before evaluating the next one, while `format!`
        // reads variables after evaluating all arguments; when an operand assigns a variable,
        // the variables it assigns are copied as they are reached.
        let mut assigned = HashSet::new();
        visit_expr(e, &mut |x| match &x.kind {
            ExprKind::Assign { target: Place::Var(target), .. }
            | ExprKind::IncDec { target: Place::Var(target), .. } => {
                assigned.insert(target.clone());
            }
            _ => {}
        });
        // An object is formatted through its `Display`, which runs its `toString` when the text is
        // formatted, after every operand; that is Java's order only when no operand has effects.
        let lazy = !self.effects_besides_text(e);
        let outer = std::mem::replace(&mut self.lazy_text, lazy);
        self.concat_parts(e, &assigned, out);
        self.lazy_text = outer;
    }

    pub(super) fn concat_parts(&mut self, e: &'a Expr, assigned: &HashSet<String>, out: &mut Vec<Part>) {
        if let Some(part) = self.null_part(e, assigned) {
            return out.push(part);
        }
        match &e.kind {
            _ if matches!(e.ty, Type::Class(_)) => out.push(Part::Arg(self.object_text(e))),
            // `x.toString()` shows `x` like concatenating it does.
            ExprKind::MethodCall { receiver, resolved: Some(index), args, .. }
                if args.is_empty() && self.displays(*index) =>
            {
                out.push(Part::Arg(self.object_text(receiver)))
            }
            // An enum prints through its `Display`, which is its name.
            _ if matches!(e.ty, Type::Enum(_)) => out.push(Part::Arg(self.expr(e))),
            ExprKind::Binary(BinOp::Add, l, r) if e.ty == Type::String => {
                self.concat_parts(l, assigned, out);
                self.concat_parts(r, assigned, out);
            }
            ExprKind::Var(name) if assigned.contains(name) => {
                let copy = if e.ty == Type::String {
                    format!("{}.clone()", self.local(name))
                } else {
                    format!("{{ {} }}", self.local(name))
                };
                let copy = self.shown(e.ty, copy);
                out.push(Part::Arg(Code::atom(copy)));
            }
            ExprKind::Str(s) => out.push(Part::Lit(s.clone())),
            ExprKind::Int(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Long(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Bool(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Double(v) => out.push(Part::Lit(java_runtime::double_to_string(*v))),
            ExprKind::Float(v) => out.push(Part::Lit(java_runtime::float_to_string(*v))),
            ExprKind::Char(c) => out.push(Part::Lit(java_runtime::char_to_string(*c))),
            ExprKind::Var(name) if e.ty == Type::String => out.push(Part::Arg(Code::atom(self.local(name)))),
            _ => {
                let code = self.expr(e);
                if let Some(literal) = code.literal {
                    // A constant made by folding a cast, such as `(long) 1e19`.
                    out.push(Part::Lit(literal_text(literal)));
                } else if matches!(e.ty, Type::Double | Type::Float | Type::Char) {
                    out.push(Part::Arg(Code::atom(self.shown(e.ty, code.text))));
                } else {
                    out.push(Part::Arg(code));
                }
            }
        }
    }

    /// Whether an expression has effects other than the `toString` calls of objects it shows.
    fn effects_besides_text(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::MethodCall { receiver, resolved: Some(index), args, .. }
                if args.is_empty() && self.displays(*index) =>
            {
                self.effects_besides_text(receiver)
            }
            ExprKind::Binary(BinOp::Add, l, r) if e.ty == Type::String => {
                self.effects_besides_text(l) || self.effects_besides_text(r)
            }
            _ => null::has_effects(e),
        }
    }

    /// A `format!`-style macro call over `parts`. Plain variables are inlined into the format
    /// string; `format!` of constant text becomes `String::from`.
    pub(super) fn format(&mut self, mac: &str, parts: Vec<Part>) -> Code {
        let mut template = String::new();
        let mut args = Vec::new();
        for part in parts {
            match part {
                Part::Lit(s) => {
                    // Braces are doubled before quoting, which writes some characters as `\u{..}`.
                    let quoted = rust_string(&s.replace('{', "{{").replace('}', "}}"));
                    template.push_str(&quoted[1..quoted.len() - 1]);
                }
                Part::Arg(code) if is_identifier(&code.text) => {
                    template.push_str(&format!("{{{}}}", code.text));
                }
                Part::Arg(code) => {
                    template.push_str("{}");
                    args.push(code.text);
                }
            }
        }
        if mac == "format!" && args.is_empty() && !template.contains('{') {
            if template.is_empty() {
                return Code::atom("String::new()");
            }
            return Code::atom(format!("String::from(\"{template}\")"));
        }
        if mac.ends_with("println!") && template.is_empty() {
            return Code::atom(format!("{mac}()"));
        }
        let args: String = args.iter().map(|a| format!(", {a}")).collect();
        Code::atom(format!("{mac}(\"{template}\"{args})"))
    }
}
