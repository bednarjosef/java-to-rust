//! Fixed fields: the instance fields that only constructors write, through `this` (Java's `final`
//! fields, and others written the same way). Once its object is built such a field keeps its
//! value, so a read of it borrows the field alone, without the object's lock or `RefCell` borrow
//! and without copying its value out (`java::fixed!`, see the runtime's `fixed.rs`). A class built
//! whole (`whole_objects`) is built by methods made of its constructors and field initializers,
//! `construct<Name>` and `initializeFields`, which count as its constructors here
//! (`Class::constructs`); a call of one on `this` runs as `super(..)` does.
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
//!
//! The fields left out so are fixed after all when the objects of their class are built alone:
//! the escape analysis finds that no method that builds one lets `this` escape
//! (`StackObjects::built_alone`), so whatever a constructor does with `this`, such as calling a
//! method on it or passing it to a lambda that keeps it nowhere, no other code holds the object
//! until it is built. That analysis runs alongside this one, so these fields
//! (`FixedFields::if_built_alone`) join the others once it is done
//! (`Analysed::with_stack_objects`); the analyses before that see them as not fixed.

use super::{FastMap, FastSet};
use crate::ast::*;

/// The fixed fields, by the type that declares them and their name.
pub struct FixedFields {
    pub fixed: FastSet<(usize, String)>,
    /// The fields that only constructors write but that are not fixed for the threads a
    /// constructor may start, which are if the objects of their class are built alone (see the
    /// module's comment).
    pub if_built_alone: Vec<(usize, String)>,
}

/// The fixed fields (see the module's comment).
pub fn fixed_fields(class: &Class, threaded: bool) -> FixedFields {
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
    for (i, m) in class.methods.iter().enumerate() {
        let in_ctor = class.constructs(i).then_some(m.class);
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
    let mut if_built_alone = Vec::new();
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
            } else {
                if_built_alone.push(key);
            }
        }
    }
    FixedFields { fixed, if_built_alone }
}

/// The classes some of whose constructors or field initializers use `this` as a value: anything
/// but the object of a field's read or write, such as an argument, a stored value, the receiver
/// of a call or the enclosing instance of a nested class's object.
fn leaking_this(class: &Class) -> FastSet<usize> {
    let mut out = FastSet::default();
    for m in (0..class.methods.len()).filter(|&m| class.constructs(m)).map(|m| &class.methods[m]) {
        let mut leaked = false;
        for s in &m.body {
            visit_stmt(s, &mut |e| leaked |= leaks_this(class, e), &mut |_| {});
        }
        if leaked {
            out.insert(m.class);
        }
    }
    for (t, decl) in class.types.iter().enumerate() {
        if decl.fields.iter().filter_map(|f| f.init.as_ref()).any(|e| leaks_this(class, e)) {
            out.insert(t);
        }
    }
    out
}

/// Whether `e` uses `this` as a value (see `leaking_this`). The receiver of a call of a method
/// that builds the object (`construction_call`) is not: that method's own uses count for its class.
fn leaks_this(class: &Class, e: &Expr) -> bool {
    let mut this: Vec<ExprId> = Vec::new();
    let mut fields: FastSet<ExprId> = FastSet::default();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::This => this.push(x.id),
        ExprKind::Field { object, .. }
        | ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => _ = fields.insert(object.id),
        ExprKind::MethodCall { receiver, .. } if construction_call(class, x).is_some() => {
            _ = fields.insert(receiver.id)
        }
        _ => {}
    });
    this.iter().any(|id| !fields.contains(id))
}

/// The class of the method that `x` calls on `this`, where that method builds the object as a
/// constructor would (`Class::constructs`): a superclass's `construct<Name>` or the class's own
/// `initializeFields`, which `whole_objects` calls where `super(..)` and the field initializers
/// ran.
fn construction_call(class: &Class, x: &Expr) -> Option<usize> {
    match &x.kind {
        ExprKind::MethodCall { receiver, resolved: Some(m), .. }
            if matches!(receiver.kind, ExprKind::This) && class.construction_methods.contains_key(m) =>
        {
            Some(class.methods[*m].class)
        }
        _ => None,
    }
}

/// The fields, as (declaring type, name), that a constructor or field initializer writes through
/// `this` after `this` may have left it, or in the statement that lets it leave. A constructor
/// runs its statements before `this(..)` or `super(..)` (javac's enclosing instance and captured
/// locals), then the superclasses' constructors, then the field initializers in order, then the
/// rest of its body; `this` may have left from the first of them that `leaking_this` would count,
/// or from a superclass that leaks it anywhere. A `this(..)` counts as leaking, as the constructor
/// it calls may. A method that builds the object (`Class::constructs`) runs as a constructor
/// does, and its call of a superclass's `construct<Name>` or its class's `initializeFields` lets
/// `this` leave where that class or a superclass lets it leave anywhere (`leaves`). A field none
/// of whose writes is here was written before any other code, on any thread, could hold the
/// object.
fn written_late(class: &Class, leaking: &FastSet<usize>) -> FastSet<(usize, String)> {
    let mut late = FastSet::default();
    // Notes the fields that statement or initializer `e` writes through `this` as late when
    // `this` may have left (`left`), after noting whether `e` lets it leave.
    let note = |e: &Expr, left: &mut bool, late: &mut FastSet<(usize, String)>| {
        *left |= leaves(class, leaking, e);
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
    for m in (0..class.methods.len()).filter(|&m| class.constructs(m)).map(|m| &class.methods[m]) {
        let (before, call, after) = split_explicit_call(&m.body);
        let mut left = false;
        let stmts = |stmts: &[Stmt], left: &mut bool, late: &mut FastSet<(usize, String)>| {
            for s in stmts {
                let mut leaked = *left;
                visit_stmt(s, &mut |e| leaked |= leaves(class, leaking, e), &mut |_| {});
                *left = leaked;
                visit_stmt(s, &mut |e| note(e, left, late), &mut |_| {});
            }
        };
        stmts(before, &mut left, &mut late);
        match call.map(|c| &c.kind) {
            Some(ExprKind::ThisCall { .. }) => left = true,
            Some(ExprKind::SuperInit { args, .. }) => {
                left |= args.iter().any(|e| leaves(class, leaking, e)) || after_inits[m.class];
            }
            _ => left |= after_inits[m.class],
        }
        stmts(after, &mut left, &mut late);
    }
    late
}

/// Whether `this` may have left once `e` has run: `e` uses it as a value (`leaks_this`), or calls
/// a method that builds the object (`construction_call`) of a class that, or one of whose
/// superclasses, lets it leave from a constructor (`leaking`).
fn leaves(class: &Class, leaking: &FastSet<usize>, e: &Expr) -> bool {
    let mut left = leaks_this(class, e);
    visit_expr(e, &mut |x| {
        if let Some(c) = construction_call(class, x) {
            left |= leaking.contains(&c) || class.hierarchy.ancestors(c).iter().any(|a| leaking.contains(a));
        }
    });
    left
}

/// The methods that call `x` may run (`Class::callees`), with an instance call's narrowed to the
/// implementations of the classes that the points-to facts give its receiver
/// (`Class::receivers`), where they give any: an interface's call made where only a few of its
/// classes' objects arrive (the server's `BlockStateBase` asks its block's properties for its
/// light through a `ToIntFunction`) runs only theirs.
fn run_by(class: &Class, x: &Expr) -> Vec<usize> {
    if let ExprKind::MethodCall { receiver, resolved: Some(m), is_super: false, .. } = &x.kind
        && !class.methods[*m].is_static
        && let Some(set) = class.receivers.get(&receiver.id)
    {
        let mut run: Vec<usize> = set.iter().filter_map(|&c| class.hierarchy.implementation(*m, c)).collect();
        run.sort_unstable();
        run.dedup();
        return run;
    }
    class.callees(x)
}

/// The classes that a constructor which may start a thread builds objects of: the class of the
/// constructor, and its superclasses, whose fields the object has too. A call runs the methods
/// that `run_by` gives it.
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
                        for callee in run_by(class, x) {
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
        .filter(|&(m, _)| class.constructs(m) && starting[m])
        .map(|(_, method)| method.class)
        .collect();
    for (t, decl) in class.types.iter().enumerate() {
        for init in decl.fields.iter().filter_map(|f| f.init.as_ref()) {
            visit_expr(init, &mut |x| {
                if matches!(x.kind, ExprKind::Builtin(Builtin::Native(Native::ThreadStart), _))
                    || run_by(class, x).iter().any(|&m| starting[m])
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
