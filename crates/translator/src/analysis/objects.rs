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
    let writes = Writes::of(class);
    let alone: Vec<bool> = (0..class.types.len()).map(|t| writes.immutable(class, t)).collect();
    (0..class.types.len())
        .filter(|&t| {
            let family = class.hierarchy.family(t);
            alone[t] && family.iter().all(|&c| alone[c])
        })
        .map(|t| class.types[t].name.clone())
        .collect()
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

/// What the program writes, gathered in one pass for every class at once.
struct Writes {
    /// The classes with an instance method or constructor that writes a field of `this` outside
    /// a constructor, or uses `this` other than as the object of a field, the receiver of a call
    /// or, in a constructor, the object of a write.
    escaping: HashSet<usize>,
    /// The types (by index) of the objects other than `this` whose fields a method writes.
    written: HashSet<usize>,
    /// For each class, the types of the objects other than `this` whose fields its instance field
    /// initializers write.
    written_in_inits: Vec<HashSet<usize>>,
}

impl Writes {
    fn of(class: &Class) -> Writes {
        let mut written = HashSet::new();
        let mut escaping = HashSet::new();
        for m in &class.methods {
            let mut uses = ThisUses { ctor: m.is_ctor, ..ThisUses::default() };
            for s in &m.body {
                visit_stmt(
                    s,
                    &mut |e| visit_expr(e, &mut |x| uses.expr(class, x, &mut written)),
                    &mut |_| {},
                );
            }
            let escapes = uses.writes_outside_ctor || !uses.selves.iter().all(|id| uses.allowed.contains(id));
            if !m.is_static && escapes {
                escaping.insert(m.class);
            }
        }
        let written_in_inits = class
            .types
            .iter()
            .map(|d| {
                let mut out = HashSet::new();
                let mut uses = ThisUses::default();
                for init in d.fields.iter().filter_map(|f| f.init.as_ref()) {
                    visit_expr(init, &mut |x| uses.expr(class, x, &mut out));
                }
                out
            })
            .collect();
        Writes { escaping, written, written_in_inits }
    }

    /// Whether the objects of class `t` never change once constructed: no instance method or
    /// constructor of it or of a superclass writes a field of `this` outside the constructors or
    /// lets `this` escape, and no code writes a field of an object that may be of `t` other than
    /// through `this`.
    fn immutable(&self, class: &Class, t: usize) -> bool {
        let h = &class.hierarchy;
        let mut chain = vec![t];
        chain.extend(h.ancestors(t));
        let may_be = |s: &usize| h.is_subtype(t, *s);
        chain.iter().all(|c| !self.escaping.contains(c))
            && !self.written.iter().any(may_be)
            && chain.iter().all(|&c| !self.written_in_inits[c].iter().any(may_be))
    }
}

/// The uses of `this` in one method or initializer.
#[derive(Default)]
struct ThisUses {
    /// Whether the code is a constructor.
    ctor: bool,
    /// `this` nodes that are a field object, a method receiver or a constructor's write target.
    allowed: HashSet<ExprId>,
    /// Every `this` node.
    selves: Vec<ExprId>,
    /// Whether it writes a field of `this` outside a constructor.
    writes_outside_ctor: bool,
}

impl ThisUses {
    /// Records the uses of `this` in `x`, and adds to `written` the type of the object other than
    /// `this` whose field `x` writes.
    fn expr(&mut self, class: &Class, x: &Expr, written: &mut HashSet<usize>) {
        match &x.kind {
            ExprKind::This => self.selves.push(x.id),
            ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This) => {
                self.allowed.insert(object.id);
            }
            ExprKind::MethodCall { receiver, .. } if matches!(receiver.kind, ExprKind::This) => {
                self.allowed.insert(receiver.id);
            }
            ExprKind::Assign { target: Place::Field { object, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => {
                if !matches!(object.kind, ExprKind::This) {
                    written.extend(class.class_index(object.ty));
                } else if self.ctor {
                    self.allowed.insert(object.id);
                } else {
                    self.writes_outside_ctor = true;
                }
            }
            _ => {}
        }
    }
}
