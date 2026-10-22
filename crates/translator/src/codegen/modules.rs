//! Packages as modules, and the name resolver. In a whole program whose classes are in packages,
//! every package becomes a Rust module nested like the package (`com.example.util` is the module
//! `com::example::util`); the program is one crate either way. The modules are made as a
//! `output::ModuleTree`, which a layout writes as one file (a translated file, the translation
//! tests) or as a file per module (a whole program's crate). Every item of a module is
//! `pub(crate)`, which allows every access Java allows. A program without packages is written
//! exactly like a file.
//!
//! Every name of a class, a static field or a static method that the generator writes goes
//! through the [`Resolver`] (`split`): a name in the module being written is its name there; a
//! name in another module is a path from the module being written, through the `use` line of its
//! module (`util::Counter`, with `use crate::com::example::util;`) where the module's own name is
//! unique in the program and clashes with nothing, and from the crate root
//! (`crate::com::example::util::Counter`) otherwise. Natural names and a split into crates change
//! the paths here.
//!
//! The functions that make names (`struct_name`, `cell_name`) are called from everywhere in the
//! generator, so the resolver is held in a thread-local for the duration of `generate` rather
//! than threaded through every call; passing it as a parameter to those functions instead is
//! mechanical.

use super::*;
use crate::output::{Item, Module, ModuleTree, Origin};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

thread_local! {
    static RESOLVER: RefCell<Option<Resolver>> = const { RefCell::new(None) };
}

/// Names of the program's classes, static fields and static methods, as written from the module
/// being written; and the modules that the module and the item being written refer to.
struct Resolver {
    /// The package of every class, by qualified name.
    packages: HashMap<String, String>,
    /// The path of each package's module from the crate root (`absolute`), such as
    /// `crate::com::example::util::`.
    paths: HashMap<String, String>,
    /// The packages whose modules are named by their own name through a `use` line, with that
    /// name: those whose module name no other module of the program has, and that names nothing
    /// else a module may have in scope.
    aliases: HashMap<String, String>,
    /// The package whose module is being written; `None` before any item is written, when names
    /// are made without a path (such as the type suffixes of overloaded methods).
    current: Option<String>,
    /// The packages that the module being written names through their aliases.
    imports: BTreeSet<String>,
    /// The packages of other modules that the item being written names ("" for the crate root).
    refers: BTreeSet<String>,
    /// The classes that the item being written names, by qualified name.
    names: BTreeSet<String>,
}

/// Names that a module of the program cannot be called by through a `use` line, as they mean
/// something else in a module: the names the generator's own `use` lines bring in, the crates,
/// and the primitive types, whose associated functions the generated code calls (`i32::from`).
const TAKEN: &str = "fmt java java_runtime std core alloc \
    bool char str i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64";

impl Resolver {
    fn new(class: &Class) -> Self {
        // A runtime value's class is the runtime's, whatever its name: the JDK's `Character`,
        // which is translated for its static members, is the box's class too.
        let packages: HashMap<String, String> = class
            .types
            .iter()
            .filter(|t| t.kind != TypeKind::Runtime)
            .map(|t| (t.name.clone(), t.package.clone()))
            .collect();
        let paths = packages.values().map(|p| (p.clone(), absolute(p))).collect();
        // Every module, by its name: the modules of all packages and of the packages around them.
        let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for package in all_packages(class) {
            let name = module_name(package.rsplit('.').next().unwrap_or_default());
            by_name.entry(name).or_default().insert(package);
        }
        // A class whose name is a module's (Java allows `class util`) would clash with its `use`.
        let types: HashSet<String> =
            packages.iter().map(|(name, package)| local_name(name, package).replace('.', "")).collect();
        let aliases = by_name
            .into_iter()
            .filter(|(name, packages)| {
                packages.len() == 1 && !TAKEN.split(' ').any(|taken| taken == name) && !types.contains(name)
            })
            .filter_map(|(name, packages)| Some((packages.into_iter().next()?, name)))
            .collect();
        Resolver {
            packages,
            paths,
            aliases,
            current: None,
            imports: BTreeSet::new(),
            refers: BTreeSet::new(),
            names: BTreeSet::new(),
        }
    }

    /// The path prefix that names a class of `package` from the module being written: empty in
    /// its own module, through its alias where it has one, else from the crate root.
    fn prefix(&mut self, package: &str) -> String {
        let Some(current) = &self.current else { return String::new() };
        if current == package {
            return String::new();
        }
        self.refers.insert(package.to_string());
        match self.aliases.get(package) {
            Some(alias) => {
                self.imports.insert(package.to_string());
                format!("{alias}::")
            }
            None => self.paths[package].clone(),
        }
    }

    /// The `use` lines of the module being written for the modules it names through their
    /// aliases. A module inside the one being written is in scope already.
    fn use_lines(&mut self) -> String {
        let current = self.current.clone().unwrap_or_default();
        let imports = std::mem::take(&mut self.imports);
        imports
            .iter()
            .filter(|package| parent(package) != current)
            .map(|package| format!("use {};\n", absolute(package).trim_end_matches("::")))
            .collect()
    }

    /// `path`, which names an item from the module being written, from the crate root.
    fn absolute_path(&self, path: &str) -> String {
        match path.split_once("::") {
            Some((alias, rest)) => match self.aliases.iter().find(|(_, a)| *a == alias) {
                Some((package, _)) => format!("{}{rest}", absolute(package)),
                None => path.to_string(),
            },
            None => path.to_string(),
        }
    }
}

/// The name of class `name` of `package` inside its package (`Outer.Inner` for a nested class).
fn local_name<'n>(name: &'n str, package: &str) -> &'n str {
    if package.is_empty() { name } else { &name[package.len() + 1..] }
}

/// The package that `package` is in ("" for a package at the root).
fn parent(package: &str) -> &str {
    package.rsplit_once('.').map_or("", |(parent, _)| parent)
}

/// The packages of the program's classes and the packages around them, without the unnamed one.
fn all_packages(class: &Class) -> BTreeSet<String> {
    let mut packages = BTreeSet::new();
    for t in &class.types {
        let mut prefix = String::new();
        for segment in t.package.split('.').filter(|s| !s.is_empty()) {
            prefix = if prefix.is_empty() { segment.to_string() } else { format!("{prefix}.{segment}") };
            packages.insert(prefix.clone());
        }
    }
    packages
}

/// Holds the resolver of a whole program while it is generated, and removes it when dropped.
pub(super) struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        RESOLVER.with_borrow_mut(|r| *r = None);
    }
}

/// Enters the resolver of a program whose classes are in packages; `None` for a program (or
/// file) whose classes are all in the unnamed package, which is generated as before.
pub(super) fn enter(class: &Class) -> Option<Scope> {
    if class.types.iter().all(|t| t.package.is_empty()) {
        return None;
    }
    let resolver = Resolver::new(class);
    RESOLVER.with_borrow_mut(|r| *r = Some(resolver));
    Some(Scope)
}

fn with<T>(f: impl FnOnce(&mut Resolver) -> T) -> Option<T> {
    RESOLVER.with_borrow_mut(|r| r.as_mut().map(f))
}

fn set_current(package: &str) {
    with(|r| {
        r.current = Some(package.to_string());
        r.imports.clear();
        r.refers.clear();
        r.names.clear();
    });
}

/// The packages of other modules and the classes that an item names.
type Named = (BTreeSet<String>, BTreeSet<String>);

/// What the item written since the last call names.
fn take_refers() -> Named {
    with(|r| (std::mem::take(&mut r.refers), std::mem::take(&mut r.names))).unwrap_or_default()
}

/// The path prefix that names a class from the module being written (empty inside its own
/// module), and its name inside its package (`Outer.Inner` for a nested class).
pub(super) fn split(name: &str) -> (String, &str) {
    // The standard exception classes are items of the crate's root, named by their simple names,
    // and so are the functional interfaces lowering provides, which have no package (the JDK's
    // own, translated from its sources, are in theirs).
    let provided = crate::lower::is_functional(name)
        && RESOLVER
            .with_borrow(|r| r.as_ref().is_none_or(|r| r.packages.get(name).is_some_and(String::is_empty)));
    if crate::lower::is_standard(name) || provided {
        let simple = crate::lower::standard_local_name(name);
        let nested = with(|r| match r.current.as_deref() {
            None => false,
            Some(current) if !current.is_empty() => {
                r.names.insert(name.to_string());
                r.refers.insert(String::new());
                true
            }
            Some(_) => {
                r.names.insert(name.to_string());
                false
            }
        });
        return (if nested == Some(true) { "crate::".to_string() } else { String::new() }, simple);
    }
    with(|r| {
        let package = r.packages.get(name)?.clone();
        if r.current.is_some() {
            r.names.insert(name.to_string());
        }
        Some((r.prefix(&package), local_name(name, &package)))
    })
    .flatten()
    .unwrap_or((String::new(), name))
}

/// Whether a class belongs to the module being written; always, outside a whole program.
fn here(decl: &TypeDecl) -> bool {
    RESOLVER
        .with_borrow(|r| r.as_ref().is_none_or(|r| r.current.as_deref().is_none_or(|p| p == decl.package)))
}

/// The absolute path prefix of a package's module, such as `crate::com::example::`.
fn absolute(package: &str) -> String {
    let mut out = String::from("crate::");
    for segment in package.split('.').filter(|s| !s.is_empty()) {
        out.push_str(&module_name(segment));
        out.push_str("::");
    }
    out
}

/// The module of a package segment. `java` is the runtime crate's name in the generated code
/// (`use java_runtime as java`), `std`, `core` and the path keywords have meanings of their
/// own, and `main` is the crate's `fn main`, which a `use` of javac's `com.sun.tools.javac.main`
/// would meet, so those take a `_`: the JDK's `java.util` is `java_::util`.
fn module_name(segment: &str) -> String {
    match snake_case(segment) {
        name if matches!(name.as_str(), "java" | "std" | "core" | "alloc" | "super" | "self" | "crate" | "main") => {
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
        let nested = RESOLVER
            .with_borrow(|r| r.as_ref().is_some_and(|r| r.current.as_deref().is_some_and(|p| !p.is_empty())));
        if nested { "pub(crate) " } else { "" }
    }

    /// The path of a static method, which is a function of its class's module.
    pub(super) fn static_fn(&self, index: usize) -> String {
        let owner = &self.class.types[self.class.methods[index].class].name;
        format!("{}{}", split(owner).0, self.fn_names[index])
    }

    /// The whole program as modules, one per package, with `fn main` at the crate root. `header`
    /// starts the crate root.
    pub(super) fn program(&mut self, live: &[bool], header: String) -> ModuleTree {
        let packages = all_packages(self.class);
        let mut tree = ModuleTree { header, modules: Vec::new(), footer: ALLOCATOR.to_string() };
        self.module("", Vec::new(), live, &packages, &mut tree.modules);
        tree
    }

    /// The module of a package, then the modules inside it, into `modules`.
    fn module(
        &mut self,
        package: &str,
        path: Vec<String>,
        live: &[bool],
        packages: &BTreeSet<String>,
        modules: &mut Vec<Module>,
    ) {
        let class = self.class;
        set_current(package);
        let outer = std::mem::take(&mut self.out);
        let outer_traits = std::mem::take(&mut self.traits);
        self.indent = 0;
        // Each piece the generator writes is an item: the text it added, and what it named.
        let mut items: Vec<(Origin, usize, Named)> = Vec::new();
        let mut item = |g: &mut Self, origin: Origin, start: usize| {
            let named = take_refers();
            if g.out.len() > start {
                items.push((origin, start, named));
            }
        };
        for (index, decl) in class.types.iter().enumerate() {
            if live[index] && decl.package == package {
                let start = self.out.len();
                self.type_items(index);
                item(self, Origin::Class(decl.name.clone()), start);
            }
        }
        for (index, decl) in class.types.iter().enumerate() {
            if self.wanted.contains(&Need::Enum(index)) && decl.package == package {
                let start = self.out.len();
                self.type_enum_items(index);
                item(self, Origin::ClassEnum(decl.name.clone()), start);
            }
        }
        for (index, decl) in class.types.iter().enumerate() {
            if decl.enum_constants.is_some() && decl.package == package {
                let start = self.out.len();
                self.enum_items(index);
                item(self, Origin::Enum(decl.name.clone()), start);
            }
        }
        let start = self.out.len();
        self.statics();
        item(self, Origin::Statics, start);
        let start = self.out.len();
        self.class_monitors();
        item(self, Origin::Monitors, start);
        for (index, m) in class.methods.iter().enumerate() {
            if self.emitted_free(index) && class.types[m.class].package == package {
                let start = self.out.len();
                self.gap();
                self.method(index);
                item(self, Origin::Function(class.types[m.class].name.clone()), start);
            }
        }
        let main = class.methods.iter().position(|m| m.is_main);
        if let Some(main) =
            main.filter(|&m| package.is_empty() && !class.types[class.methods[m].class].package.is_empty())
        {
            let start = self.out.len();
            self.gap();
            self.line("fn main() {");
            self.indent += 1;
            let call = format!("{}();", self.static_fn(main));
            self.line(&call);
            self.indent -= 1;
            self.line("}");
            item(self, Origin::Main, start);
        }
        let text = std::mem::replace(&mut self.out, outer);
        let traits = std::mem::replace(&mut self.traits, outer_traits);
        let ends = items.iter().skip(1).map(|(_, start, _)| *start).chain([text.len()]);
        let mut texts: Vec<String> =
            items.iter().zip(ends).map(|((_, start, _), end)| text[*start..end].to_string()).collect();
        let mut uses = use_lines(&text);
        // The modules it names through their own names, and the extension traits whose methods
        // it calls, from the crate root.
        let mut crate_uses = with(Resolver::use_lines).unwrap_or_default();
        for trait_path in &traits {
            let absolute = with(|r| r.absolute_path(trait_path)).unwrap_or_else(|| trait_path.clone());
            crate_uses.push_str(&format!("use {absolute};\n"));
        }
        // The crate root's items, in a package's module, by their names where nothing else has them.
        if !package.is_empty() {
            let taken: BTreeSet<String> = uses
                .iter()
                .chain([&crate_uses])
                .flat_map(|group| group.lines())
                .filter_map(|line| line.trim_end_matches(';').rsplit([':', ' ']).next())
                .map(str::to_string)
                .collect();
            let roots = packages.iter().filter(|p| !p.contains('.')).map(|p| module_name(p)).collect();
            crate_uses.push_str(&imports::import_root_items(&mut texts, &roots, &taken));
        }
        if !crate_uses.is_empty() {
            let mut lines: Vec<&str> = crate_uses.lines().collect();
            lines.sort_unstable();
            lines.dedup();
            uses.push(lines.iter().map(|line| format!("{line}\n")).collect());
        }
        let items = items
            .into_iter()
            .zip(texts)
            .map(|((origin, _, (refers, names)), text)| Item { origin, text, refers, names })
            .collect();
        modules.push(Module { package: package.to_string(), path: path.clone(), uses, items });
        let prefix = if package.is_empty() { String::new() } else { format!("{package}.") };
        let children = packages
            .iter()
            .filter(|p| p.strip_prefix(&prefix).is_some_and(|rest| !rest.is_empty() && !rest.contains('.')));
        for child in children {
            let mut child_path = path.clone();
            child_path.push(module_name(&child[prefix.len()..]));
            self.module(child, child_path, live, packages, modules);
        }
    }

    /// The classes of the module being written, for the items that are collected per module.
    pub(super) fn module_types(&self) -> impl Iterator<Item = &'a TypeDecl> + use<'a> {
        let class: &'a Class = self.class;
        class.types.iter().filter(|t| here(t))
    }

    /// `module_types`, by index.
    pub(super) fn module_type_indexes(&self) -> Vec<usize> {
        (0..self.class.types.len()).filter(|&t| here(&self.class.types[t])).collect()
    }
}
