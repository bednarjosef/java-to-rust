//! Where class hierarchies live in the Rust: which classes have a fields struct that their
//! subclasses embed, which methods only touch fields (and so live on that struct), how a value
//! of each class type is represented (one concrete class's object, or an enum over several),
//! and where each method's body is emitted.

use super::Facts;
use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// How values of a class type are represented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rep {
    /// The object of one concrete class (index into `Class::types`), as that class is
    /// represented: every value of the type is an object of that class.
    Object(usize),
    /// The enum named after the type, with a variant per concrete class it can hold.
    Enum(usize),
}

/// Where the body of a method is emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Home {
    /// In the `impl` of the struct that holds objects of exactly this concrete class.
    Class(usize),
    /// On the fields struct of this class, taking `&self` or `&mut self` of it.
    Fields(usize),
    /// On the enum of this type, taking `&self` of the enum.
    Enum(usize),
    /// A static method: a function of its class's module.
    Free,
    /// Nowhere: an abstract method, or a method no object can run.
    Nowhere,
}

/// Whether type `t` is a class with subclasses: its objects' fields are a struct of their own.
pub fn is_inner(class: &Class, t: usize) -> bool {
    class.types[t].kind == TypeKind::Class && class.hierarchy.has_subtypes(t)
}

/// The classes with subclasses that have a fields struct: those that declare instance fields,
/// have objects of their own, or have a constructor that does something. Its subclasses embed
/// it as their field `base`.
pub fn fields_structs(class: &Class) -> HashSet<usize> {
    let mut out = HashSet::new();
    // Superclasses first, so that a constructor's `super(...)` is known to do something or not.
    let mut order: Vec<usize> = (0..class.types.len()).collect();
    order.sort_by_key(|&t| class.hierarchy.ancestors(t).len());
    for t in order {
        if !is_inner(class, t) {
            continue;
        }
        let decl = &class.types[t];
        let trivial = |m: &Method| match &m.body[..] {
            [] => true,
            [Stmt::Expr(Expr { kind: ExprKind::SuperInit { args, resolved }, .. })] => {
                resolved.is_none_or(|r| !out.contains(&class.methods[r].class))
                    && !args.iter().any(has_effects)
            }
            _ => false,
        };
        let has = !decl.fields.is_empty()
            || class.hierarchy.is_concrete(t)
            || class.hierarchy.ancestors(t).iter().any(|a| out.contains(a))
            || class.methods.iter().any(|m| m.class == t && m.is_ctor && !trivial(m));
        if has {
            out.insert(t);
        }
    }
    out
}

/// The class that declares each field visible in each class, by (class name, field name).
pub fn field_owners(class: &Class) -> HashMap<(String, String), String> {
    let mut out = HashMap::new();
    for (t, decl) in class.types.iter().enumerate() {
        let mut chain = vec![t];
        chain.extend(class.hierarchy.ancestors(t));
        for &a in chain.iter().rev() {
            for f in &class.types[a].fields {
                out.insert((decl.name.clone(), f.name.clone()), class.types[a].name.clone());
            }
        }
    }
    out
}

/// The superclass whose fields struct the struct of class `t` embeds as `base`.
pub fn embedded(facts: &Facts, class: &Class, t: usize) -> Option<usize> {
    class.hierarchy.ancestors(t).into_iter().find(|a| facts.fields_structs.contains(a))
}

/// The path from the struct of class `c` to the fields struct of its superclass `d`, as field
/// accesses with a trailing dot: `base.base.`; empty when `d` is `c`.
pub fn base_path(facts: &Facts, class: &Class, c: usize, d: usize) -> String {
    let mut path = String::new();
    if c == d {
        return path;
    }
    for a in class.hierarchy.ancestors(c) {
        if facts.fields_structs.contains(&a) {
            path.push_str("base.");
        }
        if a == d {
            break;
        }
    }
    path
}

/// The concrete classes whose objects the values of type `t` can be: those under it that are
/// not plain values (an owned class is never converted to another type).
pub fn variants(class: &Class, facts: &Facts, t: usize) -> Vec<usize> {
    class
        .hierarchy
        .concretes(t)
        .into_iter()
        .filter(|&c| !facts.owned.contains(&class.types[c].name))
        .collect()
}

/// How values of class type `t` are represented.
pub fn rep(class: &Class, facts: &Facts, t: usize) -> Rep {
    if class.hierarchy.is_concrete(t) && !class.hierarchy.has_subtypes(t) {
        return Rep::Object(t);
    }
    match variants(class, facts, t)[..] {
        [only] => Rep::Object(only),
        _ => Rep::Enum(t),
    }
}

/// Where method `m` is emitted.
pub fn home(class: &Class, facts: &Facts, m: usize) -> Home {
    let method = &class.methods[m];
    let t = method.class;
    if method.is_static {
        return Home::Free;
    }
    if method.is_abstract {
        return Home::Nowhere;
    }
    if method.is_ctor || class.types[t].enum_constants.is_some() || !class.hierarchy.has_subtypes(t) {
        return Home::Class(t);
    }
    if facts.field_level.contains(&m) {
        return Home::Fields(t);
    }
    match rep(class, facts, t) {
        Rep::Object(c) => Home::Class(c),
        Rep::Enum(t) if variants(class, facts, t).is_empty() => Home::Nowhere,
        Rep::Enum(t) => Home::Enum(t),
    }
}

/// The `toString` that an object of concrete class `c` runs, if the program declares one.
pub fn to_string_impl(class: &Class, c: usize) -> Option<usize> {
    let mut chain = vec![c];
    chain.extend(class.hierarchy.ancestors(c));
    chain.into_iter().find_map(|t| {
        class.methods.iter().position(|m| {
            m.class == t && m.name == "toString" && m.params.is_empty() && !m.is_static && !m.is_abstract
        })
    })
}

/// The methods of classes with subclasses that use their object only to read and write its
/// fields and to call such methods that nothing overrides: they live on the class's fields
/// struct and borrow it, as `&self` or `&mut self`. That is only safe when nothing they run
/// reaches an object of their class tree through another handle (see `borrowing`), unless the
/// tree is immutable, whose objects have no `RefCell`.
pub fn field_level(
    class: &Class,
    reach: &[HashSet<&str>],
    immutable: &HashSet<String>,
    structs: &HashSet<usize>,
) -> HashSet<usize> {
    let h = &class.hierarchy;
    let mut level: HashSet<usize> = (0..class.methods.len())
        .filter(|&m| {
            let method = &class.methods[m];
            let t = method.class;
            if method.is_static
                || method.is_ctor
                || method.is_abstract
                || !is_inner(class, t)
                || !structs.contains(&t)
            {
                return false;
            }
            let family: Vec<&str> = h.family(t).into_iter().map(|c| class.types[c].name.as_str()).collect();
            family.iter().all(|c| immutable.contains(*c)) || !family.iter().any(|c| reach[m].contains(c))
        })
        .collect();
    loop {
        let losing: Vec<usize> = level
            .iter()
            .copied()
            .filter(|&m| {
                let method = &class.methods[m];
                let mut chain = vec![method.class];
                chain.extend(h.ancestors(method.class));
                !method.body.iter().all(|s| {
                    let mut ok = true;
                    visit_stmt(
                        s,
                        &mut |e| {
                            ok &= uses_this_for_fields(e, &|r, is_super| {
                                level.contains(&r)
                                    && chain.contains(&class.methods[r].class)
                                    && (is_super || !h.is_overridden(r))
                            })
                        },
                        &mut |_| {},
                    );
                    ok
                })
            })
            .collect();
        if losing.is_empty() {
            return level;
        }
        for m in losing {
            level.remove(&m);
        }
    }
}

/// Whether `e` uses `this` only as the object of a field access or the receiver of a call that
/// `allowed` accepts (given the callee and whether it is a `super` call).
fn uses_this_for_fields(e: &Expr, allowed: &dyn Fn(usize, bool) -> bool) -> bool {
    let mut ok_ids = HashSet::new();
    let mut selves = Vec::new();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::This => selves.push(x.id),
        ExprKind::Field { object, .. }
        | ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
            if matches!(object.kind, ExprKind::This) =>
        {
            ok_ids.insert(object.id);
        }
        ExprKind::MethodCall { receiver, resolved: Some(r), is_super, .. }
            if matches!(receiver.kind, ExprKind::This) && allowed(*r, *is_super) =>
        {
            ok_ids.insert(receiver.id);
        }
        _ => {}
    });
    selves.iter().all(|id| ok_ids.contains(id))
}
