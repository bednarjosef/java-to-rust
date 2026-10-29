//! Immutable classes: a class whose fields are written only by its own constructors (and those
//! of its superclasses), through `this`, and whose `this` never escapes. Its objects never
//! change after construction, so they can be shared as `Rc<C>` without a `RefCell` to borrow.
//! The classes of one class tree are immutable together or not at all, so that an enum over
//! them reaches the fields they share in one way.

use super::Facts;
use super::hierarchy::{Home, home};
use super::{FastMap, FastSet};
use crate::ast::*;

/// The qualified names of the classes whose objects never change once constructed.
pub fn immutable_classes(class: &Class) -> FastSet<String> {
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
pub fn remove_families(class: &Class, immutable: &mut FastSet<String>, classes: &FastSet<String>) {
    for name in classes {
        let Some(t) = class.type_index(name) else { continue };
        for f in class.hierarchy.family(t) {
            immutable.remove(&class.types[f].name);
        }
    }
}

/// The classes whose objects change and are shared, whose fields are cells in an `Arc<C>`, read
/// and written by value; and of those, the classes whose objects keep a monitor, because threads
/// share them or the program waits on, notifies or asks about their monitor: an
/// `Arc<java::Monitored<C>>` of `java::SharedField`s, where the others hold `java::Field`s. A
/// class tree is celled and monitored together or not at all, like immutability, so that an enum
/// over it reaches the fields it shares in one way; a tree with an owned class or a Java enum
/// keeps its `RefCell`s or locks. A class that no `new` creates (an abstract one) can be
/// immutable while the classes below it are not: it has no objects that could change, and its
/// tree is celled all the same.
pub fn celled_classes(
    class: &Class,
    facts: &Facts,
    monitor_classes: &FastSet<String>,
    created: &[bool],
) -> (FastSet<String>, FastSet<String>) {
    let celled = |t: usize| {
        let name = &class.types[t].name;
        class.types[t].enum_constants.is_none()
            && (!facts.immutable.contains(name) || !created[t])
            && !facts.owned.contains(name)
    };
    let monitored =
        |t: usize| monitor_classes.contains(&class.types[t].name) || facts.thread_shared.contains(&t);
    // Every class of a tree decides, its abstract ones too, whose fields structs the concrete
    // ones embed.
    let classes = || (0..class.types.len()).filter(|&t| class.types[t].kind == TypeKind::Class);
    let mut trees: FastMap<usize, (bool, bool)> = FastMap::default();
    for t in classes() {
        let tree = trees.entry(class.hierarchy.root(t)).or_insert((true, false));
        tree.0 &= celled(t);
        tree.1 |= monitored(t);
    }
    let tree = |t: usize| trees[&class.hierarchy.root(t)];
    let name = |t: usize| class.types[t].name.clone();
    (
        classes().filter(|&t| tree(t).0).map(name).collect(),
        classes().filter(|&t| tree(t) == (true, true)).map(name).collect(),
    )
}

/// Which classes are ever instantiated with `new`.
pub fn instantiated(class: &Class) -> Vec<bool> {
    let mut live = vec![false; class.types.len()];
    let mut note = |e: &Expr| {
        if let ExprKind::New { class: c, .. } = &e.kind {
            live[*c] = true;
        }
    };
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut note), &mut |_| {});
        }
    }
    for f in class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics)) {
        if let Some(init) = &f.init {
            visit_expr(init, &mut note);
        }
    }
    live
}

/// The immutable concrete classes with a method that calls on `this` a body living on the enum of
/// a type: that call needs the object's handle to make the enum's value, which a method that
/// borrows the object as `&self` does not have. Their class trees cannot be immutable.
pub fn calls_needing_handles(class: &Class, facts: &Facts) -> FastSet<usize> {
    let h = &class.hierarchy;
    let mut out = FastSet::default();
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
    written: FastSet<usize>,
    /// The same, of the field initializers of each type.
    init_written: Vec<FastSet<usize>>,
    /// The `this` nodes that are a field object or a receiver in the field initializers of each
    /// type.
    init_allowed: Vec<FastSet<ExprId>>,
    /// The methods of each type.
    by_class: Vec<Vec<usize>>,
}

/// What one method does with `this`.
#[derive(Default)]
struct MethodScan {
    /// Its `this` nodes that are neither a field object nor a receiver in any method.
    selves: Vec<ExprId>,
    /// The `this` nodes whose fields it writes.
    this_writes: FastSet<ExprId>,
}

impl Scans {
    fn new(class: &Class) -> Scans {
        let mut allowed = FastSet::default();
        let mut written = FastSet::default();
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
        let mut init_written = vec![FastSet::default(); class.types.len()];
        let mut init_allowed = vec![FastSet::default(); class.types.len()];
        for (t, decl) in class.types.iter().enumerate() {
            for init in decl.fields.iter().filter_map(|f| f.init.as_ref()) {
                // The writes to fields of `this` in an initializer change nothing: no code runs on
                // the object there (see `is_immutable`).
                let mut ignored = FastSet::default();
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
        let may_be = |written: &FastSet<usize>| supertypes.iter().any(|s| written.contains(s));
        if may_be(&self.written) || chain.iter().any(|&c| may_be(&self.init_written[c])) {
            return false;
        }
        // The code that runs on objects of the class: the instance methods and constructors of
        // the class and of its superclasses, and the default methods of its interfaces that it
        // does not override (`DirectiveVisitor.visit` passes `this` on). The constructors among
        // them build its objects.
        let defaults = supertypes
            .iter()
            .filter(|&&s| class.types[s].kind == TypeKind::Interface)
            .flat_map(|&s| &self.by_class[s])
            .filter(|&&m| !class.methods[m].is_abstract && h.implementation(m, t) == Some(m));
        let runs_on = || {
            chain
                .iter()
                .flat_map(|&c| &self.by_class[c])
                .chain(defaults.clone())
                .filter(|&&m| !class.methods[m].is_static)
        };
        if runs_on().any(|&m| !self.methods[m].this_writes.is_empty() && !class.methods[m].is_ctor) {
            return false;
        }
        let built: Vec<&FastSet<ExprId>> =
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
    allowed: &mut FastSet<ExprId>,
    written: &mut FastSet<usize>,
    this_writes: &mut FastSet<ExprId>,
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
