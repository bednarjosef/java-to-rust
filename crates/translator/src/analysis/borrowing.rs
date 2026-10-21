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
//! and an owned object has no other handle. So do the methods that live on a superclass's
//! fields struct (see `hierarchy::field_level`).
//!
//! A constructor builds a plain value and the caller wraps it, when the constructors of its class
//! use `this` the same way (no other code can reach an object under construction).
//!
//! A call may run any implementation of the method it names, so the reach of a method covers all
//! of them, and touching an object of a class type touches every concrete class under it.

use super::Facts;
use super::hierarchy::{Home, home, to_string_impl};
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
    /// Methods and constructors that touch no shared object but their own `this`, directly or
    /// through what they call.
    pub isolated: HashSet<usize>,
    /// Methods and constructors that may write a field of a shared object, directly or through
    /// what they call, so may borrow one mutably. A constructor's writes to its own `this` do not
    /// count: no other code can reach that object yet.
    pub writers: HashSet<usize>,
}

/// What each method does with objects, and the classes whose objects each may touch through a
/// handle other than its `this`, directly or through what it calls.
pub struct Reach<'c> {
    scans: Vec<Scan<'c>>,
    pub reach: Vec<HashSet<&'c str>>,
}

pub fn reach(class: &Class) -> Reach<'_> {
    let scans: Vec<Scan> = class.methods.iter().map(|m| scan(class, m)).collect();
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
    Reach { scans, reach }
}

/// The implementation that a call on `this` runs in method `m` whose object is of concrete class
/// `c` (`None` when the object may be of several classes).
fn this_target(class: &Class, call: ThisCall, c: Option<usize>) -> Option<usize> {
    let h = &class.hierarchy;
    match call {
        ThisCall::Method { resolved, is_super: true } => Some(resolved),
        ThisCall::Method { resolved, is_super: false } => match c {
            Some(c) => h.implementation(resolved, c),
            None if !h.is_overridden(resolved) && !class.methods[resolved].is_abstract => Some(resolved),
            None => None,
        },
        ThisCall::Text => c.and_then(|c| to_string_impl(class, c)),
    }
}

pub fn borrowing(class: &Class, facts: &Facts, reach: &Reach) -> Borrowing {
    let Reach { scans, reach } = reach;
    let h = &class.hierarchy;
    let name = |t: usize| class.types[t].name.as_str();
    let always = |t: usize| {
        facts.immutable.contains(name(t))
            || facts.owned.contains(name(t))
            || class.types[t].enum_constants.is_some()
    };
    let homes: Vec<Home> = (0..class.methods.len()).map(|m| home(class, facts, m)).collect();
    let mut methods: HashSet<usize> = (0..scans.len())
        .filter(|&m| match homes[m] {
            _ if class.methods[m].is_ctor => false,
            Home::Class(c) => always(c) || (!scans[m].this_as_value && !reach[m].contains(name(c))),
            Home::Fields(_) => true,
            _ => false,
        })
        .collect();
    // A method that calls a method on `this` that cannot borrow cannot borrow either.
    loop {
        let losing: Vec<usize> = methods
            .iter()
            .copied()
            .filter(|&m| {
                let Home::Class(c) = homes[m] else { return false };
                !always(c)
                    && scans[m]
                        .this_calls
                        .iter()
                        .any(|&call| this_target(class, call, Some(c)).is_some_and(|t| !methods.contains(&t)))
            })
            .collect();
        if losing.is_empty() {
            break;
        }
        for m in losing {
            methods.remove(&m);
        }
    }
    // A call may run any method of an override group, so the group changes its object, or writes
    // or touches other objects, when one of its methods does; and so does a method that calls one.
    let spread = |set: &HashSet<usize>| -> HashSet<usize> { set.iter().flat_map(|&m| h.group(m)).collect() };
    let mut mutating: HashSet<usize> = (0..scans.len()).filter(|&m| scans[m].writes_this).collect();
    loop {
        mutating = spread(&mutating);
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
    // A call on `this` in a constructor of `t` (or its field initializers) keeps the object a plain
    // value when it runs a method that borrows the struct being built: one emitted on `t`'s own
    // struct or on the fields struct of `t` or a superclass.
    let plain_call = |t: usize, call: ThisCall| {
        let leaf = !h.has_subtypes(t) && h.is_concrete(t);
        let Some(target) = this_target(class, call, leaf.then_some(t)) else { return false };
        methods.contains(&target)
            && match homes[target] {
                Home::Class(c) => c == t,
                Home::Fields(d) => d == t || h.ancestors(t).contains(&d),
                _ => false,
            }
    };
    let plain_constructors = (0..class.types.len())
        .filter(|&t| {
            let ctors =
                || (0..scans.len()).filter(move |&m| class.methods[m].is_ctor && class.methods[m].class == t);
            let initializers_ok = class.types[t].fields.iter().filter_map(|f| f.init.as_ref()).all(|init| {
                let mut s = Scan::default();
                s.expr(class, init);
                !s.this_as_value && s.this_calls.iter().all(|&call| plain_call(t, call))
            });
            initializers_ok
                && ctors().all(|m| {
                    !scans[m].this_as_value && scans[m].this_calls.iter().all(|&call| plain_call(t, call))
                })
        })
        .map(|t| class.types[t].name.clone())
        .collect();
    // Immutable and owned objects have no `RefCell` to borrow.
    let shared = |c: &str| !facts.immutable.contains(c) && !facts.owned.contains(c);
    let isolated = (0..scans.len()).filter(|&m| !reach[m].iter().any(|c| shared(c))).collect();
    let mut writers: HashSet<usize> = (0..scans.len())
        .filter(|&m| {
            let method = &class.methods[m];
            let own_shared = || {
                let t = method.class;
                shared(name(t)) && (h.is_concrete(t) || class.types[t].enum_constants.is_some())
                    || h.concretes(t).into_iter().any(|c| shared(name(c)))
            };
            (scans[m].writes_this && !method.is_ctor && own_shared())
                || scans[m].writes.iter().any(|c| shared(c))
        })
        .collect();
    loop {
        writers = spread(&writers);
        let added: Vec<usize> = (0..scans.len())
            .filter(|m| !writers.contains(m) && scans[*m].calls.iter().any(|(c, _)| writers.contains(c)))
            .collect();
        if added.is_empty() {
            break;
        }
        writers.extend(added);
    }
    // A method of an immutable class changes nothing: its object has no `RefCell` to borrow
    // mutably, whatever the other methods of its group do.
    let mutating =
        mutating.into_iter().filter(|&m| !facts.immutable.contains(name(class.methods[m].class))).collect();
    let isolated: HashSet<usize> = isolated;
    let isolated =
        isolated.iter().copied().filter(|&m| h.group(m).iter().all(|g| isolated.contains(g))).collect();
    Borrowing { methods, mutating, plain_constructors, isolated, writers }
}

/// A call on `this`: of a method (as the call names it), or of `toString` by turning `this` into
/// text.
#[derive(Clone, Copy)]
enum ThisCall {
    Method { resolved: usize, is_super: bool },
    Text,
}

/// What one method does with objects.
#[derive(Default)]
struct Scan<'c> {
    /// Classes whose objects the method touches through a handle other than `this`.
    touches: HashSet<&'c str>,
    /// Methods and constructors it may run, and whether on `this` (`this(...)` and `super(...)`
    /// count as on `this`).
    calls: Vec<(usize, bool)>,
    /// The method calls on `this`, other than `this(...)` and `super(...)`.
    this_calls: Vec<ThisCall>,
    /// Whether `this` is used other than to read or write a field or to receive a call.
    this_as_value: bool,
    /// Whether it writes a field of `this`.
    writes_this: bool,
    /// Classes whose objects' fields it writes through a handle other than `this`.
    writes: HashSet<&'c str>,
}

fn scan<'c>(class: &'c Class, m: &Method) -> Scan<'c> {
    let mut s = Scan::default();
    for stmt in &m.body {
        visit_stmt(stmt, &mut |e| s.expr(class, e), &mut |_| {});
    }
    s
}

/// The classes an object of type `ty` may be of: the type itself and the concrete classes
/// under it, by qualified name.
fn classes(class: &Class, ty: Type) -> Vec<&str> {
    let Some(t) = class.class_index(ty) else { return Vec::new() };
    let mut out = vec![class.types[t].name.as_str()];
    out.extend(class.hierarchy.concretes(t).into_iter().map(|c| class.types[c].name.as_str()));
    out
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
                        self.writes.extend(classes(class, object.ty));
                    }
                }
                ExprKind::MethodCall { receiver, resolved: Some(callee), is_super, .. } => {
                    let on_this = is_this(receiver);
                    if on_this {
                        allowed.insert(receiver.id);
                        self.this_calls.push(ThisCall::Method { resolved: *callee, is_super: *is_super });
                    } else if !class.methods[*callee].is_static {
                        self.touch(class, receiver.ty);
                    }
                    self.calls.extend(class.callees(x).into_iter().map(|c| (c, on_this)));
                }
                ExprKind::Call { resolved: Some(callee), .. } | ExprKind::New { ctor: Some(callee), .. } => {
                    self.calls.push((*callee, false));
                }
                ExprKind::ThisCall { resolved: Some(callee), .. }
                | ExprKind::SuperInit { resolved: Some(callee), .. } => self.calls.push((*callee, true)),
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
        let Some(t) = class.class_index(e.ty) else { return };
        let on_this = matches!(e.kind, ExprKind::This);
        if on_this {
            allowed.insert(e.id);
            self.this_calls.push(ThisCall::Text);
        } else {
            self.touch(class, e.ty);
        }
        let mut runs: Vec<usize> = class.hierarchy.concretes(t);
        if !runs.contains(&t) {
            runs.push(t);
        }
        for c in runs {
            if let Some(index) = to_string_impl(class, c) {
                self.calls.push((index, on_this));
            }
        }
    }

    fn touch(&mut self, class: &'c Class, ty: Type) {
        self.touches.extend(classes(class, ty));
    }
}
