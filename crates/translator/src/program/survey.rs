//! The survey of a program: which Java features the code reachable from `main` uses that the
//! translator does not support yet, and which methods of the runtime's classes it calls. It
//! measures what to support next, such as what stands between the translator and a library like
//! the JDK; `--survey` prints it, and `--survey-json` writes the census as JSON.
//!
//! The census is syntactic, on javac's trees: a piece of code (a method, a field initializer or an
//! initializer block) counts a feature when it, or the declaration of its class, uses it. It does
//! not run the lowering, so it misses the narrower rejections there. Code is either the program's
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
    CLASS_KINDS, MethodKey, OBJECT_METHODS, Reach, class_name, has_flag, members, never_true, outside,
    provided, symbol_type, text, untranslated,
};
use serde_json::{Value, json};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Up to this many example places per feature.
const EXAMPLES: usize = 3;

/// The survey: the text report, and the same census as JSON.
pub struct Survey {
    pub report: String,
    pub json: Value,
}

/// Whether code is the program's own or comes from a library root.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Part {
    Program,
    Library,
}

impl Part {
    const ALL: [Part; 2] = [Part::Program, Part::Library];

    fn name(self) -> &'static str {
        match self {
            Part::Program => "program",
            Part::Library => "library",
        }
    }

    fn title(self) -> &'static str {
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
    let layout = Layout::new(program, files);
    let origins: Vec<Origin> =
        units.iter().map(|u| layout.origin(u["file"].as_str().unwrap_or(""))).collect();
    let mut census = Census::default();
    for unit in units {
        census.declare(&unit["tree"]);
    }
    for (unit, origin) in units.iter().zip(&origins) {
        for decl in unit["tree"]["typeDecls"].as_array().into_iter().flatten() {
            census.class(decl, None, origin, reach);
        }
    }
    census.chains(&program.main);
    Survey { report: census.report(reach, &program.main), json: census.json(reach, &program.main) }
}

/// A reachable piece of code and the unsupported features it uses.
struct Piece {
    part: Part,
    class: String,
    /// `method`, `constructor`, `field initializer`, `static block` or `instance block`.
    kind: &'static str,
    /// The name of a method or field; empty for a block.
    name: String,
    /// `file:line` of the piece.
    location: String,
    /// Each feature the piece uses, with the place where it first does.
    features: BTreeMap<&'static str, String>,
}

/// What the scan of a piece of code finds: its unsupported features, each with its first place,
/// the methods of the runtime it calls, and the nodes of the call graph it refers to.
struct Scan<'a> {
    file: &'a str,
    features: BTreeMap<&'static str, String>,
    calls: BTreeSet<String>,
    refs: BTreeSet<String>,
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

#[derive(Default)]
struct Census {
    pieces: Vec<Piece>,
    /// The classes of the sources and libraries, local and anonymous ones included, to tell calls
    /// that leave them.
    declared: HashSet<String>,
    /// Methods of the runtime's classes that reachable code calls, by part, with the number of
    /// pieces of code that call each.
    runtime: BTreeMap<(Part, String), usize>,
    /// Reachable methods that have no body in Java: the JVM implements them. With each, its
    /// node in the call graph.
    natives: Vec<(Part, String, String)>,
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

    fn class(&mut self, decl: &Value, outer: Option<&Value>, origin: &Origin, reach: &Reach) {
        if !CLASS_KINDS.contains(&text(decl, "kind")) {
            return;
        }
        let class = class_name(decl);
        if !reach.classes.contains(&class) {
            return;
        }
        let header: BTreeMap<&'static str, String> = class_features(decl, outer, &self.declared)
            .into_iter()
            .map(|f| (f, place(&origin.file, decl)))
            .collect();
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
                        ));
                    }
                    reach.methods.contains(&key)
                }
                "VARIABLE" => {
                    !member["initializer"].is_null()
                        && reach.fields.contains(&(class.clone(), text(member, "name").to_string()))
                }
                "BLOCK" if member["static"].as_bool() == Some(true) => reach.initialized.contains(&class),
                "BLOCK" => reach.instantiated.contains(&class),
                _ => {
                    self.class(member, Some(decl), origin, reach);
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
            };
            if kind == "METHOD" && intersects(member) {
                scan.feature("intersection types", member);
            }
            if kind == "BLOCK" && member["static"].as_bool() != Some(true) {
                scan.feature("initializer blocks", member);
            }
            self.body(member, &mut scan);
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
                features: scan.features,
            });
        }
    }

    /// The unsupported features that a piece of code uses, and the methods of the runtime it calls.
    fn body(&self, node: &Value, scan: &mut Scan<'_>) {
        match node {
            // Reachability skips the branch, and so does the translation.
            Value::Object(_) if text(node, "kind") == "IF" && never_true(&node["condition"]) => {
                self.body(&node["condition"], scan);
                self.body(&node["elseStatement"], scan);
            }
            Value::Object(fields) => {
                let kind = text(node, "kind");
                let ty = text(node, "javaType");
                match kind {
                    "LAMBDA_EXPRESSION" => scan.feature("lambdas", node),
                    "MEMBER_REFERENCE" => scan.feature("method references", node),
                    "NEW_CLASS" if !node["classBody"].is_null() => scan.feature("anonymous classes", node),
                    "NEW_CLASS" if !node["enclosingExpression"].is_null() => {
                        scan.feature("inner classes", node)
                    }
                    // An anonymous class body is a class node too: it is counted above.
                    "CLASS" | "INTERFACE" | "ENUM" | "RECORD" if !symbol_type(node).starts_with('<') => {
                        scan.feature("local classes", node)
                    }
                    "TRY" if node["resources"].as_array().is_some_and(|r| !r.is_empty()) => {
                        scan.feature("try-with-resources", node)
                    }
                    // Tests and casts between the program's own classes translate, and from
                    // `Object` to them, to strings and to boxes; a pattern binding its variable is
                    // part of the test.
                    "INSTANCE_OF" => {
                        let value = text(&node["expression"], "javaType");
                        let target = text(&node["type"], "javaType");
                        if self.narrows(value, target) {
                            if text(&node["pattern"], "kind") == "RECORD_PATTERN" {
                                scan.feature("patterns", node);
                            }
                            self.body(&node["expression"], scan);
                            return;
                        }
                        scan.feature("instanceof on types outside the sources", node);
                    }
                    "TYPE_CAST"
                        if !is_primitive(ty)
                            && ty != "java.lang.Object"
                            && !self.narrows(text(&node["expression"], "javaType"), ty) =>
                    {
                        scan.feature("reference casts", node)
                    }
                    "ENHANCED_FOR_LOOP" if !text(&node["expression"], "javaType").ends_with(']') => {
                        scan.feature("for-each over collections", node)
                    }
                    "BINDING_PATTERN" | "RECORD_PATTERN" => scan.feature("patterns", node),
                    _ => {}
                }
                // The program's generic types are erased; one from elsewhere is not translated.
                // javac names an anonymous class `<anonymous Sink>`: that is not a generic type.
                if ty.contains('<') && !ty.starts_with('<') && !self.erases(ty) {
                    scan.feature("generic types", node);
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
                    } else if owner == "java.lang.Object"
                        && !matches!(name, "equals" | "hashCode" | "toString")
                    {
                        // `getClass`, `clone`, and the monitor's `wait` and `notify`.
                        scan.feature(
                            if name == "getClass" { "reflection" } else { "methods of Object" },
                            node,
                        );
                    } else if let Some(feature) = untranslated(owner) {
                        scan.feature(feature, node);
                        scan.calls.insert(format!("({feature}) {owner}.{name}"));
                    } else if provided(owner) {
                        let simple = owner.rsplit('.').next().unwrap_or(owner);
                        scan.calls.insert(format!("{simple}.{name}"));
                    } else if !owner.is_empty() && !self.declared.contains(owner) {
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

    /// Whether a test or a cast of a value of type `value` against type `target` translates:
    /// both are the program's own types, or the value is an `Object` and the target is one of
    /// them, a string or a box.
    fn narrows(&self, value: &str, target: &str) -> bool {
        let boxes = ["Boolean", "Byte", "Character", "Double", "Float", "Integer", "Long", "Short", "String"];
        let (value, target) = (raw(value), raw(target));
        let held = target.strip_prefix("java.lang.").is_some_and(|name| boxes.contains(&name));
        // A cast to a type variable (`(T) items[i]`) is a cast to its erasure.
        let variable = !target.contains('.') && !self.declared.contains(target.trim_end_matches("[]"));
        let declared = |t: &str| self.declared.contains(t.trim_end_matches("[]"));
        (declared(&value) || value.starts_with("java.lang.Object")) && (declared(&target) || variable)
            || value == "java.lang.Object" && held
    }

    /// Whether a parameterized type translates: one of the program's generic types, erased, or
    /// `Comparable`.
    fn erases(&self, ty: &str) -> bool {
        let raw = raw(ty);
        let raw = raw.trim_end_matches("[]");
        self.declared.contains(raw) || raw == "java.lang.Comparable"
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
    fn chain(&self, node: &str) -> Vec<String> {
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
            "Reachable from {main}: {} classes ({} instantiated), {} methods, {} fields, {} native methods.",
            reach.classes.len(),
            reach.instantiated.len(),
            reach.methods.len(),
            reach.fields.len(),
            self.natives.len()
        );
        _ = writeln!(out, "{total} pieces of code (methods, field initializers, initializer blocks):");
        for part in Part::ALL {
            let pieces = self.pieces(part).count();
            let free = self.pieces(part).filter(|p| p.features.is_empty()).count();
            _ = writeln!(
                out,
                "  {}: {pieces} pieces; {free} ({}) use no unsupported feature.",
                part.title(),
                percent(free, pieces)
            );
        }
        _ = writeln!(out, "  Together: {free} ({}) use no unsupported feature.", percent(free, total));
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
            let mut natives: Vec<(&str, &str)> = self
                .natives
                .iter()
                .filter(|(p, _, _)| *p == part)
                .map(|(_, name, node)| (name.as_str(), node.as_str()))
                .collect();
            natives.sort();
            for (native, node) in natives {
                _ = writeln!(out, "  {native}");
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
        let external: BTreeSet<&MethodKey> =
            reach.external.iter().filter(|key| outside(&key.0) || !self.declared.contains(&key.0)).collect();
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
            "natives": self.natives.iter().map(|(part, method, node)| json!({
                "source": part.name(),
                "method": method,
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

/// The unsupported features of a class declaration, which every piece of its code shares.
fn class_features(decl: &Value, outer: Option<&Value>, declared: &HashSet<String>) -> BTreeSet<&'static str> {
    let mut features = BTreeSet::new();
    let kind = text(decl, "kind");
    if kind == "ANNOTATION_TYPE" {
        features.insert("annotation types");
    }
    // The program's own interfaces translate, generic ones erased, and so does `Comparable`; one
    // from elsewhere is not translated.
    for clause in decl["implementsClause"].as_array().into_iter().flatten() {
        let interface = raw(text(clause, "javaType"));
        if !declared.contains(&interface) && interface != "java.lang.Comparable" {
            features.insert("implementing interfaces outside the sources");
        }
    }
    // Class trees of the program's own classes translate; a superclass from elsewhere does not.
    if kind == "CLASS" && !decl["extendsClause"].is_null() {
        let superclass = text(&decl["extendsClause"], "javaType");
        let superclass = superclass.split('<').next().unwrap_or(superclass);
        if !declared.contains(superclass) && !crate::lower::is_standard(superclass) {
            features.insert("extending classes outside the sources");
        }
    }
    if intersects(decl) {
        features.insert("intersection types");
    }
    let in_class = outer.is_some_and(|o| matches!(text(o, "kind"), "CLASS" | "ENUM" | "RECORD"));
    if kind == "CLASS" && in_class && !has_flag(decl, "STATIC") {
        features.insert("inner classes");
    }
    if kind == "ENUM" && members(decl).any(|m| !m["initializer"]["classBody"].is_null()) {
        features.insert("enum constants with bodies");
    }
    features
}

/// Whether a declaration has a type variable with more than one bound (`T extends A & B`).
fn intersects(decl: &Value) -> bool {
    decl["typeParameters"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|p| p["bounds"].as_array().is_some_and(|b| b.len() > 1))
}

/// A type without its type arguments: `Box<java.lang.Integer>[]` is `Box[]`.
fn raw(ty: &str) -> String {
    match (ty.find('<'), ty.rfind('>')) {
        (Some(open), Some(close)) if open > 0 => format!("{}{}", &ty[..open], &ty[close + 1..]),
        _ => ty.to_string(),
    }
}

fn is_primitive(ty: &str) -> bool {
    matches!(ty, "boolean" | "byte" | "short" | "char" | "int" | "long" | "float" | "double")
}

fn percent(part: usize, whole: usize) -> String {
    format!("{:.1}%", 100.0 * part as f64 / whole.max(1) as f64)
}
