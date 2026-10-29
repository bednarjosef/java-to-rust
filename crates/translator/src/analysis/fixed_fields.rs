//! Fixed fields: the instance fields that only constructors write, through `this` (Java's `final`
//! fields, and others written the same way). Once its object is built such a field keeps its
//! value, so a read of it borrows the field alone, without the object's lock or `RefCell` borrow
//! and without copying its value out (`java::fixed!`, see the runtime's `fixed.rs`).
//!
//! In a program with threads, a constructor that may start a thread could hand `this` to it
//! before it writes the field, and the thread's read, which takes no lock, would race with the
//! write. So the fields of a class one of whose constructors, or one of whose subclasses'
//! constructors, may start a thread are not fixed. (Java promises nothing for a `final` field
//! that another thread reads while the constructor runs either; here the read must not race.)

use super::FastSet;
use crate::ast::*;

/// The fixed fields, by the type that declares them and their name.
pub fn fixed_fields(class: &Class, threaded: bool) -> FastSet<(usize, String)> {
    let mut written = FastSet::default();
    let mut written_names = FastSet::default();
    let mut note = |x: &Expr, in_ctor: bool| {
        let (ExprKind::Assign { target: Place::Field { object, name }, .. }
        | ExprKind::IncDec { target: Place::Field { object, name }, .. }) = &x.kind
        else {
            return;
        };
        if in_ctor && matches!(object.kind, ExprKind::This) {
            return;
        }
        match class.field_owner(object.ty, name) {
            Some(owner) => written.insert((owner, name.clone())),
            // An object of no class the program declares: any field of the name may be the one.
            None => written_names.insert(name.clone()),
        };
    };
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| note(x, m.is_ctor)), &mut |_| {});
        }
    }
    // A field initializer runs in the constructor, on the object it builds.
    for init in class.types.iter().flat_map(|t| &t.fields).filter_map(|f| f.init.as_ref()) {
        visit_expr(init, &mut |x| note(x, true));
    }
    let unsafe_classes = if threaded { constructors_starting_threads(class) } else { FastSet::default() };
    let mut fixed = FastSet::default();
    for (t, decl) in class.types.iter().enumerate() {
        if unsafe_classes.contains(&t) {
            continue;
        }
        for f in &decl.fields {
            if !written_names.contains(&f.name) && !written.contains(&(t, f.name.clone())) {
                fixed.insert((t, f.name.clone()));
            }
        }
    }
    fixed
}

/// The classes that a constructor which may start a thread builds objects of: the class of the
/// constructor, and its superclasses, whose fields the object has too.
fn constructors_starting_threads(class: &Class) -> FastSet<usize> {
    // Who calls each method, then every method from which a call chain reaches `Thread.start`.
    let mut callers: Vec<Vec<usize>> = vec![Vec::new(); class.methods.len()];
    let mut starting = vec![false; class.methods.len()];
    let mut work = Vec::new();
    for (m, method) in class.methods.iter().enumerate() {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if matches!(x.kind, ExprKind::Builtin(Builtin::Native(Native::ThreadStart), _))
                            && !starting[m]
                        {
                            starting[m] = true;
                            work.push(m);
                        }
                        for callee in class.callees(x) {
                            callers[callee].push(m);
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    while let Some(m) = work.pop() {
        for &caller in &callers[m] {
            if !starting[caller] {
                starting[caller] = true;
                work.push(caller);
            }
        }
    }
    // A subclass's constructor calls its superclass's, so it starts a thread too when that one
    // does, and so does a field initializer that calls such a method.
    let mut builders: FastSet<usize> = class
        .methods
        .iter()
        .enumerate()
        .filter(|&(m, method)| method.is_ctor && starting[m])
        .map(|(_, method)| method.class)
        .collect();
    for (t, decl) in class.types.iter().enumerate() {
        for init in decl.fields.iter().filter_map(|f| f.init.as_ref()) {
            visit_expr(init, &mut |x| {
                if matches!(x.kind, ExprKind::Builtin(Builtin::Native(Native::ThreadStart), _))
                    || class.callees(x).iter().any(|&m| starting[m])
                {
                    builders.insert(t);
                }
            });
        }
    }
    let mut out = FastSet::default();
    for t in builders {
        out.insert(t);
        out.extend(class.hierarchy.ancestors(t));
    }
    out
}
