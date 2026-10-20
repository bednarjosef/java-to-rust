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

/// Rust names for the class's methods. Overloads get their parameter types appended, since Rust
/// has no overloading: `print(int)` and `print(double)` become `print_i32` and `print_f64`.
pub(super) fn method_names(class: &Class) -> Vec<String> {
    let mut used = HashSet::new();
    if class.methods.iter().any(|m| m.is_main) {
        used.insert("main".to_string());
    }
    class
        .methods
        .iter()
        .map(|m| {
            if m.is_main {
                return "main".to_string();
            }
            let mut base = snake_case(&m.name);
            let overloaded = class.methods.iter().filter(|o| o.name == m.name).count() > 1;
            if overloaded {
                for p in &m.params {
                    base = format!("{base}_{}", rust_type(p.ty).to_lowercase());
                }
            }
            unique(base, &mut used)
        })
        .collect()
}
