//! The survey of a program: which Java features the code reachable from `main` uses that the
//! translator does not support yet, and which methods of the runtime's classes it calls. It
//! measures what to support next, such as what stands between the translator and a library like
//! the JDK; `--survey` prints it.
//!
//! The census is syntactic, on javac's trees: a piece of code (a method, a field initializer or an
//! initializer block) counts a feature when it, or the declaration of its class, uses it. It does
//! not run the lowering, so it misses the narrower rejections there.

use super::reach::{
    CLASS_KINDS, RUNTIME_CLASSES, Reach, class_name, has_flag, members, symbol_type, text, untranslated,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;

/// A reachable piece of code and the unsupported features it uses.
struct Piece {
    class: String,
    features: BTreeSet<&'static str>,
}

/// The survey of the code reachable from `main`, as a text report.
pub fn survey(units: &[&Value], reach: &Reach, main: &str) -> String {
    let mut census = Census::default();
    for unit in units {
        for decl in unit["typeDecls"].as_array().into_iter().flatten() {
            census.declare(decl);
        }
    }
    for unit in units {
        for decl in unit["typeDecls"].as_array().into_iter().flatten() {
            census.class(decl, None, reach);
        }
    }
    census.report(reach, main)
}

#[derive(Default)]
struct Census {
    pieces: Vec<Piece>,
    /// The classes of the sources, to tell calls that leave them.
    declared: HashSet<String>,
    /// Methods of the runtime's classes that reachable code calls, with the number of pieces of
    /// code that call each.
    runtime: BTreeMap<String, usize>,
    /// Reachable methods that have no body in Java: the JVM implements them.
    natives: Vec<String>,
}

impl Census {
    fn declare(&mut self, decl: &Value) {
        if CLASS_KINDS.contains(&text(decl, "kind")) {
            self.declared.insert(class_name(decl));
            members(decl).for_each(|member| self.declare(member));
        }
    }

    fn class(&mut self, decl: &Value, outer: Option<&Value>, reach: &Reach) {
        if !CLASS_KINDS.contains(&text(decl, "kind")) {
            return;
        }
        let class = class_name(decl);
        if !reach.classes.contains(&class) {
            return;
        }
        let header = class_features(decl, outer);
        for member in members(decl) {
            let mut features = header.clone();
            let kind = text(member, "kind");
            let reached = match kind {
                "METHOD" => {
                    let key =
                        (class.clone(), text(member, "name").to_string(), symbol_type(member).to_string());
                    if !member["typeParameters"].as_array().is_none_or(Vec::is_empty) {
                        features.insert("generic methods");
                    }
                    if member["body"].is_null() && has_flag(member, "NATIVE") && reach.methods.contains(&key)
                    {
                        self.natives.push(format!("{class}.{}{}", key.1, key.2));
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
                    self.class(member, Some(decl), reach);
                    continue;
                }
            };
            if reached {
                let mut calls = BTreeSet::new();
                self.body(member, &mut features, &mut calls);
                for call in calls {
                    *self.runtime.entry(call).or_default() += 1;
                }
                self.pieces.push(Piece { class: class.clone(), features });
            }
        }
    }

    /// The unsupported features that a piece of code uses, and the methods of the runtime it calls.
    fn body(&self, node: &Value, features: &mut BTreeSet<&'static str>, calls: &mut BTreeSet<String>) {
        match node {
            Value::Object(fields) => {
                let kind = text(node, "kind");
                let ty = text(node, "javaType");
                match kind {
                    "LAMBDA_EXPRESSION" => _ = features.insert("lambdas"),
                    "MEMBER_REFERENCE" => _ = features.insert("method references"),
                    "NEW_CLASS" if !node["classBody"].is_null() => _ = features.insert("anonymous classes"),
                    "NEW_CLASS" if !node["enclosingExpression"].is_null() => {
                        _ = features.insert("inner classes")
                    }
                    "CLASS" | "INTERFACE" | "ENUM" | "RECORD" => _ = features.insert("local classes"),
                    "TRY" | "THROW" => _ = features.insert("exceptions"),
                    "INSTANCE_OF" => _ = features.insert("instanceof"),
                    "TYPE_CAST" if !is_primitive(ty) => _ = features.insert("reference casts"),
                    "ENHANCED_FOR_LOOP" if !text(&node["expression"], "javaType").ends_with(']') => {
                        _ = features.insert("for-each over collections")
                    }
                    "BINDING_PATTERN" | "RECORD_PATTERN" => _ = features.insert("patterns"),
                    "METHOD_INVOCATION" if text(&node["methodSelect"]["expression"], "name") == "super" => {
                        _ = features.insert("super calls")
                    }
                    _ => {}
                }
                if ty.contains('<') {
                    features.insert("generic types");
                }
                if ty == "java.lang.Object" {
                    features.insert("Object values");
                }
                if matches!(kind, "METHOD_INVOCATION" | "NEW_CLASS") {
                    let symbol = &node["symbol"];
                    let owner = text(symbol, "owner");
                    // `super()` into `Object` does nothing.
                    if owner == "java.lang.Object" && text(symbol, "name") == "<init>" {
                    } else if let Some(feature) = untranslated(owner) {
                        features.insert(feature);
                    } else if RUNTIME_CLASSES.contains(&owner) {
                        let simple = owner.rsplit('.').next().unwrap_or(owner);
                        calls.insert(format!("{simple}.{}", text(symbol, "name")));
                    } else if !owner.is_empty() && !self.declared.contains(owner) {
                        features.insert("classes outside the sources");
                        calls.insert(format!("(outside) {owner}.{}", text(symbol, "name")));
                    }
                }
                for (field, child) in fields {
                    if field != "symbol" {
                        self.body(child, features, calls);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.body(item, features, calls)),
            _ => {}
        }
    }

    fn report(&self, reach: &Reach, main: &str) -> String {
        let mut out = String::new();
        let free = self.pieces.iter().filter(|p| p.features.is_empty()).count();
        let total = self.pieces.len();
        _ = writeln!(
            out,
            "Reachable from {main}: {} classes ({} instantiated), {} methods, {} fields, {} native methods.",
            reach.classes.len(),
            reach.instantiated.len(),
            reach.methods.len(),
            reach.fields.len(),
            self.natives.len()
        );
        _ = writeln!(
            out,
            "{total} pieces of code (methods, field initializers, initializer blocks); \
             {free} ({}) use no unsupported feature.\n",
            percent(free, total)
        );
        // Each feature: the classes and pieces that use it, and the pieces it alone blocks.
        let mut table: BTreeMap<&str, (BTreeSet<&str>, usize, usize)> = BTreeMap::new();
        for piece in &self.pieces {
            for &feature in &piece.features {
                let row = table.entry(feature).or_default();
                row.0.insert(&piece.class);
                row.1 += 1;
                row.2 += usize::from(piece.features.len() == 1);
            }
        }
        let mut rows: Vec<_> = table.iter().collect();
        rows.sort_by_key(|(_, (_, pieces, _))| std::cmp::Reverse(*pieces));
        _ = writeln!(out, "{:<28} {:>8} {:>8} {:>13}", "feature", "classes", "code", "only blocker");
        for (feature, (classes, pieces, only)) in rows {
            _ = writeln!(out, "{feature:<28} {:>8} {:>8} {only:>13}", classes.len(), pieces);
        }
        _ = writeln!(out, "\nSupporting features in this order frees the most code at each step:");
        let mut supported: HashSet<&str> = HashSet::new();
        let mut freed = free;
        while supported.len() < table.len() {
            let mut gains: HashMap<&str, usize> = HashMap::new();
            for piece in &self.pieces {
                let missing: Vec<&str> =
                    piece.features.iter().copied().filter(|f| !supported.contains(f)).collect();
                if let [only] = missing[..] {
                    *gains.entry(only).or_default() += 1;
                }
            }
            // With nothing freed by one feature alone, the most used one comes next.
            let next =
                gains.iter().max_by_key(|&(f, &g)| (g, table[f].1, std::cmp::Reverse(*f))).map(|(f, _)| *f);
            let next = next.unwrap_or_else(|| {
                table
                    .iter()
                    .filter(|(f, _)| !supported.contains(*f))
                    .max_by_key(|(_, row)| row.1)
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
        let mut calls: Vec<_> = self.runtime.iter().collect();
        calls.sort_by_key(|(name, n)| (std::cmp::Reverse(**n), *name));
        _ = writeln!(
            out,
            "\nMethods of the runtime's classes, and of classes outside the sources, by pieces of code that call them:"
        );
        for (name, n) in calls {
            _ = writeln!(out, "  {n:>6}  {name}");
        }
        _ = writeln!(out, "\nReachable classes, by pieces of code, with what they use:");
        let mut classes: BTreeMap<&str, (usize, BTreeSet<&str>)> = BTreeMap::new();
        for piece in &self.pieces {
            let row = classes.entry(&piece.class).or_default();
            row.0 += 1;
            row.1.extend(&piece.features);
        }
        let mut classes: Vec<_> = classes.into_iter().collect();
        classes.sort_by_key(|(name, (n, _))| (std::cmp::Reverse(*n), *name));
        for (name, (n, features)) in classes {
            let features: Vec<&str> = features.into_iter().collect();
            _ = writeln!(out, "  {n:>5}  {name}: {}", features.join(", "));
        }
        _ = writeln!(out, "\nNative methods reached:");
        let mut natives = self.natives.clone();
        natives.sort();
        for native in natives {
            _ = writeln!(out, "  {native}");
        }
        out
    }
}

/// The unsupported features of a class declaration, which every piece of its code shares.
fn class_features(decl: &Value, outer: Option<&Value>) -> BTreeSet<&'static str> {
    let mut features = BTreeSet::new();
    let kind = text(decl, "kind");
    match kind {
        "INTERFACE" => _ = features.insert("interfaces"),
        "ANNOTATION_TYPE" => _ = features.insert("annotation types"),
        "RECORD" => _ = features.insert("records"),
        _ => {}
    }
    if !decl["implementsClause"].as_array().is_none_or(Vec::is_empty) {
        features.insert("interfaces");
    }
    if kind == "CLASS" && (!decl["extendsClause"].is_null() || has_flag(decl, "ABSTRACT")) {
        features.insert("inheritance");
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
