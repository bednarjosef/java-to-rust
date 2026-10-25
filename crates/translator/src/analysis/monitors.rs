//! Which monitors the translation keeps (see docs/multi-threading.md, section 3).
//!
//! A monitor is kept where the program can tell it from none: on an object that threads share,
//! whose lock keeps other threads out of the region, and on every object of a program that
//! waits, notifies or asks `holdsLock`, which answer as Java's do only when the monitor is held.
//! Anywhere else `synchronized` is the null check of its lock, as it was, and a program without
//! threads or `wait` is translated as before.
//!
//! The objects whose monitors are kept need a handle with a lock or an address: their classes
//! are neither plain values (`owned`) nor immutable (whose shared form, `Arc<C>`, has no lock),
//! and the methods that use the monitor of `this` take their object's handle.

use super::Facts;
use super::hierarchy;
use crate::ast::*;
use std::collections::HashSet;

/// Whether the program waits, notifies or asks whether a thread holds a monitor.
pub(super) fn observed(class: &Class) -> bool {
    class.methods.iter().any(|m| {
        any_expr(&m.body, &mut |x| {
            matches!(
                x.kind,
                ExprKind::Builtin(
                    Builtin::Native(Native::MonitorWait | Native::MonitorNotify { .. } | Native::HoldsLock),
                    _
                )
            )
        })
    })
}

impl Facts {
    /// Whether the monitor of an object of type `ty` is kept: the program observes monitors, or
    /// threads may share the object, as a builder is where they may reach builders and an array
    /// is in a program with threads (one that a single thread uses keeps its monitor by address).
    pub fn kept_monitor(&self, class: &Class, ty: Type) -> bool {
        self.monitors
            || ty == Type::StringBuilder && self.shared_builders
            || matches!(ty, Type::Array(_)) && self.threaded
            || class.class_index(ty).is_some_and(|t| self.kept_monitor_of(t))
    }

    /// Whether the monitors of the objects of class `t` are kept.
    pub fn kept_monitor_of(&self, t: usize) -> bool {
        self.monitors || self.threaded && self.thread_shared.contains(&t)
    }

    /// Whether the monitors of classes are kept: those of static synchronized methods and of
    /// `synchronized (C.class)`, which every thread may reach.
    pub fn kept_class_monitors(&self) -> bool {
        self.monitors || self.threaded
    }
}

/// The objects whose monitors the program uses: the lock of each `synchronized` block, the
/// object of each synchronized instance method, and the object of each `wait`, `notify` and
/// `holdsLock`, with the method that uses it. A class's monitor (`ClassLiteral`) is not an
/// object's.
fn monitor_uses(class: &Class) -> Vec<(usize, Type, bool)> {
    let mut uses = Vec::new();
    for (m, method) in class.methods.iter().enumerate() {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Builtin(Builtin::MonitorEnter, args)
                            if !matches!(args[0].kind, ExprKind::Builtin(Builtin::ClassLiteral(_), _)) =>
                        {
                            uses.push((m, args[0].ty, matches!(args[0].kind, ExprKind::This)));
                        }
                        ExprKind::Builtin(
                            Builtin::Native(
                                Native::MonitorWait | Native::MonitorNotify { .. } | Native::HoldsLock,
                            ),
                            args,
                        ) => uses.push((m, args[0].ty, matches!(args[0].kind, ExprKind::This))),
                        _ => {}
                    })
                },
                &mut |_| {},
            );
        }
    }
    uses
}

/// The classes, by qualified name, whose objects' monitors are kept, and the methods that use
/// the monitor of `this` where it is kept, which must take their object's handle. Called once
/// `threaded`, `thread_shared` and `monitors` are known.
pub(super) fn kept(class: &Class, facts: &Facts) -> (HashSet<String>, HashSet<usize>) {
    let mut types: HashSet<usize> = HashSet::new();
    let mut methods = HashSet::new();
    let synchronized = class.methods.iter().enumerate().filter(|(_, m)| m.is_synchronized && !m.is_static);
    for (m, method) in synchronized {
        if facts.kept_monitor_of(method.class) {
            types.insert(method.class);
            methods.insert(m);
        }
    }
    for (m, ty, this) in monitor_uses(class) {
        if !facts.kept_monitor(class, ty) {
            continue;
        }
        if let Some(t) = class.class_index(ty) {
            types.insert(t);
        }
        if this {
            methods.insert(m);
        }
    }
    if types.is_empty() {
        return (HashSet::new(), methods);
    }
    // The objects that may be values of the locks' types.
    let flows = hierarchy::flows(class, &HashSet::new(), &Default::default());
    let classes = types
        .iter()
        .flat_map(|&t| std::iter::once(t).chain(flows[t].iter().copied()))
        .map(|c| class.types[c].name.clone())
        .collect();
    (classes, methods)
}

/// The classes, by type index, whose class monitors are kept: those with a static synchronized
/// method or named by the class literal of a `synchronized` block.
pub(super) fn class_monitors(class: &Class, facts: &Facts) -> HashSet<usize> {
    if !facts.kept_class_monitors() {
        return HashSet::new();
    }
    let mut out: HashSet<usize> =
        (class.methods.iter()).filter(|m| m.is_synchronized && m.is_static).map(|m| m.class).collect();
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Builtin(Builtin::MonitorEnter, args) = &x.kind
                            && let ExprKind::Builtin(Builtin::ClassLiteral(ty), _) = args[0].kind
                            && let Some(t) = class.class_index(ty)
                        {
                            out.insert(t);
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    out
}

/// Rejects a kept monitor of what has none in the translation: a string or a box (a value), a
/// Java enum's constant (a plain value), or a class's monitor other than a class literal's. A
/// builder has its cell's monitor, and an array its elements'. The first of each method is reported.
pub(super) fn check(class: &Class, facts: &Facts) -> Result<(), Vec<Error>> {
    if !facts.monitors && !facts.threaded {
        return Ok(());
    }
    let mut errors = Vec::new();
    for method in &class.methods {
        let mut error = None;
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        let ExprKind::Builtin(Builtin::MonitorEnter, args) = &x.kind else { return };
                        let lock = &args[0];
                        let what = match lock.ty {
                            Type::JavaClass => match lock.kind {
                                ExprKind::Builtin(Builtin::ClassLiteral(ty), _)
                                    if class.class_index(ty).is_some() =>
                                {
                                    return;
                                }
                                ExprKind::Builtin(Builtin::ClassLiteral(_), _) => "the class of a value",
                                _ => "a class that is not a class literal",
                            },
                            Type::Class(..) | Type::StringBuilder | Type::Array(_) => return,
                            Type::Enum(_) => "an enum constant",
                            Type::String => "a string",
                            Type::Boxed(_) => "a box",
                            _ => "a value",
                        };
                        error.get_or_insert(Error {
                            pos: x.pos,
                            message: format!("`synchronized` on {what} not supported yet"),
                        });
                    })
                },
                &mut |_| {},
            );
        }
        errors.extend(error);
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
