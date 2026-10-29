//! Rust names for Java methods and locals: snake case, overload suffixes, keywords.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// Rust's keywords, strict and reserved, which a snake-cased Java name can be: `Return` and `If`
/// are names of javac's `TreeMaker`, though Java's own keywords never name anything.
pub(super) const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let", "loop",
    "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "self", "static",
    "struct", "super", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use", "virtual",
    "where", "while", "yield",
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

/// The Rust name of the field `name` of the class `decl`: `snake_case`, but `base_` for `base`
/// in a subclass, whose struct may hold its superclass's fields as `base`. Fields whose names
/// differ only in case (javac's `Names.module` and `Names.Module`) are numbered after the first.
pub(super) fn field_name(decl: &TypeDecl, name: &str) -> String {
    let snake = snake_case(name);
    let field = if snake == "base" && decl.extends.is_some() { "base_".to_string() } else { snake.clone() };
    // Two names whose snake cases are the same are the same without underscores and `$`, in
    // lower case: the snake case of the others is not made.
    let mut same = decl
        .fields
        .iter()
        .filter(|f| f.name == name || (folded(&f.name).eq(folded(name)) && snake_case(&f.name) == snake));
    match same.position(|f| f.name == name) {
        Some(n) if n > 0 => format!("{field}_{}", n + 1),
        _ => field,
    }
}

/// The characters of `name` without underscores and `$`, in lower case.
fn folded(name: &str) -> impl Iterator<Item = char> + '_ {
    name.chars().filter(|&c| c != '_' && c != '$').flat_map(char::to_lowercase)
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

/// `unique` among the names `used` and those `taken` in an enclosing scope, and taken in `used`.
pub(super) fn unique_beside(base: String, used: &mut HashSet<String>, taken: &HashSet<String>) -> String {
    let mut name = base.clone();
    let mut n = 2;
    while used.contains(&name) || taken.contains(&name) {
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
/// class appended: `describe_in_shape`. So does a body emitted on the enum of a type when a class
/// under the type overrides it, since the enum's dispatcher has the method's name, and a
/// `toString` there, since `to_string` is the enum's `Display`. Also returns the name of each
/// override group, by group id.
pub(super) fn method_names(class: &Class, homes: &[Home]) -> (Vec<String>, HashMap<usize, String>) {
    let h = &class.hierarchy;
    // Each class has its own `impl`, so names only need to be unique within their class tree.
    // `base` and `base_mut` are taken in each: the enum of a class tree has methods of those
    // names, the fields of the object's class (`dispatch.rs`), beside a method of the JDK's own
    // `base` (`ByteBuffer.base()`, then `base_2`). So is `view`, which lends what a value of the
    // enum holds. So is `clone`, `Clone`'s, which copies a
    // handle: a Java `clone()` (`CalendarDate.clone()`) is `clone_2`.
    // The methods that answer for an object's class on an enum (`dispatch::class_value_impls`)
    // are taken too.
    let taken = [
        "base",
        "base_mut",
        "view",
        "clone",
        "runtime_class",
        "runtime_class_name",
        "runtime_type_names",
        "object_identity_hash",
    ]
    .map(String::from);
    let mut used = vec![HashSet::from(taken); class.types.len()];
    let mut module_used: HashMap<&str, HashSet<String>> = HashMap::new();
    // A class's static initialization is a function of its package's module too
    // (`statics::init_name`): `LogManager`'s `init_log_manager` beside its static
    // `initLogManager()`, which is then `init_log_manager_2`.
    for decl in &class.types {
        let local = decl.name.strip_prefix(&format!("{}.", decl.package)).unwrap_or(&decl.name);
        let init = format!("init_{}", snake_case(&modules::flat_name(&decl.name, local)));
        let module = module_used.entry(decl.package.as_str()).or_default();
        module.insert(format!("{init}_slow"));
        module.insert(init);
    }
    for m in class.methods.iter().filter(|m| m.is_main) {
        used[m.class].insert("main".to_string());
        module_used.entry(class.types[m.class].package.as_str()).or_default().insert("main".to_string());
    }
    let mut constructors: Vec<HashSet<String>> = vec![HashSet::new(); class.types.len()];
    let mut group_names: HashMap<usize, String> = HashMap::new();
    let mut names: Vec<String> = Vec::with_capacity(class.methods.len());
    // The methods of each name in each class, in index order: the overloads of a method are
    // among them, and among the instance methods of its name in its class tree.
    let mut by_class: HashMap<(&str, usize), Vec<usize>> = HashMap::new();
    for (o, other) in class.methods.iter().enumerate() {
        by_class.entry((other.name.as_str(), other.class)).or_default().push(o);
    }
    // The override groups of the instance methods of each name in each class tree, sorted, each
    // once: found once per name and tree rather than once per method of the name, which made
    // the names of the Minecraft server's methods, a few hundred thousand, take most of a minute.
    let mut tree_groups: HashMap<(&str, usize), Vec<usize>> = HashMap::new();
    for (o, other) in class.methods.iter().enumerate() {
        if !other.is_static {
            tree_groups.entry((other.name.as_str(), h.root(other.class))).or_default().push(h.group_id(o));
        }
    }
    for groups in tree_groups.values_mut() {
        groups.sort_unstable();
        groups.dedup();
    }
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
        let root = h.root(m.class);
        let in_class = &by_class[&(m.name.as_str(), m.class)];
        let overloads = if instance {
            // The groups of the tree, and those of the class's static methods of the name.
            let tree = &tree_groups[&(m.name.as_str(), root)];
            let statics = in_class.iter().filter(|&&o| class.methods[o].is_static).map(|&o| h.group_id(o));
            let more: BTreeSet<usize> = statics.filter(|g| tree.binary_search(g).is_err()).collect();
            tree.len() + more.len()
        } else {
            in_class.len()
        };
        let mut base = if m.is_ctor { "new".to_string() } else { snake_case(&m.name) };
        if overloads > 1 {
            for p in &m.params {
                base = format!("{base}_{}", name_part(p.ty));
            }
            // A method that a bridge of its class calls takes the same parameters as the bridge
            // (`Integer next()` beside the bridge `Object next()`): its return type tells it apart.
            let same_params = |o: &Method| {
                o.params.len() == m.params.len() && o.params.iter().zip(&m.params).all(|(a, b)| a.ty == b.ty)
            };
            if instance
                && in_class.iter().any(|&o| {
                    let other = &class.methods[o];
                    o != i && h.group_id(o) != h.group_id(i) && !other.is_static && same_params(other)
                })
            {
                base = format!("{base}_{}", name_part(m.ret));
            }
        }
        names.push(if m.is_ctor {
            // Constructors are named inside their class's `impl`, so they need only be unique
            // among their class's: overloads whose parameters come to the same names
            // (`StringArray(String[], String)` and `StringArray(Object[], String)` of JNA, as a
            // `String[]` passed on as an `Object[]` is held as one) are numbered after the first.
            unique(base, &mut constructors[m.class])
        } else if m.is_static {
            // Static methods are functions of their package's module, shared by its classes.
            let module = module_used.entry(class.types[m.class].package.as_str()).or_default();
            unique_in(base, &mut used[m.class], module)
        } else {
            // The group's name is taken in the class tree of each of its methods.
            let roots: BTreeSet<usize> =
                h.group(i).into_iter().map(|o| h.root(class.methods[o].class)).collect();
            let mut name = base.clone();
            let mut n = 2;
            while roots.iter().chain([&root]).any(|&r| used[r].contains(&name)) {
                name = format!("{base}_{n}");
                n += 1;
            }
            for &r in roots.iter().chain([&root]) {
                used[r].insert(name.clone());
            }
            group_names.insert(h.group_id(i), name.clone());
            name
        });
    }
    separate_met_groups(class, &mut names, &mut group_names, &mut used);
    // The methods emitted on each struct (`Home::Class` or `Home::Fields` of a class).
    let mut on_struct: HashMap<usize, Vec<usize>> = HashMap::new();
    for (o, home) in homes.iter().enumerate() {
        if let Home::Class(d) | Home::Fields(d) = *home {
            on_struct.entry(d).or_default().push(o);
        }
    }
    for i in 0..class.methods.len() {
        let m = &class.methods[i];
        // A class's struct may hold the methods of both homes (`JarFileFields` has ZipFile's
        // `getEntry` and JarFile's own, of a JarFile that no code creates).
        let same_struct = |home: Home, c: usize| matches!(home, Home::Class(d) | Home::Fields(d) if d == c);
        let shadowed = match homes[i] {
            // A constructor's name already holds its class (`construct_error_string`), and its
            // calls name it so.
            _ if m.is_ctor => false,
            Home::Class(c) | Home::Fields(c) => {
                c != m.class
                    && on_struct[&c]
                        .iter()
                        .any(|&o| o != i && same_struct(homes[o], c) && names[o] == names[i])
            }
            Home::Enum(t) => {
                (m.name == "toString" && m.params.is_empty())
                    // `Type`'s `getTypeName()`, which a class held as a `Type` answers itself
                    // (see `dispatch::enum_body`).
                    || (m.name == "getTypeName"
                        && m.params.is_empty()
                        && class.types[m.class].name == "java.lang.reflect.Type")
                    || h.concretes(t).into_iter().any(|c| h.implementation(i, c) != Some(i))
            }
            _ => false,
        };
        if shadowed {
            let simple = class.types[m.class].name.rsplit(['.', '$']).next().unwrap_or_default();
            let base = format!("{}_in_{}", names[i], snake_case(simple));
            names[i] = unique(base, &mut used[h.root(m.class)]);
        }
    }
    (names, group_names)
}

/// Renames the override groups that came to one name and meet on a type. A group's name is taken
/// in the class trees of its methods, but a type also has the methods of its interfaces, whose
/// trees are their own: fastutil's `Reference2FloatMap` extends both `Reference2FloatFunction`,
/// whose `put(K, float)` is alone in its tree, and `Map`, whose `put(K, V)` is too, and its enum
/// has a dispatcher for each. Of two groups of one name on a type, the one named later takes the
/// first number after the name that is free in its trees and on that type; as the new name may
/// meet another group elsewhere, the types are gone through again until no two meet.
fn separate_met_groups(
    class: &Class,
    names: &mut [String],
    group_names: &mut HashMap<usize, String>,
    used: &mut [HashSet<String>],
) {
    let h = &class.hierarchy;
    let instance = |m: usize| {
        let method = &class.methods[m];
        !method.is_ctor && !method.is_static && !method.is_main
    };
    loop {
        // Only a name that two groups have can meet.
        let mut groups_of: HashMap<&str, BTreeSet<usize>> = HashMap::new();
        for (&group, name) in group_names.iter() {
            groups_of.entry(name.as_str()).or_default().insert(group);
        }
        let shared: HashSet<String> = groups_of
            .into_iter()
            .filter(|(_, groups)| groups.len() > 1)
            .map(|(n, _)| n.to_string())
            .collect();
        if shared.is_empty() {
            return;
        }
        let mut renamed = false;
        // Whether each method is an instance method of a shared name, kept as names change.
        let mut of_shared: Vec<bool> = (0..names.len())
            .map(|m| m < class.methods.len() && instance(m) && shared.contains(&names[m]))
            .collect();
        for t in 0..class.types.len() {
            // The groups of each shared name among the methods that `t` has.
            let mut met: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
            for s in h.supertypes(t) {
                for &m in h.methods_of(s) {
                    if of_shared.get(m) == Some(&true) {
                        met.entry(names[m].clone()).or_default().insert(h.group_id(m));
                    }
                }
            }
            for (name, groups) in met {
                // The first group keeps the name.
                for &group in groups.iter().skip(1) {
                    if group_names.get(&group) != Some(&name) {
                        continue;
                    }
                    let members = h.group(group);
                    let roots: BTreeSet<usize> =
                        members.iter().map(|&o| h.root(class.methods[o].class)).collect();
                    let free = |n: &String| {
                        !roots.iter().any(|&r| used[r].contains(n))
                            && !h
                                .supertypes(t)
                                .iter()
                                .any(|&s| h.methods_of(s).iter().any(|&m| names.get(m) == Some(n)))
                    };
                    let fresh = (2..).map(|n| format!("{name}_{n}")).find(free).unwrap_or_default();
                    for &r in &roots {
                        used[r].insert(fresh.clone());
                    }
                    for &o in &members {
                        names[o] = fresh.clone();
                        if let Some(flag) = of_shared.get_mut(o) {
                            *flag = o < class.methods.len() && instance(o) && shared.contains(&fresh);
                        }
                    }
                    group_names.insert(group, fresh);
                    renamed = true;
                }
            }
        }
        if !renamed {
            return;
        }
    }
}
