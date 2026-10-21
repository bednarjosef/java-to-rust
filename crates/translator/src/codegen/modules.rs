//! Packages as modules. In a whole program whose classes are in packages, every package becomes a
//! Rust module nested like the package (`com.example.util` is `mod com { mod example { mod util
//! { .. } } }`), all in one file: the program is one crate either way, a single file keeps the
//! output a single `main.rs` like a translated file, and splitting it into a file per module is
//! mechanical. A class in another module is named by its absolute path
//! (`crate::com::example::util::Strings`), and every item of a module is `pub(crate)`, which
//! allows every access Java allows. A program without packages is written exactly like a file.
//!
//! The names of classes, static fields and static methods depend on the module being written. The
//! functions that make them (`struct_name`, `cell_name`) are called from everywhere in the
//! generator, so the program's packages and the current module are held in a thread-local
//! context for the duration of `generate` rather than threaded through every call.

use super::*;
use std::cell::RefCell;
use std::collections::BTreeSet;

thread_local! {
    static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
}

struct Context {
    /// The package of every class, by qualified name.
    packages: HashMap<String, String>,
    /// The package whose module is being written; `None` before any item is written, when names
    /// are made without a path (such as the type suffixes of overloaded methods).
    current: Option<String>,
}

/// Holds the context of a whole program while it is generated, and removes it when dropped.
pub(super) struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        CONTEXT.with_borrow_mut(|c| *c = None);
    }
}

/// Enters the context of a program whose classes are in packages; `None` for a program (or file)
/// whose classes are all in the unnamed package, which is generated as before.
pub(super) fn enter(class: &Class) -> Option<Scope> {
    if class.types.iter().all(|t| t.package.is_empty()) {
        return None;
    }
    let packages = class.types.iter().map(|t| (t.name.clone(), t.package.clone())).collect();
    CONTEXT.with_borrow_mut(|c| *c = Some(Context { packages, current: None }));
    Some(Scope)
}

fn set_current(package: &str) {
    CONTEXT.with_borrow_mut(|c| {
        if let Some(c) = c {
            c.current = Some(package.to_string());
        }
    });
}

/// The path prefix that names a class from the module being written (empty inside its own
/// module), and its name inside its package (`Outer.Inner` for a nested class).
pub(super) fn split(name: &str) -> (String, &str) {
    // The standard exception classes are items of the crate's root, named by their simple names,
    // and so are the functional interfaces lowering provides, which have no package (the JDK's
    // own, translated from its sources, are in theirs).
    let provided = crate::lower::is_functional(name)
        && CONTEXT
            .with_borrow(|c| c.as_ref().is_none_or(|c| c.packages.get(name).is_some_and(String::is_empty)));
    if crate::lower::is_standard(name) || provided {
        let simple = crate::lower::standard_local_name(name);
        let root = CONTEXT.with_borrow(|c| {
            c.as_ref().and_then(|c| c.current.as_deref()).is_some_and(|current| !current.is_empty())
        });
        return (if root { "crate::".to_string() } else { String::new() }, simple);
    }
    CONTEXT.with_borrow(|c| {
        let Some((c, package)) = c.as_ref().and_then(|c| Some((c, c.packages.get(name)?))) else {
            return (String::new(), name);
        };
        let local = if package.is_empty() { name } else { &name[package.len() + 1..] };
        match &c.current {
            Some(current) if current != package => (path(package), local),
            _ => (String::new(), local),
        }
    })
}

/// Whether a class belongs to the module being written; always, outside a whole program.
fn here(decl: &TypeDecl) -> bool {
    CONTEXT.with_borrow(|c| c.as_ref().is_none_or(|c| c.current.as_deref().is_none_or(|p| p == decl.package)))
}

/// The absolute path prefix of a package's module, such as `crate::com::example::`.
fn path(package: &str) -> String {
    let mut out = String::from("crate::");
    for segment in package.split('.').filter(|s| !s.is_empty()) {
        out.push_str(&module_name(segment));
        out.push_str("::");
    }
    out
}

/// The module of a package segment. `java` is the runtime crate's name in the generated code
/// (`use java_runtime as java`), and `std`, `core` and the path keywords have meanings of their
/// own, so those take a `_`: the JDK's `java.util` is `java_::util`.
fn module_name(segment: &str) -> String {
    match snake_case(segment) {
        name if matches!(name.as_str(), "java" | "std" | "core" | "alloc" | "super" | "self" | "crate") => {
            format!("{name}_")
        }
        name => name,
    }
}

/// Whether `code` has `name` where it is not the end of a longer name or path, such as
/// `java::Ref<'_` for `Ref<'_`.
fn bare(code: &str, name: &str) -> bool {
    code.match_indices(name)
        .any(|(i, _)| !code[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == ':'))
}

/// The `use` lines that code of one module needs, found in its text.
pub(super) fn use_lines(code: &str) -> Vec<String> {
    let mut std = String::new();
    // `RefCell<` and a generic struct such as `BoxCell<` are not `Cell<`.
    if code
        .match_indices("Cell<")
        .any(|(i, _)| !code[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
    {
        std.push_str("use std::cell::Cell;\n");
    }
    if bare(code, "Ref<'_") {
        std.push_str("use std::cell::Ref;\n");
    }
    if code.contains("RefCell") {
        std.push_str("use std::cell::RefCell;\n");
    }
    if bare(code, "RefMut<'_") {
        std.push_str("use std::cell::RefMut;\n");
    }
    if code.contains("impl fmt::Display") {
        std.push_str("use std::fmt;\n");
    }
    if code.contains("Rc<") || code.contains("Rc::") {
        std.push_str("use std::rc::Rc;\n");
    }
    if bare(code, "Arc<") || bare(code, "Arc::") {
        std.push_str("use std::sync::Arc;\n");
    }
    if bare(code, "LazyLock<") {
        std.push_str("use std::sync::LazyLock;\n");
    }
    let mut uses = Vec::new();
    if !std.is_empty() {
        uses.push(std);
    }
    if code.contains("java::") {
        uses.push("use java_runtime as java;\n".to_string());
    }
    uses
}

impl<'a> Gen<'a> {
    /// The visibility of an item: `pub(crate)` inside a package's module, where code of other
    /// modules may use it; none at the crate root, whose items every module can see.
    pub(super) fn vis(&self) -> &'static str {
        let nested = CONTEXT
            .with_borrow(|c| c.as_ref().is_some_and(|c| c.current.as_deref().is_some_and(|p| !p.is_empty())));
        if nested { "pub(crate) " } else { "" }
    }

    /// The path of a static method, which is a function of its class's module.
    pub(super) fn static_fn(&self, index: usize) -> String {
        let owner = &self.class.types[self.class.methods[index].class].name;
        format!("{}{}", split(owner).0, self.fn_names[index])
    }

    /// The whole program as nested modules, one per package, with `fn main` at the crate root.
    pub(super) fn program(&mut self, live: &[bool]) -> String {
        let class = self.class;
        let mut packages = BTreeSet::new();
        for t in &class.types {
            let mut prefix = String::new();
            for segment in t.package.split('.').filter(|s| !s.is_empty()) {
                prefix = if prefix.is_empty() { segment.to_string() } else { format!("{prefix}.{segment}") };
                packages.insert(prefix.clone());
            }
        }
        let body = self.module("", live, &packages);
        format!("// Translated from the Java program `{}` by java-to-rust.\n\n{body}", class.name)
    }

    /// The items of a package's module, then its nested modules, after the `use` lines they need.
    fn module(&mut self, package: &str, live: &[bool], packages: &BTreeSet<String>) -> String {
        let class = self.class;
        let depth = if package.is_empty() { 0 } else { package.split('.').count() };
        set_current(package);
        let outer = std::mem::take(&mut self.out);
        let outer_traits = std::mem::take(&mut self.traits);
        self.indent = depth;
        for (index, decl) in class.types.iter().enumerate() {
            if live[index] && decl.package == package {
                self.type_items(index);
            }
        }
        for (index, decl) in class.types.iter().enumerate() {
            if self.wanted.contains(&Need::Enum(index)) && decl.package == package {
                self.type_enum_items(index);
            }
        }
        for (index, decl) in class.types.iter().enumerate() {
            if decl.enum_constants.is_some() && decl.package == package {
                self.enum_items(index);
            }
        }
        self.statics();
        self.class_monitors();
        for (index, m) in class.methods.iter().enumerate() {
            if self.emitted_free(index) && class.types[m.class].package == package {
                self.gap();
                self.method(index);
            }
        }
        let main = class.methods.iter().position(|m| m.is_main);
        if let Some(main) =
            main.filter(|&m| package.is_empty() && !class.types[class.methods[m].class].package.is_empty())
        {
            self.gap();
            self.line("fn main() {");
            self.indent += 1;
            let call = format!("{}();", self.static_fn(main));
            self.line(&call);
            self.indent -= 1;
            self.line("}");
        }
        let items = std::mem::replace(&mut self.out, outer);
        let traits = std::mem::replace(&mut self.traits, outer_traits);
        let pad = "    ".repeat(depth);
        let mut text = String::new();
        let mut uses = use_lines(&items);
        // The extension traits whose methods the module calls are in scope.
        if !traits.is_empty() {
            uses.push(traits.iter().map(|path| format!("use {path};\n")).collect());
        }
        for uses in uses {
            for line in uses.lines() {
                text.push_str(&format!("{pad}{line}\n"));
            }
            text.push('\n');
        }
        text.push_str(&items);
        let prefix = if package.is_empty() { String::new() } else { format!("{package}.") };
        let children = packages
            .iter()
            .filter(|p| p.strip_prefix(&prefix).is_some_and(|rest| !rest.is_empty() && !rest.contains('.')));
        for child in children {
            let inner = self.module(child, live, packages);
            if !text.is_empty() && !text.ends_with("\n\n") {
                text.push('\n');
            }
            let segment = &child[prefix.len()..];
            // A module at the root is visible everywhere; a nested one must be made so.
            let vis = if package.is_empty() { "" } else { "pub(crate) " };
            text.push_str(&format!("{pad}{vis}mod {} {{\n{inner}{pad}}}\n", module_name(segment)));
        }
        text
    }

    /// The classes of the module being written, for the items that are collected per module.
    pub(super) fn module_types(&self) -> impl Iterator<Item = &'a TypeDecl> + use<'a> {
        let class: &'a Class = self.class;
        class.types.iter().filter(|t| here(t))
    }
}
