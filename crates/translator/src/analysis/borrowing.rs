//! Methods that borrow their object. A shared object is an `Rc<RefCell<C>>`; a method that only
//! reads and writes the object's fields and calls such methods on it can borrow it for the whole
//! call, as `&self` or `&mut self`, and read its fields as `self.f`. That is only right when
//! nothing the method runs reaches the same object through another handle: the `RefCell` would
//! refuse the second borrow. So a method borrows its object when
//! - it uses `this` only to read or write a field or to call a method that borrows too, never as
//!   a value (passed, returned, stored or compared), and
//! - nothing it runs, directly or through the methods and constructors it calls, touches an
//!   object of its own class through a handle other than `this`.
//!
//! Methods of immutable and owned classes always borrow: an immutable object has no `RefCell`,
//! and an owned object has no other handle.
//!
//! A constructor builds a plain value and the caller wraps it, when the constructors of its class
//! use `this` the same way (no other code can reach an object under construction).

use crate::ast::*;
use std::collections::HashSet;

#[derive(Default)]
pub struct Borrowing {
    /// Instance methods (indexes into `Class::methods`) that take `&self` or `&mut self`.
    pub methods: HashSet<usize>,
    /// Instance methods that change their object: they take `&mut self` when they borrow it.
    pub mutating: HashSet<usize>,
    /// Classes, by qualified name, whose constructors build a plain value.
    pub plain_constructors: HashSet<String>,
    /// Methods and constructors that touch no object but their own `this`, directly or through
    /// what they call.
    pub isolated: HashSet<usize>,
}

pub fn borrowing(class: &Class, immutable: &HashSet<String>, owned: &HashSet<String>) -> Borrowing {
    let scans: Vec<Scan> = class.methods.iter().map(|m| scan(class, m)).collect();
    // The classes whose objects each method may touch through a handle other than its `this`.
    let mut reach: Vec<HashSet<&str>> = scans.iter().map(|s| s.touches.clone()).collect();
    loop {
        let mut changed = false;
        for m in 0..scans.len() {
            for &(callee, _) in &scans[m].calls {
                let extra: Vec<&str> = reach[callee].difference(&reach[m]).copied().collect();
                changed |= !extra.is_empty();
                reach[m].extend(extra);
            }
        }
        if !changed {
            break;
        }
    }
    let class_of = |m: usize| class.types[class.methods[m].class].name.as_str();
    let always = |m: usize| {
        let decl = &class.types[class.methods[m].class];
        immutable.contains(class_of(m)) || owned.contains(class_of(m)) || decl.enum_constants.is_some()
    };
    let mut methods: HashSet<usize> = (0..scans.len())
        .filter(|&m| {
            let method = &class.methods[m];
            !method.is_static
                && !method.is_ctor
                && (always(m) || (!scans[m].this_as_value && !reach[m].contains(class_of(m))))
        })
        .collect();
    // A method that calls a method on `this` that cannot borrow cannot borrow either.
    loop {
        let losing: Vec<usize> = methods
            .iter()
            .copied()
            .filter(|&m| {
                !always(m) && scans[m].calls.iter().any(|&(c, on_this)| on_this && !methods.contains(&c))
            })
            .collect();
        if losing.is_empty() {
            break;
        }
        for m in losing {
            methods.remove(&m);
        }
    }
    let mut mutating: HashSet<usize> = (0..scans.len()).filter(|&m| scans[m].writes_this).collect();
    loop {
        let added: Vec<usize> = (0..scans.len())
            .filter(|m| {
                !mutating.contains(m)
                    && scans[*m].calls.iter().any(|&(c, on_this)| on_this && mutating.contains(&c))
            })
            .collect();
        if added.is_empty() {
            break;
        }
        mutating.extend(added);
    }
    let plain_constructors = (0..class.types.len())
        .filter(|&t| {
            let ctors =
                || (0..scans.len()).filter(move |&m| class.methods[m].is_ctor && class.methods[m].class == t);
            let initializers_ok = class.types[t].fields.iter().filter_map(|f| f.init.as_ref()).all(|init| {
                let mut s = Scan::default();
                s.expr(class, init);
                !s.this_as_value && s.calls.iter().all(|&(c, on_this)| !on_this || methods.contains(&c))
            });
            initializers_ok
                && ctors().all(|m| {
                    !scans[m].this_as_value
                        && scans[m]
                            .calls
                            .iter()
                            .all(|&(c, on_this)| !on_this || methods.contains(&c) || class.methods[c].is_ctor)
                })
        })
        .map(|t| class.types[t].name.clone())
        .collect();
    let isolated = (0..scans.len()).filter(|&m| reach[m].is_empty()).collect();
    Borrowing { methods, mutating, plain_constructors, isolated }
}

/// What one method does with objects.
#[derive(Default)]
struct Scan<'c> {
    /// Classes whose objects the method touches through a handle other than `this`.
    touches: HashSet<&'c str>,
    /// Methods and constructors it calls, and whether on `this` (`this(...)` counts as on `this`).
    calls: Vec<(usize, bool)>,
    /// Whether `this` is used other than to read or write a field or to receive a call.
    this_as_value: bool,
    /// Whether it writes a field of `this`.
    writes_this: bool,
}

fn scan<'c>(class: &'c Class, m: &Method) -> Scan<'c> {
    let mut s = Scan::default();
    for stmt in &m.body {
        visit_stmt(stmt, &mut |e| s.expr(class, e), &mut |_| {});
    }
    s
}

impl<'c> Scan<'c> {
    fn expr(&mut self, class: &'c Class, e: &Expr) {
        // `this` nodes in an allowed position, so that the remaining ones are values.
        let mut allowed = HashSet::new();
        let mut selves = Vec::new();
        visit_expr(e, &mut |x| {
            let is_this = |o: &Expr| matches!(o.kind, ExprKind::This);
            match &x.kind {
                ExprKind::This => selves.push(x.id),
                ExprKind::Field { object, .. } => {
                    if is_this(object) {
                        allowed.insert(object.id);
                    } else {
                        self.touch(class, object.ty);
                    }
                }
                ExprKind::Assign { target: Place::Field { object, .. }, .. }
                | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => {
                    if is_this(object) {
                        allowed.insert(object.id);
                        self.writes_this = true;
                    } else {
                        self.touch(class, object.ty);
                    }
                }
                ExprKind::MethodCall { receiver, resolved: Some(callee), .. } => {
                    let on_this = is_this(receiver);
                    if on_this {
                        allowed.insert(receiver.id);
                    } else if !class.methods[*callee].is_static {
                        self.touch(class, receiver.ty);
                    }
                    self.calls.push((*callee, on_this));
                }
                ExprKind::Call { resolved: Some(callee), .. } | ExprKind::New { ctor: Some(callee), .. } => {
                    self.calls.push((*callee, false));
                }
                ExprKind::ThisCall { resolved: Some(callee), .. } => self.calls.push((*callee, true)),
                // Printing an object or concatenating it calls its `toString`.
                ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                    for a in args {
                        self.text(class, a, &mut allowed);
                    }
                }
                ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => {
                    self.text(class, l, &mut allowed);
                    self.text(class, r, &mut allowed);
                }
                _ => {}
            }
        });
        self.this_as_value |= selves.iter().any(|id| !allowed.contains(id));
    }

    /// An operand turned into text: an object's `toString` runs on it.
    fn text(&mut self, class: &'c Class, e: &Expr, allowed: &mut HashSet<ExprId>) {
        let Type::Class(name) = e.ty else { return };
        let on_this = matches!(e.kind, ExprKind::This);
        if on_this {
            allowed.insert(e.id);
        } else {
            self.touch(class, e.ty);
        }
        if let Some(index) = class.methods.iter().position(|m| {
            class.types[m.class].name == name && m.name == "toString" && m.params.is_empty() && !m.is_static
        }) {
            self.calls.push((index, on_this));
        }
    }

    fn touch(&mut self, class: &'c Class, ty: Type) {
        if let Type::Class(name) = ty
            && let Some(t) = class.types.iter().find(|t| t.name == name)
        {
            self.touches.insert(t.name.as_str());
        }
    }
}
