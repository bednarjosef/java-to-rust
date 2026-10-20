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

use super::Program;
use super::reach::{
    CLASS_KINDS, RUNTIME_CLASSES, Reach, class_name, has_flag, members, symbol_type, text, untranslated,
};
use serde_json::{Value, json};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
/// and the methods of the runtime it calls.
struct Scan<'a> {
    file: &'a str,
    features: BTreeMap<&'static str, String>,
    calls: BTreeSet<String>,
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
    /// Reachable methods that have no body in Java: the JVM implements them.
    natives: Vec<(Part, String)>,
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
                        self.natives.push((origin.part, format!("{class}.{}{}", key.1, key.2)));
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
            let mut scan = Scan { file: &origin.file, features: header.clone(), calls: BTreeSet::new() };
            if kind == "METHOD" && !member["typeParameters"].as_array().is_none_or(Vec::is_empty) {
                scan.feature("generic methods", member);
            }
            if kind == "BLOCK" && member["static"].as_bool() != Some(true) {
                scan.feature("initializer blocks", member);
            }
            self.body(member, &mut scan);
            for call in scan.calls {
                *self.runtime.entry((origin.part, call)).or_default() += 1;
            }
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
                    "TRY" | "THROW" => scan.feature("exceptions", node),
                    // Tests and casts between the program's own classes translate; a pattern
                    // binding its variable is part of the test.
                    "INSTANCE_OF" => {
                        let value = text(&node["expression"], "javaType");
                        let target = text(&node["type"], "javaType");
                        if self.declared.contains(value) && self.declared.contains(target) {
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
                            && !(self.declared.contains(ty)
                                && self.declared.contains(text(&node["expression"], "javaType"))) =>
                    {
                        scan.feature("reference casts", node)
                    }
                    "ENHANCED_FOR_LOOP" if !text(&node["expression"], "javaType").ends_with(']') => {
                        scan.feature("for-each over collections", node)
                    }
                    "BINDING_PATTERN" | "RECORD_PATTERN" => scan.feature("patterns", node),
                    _ => {}
                }
                // javac names an anonymous class `<anonymous Sink>`: that is not a generic type.
                if ty.contains('<') && !ty.starts_with('<') {
                    scan.feature("generic types", node);
                }
                if ty == "java.lang.Object" {
                    scan.feature("Object values", node);
                }
                if matches!(kind, "METHOD_INVOCATION" | "NEW_CLASS") {
                    let symbol = &node["symbol"];
                    let owner = text(symbol, "owner");
                    let name = text(symbol, "name");
                    // `super()` into `Object` does nothing, so it is not a call.
                    let object_init = owner == "java.lang.Object" && name == "<init>";
                    if object_init {
                        // Nothing to report.
                    } else if let Some(feature) = untranslated(owner) {
                        scan.feature(feature, node);
                    } else if RUNTIME_CLASSES.contains(&owner) {
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
            "\nMethods of the runtime's classes, and of classes outside the sources, by pieces of code that call them:"
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
            let mut natives: Vec<&str> =
                self.natives.iter().filter(|(p, _)| *p == part).map(|(_, n)| n.as_str()).collect();
            natives.sort();
            for native in natives {
                _ = writeln!(out, "  {native}");
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
            "natives": self.natives.iter().map(|(part, method)| json!({
                "source": part.name(),
                "method": method,
            })).collect::<Vec<_>>(),
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
    match kind {
        "ANNOTATION_TYPE" => _ = features.insert("annotation types"),
        "RECORD" => _ = features.insert("records"),
        _ => {}
    }
    // The program's own interfaces translate; a generic one is a generic class, and one from
    // elsewhere is not translated.
    for clause in decl["implementsClause"].as_array().into_iter().flatten() {
        let interface = text(clause, "javaType");
        if interface.contains('<') {
            features.insert("generic classes");
        } else if !declared.contains(interface) {
            features.insert("implementing interfaces outside the sources");
        }
    }
    // Class trees of the program's own classes translate; a superclass from elsewhere does not.
    if kind == "CLASS" && !decl["extendsClause"].is_null() {
        let superclass = text(&decl["extendsClause"], "javaType");
        if !declared.contains(superclass.split('<').next().unwrap_or(superclass)) {
            features.insert("extending classes outside the sources");
        }
    }
    if !decl["typeParameters"].as_array().is_none_or(Vec::is_empty) {
        features.insert("generic classes");
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

fn is_primitive(ty: &str) -> bool {
    matches!(ty, "boolean" | "byte" | "short" | "char" | "int" | "long" | "float" | "double")
}

fn percent(part: usize, whole: usize) -> String {
    format!("{:.1}%", 100.0 * part as f64 / whole.max(1) as f64)
}
