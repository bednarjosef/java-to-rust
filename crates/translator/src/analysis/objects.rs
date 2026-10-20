//! Immutable classes: a class whose fields are written only by its own constructors, through
//! `this`, and whose `this` never escapes. Its objects never change after construction, so they
//! can be shared as `Rc<C>` without a `RefCell` to borrow.

use crate::ast::*;
use std::collections::HashSet;

/// The qualified names of the classes whose objects never change once constructed.
pub fn immutable_classes(class: &Class) -> HashSet<String> {
    (0..class.types.len()).filter(|&t| is_immutable(class, t)).map(|t| class.types[t].name.clone()).collect()
}

fn is_immutable(class: &Class, t: usize) -> bool {
    let mut scan = Scan {
        name: &class.types[t].name,
        ctor: false,
        immutable: true,
        allowed: HashSet::new(),
        selves: Vec::new(),
    };
    for m in &class.methods {
        scan.ctor = m.is_ctor && m.class == t;
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| scan.expr(x)), &mut |_| {});
        }
    }
    scan.ctor = false;
    for f in &class.types[t].fields {
        if let Some(init) = &f.init {
            visit_expr(init, &mut |x| scan.expr(x));
        }
    }
    scan.immutable && scan.selves.iter().all(|id| scan.allowed.contains(id))
}

/// The state of the scan of one class's objects.
struct Scan<'a> {
    /// The qualified name of the class being checked.
    name: &'a str,
    /// Whether the code being scanned is a constructor of the class.
    ctor: bool,
    /// Cleared by anything that writes a field outside the constructors, or delegates with `this(...)`.
    immutable: bool,
    /// `this` nodes that are a field object, a method receiver or a constructor's write target.
    allowed: HashSet<ExprId>,
    /// Every `this` node of the class.
    selves: Vec<ExprId>,
}

impl Scan<'_> {
    fn is_class(&self, ty: Type) -> bool {
        matches!(ty, Type::Class(c) if c == self.name)
    }

    fn expr(&mut self, x: &Expr) {
        match &x.kind {
            ExprKind::This if self.is_class(x.ty) => self.selves.push(x.id),
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

    /// A write to a field of an object of the class: allowed only through `this` in a constructor.
    fn write(&mut self, object: &Expr) {
        if !self.is_class(object.ty) {
            return;
        }
        if self.ctor && matches!(object.kind, ExprKind::This) {
            self.allowed.insert(object.id);
        } else {
            self.immutable = false;
        }
    }
}
