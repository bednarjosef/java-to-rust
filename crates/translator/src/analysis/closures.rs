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

/// The closure parameters, as (method, parameter) indexes, that the method only calls and lends
/// on: every use of one is a call of its type's method on it, an argument of another such
/// parameter, or a capture of a lambda passed to one whose method in turn only calls and lends
/// that capture (`lent_captures`), through casts to the closure types it is one of. Nothing keeps
/// such a closure past the call, so the parameter is a `&dyn Fn(..)` (an `Option` of one where it
/// may be null) that the caller lends, and a lambda passed to it is a closure on the caller's
/// stack, with no `Rc`. The candidates are the borrowed object parameters (`borrowed`, those
/// that may be null in `optional`) and, since a capture makes the borrowing analysis count a
/// parameter as kept, the closure parameters that are never null (`non_null`); only one that is
/// never null is captured. A closure that anything stores, or that a stored closure captures, is
/// not lent; neither is a parameter of a constructor or of a lambda's own method, which is the
/// closure's. The methods of an override group take a parameter alike, so it is lent in all of
/// them or in none.
pub fn lent_closures(
    class: &Class,
    closure_types: &FastSet<usize>,
    borrowed: &FastSet<(usize, usize)>,
    optional: &FastSet<(usize, usize)>,
    non_null: &dyn Fn(usize, &str) -> bool,
) -> FastSet<(usize, usize)> {
    let closure_of = |ty: Type| class.class_index(ty).filter(|t| closure_types.contains(t));
    let mut lent: FastSet<(usize, usize)> = FastSet::default();
    for (m, method) in class.methods.iter().enumerate() {
        let c = method.class;
        if method.is_ctor
            || closure_types.contains(&c)
            || matches!(class.types[c].origin, Origin::Lambda { .. })
        {
            continue;
        }
        for (i, p) in method.params.iter().enumerate() {
            if closure_of(p.ty).is_some()
                && (borrowed.contains(&(m, i)) || non_null(m, &p.name))
                && !assigns(&method.body, &p.name)
            {
                lent.insert((m, i));
            }
        }
    }
    loop {
        let losing: Vec<(usize, usize)> = (lent.iter().copied())
            .filter(|&(m, i)| {
                let method = &class.methods[m];
                let capturable = !optional.contains(&(m, i));
                let read = Read::Var(&method.params[i].name);
                !only_called(class, closure_types, &method.body, read, &lent, capturable)
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

/// The captures, as (lambda class, field), that hold a lent closure (`lent_closures`): the lambda
/// is passed to a lent parameter, so its closure is on the stack of the code that creates it, the
/// value it captures is a lent parameter that is never null (`optional` holds those that may be)
/// or such a capture of the lambda that creates it, and its methods only call the capture and
/// lend it on. The closure then borrows the lent closure, `&dyn Fn` inside `&dyn Fn`, and the
/// parameter it came from stays lent.
pub fn lent_captures(
    class: &Class,
    closure_types: &FastSet<usize>,
    lent: &FastSet<(usize, usize)>,
    optional: &FastSet<(usize, usize)>,
) -> FastSet<(usize, String)> {
    let mut out: FastSet<(usize, String)> = FastSet::default();
    // A lambda inside a lambda's method captures the outer one's capture: outer first.
    loop {
        let mut found = Vec::new();
        for (m, method) in class.methods.iter().enumerate() {
            let source = |e: &Expr| match &peel_closure_casts(e, closure_types, class).kind {
                ExprKind::Var(v) => (method.params.iter().position(|p| p.name == *v))
                    .is_some_and(|i| lent.contains(&(m, i)) && !optional.contains(&(m, i))),
                ExprKind::Field { object, name } if matches!(object.kind, ExprKind::This) => {
                    out.contains(&(method.class, name.clone()))
                }
                _ => false,
            };
            for s in &method.body {
                visit_stmt(
                    s,
                    &mut |e| {
                        visit_expr(e, &mut |x| {
                            for (ctor, captures) in lent_lambdas(class, closure_types, x, lent) {
                                let lambda = class.methods[ctor].class;
                                for (k, capture) in captures.iter().enumerate() {
                                    if source(capture)
                                        && let Some(field) = capture_field(&class.methods[ctor], k)
                                        && !out.contains(&(lambda, field.to_string()))
                                        && field_lent(class, closure_types, lambda, field, lent)
                                    {
                                        found.push((lambda, field.to_string()));
                                    }
                                }
                            }
                        })
                    },
                    &mut |_| {},
                );
            }
        }
        if found.is_empty() {
            break;
        }
        out.extend(found);
    }
    out
}

/// A closure variable: a local or parameter of the method, or a capture of the lambda whose
/// method it is, `this.f`.
#[derive(Clone, Copy)]
enum Read<'n> {
    Var(&'n str),
    ThisField(&'n str),
}

impl Read<'_> {
    /// Whether `e` reads the variable.
    fn reads(self, e: &Expr) -> bool {
        match (&e.kind, self) {
            (ExprKind::Var(v), Read::Var(name)) => v == name,
            (ExprKind::Field { object, name: f }, Read::ThisField(name)) => {
                matches!(object.kind, ExprKind::This) && f == name
            }
            _ => false,
        }
    }
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

/// Whether every use of the closure variable `read` in `stmts` is a call of its type's method on
/// it, an argument of a parameter of `lent`, or, where it may be captured (`capturable`), a
/// capture of a lambda passed to such a parameter that only calls and lends it on.
fn only_called(
    class: &Class,
    closure_types: &FastSet<usize>,
    stmts: &[Stmt],
    read: Read,
    lent: &FastSet<(usize, usize)>,
    capturable: bool,
) -> bool {
    let mut uses = Vec::new();
    let mut allowed = FastSet::default();
    for s in stmts {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if read.reads(x) {
                        uses.push(x.id);
                    }
                    allowed.extend(called_or_lent(class, closure_types, x, read, lent, capturable));
                })
            },
            &mut |_| {},
        );
    }
    uses.iter().all(|id| allowed.contains(id))
}

/// The reads of the closure variable `read` that `x` calls the closure of, as its receiver,
/// passes as an argument of a parameter of `lent`, or, where `capturable`, captures in a lambda
/// passed to such a parameter whose methods only call and lend that capture on.
fn called_or_lent(
    class: &Class,
    closure_types: &FastSet<usize>,
    x: &Expr,
    read: Read,
    lent: &FastSet<(usize, usize)>,
    capturable: bool,
) -> Vec<ExprId> {
    let mut out = Vec::new();
    if let ExprKind::MethodCall { receiver, resolved: Some(r), .. } = &x.kind
        && read.reads(receiver)
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
                out.extend(closure_read(a, read, closure_types, class));
            }
        }
    }
    if capturable {
        for (ctor, captures) in lent_lambdas(class, closure_types, x, lent) {
            let lambda = class.methods[ctor].class;
            for (k, capture) in captures.iter().enumerate() {
                if let Some(id) = closure_read(capture, read, closure_types, class)
                    && let Some(field) = capture_field(&class.methods[ctor], k)
                    && field_lent(class, closure_types, lambda, field, lent)
                {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// The lambdas that the call `x` passes to parameters of `lent`, as each one's constructor and
/// the captures it is given: `Cast(closure type, New { lambda class })`, through casts between
/// closure types, as the generator writes such a closure on the stack.
fn lent_lambdas<'e>(
    class: &Class,
    closure_types: &FastSet<usize>,
    x: &'e Expr,
    lent: &FastSet<(usize, usize)>,
) -> Vec<(usize, &'e [Expr])> {
    let (ExprKind::Call { args, resolved: Some(callee), .. }
    | ExprKind::MethodCall { args, resolved: Some(callee), .. }) = &x.kind
    else {
        return Vec::new();
    };
    let closure = |ty: Type| class.class_index(ty).is_some_and(|t| closure_types.contains(&t));
    let mut out = Vec::new();
    for (j, a) in args.iter().enumerate() {
        if !lent.contains(&(*callee, j)) {
            continue;
        }
        let mut e = a;
        while let ExprKind::Cast(to, inner) = &e.kind
            && closure(*to)
        {
            if let ExprKind::New { args: captures, ctor: Some(ctor), .. } = &inner.kind
                && matches!(class.types[class.methods[*ctor].class].origin, Origin::Lambda { .. })
            {
                out.push((*ctor, &captures[..]));
                break;
            }
            e = inner;
        }
    }
    out
}

/// The field of the lambda class whose constructor is `ctor` that its parameter `k` is stored in.
fn capture_field(ctor: &Method, k: usize) -> Option<&str> {
    let param = &ctor.params.get(k)?.name;
    ctor.body.iter().find_map(|s| match s {
        Stmt::Expr(Expr {
            kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
            ..
        }) if matches!(object.kind, ExprKind::This)
            && matches!(&value.kind, ExprKind::Var(v) if v == param) =>
        {
            Some(name.as_str())
        }
        _ => None,
    })
}

/// Whether the methods of lambda class `lambda` only call its capture `field` and lend it on
/// (`only_called`), so that it may hold a lent closure.
fn field_lent(
    class: &Class,
    closure_types: &FastSet<usize>,
    lambda: usize,
    field: &str,
    lent: &FastSet<(usize, usize)>,
) -> bool {
    (class.methods.iter())
        .filter(|m| m.class == lambda && !m.is_ctor)
        .all(|m| only_called(class, closure_types, &m.body, Read::ThisField(field), lent, true))
}

/// `e` without its casts between closure types.
fn peel_closure_casts<'e>(e: &'e Expr, closure_types: &FastSet<usize>, class: &Class) -> &'e Expr {
    match &e.kind {
        ExprKind::Cast(to, inner) if class.class_index(*to).is_some_and(|t| closure_types.contains(&t)) => {
            peel_closure_casts(inner, closure_types, class)
        }
        _ => e,
    }
}

/// The node inside `e` that reads the closure variable `read`, when `e` reads it, through casts
/// to closure types.
fn closure_read(e: &Expr, read: Read, closure_types: &FastSet<usize>, class: &Class) -> Option<ExprId> {
    let inner = peel_closure_casts(e, closure_types, class);
    read.reads(inner).then_some(inner.id)
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
            if !assigned.contains(name)
                && only_called(class, closure_types, &m.body, Read::Var(name), lent, false)
            {
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
