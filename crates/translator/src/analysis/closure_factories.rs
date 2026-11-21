//! Closure factories: static methods whose body only returns a lambda converted to a closure
//! type (`closures::closure_types`), such as the Minecraft server's
//! `AbortableIterationConsumer.forConsumer(consumer)`, which is
//! `return e -> { consumer.accept(e); return Continuation.CONTINUE; };`. Each call of one is
//! replaced by the lambda itself, `(T) new <lambda class>(<captures>)`, with the call's arguments
//! for the parameters the lambda captures, before the analyses run.
//!
//! The lambda's closure is then made where the call was, and the analyses see what it captures
//! there: passed to a lent closure parameter (`closures::lent_closures`), or held in a local that
//! only calls it and lends it on (`closures::closure_locals`), such as the `argument$N` of an
//! ordered call (`lower::ordered_call`), it is a closure on the caller's stack with no `Rc`; and
//! the objects it captures are held by an object of the caller's own, which the escape analysis
//! follows into the methods it is lent to, where a factory's parameter that its result holds
//! escapes. Anywhere else the closure gets its `Rc` at the call rather than in the factory.
//!
//! Nothing that Java does is lost:
//! - a closure has no identity (a closure type rules out `==` and every conversion to another
//!   type), so a new one at each call is the same as the factory's;
//! - each argument is a read of a variable that the calling method never assigns, or `this`,
//!   perhaps converted to a supertype, which cannot fail: reading it where the lambda is made, in
//!   the lambda's order, and not at all for a parameter it does not capture, changes nothing, and
//!   the closure captures the value the factory would have; like every lambda's captures, each is
//!   a variable (the generator binds a capture to the variable it reads, which a literal is not);
//! - calling the factory initializes its class, and so its superclasses and the interfaces they
//!   implement: none of those has static initialization that must run eagerly
//!   (`statics::eager_and_final`), so that initialization does nothing anyone can observe.
//!
//! The calls in the code of a factory's lambda stay calls: the closure's body is written where it
//! is made, so a lambda made again inside its own body (a factory whose lambda calls it, or two
//! that call each other) would be written without end. The factory keeps its body for those
//! calls and for those whose arguments are not such reads.

use super::{FastMap, FastSet};
use crate::ast::*;

/// Replaces each call of a closure factory by the lambda that the factory returns (see the
/// module's comment). The first expression id after those of the expressions it made, when it
/// replaced any call.
pub fn inline_closure_factories(class: &mut Class) -> Option<ExprId> {
    let closure_types = super::closures::closure_types(class);
    if closure_types.is_empty() {
        return None;
    }
    let factories = factories(class, &closure_types);
    if factories.is_empty() {
        return None;
    }
    let replaced = replaced_calls(class, &factories);
    if replaced.is_empty() {
        return None;
    }
    let first = super::whole_objects::next_id(class);
    let mut next = first;
    for method in &mut class.methods {
        for stmt in &mut method.body {
            rewrite_stmt(stmt, &mut |e| {
                let ExprKind::Call { resolved: Some(callee), args, .. } = &mut e.kind else { return };
                let Some(factory) = factories.get(callee).filter(|_| replaced.contains(&e.id)) else {
                    return;
                };
                let mut args: Vec<Option<Expr>> = std::mem::take(args).into_iter().map(Some).collect();
                let captures: Vec<Expr> = factory
                    .captures
                    .iter()
                    .map(|&p| args[p].take().expect("each parameter captured once"))
                    .collect();
                let lambda = Expr {
                    kind: ExprKind::New { class: factory.lambda, args: captures, ctor: Some(factory.ctor) },
                    pos: e.pos,
                    ty: factory.lambda_ty,
                    id: next,
                };
                *e =
                    Expr { kind: ExprKind::Cast(e.ty, Box::new(lambda)), pos: e.pos, ty: e.ty, id: next + 1 };
                next += 2;
            });
        }
    }
    (next > first).then_some(next)
}

/// The calls of `factories` that are replaced, by id: those converted to a closure type that the
/// lambda is one of, each of whose arguments is a read of a variable that its method never
/// assigns, or `this`, or such a read converted to a supertype (which cannot fail), outside the
/// code of the factories' lambdas.
fn replaced_calls(class: &Class, factories: &FastMap<usize, Factory>) -> FastSet<ExprId> {
    let h = &class.hierarchy;
    let lambdas: FastSet<usize> = factories.values().map(|f| f.lambda).collect();
    // Whether class `t` is a factory's lambda class or is declared in the code of one, whose
    // calls stay: the closure's body is written where it is made, so a call in it replaced by a
    // lambda that makes it again would be written without end.
    let in_factory_lambda = |mut t: usize| loop {
        if lambdas.contains(&t) {
            return true;
        }
        match class.types[t].origin {
            Origin::Lambda { method: Some(m), .. }
            | Origin::Local { method: Some(m), .. }
            | Origin::Anonymous { method: Some(m), .. } => t = class.methods[m].class,
            _ => return false,
        }
    };
    let mut out = FastSet::default();
    for method in &class.methods {
        if in_factory_lambda(method.class) {
            continue;
        }
        // The calls of factories that convert to one of the lambda's closure types.
        let mut calls: Vec<&Expr> = Vec::new();
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Call { resolved: Some(callee), args, .. } = &x.kind
                            && let Some(factory) = factories.get(callee)
                            && class.class_index(x.ty).is_some_and(|t| factory.targets.contains(&t))
                            && args.len() == factory.params
                        {
                            calls.push(x);
                        }
                    })
                },
                &mut |_| {},
            );
        }
        if calls.is_empty() {
            continue;
        }
        let assigned = assigned_vars(&method.body);
        let plain = |a: &Expr| {
            let mut a = a;
            loop {
                match &a.kind {
                    ExprKind::Var(name) => return !assigned.contains(name),
                    ExprKind::This => return true,
                    ExprKind::Cast(to, value)
                        if class.class_index(*to).is_some_and(|t| {
                            class.class_index(value.ty).is_some_and(|s| h.is_subtype(s, t))
                        }) =>
                    {
                        a = value
                    }
                    _ => return false,
                }
            }
        };
        for call in calls {
            if let ExprKind::Call { args, .. } = &call.kind
                && args.iter().all(plain)
            {
                out.insert(call.id);
            }
        }
    }
    out
}

/// A closure factory: the lambda class whose object it returns, its constructor and type, for
/// each of the constructor's parameters (the captures) the factory's parameter it is given, how
/// many parameters the factory has, and the closure types that the lambda converts to.
struct Factory {
    lambda: usize,
    ctor: usize,
    lambda_ty: Type,
    captures: Vec<usize>,
    params: usize,
    targets: FastSet<usize>,
}

/// The closure factories, by method: static methods that are not Rust generics, whose body is
/// one `return` of a lambda converted to closure types, each of whose captures is a distinct
/// parameter, in a class whose initialization, and its superclasses' and their interfaces', need
/// not run eagerly.
fn factories(class: &Class, closure_types: &FastSet<usize>) -> FastMap<usize, Factory> {
    let h = &class.hierarchy;
    let mut out = FastMap::default();
    for (index, m) in class.methods.iter().enumerate() {
        if !m.is_static || m.is_ctor || m.is_abstract || m.is_synchronized || !m.type_params.is_empty() {
            continue;
        }
        let [Stmt::Return(Some(value), _)] = &m.body[..] else { continue };
        let mut inner = value;
        let mut converted = false;
        while let ExprKind::Cast(to, of) = &inner.kind
            && class.class_index(*to).is_some_and(|t| closure_types.contains(&t))
        {
            converted = true;
            inner = of;
        }
        let ExprKind::New { class: lambda, args, ctor: Some(ctor) } = &inner.kind else { continue };
        if !converted
            || !matches!(class.types[*lambda].origin, Origin::Lambda { .. })
            || !class.types[*lambda].type_params.is_empty()
        {
            continue;
        }
        let captures: Option<Vec<usize>> = args
            .iter()
            .map(|a| match &a.kind {
                ExprKind::Var(name) => m.params.iter().position(|p| p.name == *name),
                _ => None,
            })
            .collect();
        let Some(captures) = captures else { continue };
        let distinct: FastSet<usize> = captures.iter().copied().collect();
        if distinct.len() != captures.len() {
            continue;
        }
        let targets = closure_types.iter().copied().filter(|&t| h.is_subtype(*lambda, t)).collect();
        let (lambda, ctor, params) = (*lambda, *ctor, m.params.len());
        out.insert(index, Factory { lambda, ctor, lambda_ty: inner.ty, captures, params, targets });
    }
    if !out.is_empty() {
        let (eager, _) = super::statics::eager_and_final(class);
        let quiet_init = |c: usize| h.subtype_of(c).iter().all(|s| !eager.contains(s));
        out.retain(|&m, _| quiet_init(class.methods[m].class));
    }
    out
}

/// The variables that `stmts` assign or increment, by name.
fn assigned_vars(stmts: &[Stmt]) -> FastSet<String> {
    let mut out = FastSet::default();
    for s in stmts {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| match &x.kind {
                    ExprKind::Assign { target: Place::Var(v), .. }
                    | ExprKind::IncDec { target: Place::Var(v), .. } => {
                        out.insert(v.clone());
                    }
                    _ => {}
                })
            },
            &mut |_| {},
        );
    }
    out
}
