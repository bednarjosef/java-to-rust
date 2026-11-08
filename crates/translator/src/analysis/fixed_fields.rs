//! Fixed fields: the instance fields that only constructors write, through `this` (Java's `final`
//! fields, and others written the same way). Once its object is built such a field keeps its
//! value, so a read of it borrows the field alone, without the object's lock or `RefCell` borrow
//! and without copying its value out (`java::fixed!`, see the runtime's `fixed.rs`).
//!
//! In a program with threads, a constructor that may start a thread could hand `this` to it
//! before it writes the field, and the thread's read, which takes no lock, would race with the
//! write. So the fields of a class one of whose constructors, or one of whose subclasses'
//! constructors, may start a thread are not fixed, but for those that only the class's own
//! constructors and field initializers write while `this` cannot have left: before anything uses
//! `this` as a value (passes, stores, returns or captures it, or calls a method on it), so until
//! then no other code, on any thread, holds the object (`written_late`). When no constructor or
//! field initializer of the class or of a superclass does, that is every write; a subclass's
//! constructor runs only once they are done. (Java promises nothing for a `final` field that
//! another thread reads while the constructor runs either; here the read must not race.)

use super::{FastMap, FastSet};
use crate::ast::*;

/// The fixed fields, by the type that declares them and their name.
pub fn fixed_fields(class: &Class, threaded: bool) -> FastSet<(usize, String)> {
    let mut written = FastSet::default();
    let mut written_names = FastSet::default();
    // The classes whose constructors (or field initializers) write each field through `this`.
    let mut writers: FastMap<(usize, String), FastSet<usize>> = FastMap::default();
    let mut note = |x: &Expr, in_ctor: Option<usize>| {
        let (ExprKind::Assign { target: Place::Field { object, name }, .. }
        | ExprKind::IncDec { target: Place::Field { object, name }, .. }) = &x.kind
        else {
            return;
        };
        if let Some(writer) = in_ctor
            && matches!(object.kind, ExprKind::This)
        {
            if let Some(owner) = class.field_owner(object.ty, name) {
                writers.entry((owner, name.clone())).or_default().insert(writer);
            }
            return;
        }
        match class.field_owner(object.ty, name) {
            Some(owner) => written.insert((owner, name.clone())),
            // An object of no class the program declares: any field of the name may be the one.
            None => written_names.insert(name.clone()),
        };
    };
    for m in &class.methods {
        let in_ctor = m.is_ctor.then_some(m.class);
        for s in &m.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| note(x, in_ctor)), &mut |_| {});
        }
    }
    // A field initializer runs in the constructor, on the object it builds.
    for (t, decl) in class.types.iter().enumerate() {
        for init in decl.fields.iter().filter_map(|f| f.init.as_ref()) {
            visit_expr(init, &mut |x| note(x, Some(t)));
        }
    }
    for (t, decl) in class.types.iter().enumerate() {
        for f in decl.fields.iter().filter(|f| f.init.is_some()) {
            writers.entry((t, f.name.clone())).or_default().insert(t);
        }
    }
    let unsafe_classes = if threaded { constructors_starting_threads(class) } else { FastSet::default() };
    let leaking = if unsafe_classes.is_empty() { FastSet::default() } else { leaking_this(class) };
    let late = if unsafe_classes.is_empty() { FastSet::default() } else { written_late(class, &leaking) };
    let mut fixed = FastSet::default();
    for (t, decl) in class.types.iter().enumerate() {
        // Whether the constructors that run before any other code can hold the object never let
        // `this` leave (see the module's comment).
        let kept =
            || !leaking.contains(&t) && !class.hierarchy.ancestors(t).iter().any(|a| leaking.contains(a));
        for f in &decl.fields {
            let key = (t, f.name.clone());
            if written_names.contains(&f.name) || written.contains(&key) {
                continue;
            }
            let own = writers.get(&key).is_none_or(|w| w.iter().all(|&c| c == t));
            if !unsafe_classes.contains(&t) || (own && (kept() || !late.contains(&key))) {
                fixed.insert(key);
            }
        }
    }
    fixed
}

/// The classes some of whose constructors or field initializers use `this` as a value: anything
/// but the object of a field's read or write, such as an argument, a stored value, the receiver
/// of a call or the enclosing instance of a nested class's object.
fn leaking_this(class: &Class) -> FastSet<usize> {
    let mut out = FastSet::default();
    for m in class.methods.iter().filter(|m| m.is_ctor) {
        let mut leaked = false;
        for s in &m.body {
            visit_stmt(s, &mut |e| leaked |= leaks_this(e), &mut |_| {});
        }
        if leaked {
            out.insert(m.class);
        }
    }
    for (t, decl) in class.types.iter().enumerate() {
        if decl.fields.iter().filter_map(|f| f.init.as_ref()).any(leaks_this) {
            out.insert(t);
        }
    }
    out
}

/// Whether `e` uses `this` as a value (see `leaking_this`).
fn leaks_this(e: &Expr) -> bool {
    let mut this: Vec<ExprId> = Vec::new();
    let mut fields: FastSet<ExprId> = FastSet::default();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::This => this.push(x.id),
        ExprKind::Field { object, .. }
        | ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => _ = fields.insert(object.id),
        _ => {}
    });
    this.iter().any(|id| !fields.contains(id))
}

/// The fields, as (declaring type, name), that a constructor or field initializer writes through
/// `this` after `this` may have left it, or in the statement that lets it leave. A constructor
/// runs its statements before `this(..)` or `super(..)` (javac's enclosing instance and captured
/// locals), then the superclasses' constructors, then the field initializers in order, then the
/// rest of its body; `this` may have left from the first of them that `leaking_this` would count,
/// or from a superclass that leaks it anywhere. A `this(..)` counts as leaking, as the constructor
/// it calls may. A field none of whose writes is here was written before any other code, on any
/// thread, could hold the object.
fn written_late(class: &Class, leaking: &FastSet<usize>) -> FastSet<(usize, String)> {
    let mut late = FastSet::default();
    // Notes the fields that statement or initializer `e` writes through `this` as late when
    // `this` may have left (`left`), after noting whether `e` lets it leave.
    let note = |e: &Expr, left: &mut bool, late: &mut FastSet<(usize, String)>| {
        *left |= leaks_this(e);
        if !*left {
            return;
        }
        visit_expr(e, &mut |x| {
            if let ExprKind::Assign { target: Place::Field { object, name }, .. }
            | ExprKind::IncDec { target: Place::Field { object, name }, .. } = &x.kind
                && matches!(object.kind, ExprKind::This)
                && let Some(owner) = class.field_owner(object.ty, name)
            {
                late.insert((owner, name.clone()));
            }
        });
    };
    // Whether `this` may have left once the superclasses' constructors and the field initializers
    // of each type have run.
    let mut after_inits = vec![false; class.types.len()];
    for (t, decl) in class.types.iter().enumerate() {
        let mut left = class.hierarchy.ancestors(t).iter().any(|a| leaking.contains(a));
        for f in &decl.fields {
            let Some(init) = &f.init else { continue };
            note(init, &mut left, &mut late);
            if left {
                late.insert((t, f.name.clone()));
            }
        }
        after_inits[t] = left;
    }
    for m in class.methods.iter().filter(|m| m.is_ctor) {
        let (before, call, after) = split_explicit_call(&m.body);
        let mut left = false;
        let stmts = |stmts: &[Stmt], left: &mut bool, late: &mut FastSet<(usize, String)>| {
            for s in stmts {
                let mut leaked = *left;
                visit_stmt(s, &mut |e| leaked |= leaks_this(e), &mut |_| {});
                *left = leaked;
                visit_stmt(s, &mut |e| note(e, left, late), &mut |_| {});
            }
        };
        stmts(before, &mut left, &mut late);
        match call.map(|c| &c.kind) {
            Some(ExprKind::ThisCall { .. }) => left = true,
            Some(ExprKind::SuperInit { args, .. }) => {
                left |= args.iter().any(leaks_this) || after_inits[m.class];
            }
            _ => left |= after_inits[m.class],
        }
        stmts(after, &mut left, &mut late);
    }
    late
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
