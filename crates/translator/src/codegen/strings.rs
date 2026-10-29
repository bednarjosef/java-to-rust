//! String expressions: borrowing and owning, and concatenation as `java::concat!`.

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
    /// A string expression as a `&java::Wtf8`.
    pub(super) fn str_ref(&mut self, e: &'a Expr) -> String {
        // A shared string's text.
        if super::shared_strings() {
            return match &e.kind {
                ExprKind::Str(s) => self.wtf8_literal(s),
                ExprKind::Units(units) => self.units_literal(units),
                ExprKind::Var(name) => format!("{}.as_wtf8()", self.local(name)),
                ExprKind::Field { object, name } if self.field_in_place(object, name) => {
                    format!("{}.as_wtf8()", self.object_place(e))
                }
                ExprKind::Static { .. }
                    if !self.nullable(e)
                        && let Some(place) = self.final_place(e) =>
                {
                    format!("{place}.as_wtf8()")
                }
                _ => format!("{}.as_wtf8()", self.expr(e).at(ATOM)),
            };
        }
        match &e.kind {
            ExprKind::Str(s) => self.wtf8_literal(s),
            ExprKind::Units(units) => self.units_literal(units),
            ExprKind::Var(name) if self.borrowed.contains(name) => self.local(name),
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            ExprKind::Field { object, name } if self.field_in_place(object, name) => {
                format!("&{}", self.object_place(e))
            }
            ExprKind::Static { .. }
                if !self.nullable(e)
                    && let Some(place) = self.final_place(e) =>
            {
                format!("&{place}")
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
            // A cell is read by value.
            Type::Class(..) if self.celled_object(object) => false,
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

    /// The string literal `s` as a `&'static java::Wtf8`.
    pub(super) fn wtf8_literal(&mut self, s: &str) -> String {
        format!("{}({})", self.runtime("wtf8"), rust_string(s))
    }

    /// A string literal with a lone surrogate, of code units `units`, as a `&'static java::Wtf8`:
    /// its WTF-8 bytes, which Rust checks when it compiles the program.
    pub(super) fn units_literal(&mut self, units: &[u16]) -> String {
        format!("const {{ {}({}) }}", self.runtime("wtf8_bytes"), byte_string(&wtf8_of(units)))
    }

    /// A string expression for comparing, without copying variables.
    pub(super) fn str_value(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => self.wtf8_literal(s),
            ExprKind::Units(units) => self.units_literal(units),
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
        // An object given as it is runs its `toString` when the string is built, after every
        // operand; that is Java's order only when no operand has effects.
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
            // An object shows the string its `toString` returns.
            _ if matches!(e.ty, Type::Class(..)) => out.push(Part::Text(self.object_text(e))),
            // An enum is a `java::Piece` of the string its `toString` returns; a charset prints
            // through its `Display`, which is its name.
            _ if matches!(e.ty, Type::Enum(_)) => out.push(Part::Text(self.expr(e))),
            _ if e.ty == Type::Charset => out.push(Part::Arg(self.expr(e))),
            // A builder shows its text as it is when it is converted, before later operands run.
            _ if e.ty == Type::StringBuilder => {
                let sb = self.builder_recv(e);
                out.push(Part::Text(Code::atom(if self.lazy_text { sb } else { format!("{sb}.text()") })));
            }
            _ if matches!(e.ty, Type::Boxed(_)) => {
                let kind = e.ty.unboxed();
                let value = self.value(e, kind);
                let shown = Code::atom(self.shown(kind, value.text));
                out.push(if kind == Type::Char { Part::Text(shown) } else { Part::Arg(shown) });
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
                let copy = Code::atom(self.shown(e.ty, copy));
                out.push(if matches!(e.ty, Type::String | Type::Char) {
                    Part::Text(copy)
                } else {
                    Part::Arg(copy)
                });
            }
            ExprKind::Str(s) => out.push(Part::Lit(s.clone())),
            ExprKind::Units(units) => out.push(Part::Text(Code::atom(self.units_literal(units)))),
            ExprKind::Int(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Long(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Bool(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Double(v) => out.push(Part::Lit(java_runtime::double_to_string(*v))),
            ExprKind::Float(v) => out.push(Part::Lit(java_runtime::float_to_string(*v))),
            // A lone surrogate is half of a pair that a neighbour may complete.
            ExprKind::Char(c) if (0xD800..0xE000).contains(c) => {
                out.push(Part::Text(Code::atom(format!("{}({c:#X})", self.runtime("Char")))))
            }
            ExprKind::Char(c) => {
                out.push(Part::Lit(java_runtime::char_to_string(*c).to_string_lossy().into_owned()))
            }
            ExprKind::Var(name) if e.ty == Type::String => out.push(Part::Text(Code::atom(self.local(name)))),
            // A string field is formatted where it is, when nothing can change it first.
            ExprKind::Field { object, .. }
                if e.ty == Type::String && self.lazy_text && !matches!(object.ty, Type::Enum(_)) =>
            {
                out.push(Part::Text(Code::atom(self.object_place(e))))
            }
            _ => {
                let mut code = self.expr(e);
                let copied = matches!(&e.kind, ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This));
                if copied && assigned.contains("this") && (e.ty.is_numeric() || e.ty == Type::Boolean) {
                    code = Code::atom(format!("{{ {} }}", code.text));
                }
                if let Some(literal) = code.literal
                    && let Literal::Char(_) | Literal::Small(_, Type::Char) = literal
                    && (0xD800..0xE000).contains(&(literal_value(literal).0 as u16))
                {
                    // A lone surrogate is half of a pair that a neighbour may complete.
                    let unit = literal_value(literal).0 as u16;
                    out.push(Part::Text(Code::atom(format!("{}({unit:#X})", self.runtime("Char")))));
                } else if let Some(literal) = code.literal {
                    // A constant made by folding a cast, such as `(long) 1e19`.
                    out.push(Part::Lit(literal_text(literal)));
                } else if e.ty == Type::Char {
                    out.push(Part::Text(Code::atom(self.shown(e.ty, code.text))));
                } else if matches!(e.ty, Type::Double | Type::Float) {
                    out.push(Part::Arg(Code::atom(self.shown(e.ty, code.text))));
                } else if assigned.iter().any(|name| self.local(name) == code.text) {
                    // A cast that changes nothing, such as `(long) x`, reads the variable too.
                    out.push(Part::Arg(Code::atom(format!("{{ {} }}", code.text))));
                } else if e.ty == Type::String {
                    // A string constant is a `&'static java::Wtf8`, which the piece is as it is.
                    let constant = code
                        .text
                        .strip_prefix(&format!("{}::from(", self.runtime("JString")))
                        .and_then(|t| t.strip_suffix(')'))
                        .filter(|t| is_identifier(t))
                        .map(str::to_string);
                    out.push(Part::Text(constant.map_or(code, Code::atom)));
                } else {
                    out.push(Part::Arg(code));
                }
            }
        }
    }

    /// Whether an expression has effects other than the `toString` calls of objects it shows.
    fn effects_besides_text(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Binary(BinOp::Add, l, r) if e.ty == Type::String => {
                self.effects_besides_text(l) || self.effects_besides_text(r)
            }
            _ => has_effects(e),
        }
    }

    /// The string that concatenating `parts` makes: a `java::concat!`, made a new shared string
    /// where the program's strings are shared.
    pub(super) fn concat(&mut self, parts: Vec<Part>) -> Code {
        let text = self.concat_text(parts);
        if !super::shared_strings() {
            return text;
        }
        Code::atom(format!("{}({})", self.runtime("JStr::new"), text.text))
    }

    /// The `java::JString` that concatenating `parts` makes.
    pub(super) fn concat_text(&mut self, parts: Vec<Part>) -> Code {
        if parts.iter().all(|p| matches!(p, Part::Lit(_))) {
            let text: String =
                parts.into_iter().map(|p| if let Part::Lit(s) = p { s } else { unreachable!() }).collect();
            return Code::atom(if text.is_empty() {
                format!("{}::new()", self.runtime("JString"))
            } else {
                format!("{}::from({})", self.runtime("JString"), rust_string(&text))
            });
        }
        let pieces = self.pieces(parts);
        Code::atom(format!("{}({})", self.runtime("concat!"), pieces.join(", ")))
    }

    /// `parts` as the pieces of a `java::concat!` or `java::append!`: each string, `char` or
    /// builder as it is, and the text between them as a literal or a `format_args!`.
    pub(super) fn pieces(&mut self, parts: Vec<Part>) -> Vec<String> {
        let mut pieces = Vec::new();
        let mut run = Vec::new();
        for part in parts {
            match part {
                Part::Text(code) => {
                    if !run.is_empty() {
                        pieces.push(self.formatted(std::mem::take(&mut run)));
                    }
                    pieces.push(code.text);
                }
                part => run.push(part),
            }
        }
        if !run.is_empty() {
            pieces.push(self.formatted(run));
        }
        pieces
    }

    /// Constant text and values shown through `Display`, as one piece of a concatenation.
    fn formatted(&mut self, run: Vec<Part>) -> String {
        if let [Part::Lit(s)] = run.as_slice() {
            return rust_string(s);
        }
        self.format("format_args!", run).text
    }

    /// A `format!`-style macro call over `parts`. Plain variables are inlined into the format
    /// string. Printing shows a lone surrogate as `?`, as Java prints it, so a piece is shown
    /// through its `Display`, unless two of them could meet as the halves of a pair: then the
    /// concatenation is made first.
    pub(super) fn format(&mut self, mac: &str, parts: Vec<Part>) -> Code {
        if parts.iter().filter(|p| matches!(p, Part::Text(_))).count() > 1 {
            let text = self.concat_text(parts).text;
            return Code::atom(format!("{mac}(\"{{}}\", {text})"));
        }
        let mut template = String::new();
        let mut args = Vec::new();
        for part in parts {
            match part {
                Part::Lit(s) => {
                    // Braces are doubled before quoting, which writes some characters as `\u{..}`.
                    let quoted = rust_string(&s.replace('{', "{{").replace('}', "}}"));
                    template.push_str(&quoted[1..quoted.len() - 1]);
                }
                Part::Arg(code) | Part::Text(code) if is_identifier(&code.text) => {
                    template.push_str(&format!("{{{}}}", code.text));
                }
                Part::Arg(code) | Part::Text(code) => {
                    template.push_str("{}");
                    args.push(code.text);
                }
            }
        }
        if mac.ends_with("println!") && template.is_empty() {
            return Code::atom(format!("{mac}()"));
        }
        let args: String = args.iter().map(|a| format!(", {a}")).collect();
        Code::atom(format!("{mac}(\"{template}\"{args})"))
    }
}

/// The WTF-8 bytes of the UTF-16 code units `units`: a pair is its character's 4-byte form, a lone
/// surrogate its own 3-byte form, as `java::JString` holds them.
pub(super) fn wtf8_of(units: &[u16]) -> Vec<u8> {
    let mut out = Vec::new();
    for unit in char::decode_utf16(units.iter().copied()) {
        match unit {
            Ok(c) => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            Err(lone) => {
                let u = lone.unpaired_surrogate();
                out.extend_from_slice(&[
                    0xE0 | (u >> 12) as u8,
                    0x80 | (u >> 6 & 0x3F) as u8,
                    0x80 | (u & 0x3F) as u8,
                ]);
            }
        }
    }
    out
}
