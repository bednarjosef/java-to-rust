//! The receivers of calls that always hold an object that only one thread uses: an object of
//! `StackObjects::objects` that is never moved to the heap (docs/multi-threading.md, "Objects
//! that one thread uses").
//!
//! Such an object never leaves the method that makes it, and handing an object to another thread
//! is a way of leaving (the runtime starts threads, and what a thread runs reaches it through a
//! field or a static), so only the thread that runs that method ever reads or writes its fields,
//! and only while the method runs. An object of a class that threads share then needs none of
//! the atomic accesses of its fields: the translation calls a form of a method that reads and
//! writes them plainly (`codegen`'s confined forms).
//!
//! A receiver qualifies when it is
//!
//! - a read of a local variable that only ever holds such objects: every declaration and
//!   assignment of the variable (`Resolution`) gives it a `new` of the objects (of one class), or
//!   the value of another such variable, and it is not a parameter, a loop's or a catch's
//!   variable, nor a pattern variable;
//! - in a method of a lambda's class, `this.f`, a field that captured such a local: every `new`
//!   of the lambda's class passes one as the value of `f`, and nothing else writes `f`. The lambda
//!   holds the object, so it does not leave the method either (`Contents`): its methods run on
//!   the same thread;
//! - a read of a local that only ever holds what such locals and fields do, as the local that
//!   lowering binds a call's receiver to (`receiver$0`, `lower::ordered_call`) does.
//!
//! The methods that those calls may run, and in turn those that they call on `this`, whose
//! object is the same one, are `StackObjects::confined_methods` when they do something atomic to
//! `this`: an update of a field through `Unsafe` (`compareAndSet`), an access of a `volatile`
//! field, or a call of such a method on `this`. The other methods' accesses are acquire loads and
//! release stores, plain moves already, and a second form of them would only cost code.

use super::{FastMap, FastSet, MERGED, Resolution, StackObjects, VarKey};
use crate::ast::*;

/// `StackObjects::confined` and `StackObjects::confined_methods`, from what the analysis found.
pub(super) fn receivers(class: &Class, found: &StackObjects) -> (FastSet<ExprId>, FastSet<usize>) {
    let moved: FastSet<ExprId> = found.moves.values().flatten().map(|mv| mv.made).collect();
    let made = |e: &Expr| match e.kind {
        ExprKind::New { class: c, .. } if found.objects.contains(&e.id) && !moved.contains(&e.id) => Some(c),
        _ => None,
    };
    let mut receivers: FastMap<ExprId, usize> = FastMap::default();
    // The fields of lambdas' classes that capture such locals, with the objects' classes, and the
    // `new`s of those classes that were checked.
    let mut captures: FastMap<(usize, String), Option<usize>> = FastMap::default();
    let mut checked: FastSet<ExprId> = FastSet::default();
    for method in &class.methods {
        if !method.body.iter().any(|s| any_in(s, &mut |x| made(x).is_some())) {
            continue;
        }
        let resolved = Resolution::of(method);
        let vars = confined_vars(method, &resolved, &made);
        for s in &method.body {
            visit_stmt_exprs(s, &mut |x| match &x.kind {
                ExprKind::MethodCall { receiver, is_super: false, .. } => {
                    let r = peel(receiver);
                    if let Some(c) = var_class(&vars, &resolved, r) {
                        receivers.insert(r.id, c);
                    }
                }
                ExprKind::New { class: l, ctor: Some(ctor), args } if lambda(class, *l) => {
                    checked.insert(x.id);
                    for (field, arg) in captured_fields(&class.methods[*ctor]).into_iter().zip(args) {
                        let Some(field) = field else { continue };
                        let held = var_class(&vars, &resolved, peel(arg));
                        let entry = captures.entry((*l, field)).or_insert(held);
                        if *entry != held {
                            *entry = None;
                        }
                    }
                }
                _ => {}
            });
        }
    }
    captures.retain(|_, c| c.is_some());
    if !captures.is_empty() {
        // A `new` of the lambda's class that was not checked (in a method without such objects,
        // or in a field's initializer), and a write of the field other than its constructor's,
        // may give the field another object.
        let mut spoiled: FastSet<usize> = FastSet::default();
        let mut look = |x: &Expr, ctor_of: Option<usize>| match &x.kind {
            ExprKind::New { class: l, .. } if !checked.contains(&x.id) => _ = spoiled.insert(*l),
            ExprKind::Assign { target: Place::Field { object, name }, .. }
            | ExprKind::IncDec { target: Place::Field { object, name }, .. } => {
                if let Some(l) = class.class_index(object.ty)
                    && captures.contains_key(&(l, name.clone()))
                    && ctor_of != Some(l)
                {
                    spoiled.insert(l);
                }
            }
            _ => {}
        };
        for method in &class.methods {
            let ctor_of = method.is_ctor.then_some(method.class);
            for s in &method.body {
                visit_stmt_exprs(s, &mut |x| look(x, ctor_of));
            }
        }
        for t in &class.types {
            for f in &t.fields {
                if let Some(init) = &f.init {
                    visit_expr(init, &mut |x| look(x, None));
                }
            }
        }
        captures.retain(|(l, _), _| !spoiled.contains(l));
        let captured: FastSet<usize> = captures.keys().map(|&(l, _)| l).collect();
        for method in class.methods.iter().filter(|m| !m.is_ctor && captured.contains(&m.class)) {
            let l = method.class;
            // `this.f`, or a local that only ever holds what such fields do.
            let field = |e: &Expr| match &e.kind {
                ExprKind::Field { object, name } if matches!(object.kind, ExprKind::This) => {
                    captures.get(&(l, name.clone())).copied().flatten()
                }
                _ => None,
            };
            let resolved = Resolution::of(method);
            let vars = confined_vars(method, &resolved, &field);
            for s in &method.body {
                visit_stmt_exprs(s, &mut |x| {
                    if let ExprKind::MethodCall { receiver, is_super: false, .. } = &x.kind {
                        let r = peel(receiver);
                        if let Some(c) = field(r).or_else(|| var_class(&vars, &resolved, r)) {
                            receivers.insert(r.id, c);
                        }
                    }
                });
            }
        }
    }
    // The methods those calls may run, with the class of the object: the one each names and the
    // implementation of that class, and in turn those that they call on `this`, whose object is
    // the same.
    let mut reached: FastSet<(usize, usize)> = FastSet::default();
    let mut work: Vec<(usize, usize)> = Vec::new();
    let mut reach = |r: usize, c: usize, work: &mut Vec<(usize, usize)>| {
        for m in std::iter::once(r).chain(class.hierarchy.implementation(r, c)) {
            if reached.insert((m, c)) {
                work.push((m, c));
            }
        }
    };
    let called = |body: &[Stmt], receiver_class: &dyn Fn(&Expr) -> Option<usize>| {
        let mut out = Vec::new();
        for s in body {
            visit_stmt_exprs(s, &mut |x| {
                if let ExprKind::MethodCall { receiver, resolved: Some(r), is_super: false, .. } = &x.kind
                    && let Some(c) = receiver_class(peel(receiver))
                {
                    out.push((*r, c));
                }
            });
        }
        out
    };
    for method in &class.methods {
        for (r, c) in called(&method.body, &|r| receivers.get(&r.id).copied()) {
            reach(r, c, &mut work);
        }
    }
    // What each method reached calls on `this`, by the methods it may run.
    let mut calls: FastMap<usize, Vec<usize>> = FastMap::default();
    while let Some((k, c)) = work.pop() {
        let on_this = |r: &Expr| matches!(r.kind, ExprKind::This).then_some(c);
        for (r, c) in called(&class.methods[k].body, &on_this) {
            let runs = calls.entry(k).or_default();
            runs.push(r);
            runs.extend(class.hierarchy.implementation(r, c));
            reach(r, c, &mut work);
        }
    }
    // Of those, the methods that do something atomic to `this`: an update of a field (a
    // `compareAndSet` through `Unsafe`, `Method::field_update`) or an access of a `volatile`
    // field, or that call such a method on `this`. Only those gain from a plain form.
    let mut methods: FastSet<usize> = reached
        .iter()
        .map(|&(k, _)| k)
        .filter(|&k| {
            let m = &class.methods[k];
            m.field_update.is_some()
                || m.body.iter().any(|s| any_in(s, &mut |x| volatile_on_this(class, m.class, x)))
        })
        .collect();
    loop {
        let more: Vec<usize> = calls
            .iter()
            .filter(|(k, runs)| !methods.contains(*k) && runs.iter().any(|r| methods.contains(r)))
            .map(|(k, _)| *k)
            .collect();
        if more.is_empty() {
            break;
        }
        methods.extend(more);
    }
    (receivers.into_keys().collect(), methods)
}

/// Whether `x` reads or writes a `volatile` field of `this`, in a method of class `t`.
fn volatile_on_this(class: &Class, t: usize, x: &Expr) -> bool {
    let (ExprKind::Field { object, name }
    | ExprKind::Assign { target: Place::Field { object, name }, .. }
    | ExprKind::IncDec { target: Place::Field { object, name }, .. }) = &x.kind
    else {
        return false;
    };
    matches!(object.kind, ExprKind::This)
        && super::field_of(class, t, name).is_some_and(|(c, i)| class.types[c].fields[i].volatile)
}

/// What an assignment gives a variable: objects of one class (`None` for anything else), or
/// what another variable holds.
#[derive(Clone, Copy)]
enum Given<'a> {
    Objects(Option<usize>),
    Copy(VarKey<'a>),
}

/// The variables of `method` that only ever hold objects that `made` picks out, all of one
/// class, with that class: each declaration and assignment gives one such object, or the value
/// of another such variable (the locals that lowering binds a call's receiver to,
/// `receiver$0`).
fn confined_vars<'a>(
    method: &'a Method,
    resolved: &Resolution<'a>,
    made: &dyn Fn(&Expr) -> Option<usize>,
) -> FastMap<VarKey<'a>, usize> {
    let value = |e: &'a Expr| match &peel(e).kind {
        ExprKind::Var(name) => Given::Copy(resolved.mention(name, peel(e).id)),
        _ => Given::Objects(made(peel(e))),
    };
    let given: std::cell::RefCell<Vec<(VarKey<'a>, Given<'a>)>> = Default::default();
    let give = |key: VarKey<'a>, g: Given<'a>| given.borrow_mut().push((key, g));
    for (i, p) in method.params.iter().enumerate() {
        give(resolved.param(&p.name, i), Given::Objects(None));
    }
    for s in &method.body {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| match &x.kind {
                    ExprKind::Assign { op, target: Place::Var(name), value: v } => {
                        let g = if op.is_none() { value(v) } else { Given::Objects(None) };
                        give(resolved.mention(name, x.id), g);
                    }
                    ExprKind::IncDec { target: Place::Var(name), .. } => {
                        give(resolved.mention(name, x.id), Given::Objects(None))
                    }
                    _ => {}
                })
            },
            &mut |s| match s {
                // A declaration without a value gives the variable nothing: Java reads it only
                // once an assignment has.
                Stmt::Local { name, init: Some(init), .. } => {
                    give(resolved.declaration(name, s), value(init))
                }
                Stmt::ForEach { name, .. } => give(resolved.declaration(name, s), Given::Objects(None)),
                Stmt::Try { catches, .. } => {
                    for catch in catches {
                        give(resolved.catch(&catch.name, catch), Given::Objects(None));
                    }
                }
                _ => {}
            },
        );
    }
    let given = given.into_inner();
    let assigned: FastSet<VarKey<'a>> = given.iter().map(|&(key, _)| key).collect();
    // Each variable's class, or `None` once something else may be in it. No entry stands for
    // what nothing has excluded yet: each round takes what every assignment gives, from the
    // last round's values, until nothing changes. A copy of a variable that nothing assigns here
    // (a mention outside the body) may hold anything, and so may a pattern variable, whose
    // declarations are not all found (`MERGED`).
    let mut vars: FastMap<VarKey<'a>, Option<usize>> = FastMap::default();
    loop {
        let mut next: FastMap<VarKey<'a>, Option<usize>> = FastMap::default();
        for &(key, g) in &given {
            let c = match g {
                _ if key.1 == MERGED => Some(None),
                Given::Objects(c) => Some(c),
                Given::Copy(from) if from.1 == MERGED || !assigned.contains(&from) => Some(None),
                Given::Copy(from) => vars.get(&from).copied(),
            };
            let Some(c) = c else { continue };
            let entry = next.entry(key).or_insert(c);
            if *entry != c {
                *entry = None;
            }
        }
        if next == vars {
            break;
        }
        vars = next;
    }
    vars.into_iter().filter_map(|(key, c)| Some((key, c?))).collect()
}

/// The class of the objects that `x` holds, when it is a read of one of `vars`.
fn var_class<'a>(vars: &FastMap<VarKey<'a>, usize>, resolved: &Resolution<'a>, x: &'a Expr) -> Option<usize> {
    match &x.kind {
        ExprKind::Var(name) => vars.get(&resolved.mention(name, x.id)).copied(),
        _ => None,
    }
}

/// `e` without the casts around it, which leave its object as it is.
fn peel(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Cast(Type::Class(..), inner) => peel(inner),
        _ => e,
    }
}

/// Whether class `t` is a lambda's or a method reference's.
fn lambda(class: &Class, t: usize) -> bool {
    matches!(class.types[t].origin, Origin::Lambda { .. })
}

/// The field that a lambda's constructor stores each of its parameters in.
fn captured_fields(ctor: &Method) -> Vec<Option<String>> {
    ctor.params
        .iter()
        .map(|p| {
            ctor.body.iter().find_map(|s| match s {
                Stmt::Expr(Expr {
                    kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
                    ..
                }) if matches!(object.kind, ExprKind::This)
                    && matches!(&value.kind, ExprKind::Var(v) if *v == p.name) =>
                {
                    Some(name.clone())
                }
                _ => None,
            })
        })
        .collect()
}

/// Whether `pick` holds for some expression in `s`.
fn any_in(s: &Stmt, pick: &mut dyn FnMut(&Expr) -> bool) -> bool {
    let mut found = false;
    visit_stmt_exprs(s, &mut |x| found |= pick(x));
    found
}
