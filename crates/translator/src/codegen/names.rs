//! Rust names for Java methods and locals: snake case, overload suffixes, keywords.

use super::*;

pub(super) const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "box", "crate", "dyn", "fn", "gen", "impl", "in", "let", "loop", "macro",
    "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "self", "struct", "trait", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "yield", "become", "extern",
];

/// `maxValue` becomes `max_value`, `HTTPServer` becomes `http_server`.
pub(super) fn snake_case(name: &str) -> String {
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

pub(super) fn unique(base: String, used: &mut HashSet<String>) -> String {
    let mut name = base.clone();
    let mut n = 2;
    while used.contains(&name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    used.insert(name.clone());
    name
}

/// `unique` among the names of two scopes, and taken in both.
fn unique_in(base: String, first: &mut HashSet<String>, second: &mut HashSet<String>) -> String {
    let mut name = base.clone();
    let mut n = 2;
    while first.contains(&name) || second.contains(&name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    first.insert(name.clone());
    second.insert(name.clone());
    name
}

/// Rust names for the class's methods. Overloads get their parameter types appended, since Rust
/// has no overloading: `print(int)` and `print(double)` become `print_i32` and `print_f64`.
/// Instance methods are named within their class tree, one name per override group, so that every
/// implementation of a method has the name its calls use. A superclass's method emitted in the
/// `impl` of a subclass that overrides it (where only `super` calls reach it) gets the name of its
/// class appended: `describe_in_shape`.
pub(super) fn method_names(class: &Class, homes: &[Home]) -> Vec<String> {
    let h = &class.hierarchy;
    // Each class has its own `impl`, so names only need to be unique within their class tree.
    let mut used = vec![HashSet::new(); class.types.len()];
    let mut module_used: HashMap<&str, HashSet<String>> = HashMap::new();
    for m in class.methods.iter().filter(|m| m.is_main) {
        used[m.class].insert("main".to_string());
        module_used.entry(class.types[m.class].package.as_str()).or_default().insert("main".to_string());
    }
    let mut group_names: HashMap<usize, String> = HashMap::new();
    let mut names: Vec<String> = Vec::with_capacity(class.methods.len());
    for (i, m) in class.methods.iter().enumerate() {
        let instance = !m.is_ctor && !m.is_static;
        if m.is_main {
            names.push("main".to_string());
            continue;
        }
        if instance && let Some(name) = group_names.get(&h.group_id(i)) {
            names.push(name.clone());
            continue;
        }
        // Constructors are named inside their class's `impl`, so they need not be unique.
        let root = h.root(m.class);
        let overloads = if instance {
            let mut groups: Vec<usize> = (0..class.methods.len())
                .filter(|&o| {
                    let other = &class.methods[o];
                    other.name == m.name
                        && (other.class == m.class || (!other.is_static && h.root(other.class) == root))
                })
                .map(|o| h.group_id(o))
                .collect();
            groups.sort_unstable();
            groups.dedup();
            groups.len()
        } else {
            class.methods.iter().filter(|o| o.name == m.name && o.class == m.class).count()
        };
        let mut base = if m.is_ctor { "new".to_string() } else { snake_case(&m.name) };
        if overloads > 1 {
            for p in &m.params {
                base = format!("{base}_{}", name_part(p.ty));
            }
        }
        names.push(if m.is_ctor {
            base
        } else if m.is_static {
            // Static methods are functions of their package's module, shared by its classes.
            let module = module_used.entry(class.types[m.class].package.as_str()).or_default();
            unique_in(base, &mut used[m.class], module)
        } else {
            let name = unique(base, &mut used[root]);
            group_names.insert(h.group_id(i), name.clone());
            name
        });
    }
    for i in 0..class.methods.len() {
        let m = &class.methods[i];
        let Home::Class(c) = homes[i] else { continue };
        let shadowed = c != m.class
            && (0..class.methods.len()).any(|o| o != i && homes[o] == Home::Class(c) && names[o] == names[i]);
        if shadowed {
            let simple = class.types[m.class].name.rsplit(['.', '$']).next().unwrap_or_default();
            let base = format!("{}_in_{}", names[i], snake_case(simple));
            names[i] = unique(base, &mut used[h.root(m.class)]);
        }
    }
    names
}
