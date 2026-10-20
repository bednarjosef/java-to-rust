//! Generates Rust source from the checked syntax tree. Every construct is translated so that it
//! behaves exactly like Java: `int` arithmetic wraps, division by zero throws, doubles print
//! like Java. Where the analyses prove a plainer form equivalent (an addition that cannot
//! overflow, a divisor that cannot be zero, a counting `for` loop, a string parameter that is
//! only read) that form is used instead.

use crate::analysis::Facts;
use crate::ast::*;
use std::collections::{HashMap, HashSet};

mod arrays;

use arrays::name_part;

pub fn generate(class: &Class, facts: &Facts) -> String {
    let mut g = Gen {
        class,
        facts,
        method: 0,
        borrowed: HashSet::new(),
        int_receiver: false,
        double_receiver: false,
        annotations: Vec::new(),
        fn_names: method_names(class),
        locals: HashMap::new(),
        out: String::new(),
        indent: 0,
        loops: Vec::new(),
        ret: Type::Void,
        uses_runtime: false,
    };
    for i in 0..class.methods.len() {
        if i > 0 {
            g.out.push('\n');
        }
        g.method(i);
    }
    let mut header = format!("// Translated from the Java class `{}` by java-to-rust.\n\n", class.name);
    if g.uses_runtime {
        header.push_str("use java_runtime as java;\n\n");
    }
    header + &g.out
}

// Precedence of generated Rust expressions, from loosest to tightest.
const LOWEST: u8 = 0;
const OR: u8 = 5;
const AND: u8 = 10;
const CMP: u8 = 20;
const BIT_OR: u8 = 30;
const BIT_XOR: u8 = 35;
const BIT_AND: u8 = 40;
const SHIFT: u8 = 50;
const ADD: u8 = 60;
const MUL: u8 = 70;
const AS: u8 = 80;
const UNARY: u8 = 90;
const ATOM: u8 = 100;

/// A generated Rust expression.
struct Code {
    text: String,
    prec: u8,
    /// Set for numeric literals, whose Rust type comes from context unless suffixed.
    literal: Option<Literal>,
    /// Whether the Rust type is left to inference, as for `0` or `if c { 1 } else { 2 }`.
    /// Rust cannot call methods on such values, so their type must be written somewhere.
    untyped: bool,
    /// For an untyped expression of literals such as `24 * 60`, the text with its type written.
    typed_text: Option<String>,
}

#[derive(Clone, Copy)]
enum Literal {
    Int(i32),
    Long(i64),
    Double(f64),
}

impl Code {
    fn new(text: impl Into<String>, prec: u8) -> Code {
        Code { text: text.into(), prec, literal: None, untyped: false, typed_text: None }
    }

    fn atom(text: impl Into<String>) -> Code {
        Code::new(text, ATOM)
    }

    fn literal(literal: Literal) -> Code {
        let (text, negative) = match literal {
            Literal::Int(i32::MIN) => ("i32::MIN".to_string(), false),
            Literal::Long(i64::MIN) => ("i64::MIN".to_string(), false),
            Literal::Int(v) => (v.to_string(), v < 0),
            Literal::Long(v) => (v.to_string(), v < 0),
            Literal::Double(v) => (format!("{v:?}"), v.is_sign_negative()),
        };
        Code {
            text,
            prec: if negative { UNARY } else { ATOM },
            literal: Some(literal),
            untyped: true,
            typed_text: None,
        }
    }

    /// The text, in parentheses if it binds looser than `prec`.
    fn at(&self, prec: u8) -> String {
        if self.prec < prec { format!("({})", self.text) } else { self.text.clone() }
    }

    /// The text with a type suffix on a numeric literal, so that its type does not depend on
    /// inference.
    fn typed(&self) -> String {
        if let Some(text) = &self.typed_text {
            return text.clone();
        }
        match self.literal {
            Some(Literal::Int(v)) if v != i32::MIN => format!("{v}i32"),
            Some(Literal::Long(v)) if v != i64::MIN => format!("{v}i64"),
            Some(Literal::Double(v)) => format!("{v:?}f64"),
            _ => self.text.clone(),
        }
    }

    /// The text as the operand of `as`; a literal's type must not come from the cast.
    fn cast_operand(&self) -> String {
        if self.literal.is_some() { self.typed() } else { self.at(AS) }
    }

    /// The text as the receiver of a method call.
    fn receiver(&self) -> String {
        let text = self.typed();
        if self.prec < ATOM { format!("({text})") } else { text }
    }
}

fn rust_type(ty: Type) -> &'static str {
    match ty {
        Type::Int => "i32",
        Type::Long => "i64",
        Type::Double => "f64",
        Type::Boolean => "bool",
        Type::String => "String",
        Type::Void => "()",
        Type::Array(_) | Type::Class(_) | Type::Null => unreachable!("lowering rejects {ty}"),
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "box", "crate", "dyn", "fn", "gen", "impl", "in", "let", "loop", "macro",
    "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "self", "struct", "trait", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "yield", "become", "extern",
];

/// `maxValue` becomes `max_value`, `HTTPServer` becomes `http_server`.
fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_lower) {
                out.push('_');
            }
        }
        if c == '$' {
            out.push('_');
        } else {
            out.extend(c.to_lowercase());
        }
    }
    if RUST_KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

fn unique(base: String, used: &mut HashSet<String>) -> String {
    let mut name = base.clone();
    let mut n = 2;
    while used.contains(&name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    used.insert(name.clone());
    name
}

/// Rust names for the class's methods. Overloads get their parameter types appended, since Rust
/// has no overloading: `print(int)` and `print(double)` become `print_i32` and `print_f64`.
fn method_names(class: &Class) -> Vec<String> {
    let mut used = HashSet::new();
    if class.methods.iter().any(|m| m.is_main) {
        used.insert("main".to_string());
    }
    class
        .methods
        .iter()
        .map(|m| {
            if m.is_main {
                return "main".to_string();
            }
            let mut base = snake_case(&m.name);
            let overloaded = class.methods.iter().filter(|o| o.name == m.name).count() > 1;
            if overloaded {
                for p in &m.params {
                    base = format!("{base}_{}", name_part(p.ty));
                }
            }
            unique(base, &mut used)
        })
        .collect()
}

/// The innermost enclosing loop, which decides how `continue` is translated.
#[derive(Clone, Copy)]
enum Loop<'a> {
    /// Rust's `continue` does what Java's does.
    Plain,
    /// A `for` loop translated to `while`: `continue` must run the update first.
    For(&'a [Expr]),
    /// A `do`-`while` loop translated to `loop`: `continue` must test the condition first.
    DoWhile(&'a Expr),
}

enum Part {
    Lit(String),
    Arg(Code),
}

struct Gen<'a> {
    class: &'a Class,
    facts: &'a Facts,
    /// The index of the current method.
    method: usize,
    /// The current method's string parameters that are borrowed as `&str`.
    borrowed: HashSet<String>,
    /// Whether the current method calls a method on an `i32` or `f64` value that is not a
    /// literal. Rust needs the type of such a value to be known, so variables initialized with a
    /// bare literal then get their type written (see `annotation`).
    int_receiver: bool,
    double_receiver: bool,
    /// Placeholders for type annotations decided at the end of the method: (placeholder, type,
    /// annotation).
    annotations: Vec<(String, Type, String)>,
    fn_names: Vec<String>,
    /// Java local variable names to Rust names, for the current method.
    locals: HashMap<String, String>,
    out: String,
    indent: usize,
    loops: Vec<Loop<'a>>,
    /// The return type of the current method.
    ret: Type,
    uses_runtime: bool,
}

impl<'a> Gen<'a> {
    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn runtime(&mut self, path: &str) -> String {
        self.uses_runtime = true;
        format!("java::{path}")
    }

    fn local(&self, java: &str) -> String {
        self.locals.get(java).cloned().unwrap_or_else(|| snake_case(java))
    }

    /// `code` as the receiver of a method call on a value of Java type `ty`.
    fn recv(&mut self, code: &Code, ty: Type) -> String {
        if code.literal.is_none() {
            match ty {
                Type::Int => self.int_receiver = true,
                Type::Double => self.double_receiver = true,
                _ => {}
            }
        }
        code.receiver()
    }

    /// The type written for a variable initialized with an untyped expression: `: i64`, since an
    /// integer literal would default to `i32`; for `i32` and `f64`, which are the defaults, only
    /// when the method calls methods on such values.
    fn annotation(&mut self, ty: Type, text: String) -> String {
        if !matches!(ty, Type::Int | Type::Double) {
            return text;
        }
        let placeholder = format!("\u{1}{}\u{1}", self.annotations.len());
        self.annotations.push((placeholder.clone(), ty, text));
        placeholder
    }

    fn resolve_annotations(&mut self, from: usize) {
        let mut text = self.out.split_off(from);
        for (placeholder, ty, annotation) in self.annotations.drain(..) {
            let needed = if ty == Type::Int { self.int_receiver } else { self.double_receiver };
            text = text.replace(&placeholder, if needed { &annotation } else { "" });
        }
        self.out.push_str(&text);
    }

    fn no_overflow(&self, id: ExprId) -> bool {
        self.facts.no_overflow.contains(&id)
    }

    fn method(&mut self, index: usize) {
        let m = &self.class.methods[index];
        self.ret = m.ret;
        self.method = index;
        self.name_locals(m);
        let start = self.out.len();
        self.int_receiver = false;
        self.double_receiver = false;
        self.borrowed = (0..m.params.len())
            .filter(|&i| self.facts.borrowed_params.contains(&(index, i)))
            .map(|i| m.params[i].name.clone())
            .collect();
        let params: Vec<String> = m
            .params
            .iter()
            .map(|p| {
                let mutable = if assigns_any(&m.body, &p.name) { "mut " } else { "" };
                let ty =
                    if self.borrowed.contains(&p.name) { "&str".to_string() } else { self.ty_name(p.ty) };
                format!("{mutable}{}: {ty}", self.local(&p.name))
            })
            .collect();
        let ret = if m.ret == Type::Void { String::new() } else { format!(" -> {}", self.ty_name(m.ret)) };
        self.line(&format!("fn {}({}){ret} {{", self.fn_names[index], params.join(", ")));
        self.indent += 1;
        self.stmts(&m.body, true);
        self.indent -= 1;
        self.line("}");
        self.resolve_annotations(start);
    }

    /// Picks Rust names for the method's parameters and locals: snake case, distinct from each
    /// other and from the functions, which share Rust's value namespace.
    fn name_locals(&mut self, m: &Method) {
        let mut java_names = Vec::new();
        java_names.extend(m.params.iter().map(|p| p.name.clone()));
        collect_locals(&m.body, &mut java_names);
        let mut used: HashSet<String> = self.fn_names.iter().cloned().collect();
        used.insert("java".into());
        self.locals.clear();
        for name in java_names {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.locals.entry(name) {
                let rust = unique(snake_case(entry.key()), &mut used);
                entry.insert(rust);
            }
        }
    }

    /// The statements of a block. In a function body (`tail`), a final `return x;` becomes the
    /// tail expression `x`.
    fn stmts(&mut self, stmts: &'a [Stmt], tail: bool) {
        for (i, stmt) in stmts.iter().enumerate() {
            let rest = &stmts[i + 1..];
            match stmt {
                Stmt::Return(Some(value), _) if tail && rest.is_empty() => {
                    let code = self.returned(value);
                    self.line(&code.text);
                }
                _ => self.stmt(stmt, rest),
            }
        }
    }

    /// The body of an `if` or a loop.
    fn nested(&mut self, stmt: &'a Stmt) {
        self.indent += 1;
        match stmt {
            Stmt::Block(stmts) => self.stmts(stmts, false),
            _ => self.stmt(stmt, &[]),
        }
        self.indent -= 1;
    }

    /// One statement; `rest` holds the statements after it in the same block, where a local
    /// declared here is in scope.
    fn stmt(&mut self, stmt: &'a Stmt, rest: &'a [Stmt]) {
        match stmt {
            Stmt::Local { ty, name, init, .. } => {
                self.local_decl(*ty, name, init.as_ref(), assigns_any(rest, name));
            }
            Stmt::Expr(e) => self.effect(e),
            Stmt::ForEach { name, array, body, .. } => self.for_each(name, array, body),
            Stmt::If { cond, then, otherwise } => {
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
            Stmt::Return(None, _) => self.line("return;"),
            Stmt::Return(Some(value), _) => {
                let code = self.returned(value);
                self.line(&format!("return {};", code.text));
            }
            Stmt::Break => self.line("break;"),
            Stmt::Continue => match self.loops.last().copied() {
                Some(Loop::For(update)) => {
                    for u in update {
                        self.effect(u);
                    }
                    self.line("continue;");
                }
                Some(Loop::DoWhile(cond)) => {
                    self.exit_unless(cond);
                    self.line("continue;");
                }
                _ => self.line("continue;"),
            },
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
    fn returned(&mut self, value: &'a Expr) -> Code {
        if let ExprKind::Var(name) = &value.kind
            && value.ty == Type::String
            && !self.borrowed.contains(name)
        {
            return Code::atom(self.local(name));
        }
        self.value(value, self.ret)
    }

    /// `if !cond { break; }`, ending a translated `do`-`while` loop.
    fn exit_unless(&mut self, cond: &'a Expr) {
        let negated = self.negate(cond);
        self.line(&format!("if {negated} {{"));
        self.indent += 1;
        self.line("break;");
        self.indent -= 1;
        self.line("}");
    }

    fn loop_body(&mut self, kind: Loop<'a>, body: &'a Stmt, update: &'a [Expr]) {
        self.loops.push(kind);
        self.nested(body);
        self.indent += 1;
        for u in update {
            self.effect(u);
        }
        self.indent -= 1;
        self.loops.pop();
    }

    fn for_loop(&mut self, init: &'a [Stmt], cond: Option<&'a Expr>, update: &'a [Expr], body: &'a Stmt) {
        if let Some((var, start, end)) = counting_loop(init, cond, update, body) {
            let ty = start.ty.max_integral(end.ty);
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
                    let mutable = assigns_any(&init[i + 1..], name)
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

    fn local_decl(&mut self, ty: Type, name: &str, init: Option<&'a Expr>, mutable: bool) {
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
    fn condition(&mut self, cond: &'a Expr) -> String {
        let code = self.expr(cond);
        if code.text.starts_with('{') { format!("({})", code.text) } else { code.text }
    }

    /// The negation of a condition, flipping the comparison where that is exact.
    fn negate(&mut self, cond: &'a Expr) -> String {
        match &cond.kind {
            ExprKind::Unary(UnOp::Not, inner) => return self.condition(inner),
            ExprKind::Binary(op, l, r)
                if op.is_comparison() && l.ty != Type::Double && r.ty != Type::Double =>
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
    fn effect(&mut self, e: &'a Expr) {
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
                let text = self.index_update(*op, array, index, e.ty, value, e.id);
                self.line(&format!("{text};"));
            }
            ExprKind::IncDec { target: Place::Index(array, index), increment, prefix } => {
                let text = self.index_step(array, index, e.ty, *increment, *prefix, e.id);
                self.line(&format!("{text};"));
            }
            _ => {
                let code = self.expr(e);
                self.line(&format!("{};", code.text));
            }
        }
    }

    /// `target = value` or `target op= value` as a Rust statement, without the semicolon.
    fn assignment(
        &mut self,
        op: Option<BinOp>,
        target: &str,
        ty: Type,
        value: &'a Expr,
        id: ExprId,
    ) -> String {
        let name = self.local(target);
        let Some(op) = op else {
            return format!("{name} = {}", self.value(value, ty).text);
        };
        if ty == Type::String {
            return self.append(&name, target, value);
        }
        let compound = |op: BinOp| match op {
            BinOp::Add => "+=",
            BinOp::Sub => "-=",
            BinOp::Mul => "*=",
            BinOp::Div => "/=",
            BinOp::Rem => "%=",
            BinOp::BitAnd => "&=",
            BinOp::BitOr => "|=",
            BinOp::BitXor => "^=",
            BinOp::Shl => "<<=",
            _ => ">>=",
        };
        let result = crate::types::binary_type(op, ty, value.ty).expect("checked");
        let simple = match op {
            // Double arithmetic has no traps, and the result is already a double.
            _ if ty == Type::Double => {
                !op.is_shift() && !matches!(op, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor)
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul => result == ty && self.no_overflow(id),
            BinOp::Div | BinOp::Rem => result == ty && self.safe_division(id, value),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                ty == Type::Boolean || (ty.is_integral() && crate::types::promote(ty, value.ty) == ty)
            }
            // Shifting by a constant in range cannot overflow.
            BinOp::Shl | BinOp::Shr => ty.is_integral() && in_shift_range(value, ty),
            _ => false,
        };
        if simple {
            let v = self.value(value, if op.is_shift() { value.ty } else { ty });
            return format!("{name} {} {}", compound(op), v.text);
        }
        // Java's compound assignment casts the result back to the variable's type.
        let current = Code::atom(name.clone());
        let right = self.operand(value);
        let code = self.binary(op, Operand { code: current, ty }, right, result, id);
        format!("{name} = {}", self.convert(code, result, ty).text)
    }

    /// Whether a division or remainder can neither throw nor overflow.
    fn safe_division(&self, id: ExprId, divisor: &Expr) -> bool {
        let constant = match divisor.kind {
            ExprKind::Int(v) => Some(v as i64),
            ExprKind::Long(v) => Some(v),
            _ => None,
        };
        self.facts.safe_division.contains(&id) || constant.is_some_and(|v| v != 0 && v != -1)
    }

    /// `s += value` on a string.
    fn append(&mut self, name: &str, target: &str, value: &'a Expr) -> String {
        if !mentions(value, target) {
            if value.ty == Type::String && matches!(value.kind, ExprKind::Str(_) | ExprKind::Var(_)) {
                return format!("{name} += {}", self.str_ref(value));
            }
            let mut parts = Vec::new();
            self.parts(value, &mut parts);
            if parts.len() == 1 {
                match parts.pop().expect("one part") {
                    Part::Lit(text) => return format!("{name} += {}", rust_string(&text)),
                    Part::Arg(code) => {
                        let ty = if value.ty == Type::Double { Type::Void } else { value.ty };
                        return format!("{name} += &{}.to_string()", self.recv(&code, ty));
                    }
                }
            }
            return format!("{name} += &{}", self.format("format!", parts).text);
        }
        let mut parts = vec![Part::Arg(Code::atom(name))];
        self.parts(value, &mut parts);
        format!("{name} = {}", self.format("format!", parts).text)
    }

    /// `x += 1` or `x -= 1` as a Rust statement.
    fn step(&mut self, target: &str, ty: Type, increment: bool, id: ExprId) -> String {
        let name = self.local(target);
        if ty == Type::Double || self.no_overflow(id) {
            let one = if ty == Type::Double { "1.0" } else { "1" };
            format!("{name} {} {one}", if increment { "+=" } else { "-=" })
        } else {
            let recv = self.recv(&Code::atom(name.clone()), ty);
            let method = if increment { "wrapping_add" } else { "wrapping_sub" };
            format!("{name} = {recv}.{method}(1)")
        }
    }

    /// `e` as a value of type `ty`, applying Java's widening conversions.
    fn value(&mut self, e: &'a Expr, ty: Type) -> Code {
        let code = self.expr(e);
        self.convert(code, e.ty, ty)
    }

    fn operand(&mut self, e: &'a Expr) -> Operand {
        Operand { code: self.expr(e), ty: e.ty }
    }

    /// Converts between numeric types, folding conversions of literals.
    fn convert(&mut self, code: Code, from: Type, to: Type) -> Code {
        if from == to || !from.is_numeric() || !to.is_numeric() {
            return code;
        }
        if let Some(literal) = code.literal {
            let (i, d) = match literal {
                Literal::Int(v) => (v as i64, v as f64),
                Literal::Long(v) => (v, v as f64),
                Literal::Double(v) => (v as i64, v),
            };
            return Code::literal(match to {
                Type::Int => match literal {
                    Literal::Double(v) => Literal::Int(v as i32),
                    _ => Literal::Int(i as i32),
                },
                Type::Long => Literal::Long(i),
                _ => Literal::Double(d),
            });
        }
        Code::new(format!("{} as {}", code.cast_operand(), rust_type(to)), AS)
    }

    fn expr(&mut self, e: &'a Expr) -> Code {
        match &e.kind {
            ExprKind::Int(v) => Code::literal(Literal::Int(*v)),
            ExprKind::Long(v) => Code::literal(Literal::Long(*v)),
            ExprKind::Double(v) => Code::literal(Literal::Double(*v)),
            ExprKind::Bool(b) => Code::atom(b.to_string()),
            ExprKind::Str(s) if s.is_empty() => Code::atom("String::new()"),
            ExprKind::Str(s) => Code::atom(format!("String::from({})", rust_string(s))),
            ExprKind::Var(name) => {
                let rust = self.local(name);
                match e.ty {
                    Type::String if self.borrowed.contains(name) => Code::atom(format!("{rust}.to_string()")),
                    Type::String | Type::Array(_) => Code::atom(format!("{rust}.clone()")),
                    _ => Code::atom(rust),
                }
            }
            ExprKind::Unary(op, operand) => {
                let code = self.value(operand, e.ty);
                match (op, e.ty) {
                    (UnOp::Plus, _) => code,
                    (UnOp::Neg, Type::Double) => Code::new(format!("-{}", code.at(UNARY)), UNARY),
                    (UnOp::Neg, _) if self.no_overflow(e.id) => {
                        Code::new(format!("-{}", code.at(UNARY)), UNARY)
                    }
                    (UnOp::Neg, _) => Code::atom(format!("{}.wrapping_neg()", self.recv(&code, e.ty))),
                    (UnOp::Not | UnOp::BitNot, _) => Code::new(format!("!{}", code.at(UNARY)), UNARY),
                }
            }
            ExprKind::Binary(BinOp::Add, ..) if e.ty == Type::String => {
                let mut parts = Vec::new();
                self.parts(e, &mut parts);
                self.format("format!", parts)
            }
            ExprKind::Binary(op, l, r) => {
                let (l, r) = (self.operand(l), self.operand(r));
                self.binary(*op, l, r, e.ty, e.id)
            }
            ExprKind::Assign { op, target: Place::Var(target), value } => {
                let text = self.assignment(*op, target, e.ty, value, e.id);
                Code::new(format!("{{ {text}; {} }}", self.read(target, e.ty)), LOWEST)
            }
            ExprKind::IncDec { target: Place::Var(target), increment, prefix } => {
                let text = self.step(target, e.ty, *increment, e.id);
                let name = self.local(target);
                if *prefix {
                    Code::new(format!("{{ {text}; {name} }}"), LOWEST)
                } else {
                    let old = if name == "old" { "previous" } else { "old" };
                    Code::new(format!("{{ let {old} = {name}; {text}; {old} }}"), LOWEST)
                }
            }
            ExprKind::Assign { op: None, target: Place::Index(array, index), value } => {
                Code::atom(self.index_set(e.id, array, index, e.ty, value, true))
            }
            ExprKind::Assign { op: Some(op), target: Place::Index(array, index), value } => {
                Code::atom(self.index_update(*op, array, index, e.ty, value, e.id))
            }
            ExprKind::IncDec { target: Place::Index(array, index), increment, prefix } => {
                Code::atom(self.index_step(array, index, e.ty, *increment, *prefix, e.id))
            }
            ExprKind::Index(array, index) => self.index(e.id, array, index),
            ExprKind::Length(array) => self.length(array),
            ExprKind::NewArray(sizes) => self.new_array(e.ty, sizes),
            ExprKind::ArrayLit(items) => self.array_lit(e.ty, items),
            ExprKind::Cast(ty, value) => self.value(value, *ty),
            ExprKind::Conditional(cond, a, b) => {
                let cond = self.condition(cond);
                let a = self.value(a, e.ty);
                let b = self.value(b, e.ty);
                let mut code = Code::new(format!("if {cond} {{ {} }} else {{ {} }}", a.text, b.text), LOWEST);
                code.untyped = a.untyped && b.untyped;
                code
            }
            ExprKind::Call { args, resolved, .. } => {
                let index = resolved.expect("calls are resolved by the checker");
                let params = &self.class.methods[index].params;
                let args: Vec<String> = args
                    .iter()
                    .zip(params)
                    .enumerate()
                    .map(|(i, (a, p))| {
                        if self.facts.borrowed_params.contains(&(index, i)) {
                            self.str_ref(a)
                        } else {
                            self.value(a, p.ty).text
                        }
                    })
                    .collect();
                Code::atom(format!("{}({})", self.fn_names[index], args.join(", ")))
            }
            ExprKind::Builtin(builtin, args) => self.builtin(*builtin, args, e.ty, e.id),
        }
    }

    /// Reads a variable after assigning it, for an assignment used as a value.
    fn read(&self, target: &str, ty: Type) -> String {
        let name = self.local(target);
        if ty == Type::String { format!("{name}.clone()") } else { name }
    }

    fn binary(&mut self, op: BinOp, l: Operand, r: Operand, result: Type, id: ExprId) -> Code {
        // Comparisons promote their operands; other operators work in the result type, except
        // that a shift distance keeps its own type.
        let operand_type = if op.is_comparison() && l.ty.is_numeric() {
            crate::types::promote(l.ty, r.ty)
        } else if op.is_comparison() {
            l.ty
        } else {
            result
        };
        let left = self.convert(l.code, l.ty, operand_type);
        let right = if op.is_shift() { r.code } else { self.convert(r.code, r.ty, operand_type) };
        // Integer arithmetic that may overflow wraps, as in Java.
        let integral = operand_type.is_integral();
        let wrapping = integral && !self.no_overflow(id);
        let mut method = |name: &str, left: &Code, right: &Code| {
            Code::atom(format!("{}.{name}({})", self.recv(left, operand_type), right.text))
        };
        match op {
            BinOp::Add if wrapping => method("wrapping_add", &left, &right),
            BinOp::Sub if wrapping => method("wrapping_sub", &left, &right),
            BinOp::Mul if wrapping => method("wrapping_mul", &left, &right),
            BinOp::Div | BinOp::Rem if integral => {
                // Dividing by a constant other than 0 and -1 can neither throw nor overflow.
                let constant = match right.literal {
                    Some(Literal::Int(v)) => v != 0 && v != -1,
                    Some(Literal::Long(v)) => v != 0 && v != -1,
                    _ => false,
                };
                if constant || self.facts.safe_division.contains(&id) {
                    infix(if op == BinOp::Div { "/" } else { "%" }, MUL, &left, &right)
                } else {
                    let f = self.runtime(if op == BinOp::Div { "div" } else { "rem" });
                    Code::atom(format!("{f}({}, {})", left.typed(), right.text))
                }
            }
            BinOp::Shl | BinOp::Shr | BinOp::UShr => self.shift(op, left, right, result),
            BinOp::Add => infix("+", ADD, &left, &right),
            BinOp::Sub => infix("-", ADD, &left, &right),
            BinOp::Mul => infix("*", MUL, &left, &right),
            BinOp::Div => infix("/", MUL, &left, &right),
            BinOp::Rem => infix("%", MUL, &left, &right),
            BinOp::BitAnd => infix("&", BIT_AND, &left, &right),
            BinOp::BitOr => infix("|", BIT_OR, &left, &right),
            BinOp::BitXor => infix("^", BIT_XOR, &left, &right),
            BinOp::And => infix("&&", AND, &left, &right),
            BinOp::Or => infix("||", OR, &left, &right),
            _ => infix(op.java(), CMP, &left, &right),
        }
    }

    /// Java masks the shift distance to the width of the shifted type; so does Rust's
    /// `wrapping_shl`, and a constant distance is masked here.
    fn shift(&mut self, op: BinOp, left: Code, right: Code, ty: Type) -> Code {
        let bits = if ty == Type::Long { 64 } else { 32 };
        let unsigned = if ty == Type::Long { "u64" } else { "u32" };
        let constant = match right.literal {
            Some(Literal::Int(v)) => Some(v as i64 & (bits - 1)),
            Some(Literal::Long(v)) => Some(v & (bits - 1)),
            _ => None,
        };
        match (op, constant) {
            (BinOp::UShr, Some(0)) | (BinOp::Shl | BinOp::Shr, Some(0)) => left,
            (BinOp::Shl, Some(n)) => infix("<<", SHIFT, &left, &Code::literal(Literal::Int(n as i32))),
            (BinOp::Shr, Some(n)) => infix(">>", SHIFT, &left, &Code::literal(Literal::Int(n as i32))),
            (BinOp::UShr, Some(n)) => {
                let shifted = format!("({} as {unsigned}) >> {n}", left.cast_operand());
                Code::new(format!("({shifted}) as {}", rust_type(ty)), AS)
            }
            _ => {
                let distance = format!("{} as u32", right.cast_operand());
                match op {
                    BinOp::Shl => Code::atom(format!("{}.wrapping_shl({distance})", self.recv(&left, ty))),
                    BinOp::Shr => Code::atom(format!("{}.wrapping_shr({distance})", self.recv(&left, ty))),
                    _ => Code::new(
                        format!(
                            "({} as {unsigned}).wrapping_shr({distance}) as {}",
                            left.cast_operand(),
                            rust_type(ty)
                        ),
                        AS,
                    ),
                }
            }
        }
    }

    fn builtin(&mut self, builtin: Builtin, args: &'a [Expr], ty: Type, id: ExprId) -> Code {
        match builtin {
            Builtin::ArraysToString | Builtin::ArraysFill | Builtin::ArraysSort | Builtin::ArraysCopyOf => {
                self.arrays_builtin(builtin, args)
            }
            Builtin::Println | Builtin::Print => {
                let mut parts = Vec::new();
                if let Some(arg) = args.first() {
                    self.parts(arg, &mut parts);
                }
                let mac = if builtin == Builtin::Println { "println!" } else { "print!" };
                self.format(mac, parts)
            }
            Builtin::Exit => {
                let code = self.value(&args[0], Type::Int);
                Code::atom(format!("{}({})", self.runtime("exit"), code.text))
            }
            Builtin::MathAbs => {
                let x = self.value(&args[0], ty);
                // Math.abs(Integer.MIN_VALUE) is Integer.MIN_VALUE.
                let method = if ty == Type::Double || self.no_overflow(id) { "abs" } else { "wrapping_abs" };
                Code::atom(format!("{}.{method}()", self.recv(&x, ty)))
            }
            Builtin::MathMax | Builtin::MathMin => {
                let a = self.value(&args[0], ty);
                let b = self.value(&args[1], ty);
                let name = if builtin == Builtin::MathMax { "max" } else { "min" };
                if ty == Type::Double {
                    // f64::max ignores NaN; Java's Math.max returns it.
                    Code::atom(format!("{}({}, {})", self.runtime(name), a.text, b.text))
                } else {
                    Code::atom(format!("{}.{name}({})", self.recv(&a, ty), b.text))
                }
            }
            Builtin::MathSqrt => {
                let x = self.value(&args[0], Type::Double);
                Code::atom(format!("{}.sqrt()", self.recv(&x, Type::Double)))
            }
            Builtin::MathPow => {
                let a = self.value(&args[0], Type::Double);
                let b = self.value(&args[1], Type::Double);
                Code::atom(format!("{}.powf({})", self.recv(&a, Type::Double), b.text))
            }
            // UTF-16 length and UTF-8 length agree on ASCII text.
            Builtin::StringLength if self.facts.ascii_length.contains(&id) => {
                let s = match &args[0].kind {
                    ExprKind::Str(_) | ExprKind::Var(_) => self.str_value(&args[0]),
                    _ => self.expr(&args[0]).at(ATOM),
                };
                Code::new(format!("{s}.len() as i32"), AS)
            }
            Builtin::StringLength => {
                let s = self.str_ref(&args[0]);
                Code::atom(format!("{}({})", self.runtime("string_length"), s))
            }
            Builtin::StringEquals => {
                let a = Code::atom(self.str_value(&args[0]));
                let b = Code::atom(self.str_value(&args[1]));
                infix("==", CMP, &a, &b)
            }
        }
    }

    /// A string expression as a `&str`.
    fn str_ref(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => rust_string(s),
            ExprKind::Var(name) if self.borrowed.contains(name) => self.local(name),
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// A string expression for comparing, without copying variables.
    fn str_value(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => rust_string(s),
            ExprKind::Var(name) => self.local(name),
            _ => self.expr(e).at(CMP + 1),
        }
    }

    /// Splits `e` into the pieces of a format string: string concatenation is flattened,
    /// constants become text, and everything else is formatted the way Java prints it.
    fn parts(&mut self, e: &'a Expr, out: &mut Vec<Part>) {
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
        self.concat_parts(e, &assigned, out);
    }

    fn concat_parts(&mut self, e: &'a Expr, assigned: &HashSet<String>, out: &mut Vec<Part>) {
        match &e.kind {
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
                let copy =
                    if e.ty == Type::Double { format!("{}({copy})", self.runtime("Double")) } else { copy };
                out.push(Part::Arg(Code::atom(copy)));
            }
            ExprKind::Str(s) => out.push(Part::Lit(s.clone())),
            ExprKind::Int(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Long(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Bool(v) => out.push(Part::Lit(v.to_string())),
            ExprKind::Double(v) => out.push(Part::Lit(java_runtime::double_to_string(*v))),
            ExprKind::Var(name) if e.ty == Type::String => out.push(Part::Arg(Code::atom(self.local(name)))),
            _ => {
                let code = self.expr(e);
                if let Some(literal) = code.literal {
                    // A constant made by folding a cast, such as `(long) 1e19`.
                    out.push(Part::Lit(match literal {
                        Literal::Int(v) => v.to_string(),
                        Literal::Long(v) => v.to_string(),
                        Literal::Double(v) => java_runtime::double_to_string(v),
                    }));
                } else if e.ty == Type::Double {
                    let double = self.runtime("Double");
                    out.push(Part::Arg(Code::atom(format!("{double}({})", code.text))));
                } else {
                    out.push(Part::Arg(code));
                }
            }
        }
    }

    /// A `format!`-style macro call over `parts`. Plain variables are inlined into the format
    /// string; `format!` of constant text becomes `String::from`.
    fn format(&mut self, mac: &str, parts: Vec<Part>) -> Code {
        let mut template = String::new();
        let mut args = Vec::new();
        for part in parts {
            match part {
                Part::Lit(s) => {
                    let quoted = rust_string(&s);
                    template.push_str(&quoted[1..quoted.len() - 1].replace('{', "{{").replace('}', "}}"));
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
        if mac == "println!" && template.is_empty() {
            return Code::atom("println!()");
        }
        let args: String = args.iter().map(|a| format!(", {a}")).collect();
        Code::atom(format!("{mac}(\"{template}\"{args})"))
    }
}

struct Operand {
    code: Code,
    ty: Type,
}

fn infix(op: &str, prec: u8, left: &Code, right: &Code) -> Code {
    // `a as i64 < b` would parse `<` as the start of generic arguments.
    let left_cast = matches!(op, "<" | "<<") && ends_with_cast(&left.text) && left.prec >= prec;
    let left_prec = if prec == CMP { CMP + 1 } else { prec };
    let l = if left_cast { format!("({})", left.text) } else { left.at(left_prec) };
    let r = right.at(prec + 1);
    let mut code = Code::new(format!("{l} {op} {r}"), prec);
    if left.untyped && right.untyped && left.literal.is_some() {
        // In `7 / 2` the type is left to inference, which defaults to `i32` and `f64`: right for
        // int and double, but a long needs its type written.
        let typed = left.typed();
        let typed = if left.prec < left_prec { format!("({typed})") } else { typed };
        let typed_text = format!("{typed} {op} {r}");
        if matches!(left.literal, Some(Literal::Long(_))) {
            code.text = typed_text;
        } else if prec != CMP {
            code.untyped = true;
            code.typed_text = Some(typed_text);
        }
    }
    code
}

fn ends_with_cast(text: &str) -> bool {
    ["i32", "i64", "f64", "u32", "u64"].iter().any(|t| text.ends_with(&format!(" as {t}")))
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
        && !["true", "false"].contains(&text)
}

/// A Rust string literal with the given contents.
fn rust_string(s: &str) -> String {
    format!("{s:?}")
}

fn in_shift_range(e: &Expr, ty: Type) -> bool {
    let bits = if ty == Type::Long { 64 } else { 32 };
    match e.kind {
        ExprKind::Int(v) => (0..bits).contains(&(v as i64)),
        ExprKind::Long(v) => (0..bits).contains(&v),
        _ => false,
    }
}

/// Whether a loop body contains a `continue` of that loop (not of a nested loop).
fn has_continue(body: &Stmt) -> bool {
    match body {
        Stmt::Continue => true,
        Stmt::Block(stmts) => stmts.iter().any(has_continue),
        Stmt::If { then, otherwise, .. } => {
            has_continue(then) || otherwise.as_deref().is_some_and(has_continue)
        }
        _ => false,
    }
}

fn collect_locals(stmts: &[Stmt], out: &mut Vec<String>) {
    for stmt in stmts {
        visit_stmt(stmt, &mut |_| {}, &mut |s| {
            if let Stmt::Local { name, .. } | Stmt::ForEach { name, .. } = s {
                out.push(name.clone());
            }
        });
    }
}
