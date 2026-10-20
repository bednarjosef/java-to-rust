//! Generates Rust source from the typed syntax tree. Every construct is translated so that it
//! behaves exactly like Java: `int` arithmetic wraps, division by zero throws, doubles print
//! like Java. Where the analyses prove a plainer form equivalent (an addition that cannot
//! overflow, a divisor that cannot be zero, a counting `for` loop, a string parameter that is
//! only read) that form is used instead.

use crate::analysis::Facts;
use crate::ast::*;
use std::collections::{HashMap, HashSet};

mod code;
mod expr;
mod names;
mod null;
mod primitives;
mod stmt;
mod strings;

use code::*;
use expr::Operand;
use names::*;
use null::is_null_test;
use primitives::*;

mod arrays;
mod modules;
mod natives;
mod objects;
mod owned;
mod sync;

use arrays::name_part;
use objects::*;

pub fn generate(class: &Class, facts: &Facts) -> String {
    let live = live_types(class);
    let modules = modules::enter(class);
    let mut g = Gen {
        class,
        facts,
        method: 0,
        borrowed: HashSet::new(),
        borrowed_optional: HashSet::new(),
        int_receiver: false,
        double_receiver: false,
        owned_self: false,
        guarded: false,
        annotations: Vec::new(),
        fn_names: method_names(class),
        locals: HashMap::new(),
        elements: HashMap::new(),
        out: String::new(),
        indent: 0,
        loops: Vec::new(),
        ret: Type::Void,
        uses_runtime: false,
    };
    if modules.is_some() {
        return g.program(&live);
    }
    for (index, _) in live.iter().enumerate().filter(|(_, live)| **live) {
        g.type_items(index);
    }
    g.statics();
    for i in 0..class.methods.len() {
        if class.methods[i].is_static {
            g.gap();
            g.method(i);
        }
    }
    let mut uses = Vec::new();
    // Owned objects are plain values and immutable ones are shared as `Rc` alone; statics and
    // mutable objects also need `RefCell`.
    let refcell = class.types.iter().zip(&live).any(|(t, &l)| {
        (l && !facts.immutable.contains(&t.name) && !facts.owned.contains(&t.name)) || !t.statics.is_empty()
    });
    let shared = class.types.iter().zip(&live).any(|(t, &l)| l && !facts.owned.contains(&t.name));
    match (shared, refcell) {
        (true, true) => uses.push("use std::cell::RefCell;\nuse std::rc::Rc;\n"),
        (true, false) => uses.push("use std::rc::Rc;\n"),
        (false, true) => uses.push("use std::cell::RefCell;\n"),
        (false, false) => {}
    }
    if g.uses_runtime {
        uses.push("use java_runtime as java;\n");
    }
    let mut header = format!("// Translated from the Java class `{}` by java-to-rust.\n\n", class.name);
    if !uses.is_empty() {
        header.push_str(&(uses.join("\n") + "\n"));
    }
    header + &g.out
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
    /// Whether the code being written is a method of an owned class, where `this` is `self`.
    owned_self: bool,
    /// Whether the statement being emitted also borrows an object mutably (see `borrows_mut`).
    guarded: bool,
    /// Placeholders for type annotations decided at the end of the method: (placeholder, type,
    /// annotation).
    annotations: Vec<(String, Type, String)>,
    fn_names: Vec<String>,
    /// Java local variable names to Rust names, for the current method.
    locals: HashMap<String, String>,
    /// Reads of elements that a for-each-style loop has bound to a variable, by expression.
    elements: HashMap<ExprId, String>,
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
        self.borrowed_optional = (0..m.params.len())
            .filter(|&i| self.facts.borrowed_optional_params.contains(&(index, i)))
            .map(|i| m.params[i].name.clone())
            .collect();
        let name = &self.class.types[m.class].name;
        self.owned_self = !m.is_static && !m.is_ctor && self.facts.owned.contains(name);
        let mut params = Vec::new();
        if self.owned_self {
            params.push(if self.facts.mutating.contains(&index) { "&mut self" } else { "&self" }.to_string());
        } else if !m.is_static && !m.is_ctor {
            params.push(format!("this: {}", self.this_type(name)));
        }
        let typed: Vec<String> = m
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mutable = if assigns_any(&m.body, &p.name) { "mut " } else { "" };
                let ty = if self.borrowed_optional.contains(&p.name) {
                    "Option<&str>".to_string()
                } else if self.optional_var(index, p.ty, &p.name) {
                    "Option<String>".to_string()
                } else if self.borrowed.contains(&p.name) {
                    "&str".to_string()
                } else if let Some(&written) = self.facts.borrowed_arrays.get(&(index, i)) {
                    self.borrowed_ty(p.ty, written)
                } else {
                    self.ty_name(p.ty)
                };
                format!("{mutable}{}: {ty}", self.local(&p.name))
            })
            .collect();
        params.extend(typed);
        let ret = if m.is_ctor {
            format!(" -> {}", self.object_type(name))
        } else if m.ret == Type::Void {
            String::new()
        } else if self.optional_return() {
            " -> Option<String>".to_string()
        } else {
            format!(" -> {}", self.ty_name(m.ret))
        };
        self.line(&format!("{}fn {}({}){ret} {{", self.vis(), self.fn_names[index], params.join(", ")));
        self.indent += 1;
        if m.is_ctor {
            self.constructor(m);
        } else {
            self.stmts(&m.body, true);
        }
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
