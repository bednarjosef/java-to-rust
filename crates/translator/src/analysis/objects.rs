//! Immutable classes: a class whose fields are written only by its own constructors (and those
//! of its superclasses), through `this`, and whose `this` never escapes. Its objects never
//! change after construction, so they can be shared as `Rc<C>` without a `RefCell` to borrow.
//! The classes of one class tree are immutable together or not at all, so that an enum over
//! them reaches the fields they share in one way.

use crate::ast::*;
use std::collections::HashSet;

/// The qualified names of the classes whose objects never change once constructed.
pub fn immutable_classes(class: &Class) -> HashSet<String> {
    let alone: Vec<bool> = (0..class.types.len()).map(|t| is_immutable(class, t)).collect();
    (0..class.types.len())
        .filter(|&t| {
            let family = class.hierarchy.family(t);
            alone[t] && family.iter().all(|&c| alone[c])
        })
        .map(|t| class.types[t].name.clone())
        .collect()
}

fn is_immutable(class: &Class, t: usize) -> bool {
    let h = &class.hierarchy;
    let mut chain = vec![t];
    chain.extend(h.ancestors(t));
    let mut scan = Scan {
        class,
        t,
        ctor: false,
        runs_on: false,
        immutable: true,
        allowed: HashSet::new(),
        selves: Vec::new(),
    };
    for m in &class.methods {
        // Constructors of the class and of its superclasses build its objects.
        scan.ctor = m.is_ctor && chain.contains(&m.class);
        scan.runs_on = !m.is_static && chain.contains(&m.class);
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| scan.expr(x)), &mut |_| {});
        }
    }
    scan.ctor = false;
    scan.runs_on = false;
    for &c in &chain {
        for f in &class.types[c].fields {
            if let Some(init) = &f.init {
                visit_expr(init, &mut |x| scan.expr(x));
            }
        }
    }
    scan.immutable && scan.selves.iter().all(|id| scan.allowed.contains(id))
}

/// The state of the scan of one class's objects.
struct Scan<'a> {
    class: &'a Class,
    /// The class being checked.
    t: usize,
    /// Whether the code being scanned is a constructor that builds objects of the class.
    ctor: bool,
    /// Whether the code being scanned is an instance method or constructor that runs on objects of
    /// the class: one of the class or of a superclass.
    runs_on: bool,
    /// Cleared by anything that writes a field outside the constructors, or delegates with `this(...)`.
    immutable: bool,
    /// `this` nodes that are a field object, a method receiver or a constructor's write target.
    allowed: HashSet<ExprId>,
    /// Every `this` node that may be an object of the class.
    selves: Vec<ExprId>,
}

impl Scan<'_> {
    /// Whether a value of type `ty` may be an object of the class.
    fn may_be(&self, ty: Type) -> bool {
        self.class.class_index(ty).is_some_and(|s| self.class.hierarchy.is_subtype(self.t, s))
    }

    fn expr(&mut self, x: &Expr) {
        match &x.kind {
            ExprKind::This if self.runs_on => self.selves.push(x.id),
            ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This) => {
                self.allowed.insert(object.id);
            }
            ExprKind::MethodCall { receiver, .. } if matches!(receiver.kind, ExprKind::This) => {
                self.allowed.insert(receiver.id);
            }
            ExprKind::ThisCall { .. } if self.ctor => self.immutable = false,
            ExprKind::Assign { target: Place::Field { object, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => self.write(object),
            _ => {}
        }
    }

    /// A write to a field of an object that may be of the class: allowed only through `this` in
    /// a constructor.
    fn write(&mut self, object: &Expr) {
        if matches!(object.kind, ExprKind::This) {
            if !self.runs_on {
                return;
            }
            if self.ctor {
                self.allowed.insert(object.id);
                return;
            }
        } else if !self.may_be(object.ty) {
            return;
        }
        self.immutable = false;
    }
}
