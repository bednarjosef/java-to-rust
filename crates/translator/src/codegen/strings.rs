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

/// The `Steady` of the statement being written (and of the next one, where it is written with
/// it), found the first time a field asks (`field_in_place`): most statements read no string
/// field of a plain object, and finding it walks the statements, for each statement of a block
/// and again for those nested in it.
pub(super) struct SteadyOf<'a> {
    stmts: [Option<&'a Stmt>; 2],
    found: std::cell::OnceCell<Steady>,
}

impl<'a> SteadyOf<'a> {
    pub(super) fn new(stmt: &'a Stmt, next: Option<&'a Stmt>) -> SteadyOf<'a> {
        SteadyOf { stmts: [Some(stmt), next], found: std::cell::OnceCell::new() }
    }
}

impl<'a> Gen<'a> {
    /// The first argument of a runtime function that takes any `java::Text` (`substring`,
    /// `index_of`, `char_at`, `string_length`, ...), made from what `text_ref` or `str_ref` gave:
    /// a shared string as itself (`as_text`) rather than its text (`as_wtf8`), since the string
    /// keeps whether it is ASCII and its text would be scanned at each call. So is a string that
    /// may be null, of a program without threads, read where it is (`java::non_null(x.as_deref())`):
    /// it is unwrapped as the `&JString` (`as_ref`), not as its text.
    pub(super) fn text_owner(&self, text: String) -> String {
        if let Some(base) = text.strip_suffix(".as_wtf8()") {
            return format!("{base}.as_text()");
        }
        if let Some(inner) = text.strip_prefix("java::non_null(")
            && let Some(option) = inner.strip_suffix(".map(java::Str::as_wtf8))")
            && balanced(option)
        {
            return format!("&java::non_null({option})");
        }
        if !self.facts.threaded
            && let Some(inner) = text.strip_prefix("java::non_null(")
            && let Some(place) = inner.strip_suffix(".as_deref())")
            && !place.contains('"')
            && balanced(place)
        {
            return format!("java::non_null({place}.as_ref())");
        }
        text
    }

    /// Whether the variable `name` is a string parameter that the method being written takes as a
    /// `java::Str` (`Facts::lent_strings`).
    pub(super) fn lent_string(&self, name: &str) -> bool {
        let m = &self.class.methods[self.method];
        self.borrowed.contains(name)
            && m.params
                .iter()
                .position(|p| p.name == name)
                .is_some_and(|i| self.facts.lent_strings.contains(&(self.method, i)))
    }

    /// Whether the variable `name` is a string parameter that may be null that the method being
    /// written takes as an `Option<java::Str>`, where no null test has bound it.
    pub(super) fn lent_optional(&self, name: &str) -> bool {
        let m = &self.class.methods[self.method];
        self.borrowed_optional.contains(name)
            && !self.borrowed.contains(name)
            && m.params
                .iter()
                .position(|p| p.name == name)
                .is_some_and(|i| self.facts.lent_strings.contains(&(self.method, i)))
    }

    /// `str_arg` for a parameter that may be null, an `Option<java::Str>`.
    pub(super) fn optional_str_arg(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => "None".to_string(),
            ExprKind::Var(name) if self.lent_optional(name) => self.local(name),
            _ if self.nullable(e) => {
                let option = self.option_ref(e);
                if let Some(text) = option.strip_suffix(".map(java::Str::as_wtf8)") {
                    text.to_string()
                } else if let Some(place) = option.strip_suffix(".as_deref()") {
                    format!("{place}.as_ref().map(java::Str::of)")
                } else {
                    format!("{option}.map(java::Str::of)")
                }
            }
            _ => format!("Some({})", self.str_arg(e)),
        }
    }

    /// A string argument of a parameter that the method takes as a `java::Str`: the caller's own
    /// such parameter as it is, a literal with whether it is ASCII, and any other string as what
    /// `str_ref` gives, as a `JString` where it is one, whose kept flag the `Str` takes.
    pub(super) fn str_arg(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Var(name) if self.lent_string(name) => self.local(name),
            ExprKind::Str(s) => {
                let text = self.wtf8_literal(s);
                format!("{}({text}, {})", self.runtime("Str::known"), s.is_ascii())
            }
            // Text with a lone surrogate is not ASCII.
            ExprKind::Units(units) => {
                let text = self.units_literal(units);
                format!("{}({text}, false)", self.runtime("Str::known"))
            }
            _ => {
                let text = self.str_ref(e);
                let text = self.text_owner(text);
                format!("{}({text})", self.runtime("Str::of"))
            }
        }
    }

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
            ExprKind::Var(name) if self.lent_string(name) => format!("{}.as_wtf8()", self.local(name)),
            ExprKind::Var(name) if self.borrowed.contains(name) => self.local(name),
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            ExprKind::Field { object, name } if self.field_in_place(object, name) => {
                format!("&{}", self.object_place(e))
            }
            ExprKind::Static { .. }
                if !self.nullable(e)
                    && let Some(place) = self.final_place(e).or_else(|| self.lent_static(e)) =>
            {
                format!("&{place}")
            }
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// Whether the string field `field` of `object` can be borrowed where it is for as long as
    /// the statement runs: an immutable object never changes, nor does a fixed field of a shared
    /// object, and a shared object's borrow is
    /// scoped where the statement may change it (see `conflicts`). An owned object, or `this`
    /// borrowed by a method, may be changed by a call in the same statement, which the borrow
    /// would forbid: it is borrowed in place only where the statement being written leaves the
    /// object and the field alone (`Steady`).
    pub(super) fn field_in_place(&self, object: &Expr, field: &str) -> bool {
        match object.ty {
            // A cell is read by value, unless it is a fixed field's, which nothing writes once its
            // object is built (`fixed_cell`).
            Type::Class(..) if self.celled_object(object) => self.fixed_cell(object, field, Type::String),
            Type::Class(name, _) if self.facts.immutable.contains(name) => true,
            Type::Class(..) if self.is_plain(object) => {
                let key = match &object.kind {
                    ExprKind::This => "this",
                    ExprKind::Var(name) => name.as_str(),
                    _ => return false,
                };
                self.steady.as_ref().is_some_and(|s| {
                    let found = s.found.get_or_init(|| {
                        let stmts: Vec<&Stmt> = s.stmts.iter().flatten().copied().collect();
                        self.steady(&stmts)
                    });
                    found.keeps(key, field)
                })
            }
            Type::Class(..) => true,
            _ => false,
        }
    }

    /// Whether a static of class `class` stays as it is while the statement being
    /// written runs (and the next, where they are written together; see `SteadyOf`), in a program
    /// without threads: the statements assign no static and run no Java code that might, no
    /// method or constructor, no `toString`, `equals` or `hashCode` of an object that a library
    /// method or a concatenation calls, no other class's static initialization.
    pub(super) fn static_steady(&self, class: usize) -> bool {
        let Some(s) = self.steady.as_ref() else { return false };
        let current = self.class.methods[self.method].class;
        let objects = |ty: Type| {
            let mut ty = ty;
            while let Type::Array(inner) = ty {
                ty = *inner;
            }
            matches!(ty, Type::Class(..) | Type::Enum(_) | Type::Null)
        };
        let runs_java = |x: &Expr| match &x.kind {
            ExprKind::Assign { target: Place::Static { .. }, .. }
            | ExprKind::IncDec { target: Place::Static { .. }, .. }
            | ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. }
            | ExprKind::Switch { .. }
            | ExprKind::EnumConstant { .. } => true,
            ExprKind::Builtin(_, args) => args.iter().any(|a| objects(a.ty)),
            ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => objects(l.ty) || objects(r.ty),
            ExprKind::Static { class: c, .. } if *c != class => {
                self.facts.init_class(self.class, *c).is_some_and(|init| {
                    init != current && !self.class.hierarchy.ancestors(current).contains(&init)
                })
            }
            _ => false,
        };
        let mut steady = true;
        for stmt in s.stmts.iter().flatten() {
            visit_stmt(stmt, &mut |e| visit_expr(e, &mut |x| steady &= !runs_java(x)), &mut |_| {});
        }
        steady
    }

    /// What the statements do to the objects that `this` and the locals name (see `Steady`).
    fn steady(&self, stmts: &[&Stmt]) -> Steady {
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
            _ if matches!(e.ty, Type::Enum(_)) => {
                let text = match self.checked_enum_text(e) {
                    Some(text) => text,
                    None => self.expr(e),
                };
                out.push(Part::Text(text))
            }
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

/// Whether the parentheses of `code` pair up, none closing before it opens.
fn balanced(code: &str) -> bool {
    let mut depth = 0usize;
    for c in code.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return false,
            ')' => depth -= 1,
            _ => {}
        }
    }
    depth == 0
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
