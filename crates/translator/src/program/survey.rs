//! The survey of a program: what the code reachable from `main` reaches beyond the translated
//! sources (classes that are not translated, such as reflection and serialization, and classes
//! outside the sources), which methods of the runtime's classes it calls, and which natives it
//! reaches; `--survey` prints it, and `--survey-json` writes it as JSON.
//!
//! The survey is syntactic, on javac's trees: a piece of code (a method, a field initializer or an
//! initializer block) counts a feature when it, or the declaration of its class, uses it. It does
//! not run the translator, so it does not say which language constructs the translator rejects:
//! the census does (`census.rs`, `--census`), past every rejection. Code is either the program's
//! own (its files and source roots) or library code (the files of its library roots), and each
//! table is printed for both. Each feature names up to `EXAMPLES` places where reachable code uses
//! it.
//!
//! The scan also builds a call graph of the reachable code (methods, and each class's static and
//! instance initialization), so that each native method, and each method outside the sources that
//! is called, comes with the shortest chain of calls from `main` that reaches it: one example of
//! why it is reached. Local and anonymous classes belong to the code that declares them, as in
//! reachability.

use super::Program;
use super::reach::{
    CLASS_KINDS, MethodKey, OBJECT_METHODS, Reach, class_name, component_array, created_by_name, has_flag,
    instanceof_guard, literal_array, live_branch, members, outside, provided, replaced, runtime_arrays_call,
    runtime_stream_call, strip_arguments, symbol_type, text, untranslated,
};
use crate::ast::{Pos, runtime_native};
use crate::value::{Value, json};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Up to this many example places per feature.
const EXAMPLES: usize = 3;

/// The survey: the text report, and the same census as JSON.
pub struct Survey {
    pub report: String,
    pub json: serde_json::Value,
}

/// Whether code is the program's own or comes from a library root.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Part {
    Program,
    Library,
}

impl Part {
    pub(super) const ALL: [Part; 2] = [Part::Program, Part::Library];

    pub(super) fn name(self) -> &'static str {
        match self {
            Part::Program => "program",
            Part::Library => "library",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Part::Program => "Program",
            Part::Library => "Library",
        }
    }
}

/// Where the code of a compilation unit comes from: the program or a library, and the name of its
/// file relative to the root it came from.
struct Origin {
    part: Part,
    file: String,
}

/// Tells the program's files and roots from the libraries' when it is given units by file name.
struct Layout {
    /// The files of the program, canonical.
    files: HashSet<PathBuf>,
    /// The source and library roots, canonical.
    roots: Vec<PathBuf>,
}

impl Layout {
    fn new(program: &Program, files: &[PathBuf]) -> Self {
        Layout {
            files: files.iter().map(|f| canonical(f)).collect(),
            roots: program.sources.iter().chain(&program.libraries).map(|r| canonical(r)).collect(),
        }
    }

    /// A unit's origin from its file name, as javac reports it. A file is the program's when it was
    /// given or lies under a source root; any other file javac read is library code. Its name is
    /// relative to the deepest root that holds it, so the shortest one.
    fn origin(&self, file: &str) -> Origin {
        let path = canonical(Path::new(file));
        let part = if self.files.contains(&path) { Part::Program } else { Part::Library };
        let relative = self
            .roots
            .iter()
            .filter_map(|root| path.strip_prefix(root).ok())
            .min_by_key(|rest| rest.components().count())
            .map_or_else(|| file.to_string(), |rest| rest.display().to_string());
        Origin { part, file: relative }
    }
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The survey of the code reachable from `main`. `units` are javac's units, each with its `file`
/// and its `tree`; `files` are the program's own files.
pub fn survey(units: &[&Value], reach: &Reach, program: &Program, files: &[PathBuf]) -> Survey {
    let census = scan(units, reach, program, files);
    Survey { report: census.report(reach, &program.main), json: (&census.json(reach, &program.main)).into() }
}

/// The name of each unit's file relative to the root it came from, as the survey prints it.
pub(super) fn unit_files(units: &[&Value], program: &Program, files: &[PathBuf]) -> Vec<String> {
    let layout = Layout::new(program, files);
    units.iter().map(|u| layout.origin(u["file"].as_str().unwrap_or("")).file).collect()
}

/// The scan of the code reachable from `main` that the survey reports: its pieces, the runtime's
/// methods it calls and the natives it reaches.
pub(super) fn scan(units: &[&Value], reach: &Reach, program: &Program, files: &[PathBuf]) -> Census {
    let layout = Layout::new(program, files);
    let origins: Vec<Origin> =
        units.iter().map(|u| layout.origin(u["file"].as_str().unwrap_or(""))).collect();
    let mut census = Census::default();
    for unit in units {
        census.declare(&unit["tree"]);
    }
    for (index, (unit, origin)) in units.iter().zip(&origins).enumerate() {
        for decl in unit["tree"]["typeDecls"].as_array().into_iter().flatten() {
            census.class(decl, (index, origin), reach);
        }
    }
    census.chains(&program.main);
    census
}

/// A reachable piece of code and the unsupported features it uses.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Piece {
    pub(super) part: Part,
    pub(super) class: String,
    /// `method`, `constructor`, `field initializer`, `static block` or `instance block`.
    #[serde(deserialize_with = "crate::ast::static_str")]
    pub(super) kind: crate::ast::StaticStr,
    /// The name of a method or field; empty for a block.
    pub(super) name: String,
    /// `file:line` of the piece.
    pub(super) location: String,
    /// The index of its unit, and the first and the last position of its nodes: every node of
    /// its code, local and anonymous classes and lambdas included, is between them.
    pub(super) unit: usize,
    pub(super) span: (Pos, Pos),
    /// The binary name of the missing class that the piece's code throws `NoClassDefFoundError`
    /// for, as Java would, where it uses one (javac's `missingClass`, with `--keep-going`).
    pub(super) missing_class: Option<String>,
    /// Each feature the piece uses, with the place where it first does.
    #[serde(deserialize_with = "crate::ast::static_keys")]
    features: BTreeMap<crate::ast::StaticStr, String>,
}

/// What the scan of a piece of code finds: its unsupported features, each with its first place,
/// the methods of the runtime it calls, and the nodes of the call graph it refers to.
struct Scan<'a> {
    file: &'a str,
    features: BTreeMap<&'static str, String>,
    calls: BTreeSet<String>,
    refs: BTreeSet<String>,
    /// The calls that the runtime makes itself on `System.out` and `System.err`
    /// (`runtime_stream_call`), which reachability does not follow.
    stream_calls: BTreeSet<MethodKey>,
    /// Whether the program's streams stay sequential (`Reach::sequential`).
    sequential: bool,
    /// The classes without objects that `instanceof`s test for (`Reach::without_objects`).
    without_objects: &'a HashSet<String>,
}

impl Scan<'_> {
    fn feature(&mut self, feature: &'static str, node: &Value) {
        let place = place(self.file, node);
        self.features.entry(feature).or_insert(place);
    }
}

/// `file:line` of a node.
fn place(file: &str, node: &Value) -> String {
    format!("{file}:{}", node["line"].as_u64().unwrap_or(0))
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(super) struct Census {
    pub(super) pieces: Vec<Piece>,
    /// The classes of the sources and libraries, local and anonymous ones included, to tell calls
    /// that leave them.
    declared: HashSet<String>,
    /// Methods of the runtime's classes that reachable code calls, by part, with the number of
    /// pieces of code that call each.
    runtime: BTreeMap<(Part, String), usize>,
    /// The calls that the runtime makes on `System.out` and `System.err`: methods of the
    /// runtime, as those of `Reach::external` are.
    stream_calls: BTreeSet<MethodKey>,
    /// Reachable methods that have no body in Java: the JVM implements them. With each, its
    /// node in the call graph and whether the runtime implements it (`ast::runtime_native`).
    pub(super) natives: Vec<(Part, String, String, bool)>,
    /// The call graph of the reachable code: from each node (a method, or a class's static or
    /// instance initialization, see `method_node`) to the nodes its code may run.
    edges: HashMap<String, BTreeSet<String>>,
    /// For each node that `main` reaches through the graph, the node it is first reached from,
    /// by breadth: the shortest chains of calls.
    parent: HashMap<String, String>,
}

impl Census {
    /// Declares every class in a tree. Local and anonymous classes are declared inside method
    /// bodies, not among the members, so the whole tree is searched.
    fn declare(&mut self, node: &Value) {
        match node {
            Value::Object(fields) => {
                if CLASS_KINDS.contains(&text(node, "kind")) {
                    let name = class_name(node);
                    // An anonymous class has no name: javac calls it `<anonymous Sink>`.
                    self.declared.insert(if name.is_empty() { symbol_type(node).to_string() } else { name });
                }
                for (field, child) in fields {
                    if field != "symbol" {
                        self.declare(child);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.declare(item)),
            _ => {}
        }
    }

    fn class(&mut self, decl: &Value, (unit, origin): (usize, &Origin), reach: &Reach) {
        if !CLASS_KINDS.contains(&text(decl, "kind")) {
            return;
        }
        let class = class_name(decl);
        if !reach.classes.contains(&class) {
            return;
        }
        let header: BTreeMap<&'static str, String> =
            class_features(self, decl).into_iter().map(|f| (f, place(&origin.file, decl))).collect();
        for member in members(decl) {
            let kind = text(member, "kind");
            let reached = match kind {
                "METHOD" => {
                    let key =
                        (class.clone(), text(member, "name").to_string(), symbol_type(member).to_string());
                    if member["body"].is_null() && has_flag(member, "NATIVE") && reach.methods.contains(&key)
                    {
                        self.natives.push((
                            origin.part,
                            format!("{class}.{}{}", key.1, key.2),
                            method_node(&key),
                            runtime_native(&class, &key.1, &key.2),
                        ));
                    }
                    reach.methods.contains(&key) && !reach.dormant.contains(&key)
                }
                "VARIABLE" => {
                    !member["initializer"].is_null()
                        && reach.fields.contains(&(class.clone(), text(member, "name").to_string()))
                }
                "BLOCK" if member["static"].as_bool() == Some(true) => reach.initialized.contains(&class),
                "BLOCK" => reach.instantiated.contains(&class),
                _ => {
                    self.class(member, (unit, origin), reach);
                    continue;
                }
            };
            if !reached {
                continue;
            }
            let mut scan = Scan {
                file: &origin.file,
                features: header.clone(),
                calls: BTreeSet::new(),
                refs: BTreeSet::new(),
                stream_calls: BTreeSet::new(),
                sequential: reach.sequential,
                without_objects: &reach.without_objects,
            };
            self.body(member, &mut scan);
            self.stream_calls.append(&mut scan.stream_calls);
            for call in scan.calls {
                *self.runtime.entry((origin.part, call)).or_default() += 1;
            }
            let node = self.graph_node(&class, member);
            self.edges.entry(node).or_default().extend(scan.refs);
            self.pieces.push(Piece {
                part: origin.part,
                class: class.clone(),
                kind: piece_kind(member),
                name: if kind == "BLOCK" { String::new() } else { text(member, "name").to_string() },
                location: place(&origin.file, member),
                unit,
                span: span(member),
                missing_class: member["missingClass"].as_str().map(str::to_string),
                features: scan.features,
            });
        }
    }

    /// The unsupported features that a piece of code uses, and the methods of the runtime it calls.
    fn body(&self, node: &Value, scan: &mut Scan<'_>) {
        match node {
            // Reachability skips the branch, and so does the translation.
            Value::Object(_) if let Some(branch) = live_branch(node, scan.sequential) => {
                self.body(&node["condition"], scan);
                self.body(branch, scan);
            }
            // So is the `then` branch of an `if` that tests for a class without objects.
            Value::Object(_)
                if instanceof_guard(node).is_some_and(|class| scan.without_objects.contains(&class)) =>
            {
                self.body(&node["condition"], scan);
                self.body(&node["elseStatement"], scan);
            }
            // An object created from a class name translates (`reach::by_name`): only the name is
            // code.
            Value::Object(_) if let Some(name) = created_by_name(node) => self.body(name, scan),
            // A new array of a class literal's class cast to its type is a new array of it.
            Value::Object(_) if let Some(lengths) = literal_array(node) => {
                scan.calls.insert("Array.newInstance".to_string());
                lengths.into_iter().for_each(|length| self.body(length, scan));
            }
            // A new array of another array's class is a new array of its type.
            Value::Object(_) if let Some((array, length)) = component_array(node) => {
                scan.calls.insert("Array.newInstance".to_string());
                self.body(array, scan);
                self.body(length, scan);
            }
            Value::Object(fields) => {
                let kind = text(node, "kind");
                let ty = text(node, "javaType");
                match kind {
                    // A method reference whose receiver is computed.
                    "MEMBER_REFERENCE" if crate::lower::computed_receiver(node, false).is_some() => {
                        scan.feature("computed method receivers", node)
                    }
                    // Tests and casts against the program's classes, the runtime's and the
                    // translator's translate; against another class, it is not translated.
                    "INSTANCE_OF" if self.outside(text(&node["type"], "javaType")) => {
                        scan.feature("instanceof on types outside the sources", node)
                    }
                    "TYPE_CAST" if self.outside(ty) => {
                        scan.feature("casts to types outside the sources", node)
                    }
                    // A for-each over one of the program's `Iterable`s calls its iterator.
                    "ENHANCED_FOR_LOOP" if self.outside(text(&node["expression"], "javaType")) => {
                        scan.feature("for-each over types outside the sources", node)
                    }
                    _ => {}
                }
                // The program's generic types are erased; one from elsewhere is not translated.
                // javac names an anonymous class `<anonymous Sink>`: that is not a generic type,
                // and nor is the method type of a method's name (`(java.lang.Class<?>)int`, or
                // `<T>(T)T` for a generic method), whose call has a type of its own.
                if ty.contains('<') && !ty.starts_with(['<', '(']) && !self.erases(ty) {
                    scan.feature("generic types outside the sources", node);
                }
                refer(&node["symbol"], scan);
                if matches!(kind, "METHOD_INVOCATION" | "NEW_CLASS") {
                    let symbol = &node["symbol"];
                    let owner = text(symbol, "owner");
                    let name = text(symbol, "name");
                    // `super()` into `Object` does nothing, so it is not a call.
                    let object_init = owner == "java.lang.Object" && name == "<init>";
                    if object_init {
                        // Nothing to report.
                    } else if owner == "java.lang.Object" && matches!(name, "wait" | "notify" | "notifyAll") {
                        scan.feature("monitor methods of Object", node);
                    } else if owner == "java.lang.Object" {
                        // `equals`, `hashCode`, `toString`, `getClass` and `clone` translate (the
                        // census says where `clone` does not).
                    } else if crate::program::caller_query(owner, name, text(symbol, "type")).is_some() {
                        let simple = owner.rsplit('.').next().unwrap_or(owner);
                        scan.calls.insert(format!("{simple}.{name}"));
                    } else if crate::program::redirected_call(owner, name, text(symbol, "type")).is_some() {
                        // The translator's method runs instead (`REDIRECTED_CALLS`).
                        let simple = owner.rsplit('.').next().unwrap_or(owner);
                        scan.calls.insert(format!("{simple}.{name}"));
                    } else if let Some(feature) = untranslated(owner) {
                        scan.feature(feature, node);
                        scan.calls.insert(format!("({feature}) {owner}.{name}"));
                    } else if runtime_stream_call(node) {
                        let simple = owner.rsplit('.').next().unwrap_or(owner);
                        scan.calls.insert(format!("{simple}.{name}"));
                        let key =
                            (strip_arguments(owner), name.to_string(), text(symbol, "type").to_string());
                        scan.stream_calls.insert(key);
                    } else if provided(owner)
                        || replaced(owner, name, text(symbol, "type"))
                        || runtime_arrays_call(node)
                    {
                        let simple = owner.rsplit('.').next().unwrap_or(owner);
                        scan.calls.insert(format!("{simple}.{name}"));
                    } else if !owner.is_empty() && !self.known(owner) {
                        scan.feature("classes outside the sources", node);
                        scan.calls.insert(format!("(outside) {owner}.{name}"));
                    }
                }
                for (field, child) in fields {
                    if field != "symbol" {
                        self.body(child, scan);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.body(item, scan)),
            _ => {}
        }
    }

    /// Whether a class is translated with the program: one of the sources' classes, or an
    /// interface or class that lowering provides (`Runnable`, the standard exceptions).
    fn known(&self, class: &str) -> bool {
        self.declared.contains(class)
            || crate::lower::is_functional(class)
            || crate::lower::is_standard(class)
    }

    /// Whether a type, as a test, a cast or a for-each meets it, is a class that is not
    /// translated: not a primitive, an array of them, `Object`, a type variable (whose erasure the
    /// code uses), one of the runtime's classes or one that the translation has.
    fn outside(&self, ty: &str) -> bool {
        let class = raw(ty);
        let class = class.trim_end_matches("[]");
        let variable = !class.contains('.') && !self.declared.contains(class);
        !(is_primitive(class) || variable || class == "java.lang.Object" || self.erases(class))
            && !self.known(class)
    }

    /// Whether a parameterized type translates as far as its type arguments go: one of the
    /// program's generic types, erased, `Comparable`, or a class of the runtime, whose type
    /// arguments are erased too (`Class<?>` translates as far as `Class` does).
    fn erases(&self, ty: &str) -> bool {
        let raw = raw(ty);
        let raw = raw.trim_end_matches("[]");
        self.known(raw) || raw == "java.lang.Comparable" || provided(raw)
    }

    /// The node of the call graph that a reachable member's code belongs to, with the edges that
    /// its declaration adds: a call of the method it overrides may run it, and a constructor runs
    /// its class's initializers and makes the methods of `Object` it overrides callable.
    fn graph_node(&mut self, class: &str, member: &Value) -> String {
        let is_static = member["static"].as_bool() == Some(true) || has_flag(member, "STATIC");
        if text(member, "kind") != "METHOD" {
            return if is_static { static_node(class) } else { instance_node(class) };
        }
        let key: MethodKey = (class.into(), text(member, "name").into(), symbol_type(member).into());
        let node = method_node(&key);
        for overridden in member["symbol"]["overrides"].as_array().into_iter().flatten() {
            let overridden =
                (text(overridden, "owner").to_string(), key.1.clone(), text(overridden, "type").to_string());
            self.edges.entry(method_node(&overridden)).or_default().insert(node.clone());
        }
        if key.1 == "<init>" {
            let runs = self.edges.entry(node.clone()).or_default();
            runs.insert(instance_node(class));
            runs.insert(static_node(class));
            for (name, signature) in OBJECT_METHODS {
                runs.insert(method_node(&(class.into(), name.to_string(), signature.to_string())));
            }
        }
        node
    }

    /// The shortest chain of calls from `main` to every node of the call graph it reaches.
    fn chains(&mut self, main: &str) {
        let entry = (main.to_string(), "main".to_string(), "(java.lang.String[])void".to_string());
        let mut work = VecDeque::new();
        for root in [static_node(main), method_node(&entry)] {
            self.parent.insert(root.clone(), String::new());
            work.push_back(root);
        }
        while let Some(node) = work.pop_front() {
            for next in self.edges.get(&node).into_iter().flatten() {
                if !self.parent.contains_key(next) {
                    self.parent.insert(next.clone(), node.clone());
                    work.push_back(next.clone());
                }
            }
        }
    }

    /// The chain of calls from `main` to a node, as the names of the methods and initializers on
    /// it; empty when the graph does not reach it.
    pub(super) fn chain(&self, node: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut at = node;
        while let Some(parent) = self.parent.get(at) {
            chain.push(label(at).to_string());
            at = parent;
        }
        chain.reverse();
        chain
    }

    /// The classes whose objects are created, each with the shortest chain of calls to one of its
    /// constructors: why its instance methods can run.
    fn instantiated(&self, reach: &Reach) -> Vec<Value> {
        let mut chains: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for key in reach.methods.iter().filter(|key| key.1 == "<init>") {
            let chain = self.chain(&method_node(key));
            let best = chains.entry(&key.0).or_default();
            if best.is_empty() || (!chain.is_empty() && chain.len() < best.len()) {
                *best = chain;
            }
        }
        chains.into_iter().map(|(class, chain)| json!({"class": class, "chain": chain})).collect()
    }

    /// The pieces of one part of the code.
    fn pieces(&self, part: Part) -> impl Iterator<Item = &Piece> {
        self.pieces.iter().filter(move |p| p.part == part)
    }

    fn report(&self, reach: &Reach, main: &str) -> String {
        let mut out = String::new();
        let total = self.pieces.len();
        let free = self.pieces.iter().filter(|p| p.features.is_empty()).count();
        _ = writeln!(
            out,
            "Reachable from {main}: {} classes ({} instantiated), {} methods, {} fields, {} native methods \
             ({} in the runtime).",
            reach.classes.len(),
            reach.instantiated.len(),
            reach.methods.len(),
            reach.fields.len(),
            self.natives.len(),
            self.natives.iter().filter(|n| n.3).count()
        );
        _ = writeln!(
            out,
            "This survey says what the code reaches outside the translated sources; `--census` runs the \
             translator over it and says what it rejects."
        );
        _ = writeln!(out, "{total} pieces of code (methods, field initializers, initializer blocks):");
        for part in Part::ALL {
            let pieces = self.pieces(part).count();
            let free = self.pieces(part).filter(|p| p.features.is_empty()).count();
            _ = writeln!(
                out,
                "  {}: {pieces} pieces; {free} ({}) reach nothing outside the sources.",
                part.title(),
                percent(free, pieces)
            );
        }
        _ = writeln!(out, "  Together: {free} ({}) reach nothing outside the sources.", percent(free, total));
        for part in Part::ALL {
            _ = writeln!(out, "\n{} code: features used by its pieces of code", part.title());
            self.feature_table(&mut out, part);
        }
        self.greedy_order(&mut out, free);
        _ = writeln!(
            out,
            "\nMethods of the runtime's classes, of classes not translated and of classes outside the sources, by pieces of code that call them:"
        );
        for part in Part::ALL {
            _ = writeln!(out, "{}:", part.title());
            let mut calls: Vec<(&String, &usize)> = self
                .runtime
                .iter()
                .filter(|((p, _), _)| *p == part)
                .map(|((_, name), n)| (name, n))
                .collect();
            calls.sort_by_key(|(name, n)| (Reverse(**n), *name));
            for (name, n) in calls {
                _ = writeln!(out, "  {n:>6}  {name}");
            }
        }
        _ = writeln!(out, "\nReachable classes, by pieces of code, with what they use:");
        for part in Part::ALL {
            _ = writeln!(out, "{}:", part.title());
            let mut classes: BTreeMap<&str, (usize, BTreeSet<&str>)> = BTreeMap::new();
            for piece in self.pieces(part) {
                let row = classes.entry(&piece.class).or_default();
                row.0 += 1;
                row.1.extend(piece.features.keys().copied());
            }
            let mut classes: Vec<_> = classes.into_iter().collect();
            classes.sort_by_key(|(name, (n, _))| (Reverse(*n), *name));
            for (name, (n, features)) in classes {
                let features: Vec<&str> = features.into_iter().collect();
                _ = writeln!(out, "  {n:>5}  {name}: {}", features.join(", "));
            }
        }
        _ = writeln!(out, "\nNative methods reached:");
        for part in Part::ALL {
            _ = writeln!(out, "{}:", part.title());
            let mut natives: Vec<(&str, &str, bool)> = self
                .natives
                .iter()
                .filter(|(p, ..)| *p == part)
                .map(|(_, name, node, runtime)| (name.as_str(), node.as_str(), *runtime))
                .collect();
            natives.sort();
            for (native, node, runtime) in natives {
                _ = writeln!(out, "  {native}{}", if runtime { "  (in the runtime)" } else { "" });
                let chain = self.chain(node);
                if !chain.is_empty() {
                    _ = writeln!(out, "      via {}", chain.join(" -> "));
                }
            }
        }
        out
    }

    /// The feature table of one part's code: per feature, the classes and pieces that use it, the
    /// pieces it alone blocks, and up to `EXAMPLES` places where it is used.
    fn feature_table(&self, out: &mut String, part: Part) {
        let mut table: BTreeMap<&str, Row> = BTreeMap::new();
        for piece in self.pieces(part) {
            for (&feature, place) in &piece.features {
                let row = table.entry(feature).or_default();
                row.classes.insert(&piece.class);
                row.code += 1;
                row.only += usize::from(piece.features.len() == 1);
                if row.examples.len() < EXAMPLES && !row.examples.contains(&place.as_str()) {
                    row.examples.push(place);
                }
            }
        }
        _ = writeln!(
            out,
            "{:<28} {:>8} {:>8} {:>13}  examples",
            "feature", "classes", "code", "only blocker"
        );
        let mut rows: Vec<_> = table.iter().collect();
        rows.sort_by_key(|(_, row)| Reverse(row.code));
        for (feature, row) in rows {
            _ = writeln!(
                out,
                "{feature:<28} {:>8} {:>8} {:>13}  {}",
                row.classes.len(),
                row.code,
                row.only,
                row.examples.join(", ")
            );
        }
    }

    /// The order in which supporting features frees the most code at each step, over all the
    /// code: each step frees the pieces that only the feature next in line blocks, or else the
    /// most used feature frees none alone.
    fn greedy_order(&self, out: &mut String, free: usize) {
        let mut used: BTreeMap<&str, usize> = BTreeMap::new();
        for piece in &self.pieces {
            for &feature in piece.features.keys() {
                *used.entry(feature).or_default() += 1;
            }
        }
        let total = self.pieces.len();
        _ = writeln!(out, "\nSupporting features in this order frees the most code at each step:");
        let mut supported: HashSet<&str> = HashSet::new();
        let mut freed = free;
        while supported.len() < used.len() {
            let mut gains: HashMap<&str, usize> = HashMap::new();
            for piece in &self.pieces {
                let missing: Vec<&str> =
                    piece.features.keys().copied().filter(|f| !supported.contains(f)).collect();
                if let [only] = missing[..] {
                    *gains.entry(only).or_default() += 1;
                }
            }
            let next = gains.iter().max_by_key(|&(f, &g)| (g, used[f], Reverse(*f))).map(|(f, _)| *f);
            let next = next.unwrap_or_else(|| {
                used.iter()
                    .filter(|(f, _)| !supported.contains(*f))
                    .max_by_key(|(_, n)| **n)
                    .map(|(f, _)| *f)
                    .unwrap()
            });
            freed += gains.get(next).copied().unwrap_or(0);
            supported.insert(next);
            _ = writeln!(
                out,
                "  {next:<28} +{:<6} {}",
                gains.get(next).copied().unwrap_or(0),
                percent(freed, total)
            );
        }
    }

    fn json(&self, reach: &Reach, main: &str) -> Value {
        // The methods of `Object` that a translated class does not override are `Object`'s.
        let external: BTreeSet<&MethodKey> = reach
            .external
            .iter()
            .filter(|key| outside(&key.0) || !self.declared.contains(&key.0))
            .chain(&self.stream_calls)
            .collect();
        json!({
            "main": main,
            "reachable": {
                "classes": reach.classes.len(),
                "instantiated": reach.instantiated.len(),
                "methods": reach.methods.len(),
                "fields": reach.fields.len(),
            },
            "pieces": self.pieces.iter().map(|p| json!({
                "source": p.part.name(),
                "class": p.class,
                "kind": p.kind,
                "name": p.name,
                "location": p.location,
                "features": p.features,
            })).collect::<Vec<_>>(),
            "runtime_calls": self.runtime.iter().map(|((part, method), n)| json!({
                "source": part.name(),
                "method": method,
                "pieces": n,
            })).collect::<Vec<_>>(),
            "natives": self.natives.iter().map(|(part, method, node, runtime)| json!({
                "source": part.name(),
                "method": method,
                "runtime": runtime,
                "chain": self.chain(node),
            })).collect::<Vec<_>>(),
            "external": external.iter().map(|key| json!({
                "method": format!("{}.{}{}", key.0, key.1, key.2),
                "chain": self.chain(&method_node(key)),
            })).collect::<Vec<_>>(),
            "instantiated": self.instantiated(reach),
        })
    }
}

/// One feature's row in a part's table.
#[derive(Default)]
struct Row<'a> {
    classes: BTreeSet<&'a str>,
    code: usize,
    only: usize,
    examples: Vec<&'a str>,
}

/// The node of the call graph for a method or constructor: its class and name, then its signature
/// after a tab, which no name holds.
fn method_node(key: &MethodKey) -> String {
    format!("{}.{}\t{}", key.0, key.1, key.2)
}

/// The node of a class's static initialization: its static initializer blocks and the initializers
/// of its static fields.
fn static_node(class: &str) -> String {
    format!("{class}.<clinit>\t")
}

/// The node of the initializers that every constructor of a class runs.
fn instance_node(class: &str) -> String {
    format!("{class}.<instance initializers>\t")
}

/// The name of a node of the call graph without the signature.
fn label(node: &str) -> &str {
    node.split('\t').next().unwrap_or(node)
}

/// Adds to the scan the nodes of the call graph that a symbol refers to: a method or constructor,
/// and the static initialization of its class when it is static or creates an object (or is a
/// static field).
fn refer(symbol: &Value, scan: &mut Scan<'_>) {
    let owner = text(symbol, "owner");
    let is_static = symbol["static"].as_bool() == Some(true);
    match text(symbol, "kind") {
        kind @ ("METHOD" | "CONSTRUCTOR") => {
            let key = (owner.to_string(), text(symbol, "name").to_string(), text(symbol, "type").to_string());
            scan.refs.insert(method_node(&key));
            if is_static || kind == "CONSTRUCTOR" {
                scan.refs.insert(static_node(owner));
            }
        }
        "FIELD" | "ENUM_CONSTANT" if is_static => _ = scan.refs.insert(static_node(owner)),
        _ => {}
    }
}

/// The first and the last position of the nodes of a tree.
fn span(node: &Value) -> (Pos, Pos) {
    fn visit(node: &Value, span: &mut (Pos, Pos)) {
        match node {
            Value::Object(fields) => {
                if let Some(line) = node["line"].as_u64() {
                    let pos = Pos::new(line as usize, node["column"].as_u64().unwrap_or(0) as usize);
                    let key = |p: Pos| (p.line, p.column);
                    if key(pos) < key(span.0) {
                        span.0 = pos;
                    }
                    if key(pos) > key(span.1) {
                        span.1 = pos;
                    }
                }
                for (field, child) in fields {
                    if field != "symbol" {
                        visit(child, span);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| visit(item, span)),
            _ => {}
        }
    }
    let mut span = (Pos::new(usize::MAX, usize::MAX), Pos::default());
    visit(node, &mut span);
    span
}

/// The kind of a piece of code, from the member that holds it.
fn piece_kind(member: &Value) -> &'static str {
    match text(member, "kind") {
        "METHOD" if text(member, "name") == "<init>" => "constructor",
        "METHOD" => "method",
        "VARIABLE" => "field initializer",
        _ if member["static"].as_bool() == Some(true) => "static block",
        _ => "instance block",
    }
}

/// What a class declaration reaches outside the sources, which every piece of its code shares.
fn class_features(census: &Census, decl: &Value) -> BTreeSet<&'static str> {
    let mut features = BTreeSet::new();
    let kind = text(decl, "kind");
    // The program's own interfaces translate, generic ones erased, and so do `Comparable` and the
    // interfaces that lowering provides; one from elsewhere is not translated.
    for clause in decl["implementsClause"].as_array().into_iter().flatten() {
        let interface = raw(text(clause, "javaType"));
        if !census.known(&interface) && interface != "java.lang.Comparable" {
            features.insert("implementing interfaces outside the sources");
        }
    }
    // Class trees of the program's own classes translate; a superclass from elsewhere does not.
    if kind == "CLASS" && !decl["extendsClause"].is_null() {
        let superclass = raw(text(&decl["extendsClause"], "javaType"));
        if !census.known(&superclass) {
            features.insert("extending classes outside the sources");
        }
    }
    features
}

/// A type without its type arguments, those of its outer types too: `Box<java.lang.Integer>[]`
/// is `Box[]`, and `Outer<T>.Inner<U>` is `Outer.Inner`.
fn raw(ty: &str) -> String {
    crate::lower::raw_type(ty)
}

fn is_primitive(ty: &str) -> bool {
    matches!(ty, "boolean" | "byte" | "short" | "char" | "int" | "long" | "float" | "double")
}

fn percent(part: usize, whole: usize) -> String {
    format!("{:.1}%", 100.0 * part as f64 / whole.max(1) as f64)
}
