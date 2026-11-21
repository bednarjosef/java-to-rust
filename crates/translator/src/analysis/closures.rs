//! Which functional interfaces are closure types (`docs/closures-and-nested-classes.md`, 4.2):
//! their values are `Rc<dyn Fn(A) -> R>`, each lambda a Rust closure, and a call of the
//! interface's method a call of the closure.
//!
//! An interface is one when every value of it comes from a lambda or a method reference converted
//! to it, and nothing does with a value of it anything but call its method, store it and pass it
//! on: no conversion to another type (to `Object`, for printing, `equals` or a collection), no
//! `==`, no `instanceof`, no call but of its method and its default methods. The interface
//! declares one abstract method besides `Object`'s, or extends one interface and declares none,
//! and the interfaces above and below it are closure types too, which share its closures; each
//! lambda class implements one interface, and its method reads its object only through the
//! fields that hold its captures. A generic interface's closure takes and returns the erased
//! types: the lambda's method, which javac typed with the substituted ones, is behind a bridge,
//! and the closure's body is the bridge's conversions around it. Its default methods are those
//! of an extension trait of the closure type, whose `self` is the closure (`codegen::closures`).
//! Everything else keeps the object model's form, an enum with a
//! variant per lambda class.

use super::{FastMap, FastSet};
use crate::ast::*;

/// The interfaces (indexes into `Class::types`) whose values are closures.
pub fn closure_types(class: &Class) -> FastSet<usize> {
    let h = &class.hierarchy;
    let object = class.type_index(OBJECT);
    let mut candidates: FastSet<usize> = FastSet::default();
    let supers = |t: usize| -> Vec<usize> {
        class.types[t].implements.iter().copied().filter(|&i| Some(i) != object).collect()
    };
    for (t, decl) in class.types.iter().enumerate() {
        if decl.kind != TypeKind::Interface || decl.name == OBJECT || decl.name == COMPARABLE {
            continue;
        }
        // One abstract method besides `Object`'s, which `Comparator` declares again, its own or
        // that of the one interface it extends (`UnaryOperator<T> extends Function<T, T>`).
        let abstracts = class.methods.iter().filter(|m| m.class == t && is_functional_method(m)).count();
        let functional = match supers(t)[..] {
            [] => abstracts == 1,
            [_] => abstracts == 0,
            _ => false,
        };
        // Every class below it is a lambda's that implements one interface.
        let below: Vec<usize> = (0..class.types.len()).filter(|&s| s != t && h.is_subtype(s, t)).collect();
        let lambdas = below.iter().all(|&s| {
            let d = &class.types[s];
            d.kind == TypeKind::Interface
                || (matches!(d.origin, Origin::Lambda { .. }) && d.extends.is_none() && supers(s).len() == 1)
        });
        if functional && lambdas && below.iter().any(|&s| class.types[s].kind != TypeKind::Interface) {
            candidates.insert(t);
        }
    }
    // The interfaces above and below a closure type are closure types too: their values are the
    // same closures.
    loop {
        let family = |t: usize, u: usize| h.is_subtype(t, u) || h.is_subtype(u, t);
        let out: Vec<usize> = (candidates.iter().copied())
            .filter(|&t| {
                (0..class.types.len()).any(|u| {
                    class.types[u].kind == TypeKind::Interface
                        && Some(u) != object
                        && family(t, u)
                        && !candidates.contains(&u)
                })
            })
            .collect();
        if out.is_empty() {
            break;
        }
        for t in out {
            candidates.remove(&t);
        }
    }
    if candidates.is_empty() {
        return candidates;
    }
    let abstract_method = |t: usize| closure_method(class, t);
    // A default method of the interface takes the closure as `this`.
    let callable = |t: usize, m: usize| {
        let method = &class.methods[m];
        m == abstract_method(t) || (h.is_subtype(t, method.class) && !method.is_static && !method.is_abstract)
    };
    let of = |ty: Type| class.class_index(ty).filter(|t| candidates.contains(t));
    let lambda_of = |c: usize| candidates.iter().copied().find(|&t| h.is_subtype(c, t));
    // The interfaces some use rules out, and how many `new`s of each lambda class are converted
    // to its interface directly, of how many in all.
    let mut ruled_out: FastSet<usize> = FastSet::default();
    let mut news: FastMap<usize, (usize, usize)> = FastMap::default();
    let mut on_expr = |x: &Expr| match &x.kind {
        ExprKind::Cast(to, value) => {
            if let ExprKind::New { class: c, .. } = value.kind
                && lambda_of(c).is_some()
                && of(*to).is_some_and(|t| h.is_subtype(c, t))
            {
                news.entry(c).or_default().0 += 1;
            } else if let Some(t) = of(value.ty)
                && value.ty != *to
                // An interface's closure is one of the interface it extends.
                && !of(*to).is_some_and(|u| h.is_subtype(t, u))
            {
                ruled_out.insert(t);
            }
        }
        ExprKind::New { class: c, .. } => {
            if lambda_of(*c).is_some() {
                news.entry(*c).or_default().1 += 1;
            }
        }
        ExprKind::Binary(_, l, r) => {
            for operand in [l, r] {
                if let Some(t) = of(operand.ty) {
                    ruled_out.insert(t);
                }
            }
        }
        ExprKind::InstanceOf { value, .. } => {
            if let Some(t) = of(value.ty) {
                ruled_out.insert(t);
            }
        }
        ExprKind::MethodCall { receiver, resolved, .. } => {
            if let Some(t) = of(receiver.ty)
                && !resolved.is_some_and(|m| callable(t, m))
            {
                ruled_out.insert(t);
            }
        }
        ExprKind::Builtin(_, args) => {
            for a in args {
                if let Some(t) = of(a.ty) {
                    ruled_out.insert(t);
                }
            }
        }
        _ => {}
    };
    let mut visit = |e: &Expr| visit_expr(e, &mut on_expr);
    for m in &class.methods {
        for s in &m.body {
            visit_stmt(s, &mut visit, &mut |_| {});
        }
    }
    for decl in &class.types {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(init) = &f.init {
                visit(init);
            }
        }
        for constant in decl.enum_constants.iter().flatten() {
            constant.values.iter().for_each(&mut visit);
        }
    }
    for (&c, &(converted, all)) in &news {
        if converted != all
            && let Some(t) = lambda_of(c)
        {
            ruled_out.insert(t);
        }
    }
    // A lambda's method reads its object only through its fields.
    for &t in &candidates {
        for c in h.concretes(t) {
            let Some(m) = h.implementation(abstract_method(t), c) else {
                ruled_out.insert(t);
                continue;
            };
            // A bridge converts the arguments for the lambda's own method, whose body is the
            // lambda's.
            let m = match bridged_call(&class.methods[m]) {
                Some(ExprKind::MethodCall { resolved: Some(sub), .. }) if class.methods[*sub].class == c => {
                    *sub
                }
                _ => m,
            };
            if class.methods[m].body.iter().any(uses_this) {
                ruled_out.insert(t);
            }
        }
    }
    // What rules out an interface rules out those above and below it.
    let ruled_out: FastSet<usize> = (candidates.iter().copied())
        .filter(|&t| ruled_out.iter().any(|&r| h.is_subtype(t, r) || h.is_subtype(r, t)))
        .collect();
    candidates.retain(|t| !ruled_out.contains(t));
    candidates
}

/// The closure parameters, as (method, parameter) indexes among the borrowed object parameters
/// (`borrowed`), that the method only calls and lends on: every use of one is a call of its
/// type's method on it or an argument of another such parameter, through casts to the closure
/// types it is one of. Nothing keeps such a closure past the call, so the parameter is a
/// `&dyn Fn(..)` (an `Option` of one where it may be null) that the caller lends, and a lambda
/// passed to it is a closure on the caller's stack, with no `Rc`. A closure that another
/// closure captures or that anything stores is not lent; neither is a parameter of a
/// constructor or of a lambda's own method, which is the closure's. The methods of an override
/// group take a parameter alike, so it is lent in all of them or in none.
pub fn lent_closures(
    class: &Class,
    closure_types: &FastSet<usize>,
    borrowed: &FastSet<(usize, usize)>,
) -> FastSet<(usize, usize)> {
    let closure_of = |ty: Type| class.class_index(ty).filter(|t| closure_types.contains(t));
    let mut lent: FastSet<(usize, usize)> = (borrowed.iter().copied())
        .filter(|&(m, i)| {
            let method = &class.methods[m];
            let c = method.class;
            !method.is_ctor
                && !closure_types.contains(&c)
                && !matches!(class.types[c].origin, Origin::Lambda { .. })
                && closure_of(method.params[i].ty).is_some()
                && !assigns(&method.body, &method.params[i].name)
        })
        .collect();
    loop {
        let losing: Vec<(usize, usize)> = (lent.iter().copied())
            .filter(|&(m, i)| {
                let method = &class.methods[m];
                !only_called(class, closure_types, &method.body, &method.params[i].name, &lent)
            })
            .collect();
        for p in &losing {
            lent.remove(p);
        }
        if !super::align_groups(class, &mut lent) && losing.is_empty() {
            break;
        }
    }
    lent
}

/// Whether `stmts` assign the variable `name`, which a lent parameter, a borrow, cannot take.
fn assigns(stmts: &[Stmt], name: &str) -> bool {
    let mut found = false;
    for s in stmts {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    found |= matches!(&x.kind, ExprKind::Assign { target: Place::Var(v), .. } if v == name);
                })
            },
            &mut |_| {},
        );
    }
    found
}

/// Whether every use of the closure variable `name` in `stmts` is a call of its type's method on
/// it, or an argument of a parameter of `lent`.
fn only_called(
    class: &Class,
    closure_types: &FastSet<usize>,
    stmts: &[Stmt],
    name: &str,
    lent: &FastSet<(usize, usize)>,
) -> bool {
    let mut uses = Vec::new();
    let mut allowed = FastSet::default();
    for s in stmts {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::Var(v) = &x.kind
                        && v == name
                    {
                        uses.push(x.id);
                    }
                    allowed.extend(called_or_lent(class, closure_types, x, name, lent));
                })
            },
            &mut |_| {},
        );
    }
    uses.iter().all(|id| allowed.contains(id))
}

/// The reads of the closure variable `name` that `x` calls the closure of, as its receiver, or
/// passes as an argument of a parameter of `lent`.
fn called_or_lent(
    class: &Class,
    closure_types: &FastSet<usize>,
    x: &Expr,
    name: &str,
    lent: &FastSet<(usize, usize)>,
) -> Vec<ExprId> {
    let mut out = Vec::new();
    if let ExprKind::MethodCall { receiver, resolved: Some(r), .. } = &x.kind
        && let ExprKind::Var(v) = &receiver.kind
        && v == name
        && let Some(t) = class.class_index(receiver.ty)
        && closure_types.contains(&t)
        && *r == closure_method(class, t)
    {
        out.push(receiver.id);
    }
    if let ExprKind::Call { args, resolved: Some(callee), .. }
    | ExprKind::MethodCall { args, resolved: Some(callee), .. } = &x.kind
    {
        for (j, a) in args.iter().enumerate() {
            if lent.contains(&(*callee, j)) {
                out.extend(closure_read(a, name, closure_types, class));
            }
        }
    }
    out
}

/// The `Var` node inside `e` when `e` reads the variable `name`, through casts to closure types.
fn closure_read(e: &Expr, name: &str, closure_types: &FastSet<usize>, class: &Class) -> Option<ExprId> {
    match &e.kind {
        ExprKind::Var(v) if v == name => Some(e.id),
        ExprKind::Cast(to, inner) if class.class_index(*to).is_some_and(|t| closure_types.contains(&t)) => {
            closure_read(inner, name, closure_types, class)
        }
        _ => None,
    }
}

/// The locals of closure types, by method and name, that are only called: each declaration of
/// the name in the method has a lambda converted to its type as its initializer, and every other
/// use of it is a call of its type's method on it or an argument of a lent closure parameter
/// (`lent_closures`). Nothing else holds such a closure, so it is a plain Rust closure that the
/// method owns, with no `Rc`.
pub fn closure_locals(
    class: &Class,
    closure_types: &FastSet<usize>,
    lent: &FastSet<(usize, usize)>,
) -> FastSet<(usize, String)> {
    let mut out = FastSet::default();
    for (index, m) in class.methods.iter().enumerate() {
        // Per name: whether every declaration qualifies, and the uses that are calls.
        let mut declared: FastMap<String, bool> = FastMap::default();
        for s in &m.body {
            visit_stmt(s, &mut |_| {}, &mut |s| {
                if let Stmt::Local { ty, name, init, .. } = s {
                    let lambda = closure_types.contains(&class.class_index(*ty).unwrap_or(usize::MAX))
                        && matches!(init, Some(Expr { kind: ExprKind::Cast(to, value), .. })
                            if to == ty && matches!(value.kind, ExprKind::New { ctor: Some(_), .. }));
                    *declared.entry(name.clone()).or_insert(true) &= lambda;
                }
            });
        }
        declared.retain(|_, lambda| *lambda);
        if declared.is_empty() {
            continue;
        }
        let mut assigned: FastSet<String> = FastSet::default();
        for s in &m.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Assign { target: Place::Var(name), .. } = &x.kind {
                            assigned.insert(name.clone());
                        }
                    })
                },
                &mut |_| {},
            );
        }
        for name in declared.keys() {
            if !assigned.contains(name) && only_called(class, closure_types, &m.body, name, lent) {
                out.insert((index, name.clone()));
            }
        }
    }
    out
}

/// The abstract method of closure type `t`, which its closures run.
pub fn closure_method(class: &Class, t: usize) -> usize {
    match class.methods.iter().position(|m| m.class == t && is_functional_method(m)) {
        Some(m) => m,
        None => {
            let object = class.type_index(OBJECT);
            let above = class.types[t].implements.iter().copied().find(|&i| Some(i) != object);
            closure_method(class, above.expect("a functional interface's method"))
        }
    }
}

/// Whether `m` is abstract and not one of `Object`'s public methods declared again.
fn is_functional_method(m: &Method) -> bool {
    let object_method =
        matches!((m.name.as_str(), m.params.len()), ("equals", 1) | ("hashCode", 0) | ("toString", 0));
    m.is_abstract && !m.is_static && !object_method
}

/// The call of a bridge `m` to the method it converts the arguments and the result for: `m`'s one
/// statement is the call of a method of its own class on `this`, or returns it, converted.
pub fn bridged_call(m: &Method) -> Option<&ExprKind> {
    let value = match &m.body[..] {
        [Stmt::Expr(e)] | [Stmt::Return(Some(e), _)] => e,
        _ => return None,
    };
    let mut value = value;
    // Only the conversions of the result are around the call.
    loop {
        match &value.kind {
            ExprKind::Cast(_, inner) => value = inner,
            call @ ExprKind::MethodCall { receiver, resolved: Some(_), .. }
                if matches!(receiver.kind, ExprKind::This) =>
            {
                return Some(call);
            }
            _ => return None,
        }
    }
}

/// Whether the statement uses `this` other than to read one of its fields.
fn uses_this(s: &Stmt) -> bool {
    let mut found = false;
    visit_stmt(s, &mut |e| found |= this_outside_fields(e), &mut |_| {});
    found
}

fn this_outside_fields(e: &Expr) -> bool {
    let mut found = false;
    let mut fields = 0usize;
    let mut this = 0usize;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This) => fields += 1,
        ExprKind::This => this += 1,
        ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
            if matches!(object.kind, ExprKind::This) =>
        {
            found = true
        }
        _ => {}
    });
    found || this > fields
}
