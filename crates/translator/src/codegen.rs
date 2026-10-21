//! Generates Rust source from the typed syntax tree. Every construct is translated so that it
//! behaves exactly like Java: `int` arithmetic wraps, division by zero throws, doubles print
//! like Java. Where the analyses prove a plainer form equivalent (an addition that cannot
//! overflow, a divisor that cannot be zero, a counting `for` loop, a string parameter that is
//! only read) that form is used instead.

use crate::analysis::Facts;
use crate::analysis::hierarchy::{self, Home};
use crate::ast::*;
use std::collections::{HashMap, HashSet};

mod boxing;
mod character;
mod charsets;
mod code;
mod code_point;
mod dispatch;
mod enums;
mod expr;
mod files;
mod idiomatic;
mod math;
mod names;
mod null;
mod primitives;
mod statics;
mod stmt;
mod strings;
mod switch;
mod tail;
mod text;
mod unsigned;

use code::*;
use dispatch::{Need, Needs};
use expr::Operand;
use idiomatic::{bool_test, literal_shift};
use names::*;
use null::is_null_test;
use primitives::*;
use switch::{Frame, Kind};

mod array_writes;
mod arrays;
mod java_arrays;
mod modules;
mod natives;
mod objects;
mod owned;
mod plain_arrays;
mod sliced_loops;
mod sync;

use arrays::name_part;
use boxing::boxed_locals;
use objects::*;

pub fn generate(class: &Class, facts: &Facts) -> String {
    // The enums of class types get the items that the code uses, which writing the code finds:
    // the program is written again until it asks for no item that was not written.
    let mut wanted = Needs::new();
    loop {
        let (text, needs) = generate_with(class, facts, &wanted);
        if needs.is_subset(&wanted) {
            return text;
        }
        wanted.extend(needs);
    }
}

/// The program, with the enum items `wanted`, and the enum items that it uses.
fn generate_with(class: &Class, facts: &Facts, wanted: &Needs) -> (String, Needs) {
    let live = live_types(class, facts);
    let modules = modules::enter(class);
    let homes: Vec<Home> = (0..class.methods.len()).map(|m| hierarchy::home(class, facts, m)).collect();
    let (fn_names, group_names) = method_names(class, &homes);
    let mut g = Gen {
        class,
        facts,
        fn_names,
        group_names,
        live: live.clone(),
        wanted: wanted.clone(),
        needs: std::cell::RefCell::new(Needs::new()),
        homes,
        super_targets: super_targets(class),
        this_struct: 0,
        method: 0,
        borrowed: HashSet::new(),
        borrowed_optional: HashSet::new(),
        int_receiver: false,
        double_receiver: false,
        this_form: ThisForm::Shared,
        display: false,
        lazy_text: false,
        formatting: false,
        chars: false,
        exhaustive: false,
        field_locals: HashMap::new(),
        ctor_result: None,
        printing: false,
        guarded: false,
        annotations: Vec::new(),
        locals: HashMap::new(),
        boxed: HashMap::new(),
        elements: HashMap::new(),
        sliced: HashMap::new(),
        sliced_accesses: HashSet::new(),
        out: String::new(),
        indent: 0,
        frames: Vec::new(),
        pending: None,
        labels: 0,
        ret: Type::Void,
        uses_runtime: false,
    };
    if modules.is_some() {
        let text = g.program(&live);
        return (text, g.needs.take());
    }
    for (index, _) in live.iter().enumerate().filter(|(_, live)| **live) {
        g.type_items(index);
    }
    for t in 0..class.types.len() {
        if wanted.contains(&Need::Enum(t)) {
            g.type_enum_items(t);
        }
    }
    // An enum is never built with `new`, so it is emitted whether or not a `new` names it.
    for index in (0..class.types.len()).filter(|&t| class.types[t].enum_constants.is_some()) {
        g.enum_items(index);
    }
    g.statics();
    for i in 0..class.methods.len() {
        if g.emitted_free(i) {
            g.gap();
            g.method(i);
        }
    }
    let mut header = format!("// Translated from the Java class `{}` by java-to-rust.\n\n", class.name);
    let uses = modules::use_lines(&g.out);
    if !uses.is_empty() {
        header.push_str(&(uses.join("\n") + "\n"));
    }
    (header + &g.out, g.needs.take())
}

/// How `this` is written in the code being generated.
#[derive(Clone, Copy, PartialEq)]
enum ThisForm {
    /// `this`, a reference to the shared object: `&Rc<RefCell<C>>`, or `&C` for an immutable class.
    Shared,
    /// `self`, in a method that borrows its object.
    SelfRef,
    /// `this`, the plain value that a constructor is building.
    Local,
    /// `self`, the value of a type's enum, in a method of the enum.
    Enum,
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
    /// Where each method is emitted (see `analysis::hierarchy`).
    homes: Vec<Home>,
    /// The name of each override group (by group id): the name of its dispatchers.
    group_names: HashMap<usize, String>,
    /// Which classes have a struct (see `live_types`).
    live: Vec<bool>,
    /// The enum items to write, which an earlier pass found used (see `generate`).
    wanted: Needs,
    /// The enum items that the code written so far uses.
    needs: std::cell::RefCell<Needs>,
    /// The methods that a `super.m(...)` call names, which run although a subclass overrides them.
    super_targets: HashSet<usize>,
    /// The class (index into `Class::types`) whose struct `this` is in the code being written:
    /// the class of the `impl` that the method is emitted in.
    this_struct: usize,
    /// The index of the current method.
    method: usize,
    /// The current method's string variables that are `&str`: borrowed parameters, and
    /// possibly-null strings inside a null check that narrowed them.
    borrowed: HashSet<String>,
    /// The current method's possibly-null string parameters that are borrowed as `Option<&str>`.
    borrowed_optional: HashSet<String>,
    /// Whether the current method calls a method on an `i32` or `f64` value that is not a
    /// literal. Rust needs the type of such a value to be known, so variables initialized with a
    /// bare literal then get their type written (see `annotation`).
    int_receiver: bool,
    double_receiver: bool,
    /// How `this` is written in the code being generated.
    this_form: ThisForm,
    /// Whether the method being written is a `toString` written as `Display::fmt`, whose returns
    /// write to the formatter `f`.
    display: bool,
    /// Whether the text being formatted shows objects through `Display`, when they are formatted
    /// rather than converted to a `String` first (see `parts`).
    lazy_text: bool,
    /// Whether the code being written is an operand of a string concatenation.
    formatting: bool,
    /// Whether the switch being written matches its `char` selector as a Rust `char`.
    chars: bool,
    /// Whether the cases of the switch being written cover every value of its enum selector.
    exhaustive: bool,
    /// The locals that hold the fields that a constructor's initializers computed first, by
    /// Java field name: the later initializers read them there.
    field_locals: HashMap<String, String>,
    /// In a constructor, what a `return` gives back: the object being built.
    ctor_result: Option<&'static str>,
    /// Whether the text being formatted is printed.
    printing: bool,
    /// Whether the statement being emitted also borrows an object mutably (see `conflicts`).
    guarded: bool,
    /// Placeholders for type annotations decided at the end of the method: (placeholder, type,
    /// annotation).
    annotations: Vec<(String, Type, String)>,
    fn_names: Vec<String>,
    /// Java local variable names to Rust names, for the current method.
    locals: HashMap<String, String>,
    /// The boxed types of the current method's parameters and locals, by Java name.
    boxed: HashMap<String, Type>,
    /// Reads of elements that a for-each-style loop has bound to a variable, by expression.
    elements: HashMap<ExprId, String>,
    /// The plain arrays that the loop being written reaches through slices (see
    /// `sliced_loops.rs`): Java name to the slice's Rust name and how many rows it skips.
    sliced: HashMap<String, (String, usize)>,
    /// The accesses that the check before a sliced loop puts in bounds.
    sliced_accesses: HashSet<ExprId>,
    out: String,
    indent: usize,
    /// The statements that Java can break out of or continue, innermost last (see `switch.rs`).
    frames: Vec<Frame<'a>>,
    /// The Java label of the statement whose frame opens next.
    pending: Option<String>,
    /// How many Rust labels have been made for statements without a Java label.
    labels: usize,
    /// The return type of the current method.
    ret: Type,
    uses_runtime: bool,
}

impl<'a> Gen<'a> {
    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        if self.guarded {
            self.out.push_str(&scoped_arguments(text));
        } else {
            self.out.push_str(text);
        }
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

    /// Sets up the state for writing method `index`: its locals and the forms of its parameters.
    fn enter_method(&mut self, index: usize) {
        let m = &self.class.methods[index];
        self.ret = m.ret;
        self.method = index;
        self.name_locals(m);
        self.int_receiver = false;
        self.double_receiver = false;
        self.borrowed = (0..m.params.len())
            .filter(|&i| {
                self.facts.borrowed_params.contains(&(index, i))
                    || self.facts.borrowed_objects.contains(&(index, i))
            })
            .map(|i| m.params[i].name.clone())
            .collect();
        self.borrowed_optional = (0..m.params.len())
            .filter(|&i| self.facts.borrowed_optional_params.contains(&(index, i)))
            .map(|i| m.params[i].name.clone())
            .collect();
    }

    /// The parameters of method `index` with their Rust types. For the method's own body
    /// (`body`), a parameter that it assigns is `mut` and one that it ignores is `_x`.
    fn typed_params(&mut self, index: usize, body: bool) -> Vec<String> {
        let m = &self.class.methods[index];
        m.params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mutable = if body && assigns_any(&m.body, &p.name) { "mut " } else { "" };
                let ty = if self.borrowed_optional.contains(&p.name) {
                    "Option<&str>".to_string()
                } else if self.optional_var(index, p.ty, &p.name) {
                    format!("Option<{}>", self.ty_name(p.ty))
                } else if self.facts.borrowed_objects.contains(&(index, i)) {
                    format!("&{}", self.ty_name(p.ty))
                } else if self.borrowed.contains(&p.name) {
                    "&str".to_string()
                } else if let Some(&written) = self.facts.borrowed_arrays.get(&(index, i)) {
                    self.borrowed_ty(p.ty, written)
                } else {
                    self.ty_name(p.ty)
                };
                // A body that ignores a parameter (a default that overrides replace) names it `_x`.
                let mut used = !body;
                for s in &m.body {
                    visit_stmt(s, &mut |e| used |= mentions(e, &p.name), &mut |_| {});
                }
                let name = self.local(&p.name);
                let name = if used { name } else { format!("_{}", name.trim_start_matches("r#")) };
                format!("{mutable}{name}: {ty}")
            })
            .collect()
    }

    /// The return type of method `index` as it is written after its parameters: ` -> T`, or
    /// nothing.
    fn ret_text(&mut self, index: usize) -> String {
        let m = &self.class.methods[index];
        let name = &self.class.types[m.class].name;
        if m.is_ctor && self.builds_plain(name) {
            format!(" -> {}", self.struct_of(m.class))
        } else if m.is_ctor {
            format!(" -> {}", self.object_type(name))
        } else if m.ret == Type::Void {
            String::new()
        } else if self.optional_return() {
            format!(" -> Option<{}>", self.ty_name(m.ret))
        } else if self.facts.owned_returns.contains(&index) {
            format!(" -> {}", self.plain_ty(m.ret))
        } else {
            format!(" -> {}", self.ty_name(m.ret))
        }
    }

    /// Whether method `index` is a function of its class's module: a static method, or the
    /// `init` of a class whose static initialization runs eagerly.
    fn emitted_free(&self, index: usize) -> bool {
        let m = &self.class.methods[index];
        if m.name == "<clinit>" { self.facts.eager_init.contains(&m.class) } else { m.is_static }
    }

    fn method(&mut self, index: usize) {
        let m = &self.class.methods[index];
        self.enter_method(index);
        let start = self.out.len();
        let name = &self.class.types[m.class].name;
        self.this_struct = match self.homes[index] {
            Home::Class(c) | Home::Fields(c) | Home::Enum(c) => c,
            Home::Free | Home::Nowhere => m.class,
        };
        let is_enum = self.class.types[self.this_struct].enum_constants.is_some();
        let on_enum = matches!(self.homes[index], Home::Enum(_)) && !m.is_ctor;
        let borrows =
            !m.is_static && !m.is_ctor && !on_enum && (is_enum || self.facts.self_methods.contains(&index));
        self.display = borrows && !is_enum && self.displays(index);
        self.this_form = match () {
            _ if on_enum => ThisForm::Enum,
            _ if borrows => ThisForm::SelfRef,
            _ => ThisForm::Shared,
        };
        let mut params = Vec::new();
        if borrows && is_enum {
            // An enum is a `Copy` value, so its methods take `self` by value.
            params.push("self".to_string());
        } else if borrows {
            params.push(if self.facts.mutating.contains(&index) { "&mut self" } else { "&self" }.to_string());
        } else if on_enum {
            params.push("&self".to_string());
        } else if !m.is_static && !m.is_ctor {
            params.push(format!("this: {}", self.this_type()));
        }
        params.extend(self.typed_params(index, true));
        let ret = self.ret_text(index);
        if self.display {
            self.line("fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {");
        } else {
            let fn_name =
                if m.name == "<clinit>" { statics::init_name(name) } else { self.fn_names[index].clone() };
            self.line(&format!("{}fn {fn_name}({}){ret} {{", self.vis(), params.join(", ")));
        }
        self.indent += 1;
        if m.name == "<clinit>" {
            self.init_guard(name);
            // Java initializes the superclass first.
            let superclass = self.class.hierarchy.superclass(m.class);
            if let Some(s) = superclass.and_then(|s| self.facts.init_class(self.class, s)) {
                self.line(&format!("{}();", statics::init_name(&self.class.types[s].name)));
            }
        } else if self.facts.entry_init.contains(&index)
            && let Some(t) = self.facts.init_class(self.class, m.class)
        {
            self.line(&format!("{}();", statics::init_name(&self.class.types[t].name)));
        }
        if m.is_ctor {
            self.constructor(m);
        } else {
            self.stmts(&m.body, true);
        }
        self.indent -= 1;
        self.line("}");
        self.display = false;
        self.resolve_annotations(start);
    }

    /// Picks Rust names for the method's parameters and locals: snake case, distinct from each
    /// other and from the free functions, which share Rust's value namespace.
    fn name_locals(&mut self, m: &Method) {
        let mut java_names = Vec::new();
        java_names.extend(m.params.iter().map(|p| p.name.clone()));
        collect_locals(&m.body, &mut java_names);
        // Static methods are free functions, in the value namespace that locals share; instance
        // methods and constructors are reached through their type or object.
        let mut used: HashSet<String> = (0..self.fn_names.len())
            .filter(|&i| self.class.methods[i].is_static)
            .map(|i| self.fn_names[i].clone())
            .collect();
        used.insert("java".into());
        if self.display {
            used.insert("f".into());
        }
        self.locals.clear();
        for name in java_names {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.locals.entry(name) {
                let rust = unique(snake_case(entry.key()), &mut used);
                entry.insert(rust);
            }
        }
        self.boxed = boxed_locals(m);
    }
}

/// A borrow scoped with braces, `{ x.borrow().f }`, is a function argument of its own in a
/// guarded statement, where rustc's unused_braces lint would ask to drop braces that end the
/// borrow; there it is written `{ let value = x.borrow().f; value }`. The lint leaves the
/// arguments of a macro alone, and so does this. Text in string literals is left alone.
fn scoped_arguments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;
    // For each open parenthesis, whether it holds a macro's arguments.
    let mut parens: Vec<bool> = Vec::new();
    while i < bytes.len() {
        let c = bytes[i];
        if escaped {
            escaped = false;
        } else if in_string {
            escaped = c == b'\\';
            in_string = c != b'"';
        } else if c == b'"' {
            in_string = true;
        } else if c == b'(' {
            parens.push(text[..i].ends_with('!'));
        } else if c == b')' {
            parens.pop();
        } else if parens.last() == Some(&false)
            && (text[..i].ends_with('(') || text[..i].ends_with(", "))
            && text[i..].starts_with("{ ")
        {
            // A block with no braces or quotes inside, followed by the end of the argument.
            if let Some(len) = text[i + 2..].find(['{', '}', '"'])
                && text[i + 2 + len..].starts_with('}')
                && text[i + 2..i + 2 + len].ends_with(' ')
                && !text[i + 2..i + 2 + len].contains("let ")
                && matches!(bytes.get(i + 3 + len), Some(b')' | b','))
            {
                let inner = text[i + 2..i + 1 + len].trim_end();
                out.push_str(&format!("{{ let value = {inner}; value }}"));
                i += 3 + len;
                continue;
            }
        }
        let next = i + text[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&text[i..next]);
        i = next;
    }
    out
}

fn collect_locals(stmts: &[Stmt], out: &mut Vec<String>) {
    for stmt in stmts {
        visit_stmt(stmt, &mut |_| {}, &mut |s| {
            if let Stmt::Local { name, .. } | Stmt::ForEach { name, .. } = s {
                out.push(name.clone());
            }
        });
    }
    // The variables of patterns.
    for stmt in stmts {
        visit_stmt(
            stmt,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::InstanceOf { binding: Some(name), .. } = &x.kind {
                        out.push(name.clone());
                    }
                })
            },
            &mut |_| {},
        );
    }
}
