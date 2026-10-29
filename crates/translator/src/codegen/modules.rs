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
    /// The names in their packages of the nested classes whose names there, without the dots,
    /// are another class's of the package too (`flat_name`).
    static CLASHING: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
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
            packages.iter().map(|(name, package)| flat_name(name, local_name(name, package))).collect();
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

    /// The `use` lines of the module of package `current` for the modules it names through
    /// their aliases, `imports`. A module inside it is in scope already.
    fn use_lines(&self, current: &str, imports: &BTreeSet<String>) -> String {
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

/// Holds the names of a whole program's classes and its resolver while it is generated, and removes
/// them when dropped.
pub(super) struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        RESOLVER.with_borrow_mut(|r| *r = None);
        CLASHING.with_borrow_mut(HashMap::clear);
    }
}

/// Enters the names of a program's classes (`flat_name`), and the resolver of a program whose
/// classes are in packages; a program (or file) whose classes are all in the unnamed package has
/// none, and is generated as before.
pub(super) fn enter(class: &Class) -> Scope {
    CLASHING.with_borrow_mut(|c| *c = clashing(class));
    if class.types.iter().any(|t| !t.package.is_empty()) {
        let resolver = Resolver::new(class);
        RESOLVER.with_borrow_mut(|r| *r = Some(resolver));
    }
    Scope
}

/// The classes of the program whose names would be another's without the dots, with the names
/// they get instead: of the classes of a package with the same name so, all but the one with the
/// fewest dots (and of those, the first by name) take the first number after it that no class of
/// the package has, as Guava's `StandardTable.Column.KeySet` (`StandardTableColumnKeySet2`)
/// beside `StandardTable.ColumnKeySet`.
fn clashing(class: &Class) -> HashMap<String, String> {
    let mut by_flat: BTreeMap<(&str, String), Vec<&str>> = BTreeMap::new();
    for t in class.types.iter().filter(|t| t.kind != TypeKind::Runtime) {
        let flat = local_name(&t.name, &t.package).replace('.', "");
        by_flat.entry((t.package.as_str(), flat)).or_default().push(&t.name);
    }
    let mut clashing = HashMap::new();
    let groups: Vec<_> = by_flat.iter().filter(|(_, names)| names.len() > 1).collect();
    for ((package, flat), names) in groups {
        let mut names = names.clone();
        names.sort_by_key(|name| (name.matches('.').count(), *name));
        let mut free =
            (2..).map(|n| format!("{flat}{n}")).filter(|f| !by_flat.contains_key(&(*package, f.clone())));
        for name in &names[1..] {
            clashing.insert(name.to_string(), free.next().expect("a free name"));
        }
    }
    clashing
}

/// The name in its package of class `name`, whose name there is `local`, as one Rust name:
/// `Outer.Inner` is `OuterInner`, or another name where that would be another class's
/// (`clashing`).
pub(super) fn flat_name(name: &str, local: &str) -> String {
    CLASHING.with_borrow(|c| c.get(name).cloned()).unwrap_or_else(|| local.replace('.', ""))
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

/// A piece of a module that the generator writes as one item (`Gen::piece`).
#[derive(Clone, Copy)]
enum Piece {
    /// A class's struct, its `impl`s and its methods.
    Class(usize),
    /// The enum that stands for a class type, with the items of it that the code uses.
    ClassEnum(usize),
    /// A Java `enum`'s Rust enum.
    Enum(usize),
    /// The statics of the module's classes.
    Statics,
    /// The monitors of the module's classes.
    Monitors,
    /// A static method, a function of its class's module.
    Function(usize),
    /// The crate's `fn main`, which calls the main method.
    Main(usize),
}

/// A module as the generator writes it, before its `use` lines are found (`Gen::finish`). The
/// first round's modules are kept so, for the last round to put the enum items of class types
/// among their items (`merge`).
pub(super) struct Written {
    package: String,
    path: Vec<String>,
    items: Vec<WrittenItem>,
    /// Where the enum items of class types are among the items: after the classes' own.
    enums_at: usize,
    /// Whether the last item, after the enum items, is written again to take the place of the
    /// first round's item there (`Gen::module`).
    replaces: bool,
    /// The packages that the items name through their aliases (`Resolver::imports`).
    imports: BTreeSet<String>,
    /// The extension traits whose methods the items call (`Gen::traits`).
    traits: BTreeSet<String>,
    /// What writing the module found that cannot be translated (`Gen::rejected`), in order.
    pub(super) rejected: Vec<Error>,
}

/// An item of a module as the generator writes it (`Written`).
struct WrittenItem {
    piece: Piece,
    origin: Origin,
    text: String,
    named: Named,
    /// The item's methods with a call that throws inside a closure (`Gen::closures`).
    closures: Vec<Error>,
}

/// The first round's modules `kept` with the last round's enum items of class types, `enums`
/// (`plans`), which are of the same modules in the same order.
pub(super) fn merge(kept: Vec<Written>, enums: Vec<Written>) -> Vec<Written> {
    kept.into_iter()
        .zip(enums)
        .map(|(mut kept, enums)| {
            let after = kept.items.split_off(kept.enums_at);
            let skip = usize::from(enums.replaces);
            kept.items.extend(enums.items);
            kept.items.extend(after.into_iter().skip(skip));
            kept.imports.extend(enums.imports);
            kept.traits.extend(enums.traits);
            kept.rejected.extend(enums.rejected);
            kept
        })
        .collect()
}

/// What an item's text ends with as far as the next item's `gap` sees it.
fn ending(text: &str) -> &'static str {
    if ends_statement(text) { ";\n" } else { "\n" }
}

/// What the item written since the last call names.
fn take_refers() -> Named {
    with(|r| (std::mem::take(&mut r.refers), std::mem::take(&mut r.names))).unwrap_or_default()
}

/// The path prefix that names a class from the module being written (empty inside its own
/// module), and its name inside its package (`Outer.Inner` for a nested class).
pub(super) fn split(name: &str) -> (String, &str) {
    if at_root(name) {
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

/// A module of the program as every round writes it (`Gen::module`): its package, its path from
/// the crate root, and the types and the methods of the package, in index order.
#[derive(PartialEq)]
pub(super) struct Plan {
    package: String,
    path: Vec<String>,
    types: Vec<usize>,
    methods: Vec<usize>,
}

impl Plan {
    /// The module's types, in index order.
    pub(super) fn types(&self) -> &[usize] {
        &self.types
    }

    /// The module's methods, in index order.
    pub(super) fn methods(&self) -> &[usize] {
        &self.methods
    }
}

/// The program's modules in the order they are written, one per package: the crate root first,
/// and each module before the modules inside it, which are in the order of their names.
pub(super) fn plans(class: &Class) -> Vec<Plan> {
    let packages = all_packages(class);
    let mut members: HashMap<&str, (Vec<usize>, Vec<usize>)> = HashMap::new();
    for (index, decl) in class.types.iter().enumerate() {
        members.entry(decl.package.as_str()).or_default().0.push(index);
    }
    for (index, m) in class.methods.iter().enumerate() {
        members.entry(class.types[m.class].package.as_str()).or_default().1.push(index);
    }
    fn add(
        package: &str,
        path: Vec<String>,
        packages: &BTreeSet<String>,
        members: &mut HashMap<&str, (Vec<usize>, Vec<usize>)>,
        plans: &mut Vec<Plan>,
    ) {
        let (types, methods) = members.remove(package).unwrap_or_default();
        plans.push(Plan { package: package.to_string(), path: path.clone(), types, methods });
        let prefix = if package.is_empty() { String::new() } else { format!("{package}.") };
        let children = packages
            .iter()
            .filter(|p| p.strip_prefix(&prefix).is_some_and(|rest| !rest.is_empty() && !rest.contains('.')));
        for child in children {
            let mut child_path = path.clone();
            child_path.push(module_name(&child[prefix.len()..]));
            add(child, child_path, packages, members, plans);
        }
    }
    let mut plans = Vec::new();
    add("", Vec::new(), &packages, &mut members, &mut plans);
    plans
}

/// Whether class `name` is an item of the crate's root named by its simple name: the standard
/// exception classes are, and so are the functional interfaces lowering provides, which have no
/// package (the JDK's own, translated from its sources, are in theirs).
fn at_root(name: &str) -> bool {
    let provided = crate::lower::is_functional(name)
        && RESOLVER
            .with_borrow(|r| r.as_ref().is_none_or(|r| r.packages.get(name).is_some_and(String::is_empty)));
    crate::lower::is_standard(name) || provided
}

/// The name of class `name` inside its package, as `split` gives it, without the item being
/// written naming the class: for names made once for the whole program, wherever they are first
/// asked for (`variant_names_of`).
pub(super) fn name_in_package(name: &str) -> &str {
    if at_root(name) {
        return crate::lower::standard_local_name(name);
    }
    let package = RESOLVER.with_borrow(|r| r.as_ref().and_then(|r| r.packages.get(name).map(String::len)));
    match package {
        Some(len) if len > 0 => &name[len + 1..],
        _ => name,
    }
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
        name if matches!(
            name.as_str(),
            "java" | "std" | "core" | "alloc" | "super" | "self" | "crate" | "main"
        ) =>
        {
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
    if bare(code, "LazyLock<") {
        std.push_str("use std::sync::LazyLock;\n");
    }
    let mut uses = Vec::new();
    if !std.is_empty() {
        uses.push(std);
    }
    let mut runtime = String::new();
    if code.contains("java::") {
        runtime.push_str("use java_runtime as java;\n");
    }
    // The handle of objects that threads share (`java::Arc`, see the runtime's `shared.rs`).
    if bare(code, "Arc<") || bare(code, "Arc::") {
        runtime.push_str("use java_runtime::Arc;\n");
    }
    if !runtime.is_empty() {
        uses.push(runtime);
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

    /// The program's modules as a tree, `header` starting the crate root, and the methods written
    /// with a call that throws inside a closure (`Gen::closures`), in the order of the items.
    pub(super) fn finish(&mut self, header: String, written: Vec<Written>) -> (ModuleTree, Vec<Error>) {
        let packages = all_packages(self.class);
        let mut tree = ModuleTree { header, modules: Vec::new(), footer: ALLOCATOR.to_string() };
        let mut closures = Vec::new();
        for module in written {
            let Written { package, path, items, imports, traits, .. } = module;
            let text: String = items.iter().map(|item| item.text.as_str()).collect();
            let mut uses = use_lines(&text);
            drop(text);
            // The modules it names through their own names, and the extension traits whose
            // methods it calls, from the crate root.
            let mut crate_uses = with(|r| r.use_lines(&package, &imports)).unwrap_or_default();
            for trait_path in &traits {
                let absolute = with(|r| r.absolute_path(trait_path)).unwrap_or_else(|| trait_path.clone());
                crate_uses.push_str(&format!("use {absolute};\n"));
            }
            let mut texts = Vec::with_capacity(items.len());
            let mut rest = Vec::with_capacity(items.len());
            for item in items {
                texts.push(item.text);
                closures.extend(item.closures);
                rest.push((item.origin, item.named));
            }
            // The crate root's items, in a package's module, by their names where nothing else
            // has them.
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
            let items = rest
                .into_iter()
                .zip(texts)
                .map(|((origin, (refers, names)), text)| Item { origin, text, refers, names })
                .collect();
            tree.modules.push(Module { package, path, uses, items });
        }
        // The classes as values, which every module may name, once each at the crate root.
        let (text, names) = self.class_value_constants();
        if !text.is_empty() {
            let root = &mut tree.modules[0];
            if !root.uses.iter().any(|group| group.contains("use java_runtime as java;")) {
                root.uses.push("use java_runtime as java;\n".to_string());
            }
            root.items.push(Item {
                origin: Origin::ClassValues,
                text: format!("\n{text}"),
                refers: BTreeSet::new(),
                names,
            });
        }
        (tree, closures)
    }

    /// Starts an item of a module, which numbers its own labels (`Gen::labels`): its text then
    /// depends on the items written before it only through the blank line that may start it
    /// (`gap`). Returns where its text starts.
    fn item_start(&mut self) -> usize {
        self.labels = 0;
        self.out.len()
    }

    /// Writes `piece` as the module's next item, and gives what it was made from.
    fn piece(&mut self, piece: Piece) -> Origin {
        let class = self.class;
        let name = |t: usize| class.types[t].name.clone();
        match piece {
            Piece::Class(t) => {
                self.type_items(t);
                Origin::Class(name(t))
            }
            Piece::ClassEnum(t) => {
                self.type_enum_items(t);
                Origin::ClassEnum(name(t))
            }
            Piece::Enum(t) => {
                self.enum_items(t);
                Origin::Enum(name(t))
            }
            Piece::Statics => {
                self.statics();
                Origin::Statics
            }
            Piece::Monitors => {
                self.class_monitors();
                Origin::Monitors
            }
            Piece::Function(index) => {
                self.gap();
                self.method(index);
                Origin::Function(name(class.methods[index].class))
            }
            Piece::Main(main) => {
                self.gap();
                self.line("fn main() {");
                self.indent += 1;
                let call = format!("{}();", self.static_fn(main));
                self.line(&call);
                self.indent -= 1;
                self.line("}");
                Origin::Main
            }
        }
    }

    /// The module of `plan`, before its `use` lines are found (`finish`). With `kept_module`,
    /// the first round's, only the enum items of class types, which go among its items
    /// (`merge`).
    pub(super) fn module(&mut self, plan: &Plan, live: &[bool], kept_module: Option<&Written>) -> Written {
        let class = self.class;
        let Plan { package, path, types, methods } = plan;
        set_current(package);
        let outer = std::mem::take(&mut self.out);
        let outer_traits = std::mem::take(&mut self.traits);
        self.indent = 0;
        let mut items: Vec<WrittenItem> = Vec::new();
        // Each piece the generator writes is an item: the text it added, and what it named.
        let write = |g: &mut Self, piece: Piece, items: &mut Vec<WrittenItem>| {
            let start = g.item_start();
            let origin = g.piece(piece);
            let named = take_refers();
            let closures = std::mem::take(&mut g.closures);
            if g.out.len() > start {
                let text = g.out.split_off(start);
                // What follows sees how this ends (`gap`), and nothing else of it.
                g.out.push_str(ending(&text));
                items.push(WrittenItem { piece, origin, text, named, closures });
            } else {
                g.closures = closures;
            }
        };
        if self.only_enums.is_none() {
            for &index in types.iter().filter(|&&index| live[index]) {
                write(self, Piece::Class(index), &mut items);
            }
        }
        let enums_at = items.len();
        // The first round's items before the enum items end as they did there.
        if let Some(before) = kept_module.and_then(|m| m.enums_at.checked_sub(1).map(|i| &m.items[i].text)) {
            self.out.push_str(ending(before));
        }
        for &index in types.iter() {
            if self.wanted.contains(&Need::Enum(index))
                && self.only_enums.as_ref().is_none_or(|only| only.types.contains(&index))
            {
                write(self, Piece::ClassEnum(index), &mut items);
            }
        }
        if self.only_enums.is_none() {
            for &index in types.iter().filter(|&&index| class.types[index].enum_constants.is_some()) {
                write(self, Piece::Enum(index), &mut items);
            }
            write(self, Piece::Statics, &mut items);
            write(self, Piece::Monitors, &mut items);
            for &index in methods.iter() {
                if self.emitted_free(index) {
                    write(self, Piece::Function(index), &mut items);
                }
            }
            let main = class.methods.iter().position(|m| m.is_main);
            if let Some(main) = main
                .filter(|&m| package.is_empty() && !class.types[class.methods[m].class].package.is_empty())
            {
                write(self, Piece::Main(main), &mut items);
            }
        }
        // The first round's item after the enum items starts with a blank line or not as what
        // is before it ends (`gap`): where the enum items end otherwise than what was before
        // them there, it is written again after them and takes its place (`merge`).
        let mut replaces = false;
        if let Some(kept_module) = kept_module
            && !items.is_empty()
            && let Some(next) = kept_module.items.get(kept_module.enums_at)
        {
            let before = kept_module.enums_at.checked_sub(1).map(|i| &kept_module.items[i].text);
            if before.is_some_and(|before| ends_statement(before)) != ends_statement(&self.out) {
                write(self, next.piece, &mut items);
                replaces = true;
            }
        }
        self.out = outer;
        let traits = std::mem::replace(&mut self.traits, outer_traits);
        let imports = with(|r| std::mem::take(&mut r.imports)).unwrap_or_default();
        Written {
            package: package.to_string(),
            path: path.clone(),
            items,
            enums_at,
            imports,
            traits,
            replaces,
            rejected: std::mem::take(&mut self.rejected),
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
