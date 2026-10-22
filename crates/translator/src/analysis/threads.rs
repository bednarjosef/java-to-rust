//! Which programs run more than one thread, and which of their classes two threads may reach
//! (see `docs/multi-threading.md`, section 4).
//!
//! A program that never starts a thread is translated as before: its statics are thread-local
//! cells and its objects `Rc<RefCell<..>>`, which cost nothing to share. One that starts a thread
//! has process-wide statics, `main` waits for the threads it started before the program ends,
//! and the objects that may be reached from two threads are `Arc<java::Locked<..>>` (or `Arc<..>`
//! when immutable).
//!
//! Which objects two threads may reach is decided by type: a thread reaches its `Thread` object
//! and the statics, and from an object, whatever its fields may hold. So the classes of the
//! `Thread` family and the types of the statics are shared, and so are the types of the fields of
//! a shared class, with every class of the family of a shared class (the enum of a family holds
//! all of its classes the same way). A class that no thread can reach keeps its `Rc`.
//!
//! The data lock of a shared object is held only while its fields are read or written: a method
//! that borrows its object (`&self`, `&mut self`) holds the lock for the whole call, so in a
//! shared class only a short method may: one without loops or calls, that touches no other
//! object and does nothing that blocks (see `long_methods`). Any other method takes the object's
//! handle and locks it for each access, as Java reads a field each time, so a thread waiting in
//! `join` never holds a lock that the thread it waits for needs.

use super::Facts;
use crate::ast::*;
use std::collections::HashSet;

/// Whether the program starts a thread: some method calls the native that starts one, which
/// only `Thread.start()` does.
pub(super) fn threaded(class: &Class) -> bool {
    class.methods.iter().any(|m| {
        any_expr(&m.body, &mut |x| {
            matches!(x.kind, ExprKind::Builtin(Builtin::Native(Native::ThreadStart), _))
        })
    })
}

/// Whether the program interrupts a thread: some method calls the native of `Thread.interrupt()`.
/// Until one does, no `sleep`, `join` or `wait` throws `InterruptedException`.
pub(super) fn interrupts(class: &Class) -> bool {
    class.methods.iter().any(|m| {
        any_expr(&m.body, &mut |x| {
            matches!(x.kind, ExprKind::Builtin(Builtin::Native(Native::ThreadInterrupt), _))
        })
    })
}

/// Whether the program sets an uncaught-exception handler: some method calls the native that
/// gives the runtime the dispatch to the handlers (`Native::InstallUncaught`).
pub(super) fn handles_uncaught(class: &Class) -> bool {
    class.methods.iter().any(|m| {
        any_expr(&m.body, &mut |x| {
            matches!(x.kind, ExprKind::Builtin(Builtin::Native(Native::InstallUncaught), _))
        })
    })
}

/// The classes, by type index, whose objects two threads may reach in a threaded program.
pub(super) fn thread_shared(class: &Class) -> HashSet<usize> {
    let h = &class.hierarchy;
    let mut work = Vec::new();
    if let Some(thread) = class.type_index("java.lang.Thread") {
        work.push(thread);
    }
    for decl in &class.types {
        for f in &decl.statics {
            types_of(class, f.ty, &mut work);
        }
    }
    // The objects that become values of each type, as the representation's `flows` finds them
    // but with every method that a class inherits run on its whole object: as many or more.
    let flows = super::hierarchy::flows(class, &HashSet::new(), &Default::default());
    let mut shared = HashSet::new();
    while let Some(t) = work.pop() {
        if !shared.insert(t) {
            continue;
        }
        let decl = &class.types[t];
        // The whole class tree, whose enum borrows each object's fields struct the same way, and
        // every class whose objects become values of an interface: its enum's variants are among
        // them (see `hierarchy::variants`). An interface's enum, and `Object`'s, has no fields,
        // so their other classes may stay thread-local.
        let object = class.type_index("java.lang.Object");
        work.extend(h.ancestors(t).into_iter().filter(|&a| Some(a) != object));
        if decl.kind == TypeKind::Interface {
            work.extend(flows[t].iter().copied());
        } else {
            work.extend(h.subtypes[t].iter().copied());
        }
        for f in &decl.fields {
            types_of(class, f.ty, &mut work);
        }
        for c in &decl.captures {
            types_of(class, c.ty, &mut work);
        }
    }
    // A Java enum's values are plain Rust values, which every thread may hold.
    shared.retain(|&t| class.types[t].enum_constants.is_none());
    shared
}

/// The classes that a value of type `ty` may be an object of.
fn types_of(class: &Class, ty: Type, out: &mut Vec<usize>) {
    match ty {
        Type::Array(element) => types_of(class, *element, out),
        Type::Class(_, args) => {
            out.extend(class.class_index(ty));
            for &a in args {
                types_of(class, a, out);
            }
        }
        _ => {}
    }
}

/// The methods of shared classes that must not hold their object's lock for the whole call:
/// those with a loop, a call of a method or constructor, an access to an object or array other
/// than `this`'s fields, or a native that blocks.
pub(super) fn long_methods(class: &Class, shared: &HashSet<usize>) -> HashSet<usize> {
    (0..class.methods.len())
        .filter(|&m| {
            let method = &class.methods[m];
            if !shared.contains(&method.class) {
                return false;
            }
            let long = std::cell::Cell::new(false);
            for s in &method.body {
                visit_stmt(
                    s,
                    &mut |e| visit_expr(e, &mut |x| long.set(long.get() || lingers(x))),
                    &mut |s| {
                        let looping = matches!(
                            s,
                            Stmt::While { .. }
                                | Stmt::DoWhile { .. }
                                | Stmt::For { .. }
                                | Stmt::ForEach { .. }
                        );
                        long.set(long.get() || looping);
                    },
                );
            }
            long.get()
        })
        .collect()
}

/// Whether evaluating `x` itself may run for long, wait, or take another object's lock.
fn lingers(x: &Expr) -> bool {
    let other = |object: &Expr| !matches!(object.kind, ExprKind::This);
    match &x.kind {
        ExprKind::Call { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::New { .. }
        | ExprKind::ThisCall { .. }
        | ExprKind::SuperInit { .. }
        | ExprKind::Index(..)
        | ExprKind::Length(_)
        | ExprKind::Switch { .. } => true,
        ExprKind::Field { object, .. } => other(object),
        ExprKind::Assign { target, .. } | ExprKind::IncDec { target, .. } => match target {
            Place::Field { object, .. } => other(object),
            Place::Index(..) => true,
            Place::Var(_) | Place::Static { .. } => false,
        },
        // Entering a monitor waits for the thread that holds it.
        ExprKind::Builtin(Builtin::MonitorEnter, _) => true,
        ExprKind::Builtin(Builtin::Native(native), _) => matches!(
            native,
            Native::Sleep { .. }
                | Native::Park
                | Native::MonitorWait
                | Native::ThreadJoin
                | Native::ThreadStart
                | Native::FileRead
                | Native::FileReadBytes
        ),
        _ => false,
    }
}

/// Rejects a value that threads may share and that has no shared form yet: a `StringBuilder`, or
/// a box whose identity the program observes, in a static or in a field of a shared class. Both
/// are `Rc`s, which rustc would reject in a value another thread may hold. Every such field is
/// reported.
pub(super) fn check(class: &Class, facts: &Facts) -> Result<(), Vec<Error>> {
    let unshareable = |ty: Type| {
        let mut ty = ty;
        while let Type::Array(element) = ty {
            ty = *element;
        }
        match ty {
            Type::StringBuilder => Some("a `StringBuilder`"),
            Type::Boxed(_) if facts.faithful_boxes.contains(&ty) => Some("a box compared by identity"),
            _ => None,
        }
    };
    let mut errors = Vec::new();
    for (t, decl) in class.types.iter().enumerate() {
        let fields = if facts.thread_shared.contains(&t) { &decl.fields[..] } else { &[] };
        for f in decl.statics.iter().chain(fields) {
            if let Some(what) = unshareable(f.ty) {
                errors.push(Error {
                    pos: f.pos,
                    message: format!("{what} that threads may share not supported yet"),
                });
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Whether `f` holds for some expression in `body`.
fn any_expr(body: &[Stmt], f: &mut dyn FnMut(&Expr) -> bool) -> bool {
    let mut found = false;
    for s in body {
        visit_stmt(s, &mut |e| visit_expr(e, &mut |x| found |= f(x)), &mut |_| {});
    }
    found
}
