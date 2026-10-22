//! String expressions: borrowing and owning, and concatenation as `format!`.

use super::*;

/// The objects named by `this` and by locals (keyed `this` and by the local's name) that a
/// statement changes as a whole or passes on, and the fields of them it assigns. A string field
/// of a plain object that the statement neither assigns nor passes on can be borrowed where it
/// is: no other reference to a plain object exists, so nothing else changes it while the borrow
/// lasts.
#[derive(Clone, Default)]
pub(super) struct Steady {
    disturbed: HashSet<String>,
    written: HashSet<(String, String)>,
}

impl Steady {
    fn keeps(&self, object: &str, field: &str) -> bool {
        !self.disturbed.contains(object) && !self.written.contains(&(object.to_string(), field.to_string()))
    }
}

impl<'a> Gen<'a> {
    /// A string expression as a `&str`.
    pub(super) fn str_ref(&mut self, e: &'a Expr) -> String {
        // A shared string's text, which `str`'s methods take as a pattern.
        if super::shared_strings() {
            return match &e.kind {
                ExprKind::Str(s) => rust_string(s),
                ExprKind::Var(name) => format!("{}.as_str()", self.local(name)),
                ExprKind::Field { object, name } if self.field_in_place(object, name) => {
                    format!("{}.as_str()", self.object_place(e))
                }
                _ => format!("{}.as_str()", self.expr(e).at(ATOM)),
            };
        }
        match &e.kind {
            ExprKind::Str(s) => rust_string(s),
            ExprKind::Var(name) if self.borrowed.contains(name) => self.local(name),
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            ExprKind::Field { object, name } if self.field_in_place(object, name) => {
                format!("&{}", self.object_place(e))
            }
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// Whether the string field `field` of `object` can be borrowed where it is for as long as
    /// the statement runs: an immutable object never changes, and a shared object's borrow is
    /// scoped where the statement may change it (see `conflicts`). An owned object, or `this`
    /// borrowed by a method, may be changed by a call in the same statement, which the borrow
    /// would forbid: it is borrowed in place only where the statement being written leaves the
    /// object and the field alone (`Steady`).
    pub(super) fn field_in_place(&self, object: &Expr, field: &str) -> bool {
        match object.ty {
            Type::Class(name, _) if self.facts.immutable.contains(name) => true,
            Type::Class(..) if self.is_plain(object) => {
                let key = match &object.kind {
                    ExprKind::This => "this",
                    ExprKind::Var(name) => name.as_str(),
                    _ => return false,
                };
                self.steady.as_ref().is_some_and(|s| s.keeps(key, field))
            }
            Type::Class(..) => true,
            _ => false,
        }
    }

    /// What the statements and expressions do to the objects that `this` and the locals name
    /// (see `Steady`).
    pub(super) fn steady(&self, stmts: &[&Stmt], exprs: &[&Expr]) -> Steady {
        // Per object: its uses, and those that only read a field or call a `&self` method.
        let mut uses: HashMap<String, (usize, usize)> = HashMap::new();
        let mut steady = Steady::default();
        let key = |e: &Expr| match &e.kind {
            ExprKind::This => Some("this".to_string()),
            ExprKind::Var(name) => Some(name.clone()),
            _ => None,
        };
        let mut on_expr = |x: &Expr| {
            let mut harmless = |e: &Expr| {
                if let Some(k) = key(e) {
                    uses.entry(k).or_default().1 += 1;
                }
            };
            match &x.kind {
                ExprKind::This | ExprKind::Var(_) => {
                    uses.entry(key(x).expect("a name")).or_default().0 += 1;
                }
                ExprKind::Field { object, .. } => harmless(object),
                // Writing a field changes that field only: Rust borrows the others apart.
                ExprKind::Assign { target: Place::Field { object, name }, .. }
                | ExprKind::IncDec { target: Place::Field { object, name }, .. } => {
                    harmless(object);
                    if let Some(k) = key(object) {
                        steady.written.insert((k, name.clone()));
                    }
                }
                ExprKind::Assign { target: Place::Var(name), .. }
                | ExprKind::IncDec { target: Place::Var(name), .. } => {
                    steady.disturbed.insert(name.clone());
                }
                ExprKind::MethodCall { receiver, .. }
                    if self.target(x).is_some_and(|m| !self.facts.mutating.contains(&m)) =>
                {
                    harmless(receiver)
                }
                _ => {}
            }
        };
        for stmt in stmts {
            visit_stmt(stmt, &mut |e| visit_expr(e, &mut on_expr), &mut |_| {});
        }
        for e in exprs {
            visit_expr(e, &mut on_expr);
        }
        // Any other use (an argument, a returned or stored value, a call of a `&mut self` method)
        // may move the object or borrow it mutably.
        steady
            .disturbed
            .extend(uses.into_iter().filter(|(_, (all, harmless))| all > harmless).map(|(k, _)| k));
        steady
    }

    /// A string expression for comparing, without copying variables.
    pub(super) fn str_value(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => rust_string(s),
            ExprKind::Var(name) => self.local(name),
            ExprKind::Field { object, name } if self.field_in_place(object, name) => self.object_place(e),
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
            // A field of `this` that a call or an assignment of the concatenation may change is
            // copied as it is reached too, which also ends its borrow before `self` is borrowed
            // mutably. No local is named `this`.
            ExprKind::MethodCall { receiver, resolved: Some(m), .. }
                if matches!(receiver.kind, ExprKind::This) && self.facts.mutating.contains(m) =>
            {
                assigned.insert("this".to_string());
            }
            ExprKind::Assign { target: Place::Field { object, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
                if matches!(object.kind, ExprKind::This) =>
            {
                assigned.insert("this".to_string());
            }
            _ => {}
        });
        // An object is formatted through its `Display`, which runs its `toString` when the text is
        // formatted, after every operand; that is Java's order only when no operand has effects.
        let lazy = !self.effects_besides_text(e);
        let outer = std::mem::replace(&mut self.lazy_text, lazy);
        let formatting = std::mem::replace(&mut self.formatting, true);
        self.concat_parts(e, &assigned, out);
        self.formatting = formatting;
        self.lazy_text = outer;
    }

    pub(super) fn concat_parts(&mut self, e: &'a Expr, assigned: &HashSet<String>, out: &mut Vec<Part>) {
        if let Some(part) = self.null_part(e, assigned) {
            return out.push(part);
        }
        match &e.kind {
            _ if matches!(e.ty, Type::Class(..)) => out.push(Part::Arg(self.object_text(e))),
            // `x.toString()` shows `x` like concatenating it does.
            ExprKind::MethodCall { receiver, resolved: Some(_), args, is_super: false, .. }
                if args.is_empty() && self.target(e).is_some_and(|t| self.displays(t)) =>
            {
                out.push(Part::Arg(self.object_text(receiver)))
            }
            // An enum prints through its `Display`, which is its name; so does a charset.
            _ if matches!(e.ty, Type::Enum(_) | Type::Charset) => out.push(Part::Arg(self.expr(e))),
            // A builder shows its text as it is when it is converted, before later operands run.
            _ if e.ty == Type::StringBuilder => {
                let sb = self.builder_recv(e);
                out.push(Part::Arg(Code::atom(if self.lazy_text {
                    sb
                } else {
                    format!("{sb}.to_string()")
                })));
            }
            _ if matches!(e.ty, Type::Boxed(_)) => {
                let kind = e.ty.unboxed();
                let value = self.value(e, kind);
                out.push(Part::Arg(Code::atom(self.shown(kind, value.text))));
            }
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
            // A string field is formatted where it is, when nothing can change it first.
            ExprKind::Field { object, .. }
                if e.ty == Type::String && self.lazy_text && !matches!(object.ty, Type::Enum(_)) =>
            {
                out.push(Part::Arg(Code::atom(self.object_place(e))))
            }
            _ => {
                let mut code = self.expr(e);
                let copied = matches!(&e.kind, ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This));
                if copied && assigned.contains("this") && (e.ty.is_numeric() || e.ty == Type::Boolean) {
                    code = Code::atom(format!("{{ {} }}", code.text));
                }
                if let Some(literal) = code.literal {
                    // A constant made by folding a cast, such as `(long) 1e19`.
                    out.push(Part::Lit(literal_text(literal)));
                } else if matches!(e.ty, Type::Double | Type::Float | Type::Char) {
                    out.push(Part::Arg(Code::atom(self.shown(e.ty, code.text))));
                } else if assigned.iter().any(|name| self.local(name) == code.text) {
                    // A cast that changes nothing, such as `(long) x`, reads the variable too.
                    out.push(Part::Arg(Code::atom(format!("{{ {} }}", code.text))));
                } else {
                    out.push(Part::Arg(code));
                }
            }
        }
    }

    /// Whether an expression has effects other than the `toString` calls of objects it shows.
    fn effects_besides_text(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::MethodCall { receiver, resolved: Some(_), args, is_super: false, .. }
                if args.is_empty() && self.target(e).is_some_and(|t| self.displays(t)) =>
            {
                self.effects_besides_text(receiver)
            }
            ExprKind::Binary(BinOp::Add, l, r) if e.ty == Type::String => {
                self.effects_besides_text(l) || self.effects_besides_text(r)
            }
            _ => has_effects(e),
        }
    }

    /// The string that concatenating `parts` makes: a `format!`, made a new shared string where
    /// the program's strings are shared.
    pub(super) fn concat(&mut self, parts: Vec<Part>) -> Code {
        let text = self.format("format!", parts);
        if !super::shared_strings() {
            return text;
        }
        Code::atom(format!("{}({})", self.runtime("JStr::new"), text.text))
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
