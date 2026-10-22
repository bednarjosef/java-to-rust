//! Immutable classes: a class whose fields are written only by its own constructors (and those
//! of its superclasses), through `this`, and whose `this` never escapes. Its objects never
//! change after construction, so they can be shared as `Rc<C>` without a `RefCell` to borrow.
//! The classes of one class tree are immutable together or not at all, so that an enum over
//! them reaches the fields they share in one way.

use super::Facts;
use super::hierarchy::{Home, home};
use crate::ast::*;
use std::collections::HashSet;

/// The qualified names of the classes whose objects never change once constructed.
pub fn immutable_classes(class: &Class) -> HashSet<String> {
    let scans = Scans::new(class);
    let alone: Vec<bool> = (0..class.types.len()).map(|t| scans.is_immutable(class, t)).collect();
    (0..class.types.len())
        .filter(|&t| {
            let family = class.hierarchy.family(t);
            alone[t] && family.iter().all(|&c| alone[c])
        })
        .map(|t| class.types[t].name.clone())
        .collect()
}

/// Removes from `immutable` the families of the classes named in `classes`: a class tree is
/// immutable together or not at all, so one class with a kept monitor makes its whole tree mutable.
pub fn remove_families(class: &Class, immutable: &mut HashSet<String>, classes: &HashSet<String>) {
    for name in classes {
        let Some(t) = class.type_index(name) else { continue };
        for f in class.hierarchy.family(t) {
            immutable.remove(&class.types[f].name);
        }
    }
}

/// The immutable concrete classes with a method that calls on `this` a body living on the enum of
/// a type: that call needs the object's handle to make the enum's value, which a method that
/// borrows the object as `&self` does not have. Their class trees cannot be immutable.
pub fn calls_needing_handles(class: &Class, facts: &Facts) -> HashSet<usize> {
    let h = &class.hierarchy;
    let mut out = HashSet::new();
    for m in 0..class.methods.len() {
        let Home::Class(c) = home(class, facts, m) else { continue };
        if !facts.immutable.contains(&class.types[c].name) || out.contains(&c) {
            continue;
        }
        let mut needs = false;
        for s in &class.methods[m].body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::MethodCall { receiver, resolved: Some(r), is_super, .. } = &x.kind
                            && matches!(receiver.kind, ExprKind::This)
                        {
                            let target = if *is_super { Some(*r) } else { h.implementation(*r, c) };
                            needs |= target.is_some_and(|t| matches!(home(class, facts, t), Home::Enum(_)));
                        }
                    })
                },
                &mut |_| {},
            );
        }
        if needs {
            out.insert(c);
        }
    }
    out
}

/// What every method and field initializer does with `this` and with fields, scanned once for
/// all the classes (scanning the whole program again for each class takes the number of classes
/// times the size of the program). A class's objects change after construction when code that
/// runs on them writes a field of `this` outside a constructor that builds them, when any code
/// writes a field of an object whose type the class is a subtype of, or when `this` escapes code
/// that runs on them: is used other than to read a field, to receive a call, or as the target of
/// a constructor's write.
struct Scans {
    /// What each method does with `this`.
    methods: Vec<MethodScan>,
    /// The types of the objects whose fields the methods write through a handle other than `this`.
    written: HashSet<usize>,
    /// The same, of the field initializers of each type.
    init_written: Vec<HashSet<usize>>,
    /// The `this` nodes that are a field object or a receiver in the field initializers of each
    /// type.
    init_allowed: Vec<HashSet<ExprId>>,
    /// The methods of each type.
    by_class: Vec<Vec<usize>>,
}

/// What one method does with `this`.
#[derive(Default)]
struct MethodScan {
    /// Its `this` nodes that are neither a field object nor a receiver in any method.
    selves: Vec<ExprId>,
    /// The `this` nodes whose fields it writes.
    this_writes: HashSet<ExprId>,
}

impl Scans {
    fn new(class: &Class) -> Scans {
        let mut allowed = HashSet::new();
        let mut written = HashSet::new();
        let mut methods = Vec::with_capacity(class.methods.len());
        let mut by_class = vec![Vec::new(); class.types.len()];
        for (index, m) in class.methods.iter().enumerate() {
            by_class[m.class].push(index);
            let mut scan = MethodScan::default();
            for s in &m.body {
                visit_stmt(
                    s,
                    &mut |e| {
                        visit_expr(e, &mut |x| {
                            if matches!(x.kind, ExprKind::This) {
                                scan.selves.push(x.id);
                            }
                            note(class, x, &mut allowed, &mut written, &mut scan.this_writes);
                        })
                    },
                    &mut |_| {},
                );
            }
            methods.push(scan);
        }
        for scan in &mut methods {
            scan.selves.retain(|id| !allowed.contains(id));
        }
        let mut init_written = vec![HashSet::new(); class.types.len()];
        let mut init_allowed = vec![HashSet::new(); class.types.len()];
        for (t, decl) in class.types.iter().enumerate() {
            for init in decl.fields.iter().filter_map(|f| f.init.as_ref()) {
                // The writes to fields of `this` in an initializer change nothing: no code runs on
                // the object there (see `is_immutable`).
                let mut ignored = HashSet::new();
                visit_expr(init, &mut |x| {
                    note(class, x, &mut init_allowed[t], &mut init_written[t], &mut ignored)
                });
            }
        }
        Scans { methods, written, init_written, init_allowed, by_class }
    }

    fn is_immutable(&self, class: &Class, t: usize) -> bool {
        let h = &class.hierarchy;
        let mut chain = vec![t];
        chain.extend(h.ancestors(t));
        // A write through another handle to an object that may be of the class.
        let supertypes = h.supertypes(t);
        let may_be = |written: &HashSet<usize>| supertypes.iter().any(|s| written.contains(s));
        if may_be(&self.written) || chain.iter().any(|&c| may_be(&self.init_written[c])) {
            return false;
        }
        // The code that runs on objects of the class: the instance methods and constructors of
        // the class and of its superclasses. The constructors among them build its objects.
        let runs_on =
            || chain.iter().flat_map(|&c| &self.by_class[c]).filter(|&&m| !class.methods[m].is_static);
        if runs_on().any(|&m| !self.methods[m].this_writes.is_empty() && !class.methods[m].is_ctor) {
            return false;
        }
        let built: Vec<&HashSet<ExprId>> =
            runs_on().filter(|&&m| class.methods[m].is_ctor).map(|&m| &self.methods[m].this_writes).collect();
        runs_on().flat_map(|&m| &self.methods[m].selves).all(|id| {
            built.iter().any(|w| w.contains(id)) || chain.iter().any(|&c| self.init_allowed[c].contains(id))
        })
    }
}

/// Notes what expression `x` does with fields: a `this` that is a field object or a receiver goes
/// into `allowed`, the type of an object whose field is written through another handle into
/// `written`, and a `this` whose field is written into `this_writes`.
fn note(
    class: &Class,
    x: &Expr,
    allowed: &mut HashSet<ExprId>,
    written: &mut HashSet<usize>,
    this_writes: &mut HashSet<ExprId>,
) {
    match &x.kind {
        ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This) => {
            allowed.insert(object.id);
        }
        ExprKind::MethodCall { receiver, .. } if matches!(receiver.kind, ExprKind::This) => {
            allowed.insert(receiver.id);
        }
        ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => {
            if matches!(object.kind, ExprKind::This) {
                this_writes.insert(object.id);
            } else if let Some(s) = class.class_index(object.ty) {
                written.insert(s);
            }
        }
        _ => {}
    }
}
