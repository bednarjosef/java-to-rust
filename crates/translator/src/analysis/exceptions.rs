//! Which methods throw. A method returns `Result<_, Throwable>` exactly when some caller
//! observes an exception that may escape it: the call is inside a `try` whose `catch` matches the
//! exception, or in a method whose own callers observe it. Every other exception ends the program
//! where it is thrown, as Java's would once nothing catches it, so the methods that nothing
//! observes keep their plain signatures.
//!
//! Two fixpoints over the calls (every implementation a virtual call may run, constructors,
//! `this(...)`, `super(...)`, and the `toString` that printing an object runs):
//!
//! - what may *escape* each method: the classes that its `throw`s throw and that its calls let
//!   escape, minus what an enclosing `catch` of the method catches;
//! - what the callers of each method *observe*: the classes that the `catch` clauses around each
//!   call match, and what the caller's own callers observe.
//!
//! The failures of the runtime (a division by zero, an index out of bounds, a null dereference,
//! a malformed number) end the program in the translation, except where a `catch` or a
//! `finally` would see one: there the operation is written with the runtime's checked form
//! (`Facts::raising`), whose failure is an exception of the program like a `throw`'s, and its
//! standard class gets objects (`Facts::raised_classes`). An operation that has no checked form
//! yet (a null dereference, an array creation, a cast, ...) is rejected where its failure would
//! be seen.

use super::Facts;
use super::hierarchy;
use super::nullness::{maybe_null, non_null_names};
use crate::ast::*;
use crate::lower::standard_subclass;
use std::collections::{BTreeSet, HashSet};

/// An exception that may be raised: an object of a concrete class of the program (or a standard
/// class it creates), or a failure of the runtime, by its standard class's name (`"?"` when the
/// operation's failures are not known exactly).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Class(usize),
    Runtime(&'static str),
}

/// What a point raises.
enum Raise {
    /// `throw`: an object of a class under this type.
    Throw(usize),
    /// A call of one of these methods.
    Call(Vec<usize>),
    /// A failure of the runtime, of the standard class `class`, by the expression `id`;
    /// `checked`: the generator has the runtime's checked form of the operation.
    Runtime { class: &'static str, checked: bool, id: ExprId },
}

/// A `try` around a point, whose body or one of whose `catch` clauses the point is in: what its
/// `catch` clauses catch (nothing, for a point in a `catch` clause).
#[derive(Clone)]
struct Frame {
    /// The address of the `try` statement.
    id: usize,
    catches: Vec<usize>,
    finally: bool,
}

struct Point {
    raise: Raise,
    pos: Pos,
    /// What the point raises, in words, for an error message.
    what: &'static str,
    frames: Vec<Frame>,
}

/// What the callers of a method observe: the exceptions under these types, or all of them.
#[derive(Clone, Default, PartialEq)]
struct Seen {
    types: BTreeSet<usize>,
    all: bool,
}

pub(super) fn analyze(class: &Class, facts: &mut Facts) -> Result<(), Error> {
    if class.type_index("java.lang.Throwable").is_none() {
        return Ok(());
    }
    let h = &class.hierarchy;
    let points: Vec<Vec<Point>> = (0..class.methods.len()).map(|m| method_points(class, facts, m)).collect();
    let matches = |k: &Kind, t: usize| match k {
        Kind::Class(c) => h.is_subtype(*c, t),
        Kind::Runtime(name) => {
            let decl = &class.types[t].name;
            crate::lower::is_standard(decl) && (*name == "?" || standard_subclass(name, decl))
        }
    };
    let caught = |k: &Kind, frames: &[Frame]| frames.iter().any(|f| f.catches.iter().any(|&t| matches(k, t)));
    let raised = |p: &Point, escapes: &[BTreeSet<Kind>]| -> BTreeSet<Kind> {
        match &p.raise {
            Raise::Throw(t) => h.concretes(*t).into_iter().map(Kind::Class).collect(),
            Raise::Call(callees) => callees.iter().flat_map(|&c| escapes[c].iter().cloned()).collect(),
            Raise::Runtime { class, .. } => BTreeSet::from([Kind::Runtime(class)]),
        }
    };

    // What escapes each method.
    let mut escapes: Vec<BTreeSet<Kind>> = vec![BTreeSet::new(); class.methods.len()];
    loop {
        let mut changed = false;
        for m in 0..class.methods.len() {
            let mut out = BTreeSet::new();
            for p in &points[m] {
                out.extend(raised(p, &escapes).into_iter().filter(|k| !caught(k, &p.frames)));
            }
            if out != escapes[m] {
                escapes[m] = out;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // What the callers of each method observe.
    let mut seen: Vec<Seen> = vec![Seen::default(); class.methods.len()];
    loop {
        let mut next = seen.clone();
        for m in 0..class.methods.len() {
            for p in &points[m] {
                let Raise::Call(callees) = &p.raise else { continue };
                let mut here = seen[m].clone();
                for f in &p.frames {
                    here.types.extend(f.catches.iter().copied());
                    here.all |= f.finally;
                }
                for &c in callees {
                    next[c].types.extend(here.types.iter().copied());
                    next[c].all |= here.all;
                }
            }
        }
        // A call may run any method of an override group.
        for m in 0..class.methods.len() {
            for g in h.group(m) {
                if g != m {
                    let other = next[g].clone();
                    next[m].types.extend(other.types);
                    next[m].all |= other.all;
                }
            }
        }
        if next == seen {
            break;
        }
        seen = next;
    }
    let observed = |k: &Kind, s: &Seen| s.all || s.types.iter().any(|&t| matches(k, t));

    let mut throwing: HashSet<usize> =
        (0..class.methods.len()).filter(|&m| escapes[m].iter().any(|k| observed(k, &seen[m]))).collect();
    let groups: Vec<usize> = throwing.iter().flat_map(|&m| h.group(m)).collect();
    throwing.extend(groups);

    // The failures of the runtime that something would see, the `try`s that catch something,
    // and those that an exception of the program leaves.
    let mut live_tries = HashSet::new();
    let mut raising_tries = HashSet::new();
    let mut raising = HashSet::new();
    let mut raised_classes = BTreeSet::new();
    for (m, method) in class.methods.iter().enumerate() {
        for p in &points[m] {
            for k in raised(p, &escapes) {
                let catcher = p.frames.iter().rev().find(|f| f.catches.iter().any(|&t| matches(&k, t)));
                if let Some(f) = catcher {
                    live_tries.insert(f.id);
                }
                for f in p.frames.iter().rev() {
                    raising_tries.insert(f.id);
                    if f.catches.iter().any(|&t| matches(&k, t)) {
                        break;
                    }
                }
                let Kind::Runtime(name) = k else { continue };
                let Raise::Runtime { checked, id, .. } = p.raise else { continue };
                let seen_here = catcher.is_some()
                    || p.frames.iter().any(|f| f.finally)
                    || observed(&Kind::Runtime(name), &seen[m]);
                let raised_class = class.type_index(name).filter(|&c| class.hierarchy.is_concrete(c));
                if seen_here
                    && checked
                    && let Some(c) = raised_class
                {
                    raising.insert(id);
                    raised_classes.insert(c);
                } else if seen_here {
                    let simple = name.rsplit('.').next().unwrap_or_default();
                    let exception =
                        if name == "?" { "an exception".to_string() } else { format!("the `{simple}`") };
                    return error(
                        p.pos,
                        format!("catching {exception} that {} may throw not supported yet", p.what),
                    );
                }
            }
        }
        if !throwing.contains(&m) {
            continue;
        }
        if method.is_main {
            return error(method.pos, "a call of `main` that is caught not supported yet");
        }
        // An exception object is built where it is thrown, and an enum constant where the enum
        // is initialized, outside any `try` of the program.
        let throwable = class.type_index("java.lang.Throwable");
        let decl = &class.types[method.class];
        if method.is_ctor && throwable.is_some_and(|t| class.hierarchy.is_subtype(method.class, t)) {
            return error(method.pos, "a constructor of an exception that throws not supported yet");
        }
        if method.is_ctor && decl.enum_constants.is_some() {
            return error(method.pos, "a constructor of an enum that throws not supported yet");
        }
        if method.name == "toString" && method.params.is_empty() {
            return error(method.pos, "a toString that throws an exception that is caught not supported yet");
        }
    }

    // A try-with-resources closes its resources in a `finally`, which is Java's behaviour only
    // when `close` cannot fail (see `lower/exceptions.rs`).
    let fails = may_fail(class, facts);
    for &(close, pos) in &class.closes {
        if class.hierarchy.group(close).iter().any(|&g| fails[g]) {
            return error(pos, "a resource whose `close` may fail not supported yet");
        }
    }

    // Static initializers, and the initializers of fields and enum constants, run outside any
    // `try` of the program: Java would wrap what they throw in `ExceptionInInitializerError`.
    let escapes_ref = &escapes;
    let initializer_raises = |e: &Expr, m: usize| {
        let mut out = Vec::new();
        expr_points(class, facts, m, e, &[], &[], &mut out);
        out.iter()
            .any(|p| raised(p, escapes_ref).iter().any(|k| matches!(k, Kind::Class(_))))
            .then_some(e.pos)
    };
    for (m, method) in class.methods.iter().enumerate() {
        if method.name == "<clinit>" && escapes[m].iter().any(|k| matches!(k, Kind::Class(_))) {
            return error(method.pos, "a static initializer that throws not supported yet");
        }
    }
    for decl in &class.types {
        for f in decl.fields.iter().chain(&decl.statics) {
            if let Some(pos) = f.init.as_ref().and_then(|e| initializer_raises(e, usize::MAX)) {
                return error(pos, "a field initializer that throws not supported yet");
            }
        }
        for constant in decl.enum_constants.iter().flatten() {
            if let Some(pos) = constant.values.iter().find_map(|e| initializer_raises(e, usize::MAX)) {
                return error(pos, "an enum constant whose arguments throw not supported yet");
            }
        }
    }

    // `throw null` throws a `NullPointerException` instead.
    for (m, method) in class.methods.iter().enumerate() {
        let mut found = None;
        for s in &method.body {
            visit_stmt(s, &mut |_| {}, &mut |s| {
                if let Stmt::Throw(e) = s {
                    let thrown = match &e.kind {
                        ExprKind::Cast(_, inner) => inner,
                        _ => e,
                    };
                    if matches!(thrown.kind, ExprKind::Null) || maybe_null(facts, m, thrown) {
                        found.get_or_insert(e.pos);
                    }
                }
            });
        }
        if let Some(pos) = found {
            return error(pos, "throwing a value that may be null not supported yet");
        }
    }

    facts.throwing = throwing;
    facts.live_tries = live_tries;
    facts.raising_tries = raising_tries;
    facts.raising = raising;
    facts.raised_classes = raised_classes;
    Ok(())
}

/// Which methods may fail at all: throw, fail in the runtime (whether or not a handler would see
/// it), or call a method that may fail.
fn may_fail(class: &Class, facts: &Facts) -> Vec<bool> {
    let points: Vec<Vec<Point>> = (0..class.methods.len()).map(|m| method_points(class, facts, m)).collect();
    let mut fails = vec![false; class.methods.len()];
    loop {
        let mut changed = false;
        for m in 0..class.methods.len() {
            if !fails[m]
                && points[m].iter().any(|p| match &p.raise {
                    Raise::Call(callees) => callees.iter().any(|&c| fails[c]),
                    Raise::Throw(_) | Raise::Runtime { .. } => true,
                })
            {
                fails[m] = true;
                changed = true;
            }
        }
        if !changed {
            return fails;
        }
    }
}

/// The points of method `m` that may raise an exception, with the `try`s around each.
fn method_points(class: &Class, facts: &Facts, m: usize) -> Vec<Point> {
    let method = &class.methods[m];
    let mut out = Vec::new();
    let mut frames = Vec::new();
    for s in &method.body {
        stmt_points(class, facts, m, s, &mut frames, &[], &mut out);
    }
    // A constructor runs the initializers of its class's fields, unless it starts with `this(..)`.
    if method.is_ctor
        && !matches!(method.body.first(), Some(Stmt::Expr(Expr { kind: ExprKind::ThisCall { .. }, .. })))
    {
        for f in &class.types[method.class].fields {
            if let Some(init) = &f.init {
                expr_points(class, facts, m, init, &[], &[], &mut out);
            }
        }
    }
    out
}

/// The names that `cond` proves not null when it is `when`, added to `safe`, leaving out those
/// that `assigned` says the code they hold for may assign.
fn narrowed(safe: &[String], cond: &Expr, when: bool, assigned: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out = safe.to_vec();
    out.extend(non_null_names(cond, when).into_iter().filter(|v| !assigned(v)).map(str::to_string));
    out
}

/// The points of statement `s`, with the `try`s around it (`frames`) and the variables known not
/// to be null there (`safe`: tested with `!= null` by an enclosing condition, and not assigned
/// in the code that the test guards).
fn stmt_points(
    class: &Class,
    facts: &Facts,
    m: usize,
    s: &Stmt,
    frames: &mut Vec<Frame>,
    safe: &[String],
    out: &mut Vec<Point>,
) {
    match s {
        Stmt::If { cond, then, otherwise } => {
            expr_points(class, facts, m, cond, frames, safe, out);
            let then_safe = narrowed(safe, cond, true, |v| assigns_stmt(then, v));
            stmt_points(class, facts, m, then, frames, &then_safe, out);
            if let Some(o) = otherwise {
                let else_safe = narrowed(safe, cond, false, |v| assigns_stmt(o, v));
                stmt_points(class, facts, m, o, frames, &else_safe, out);
            }
        }
        Stmt::While { cond, body } => {
            expr_points(class, facts, m, cond, frames, safe, out);
            let body_safe = narrowed(safe, cond, true, |v| assigns_stmt(body, v));
            stmt_points(class, facts, m, body, frames, &body_safe, out);
        }
        Stmt::For { init, cond, update, body } => {
            for i in init {
                stmt_points(class, facts, m, i, frames, safe, out);
            }
            let body_safe = match cond {
                Some(c) => {
                    expr_points(class, facts, m, c, frames, safe, out);
                    narrowed(safe, c, true, |v| assigns_stmt(body, v))
                }
                None => safe.to_vec(),
            };
            for u in update {
                expr_points(class, facts, m, u, frames, safe, out);
            }
            stmt_points(class, facts, m, body, frames, &body_safe, out);
        }
        Stmt::Throw(e) => {
            expr_points(class, facts, m, e, frames, safe, out);
            // The value is converted to `Throwable`; what it is comes from below the conversion.
            let thrown = match &e.kind {
                ExprKind::Cast(_, inner) => inner,
                _ => e,
            };
            if let Some(t) = class.class_index(thrown.ty) {
                out.push(Point {
                    raise: Raise::Throw(t),
                    pos: e.pos,
                    what: "this throw",
                    frames: frames.clone(),
                });
            }
        }
        Stmt::Try { body, catches, finally } => {
            frames.push(Frame {
                id: s as *const Stmt as usize,
                catches: catches.iter().flat_map(|c| c.types.iter().copied()).collect(),
                finally: finally.is_some(),
            });
            for b in body {
                stmt_points(class, facts, m, b, frames, safe, out);
            }
            frames.pop();
            // What a `catch` clause throws leaves the `try`, through its `finally`.
            frames.push(Frame {
                id: s as *const Stmt as usize,
                catches: Vec::new(),
                finally: finally.is_some(),
            });
            for catch in catches {
                for b in &catch.body {
                    stmt_points(class, facts, m, b, frames, safe, out);
                }
            }
            frames.pop();
            for b in finally.iter().flatten() {
                stmt_points(class, facts, m, b, frames, safe, out);
            }
        }
        _ => {
            // The expressions of this statement, then its nested statements.
            visit_own(s, &mut |e| expr_points(class, facts, m, e, frames, safe, out));
            for nested in children(s) {
                stmt_points(class, facts, m, nested, frames, safe, out);
            }
        }
    }
}

/// Calls `f` on the expressions that a statement holds itself (not those of nested statements).
fn visit_own(s: &Stmt, f: &mut dyn FnMut(&Expr)) {
    match s {
        Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Yield(e) => f(e),
        Stmt::If { cond, .. } | Stmt::While { cond, .. } | Stmt::DoWhile { cond, .. } => f(cond),
        Stmt::For { cond, update, .. } => {
            cond.iter().for_each(&mut *f);
            update.iter().for_each(f);
        }
        Stmt::ForEach { array, .. } => f(array),
        Stmt::Switch { selector, cases } => {
            f(selector);
            for case in cases {
                for label in &case.labels {
                    if let CaseLabel::Value(e) = label {
                        f(e);
                    }
                }
            }
        }
        _ => {}
    }
}

/// The statements nested directly in a statement.
fn children(s: &Stmt) -> Vec<&Stmt> {
    match s {
        Stmt::If { then, otherwise, .. } => std::iter::once(&**then).chain(otherwise.as_deref()).collect(),
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::ForEach { body, .. } => vec![body],
        Stmt::Labeled { body, .. } => vec![body],
        Stmt::For { init, body, .. } => init.iter().chain(std::iter::once(&**body)).collect(),
        Stmt::Block(stmts) => stmts.iter().collect(),
        Stmt::Switch { cases, .. } => cases.iter().flat_map(|c| &c.body).collect(),
        _ => Vec::new(),
    }
}

/// The points inside an expression: its calls, the `toString`s it runs, and the failures of the
/// runtime it may raise, in the order Java evaluates them.
fn expr_points(
    class: &Class,
    facts: &Facts,
    m: usize,
    x: &Expr,
    frames: &[Frame],
    safe: &[String],
    out: &mut Vec<Point>,
) {
    let mut push = |raise: Raise, pos: Pos, what: &'static str| {
        out.push(Point { raise, pos, what, frames: frames.to_vec() });
    };
    let callees = class.callees(x);
    if !callees.is_empty() {
        push(Raise::Call(callees), x.pos, "this call");
    }
    for object in text_objects(x) {
        let shown = to_string_impls(class, object.ty);
        if !shown.is_empty() {
            push(Raise::Call(shown), object.pos, "this toString");
        }
    }
    for (name, what, checked) in runtime_failures(class, facts, m, x, safe) {
        push(Raise::Runtime { class: name, checked, id: x.id }, x.pos, what);
    }
    if let ExprKind::Switch { selector, cases } = &x.kind {
        // The statements of a switch expression's cases may `throw` and hold `try`s.
        expr_points(class, facts, m, selector, frames, safe, out);
        let mut nested = frames.to_vec();
        for case in cases {
            for s in &case.body {
                stmt_points(class, facts, m, s, &mut nested, safe, out);
            }
        }
        return;
    }
    // The right of `&&` and `||`, and the arms of `?:`, see what their condition proves.
    match &x.kind {
        ExprKind::Binary(op @ (BinOp::And | BinOp::Or), l, r) => {
            expr_points(class, facts, m, l, frames, safe, out);
            let right_safe = narrowed(safe, l, *op == BinOp::And, |v| assigns(r, v));
            expr_points(class, facts, m, r, frames, &right_safe, out);
            return;
        }
        ExprKind::Conditional(c, a, b) => {
            expr_points(class, facts, m, c, frames, safe, out);
            let a_safe = narrowed(safe, c, true, |v| assigns(a, v));
            expr_points(class, facts, m, a, frames, &a_safe, out);
            let b_safe = narrowed(safe, c, false, |v| assigns(b, v));
            expr_points(class, facts, m, b, frames, &b_safe, out);
            return;
        }
        _ => {}
    }
    let mut children = Vec::new();
    match &x.kind {
        ExprKind::Unary(_, a)
        | ExprKind::Cast(_, a)
        | ExprKind::Length(a)
        | ExprKind::Field { object: a, .. } => children.push(&**a),
        ExprKind::Assign { target, value, .. } => {
            place_children(target, &mut children);
            children.push(value);
        }
        ExprKind::IncDec { target, .. } => place_children(target, &mut children),
        ExprKind::Binary(_, l, r) | ExprKind::Index(l, r) => children.extend([&**l, &**r]),
        ExprKind::NewArray(items) | ExprKind::ArrayLit(items) => children.extend(items),
        ExprKind::Conditional(c, a, b) => children.extend([&**c, &**a, &**b]),
        ExprKind::MethodCall { receiver, args, .. } => {
            children.push(receiver);
            children.extend(args);
        }
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. } => children.extend(args),
        _ => {}
    }
    for c in children {
        expr_points(class, facts, m, c, frames, safe, out);
    }
}

fn place_children<'e>(place: &'e Place, out: &mut Vec<&'e Expr>) {
    match place {
        Place::Index(array, index) => out.extend([&**array, &**index]),
        Place::Field { object, .. } => out.push(object),
        Place::Var(_) | Place::Static { .. } => {}
    }
}

/// The objects whose text an expression makes, which runs their `toString`.
fn text_objects(x: &Expr) -> Vec<&Expr> {
    let is_object = |a: &&Expr| matches!(a.ty, Type::Class(..));
    match &x.kind {
        ExprKind::Builtin(Builtin::ClassName { .. }, _) => Vec::new(),
        ExprKind::Builtin(_, args) => args.iter().filter(is_object).collect(),
        ExprKind::Binary(BinOp::Add, l, r) if x.ty == Type::String => {
            [&**l, &**r].into_iter().filter(is_object).collect()
        }
        ExprKind::Assign { op: Some(BinOp::Add), value, .. } if x.ty == Type::String => {
            std::iter::once(&**value).filter(is_object).collect()
        }
        _ => Vec::new(),
    }
}

/// The `toString` methods that showing a value of type `ty` may run.
fn to_string_impls(class: &Class, ty: Type) -> Vec<usize> {
    let Some(t) = class.class_index(ty) else { return Vec::new() };
    let mut out = Vec::new();
    for c in class.hierarchy.concretes(t) {
        if let Some(i) = hierarchy::to_string_impl(class, c)
            && !out.contains(&i)
        {
            out.push(i);
        }
    }
    out
}

/// What a method of the library that the runtime implements may throw besides a
/// `NullPointerException` for a null argument: `None` when nothing, `"?"` when it is not known
/// exactly.
/// Whether `builtin` is a regular expression call whose pattern (and replacement) are literals
/// that compile, so that it never throws.
fn constant_pattern(builtin: &Builtin, args: &[Expr]) -> bool {
    let text = |i: usize| match args.get(i).map(|a| &a.kind) {
        Some(ExprKind::Str(s)) => Some(s.as_str()),
        _ => None,
    };
    match builtin {
        Builtin::String(StringOp::Split | StringOp::Matches) => {
            text(1).is_some_and(|p| java_runtime::regex::never_fails(p, None))
        }
        Builtin::String(StringOp::ReplaceAll | StringOp::ReplaceFirst) => match (text(1), text(2)) {
            (Some(p), Some(r)) => java_runtime::regex::never_fails(p, Some(r)),
            _ => false,
        },
        _ => false,
    }
}

fn library_failure(builtin: &Builtin) -> Option<&'static str> {
    match builtin {
        Builtin::String(op) => match op {
            StringOp::Substring | StringOp::FromChars => Some("java.lang.StringIndexOutOfBoundsException"),
            StringOp::Repeat => Some("java.lang.IllegalArgumentException"),
            // A malformed pattern, a bad group in a replacement, an unknown charset.
            StringOp::ReplaceAll
            | StringOp::ReplaceFirst
            | StringOp::Matches
            | StringOp::Split
            | StringOp::GetBytes
            | StringOp::FromBytes => Some("?"),
            _ => None,
        },
        Builtin::Builder(
            BuilderOp::New
            | BuilderOp::Append
            | BuilderOp::Reverse
            | BuilderOp::IndexOf
            | BuilderOp::LastIndexOf
            | BuilderOp::Length
            | BuilderOp::ToString
            | BuilderOp::IsEmpty
            | BuilderOp::CompareTo,
        ) => None,
        // An index out of the builder's bounds.
        Builtin::Builder(_) => Some("?"),
        Builtin::ArraysSort | Builtin::ArraysToString => None,
        Builtin::Arrays(
            ArraysOp::Equals | ArraysOp::HashCode | ArraysOp::SortAll | ArraysOp::BinarySearch,
        ) => None,
        Builtin::Arrays(ArraysOp::DeepToString | ArraysOp::Clone) => None,
        Builtin::Bits(BitsOp::ParseRadix, _) => Some("java.lang.NumberFormatException"),
        Builtin::EnumValueOf => Some("java.lang.IllegalArgumentException"),
        Builtin::Bits(..) => None,
        Builtin::Boxed(BoxedOp::Unbox | BoxedOp::Equals, _) => None,
        Builtin::Unsigned(UnsignedOp::Compare | UnsignedOp::ToString | UnsignedOp::Widen, _) => None,
        Builtin::CodePoint(
            CodePointOp::CharCount
            | CodePointOp::ToCodePoint
            | CodePointOp::HighSurrogate
            | CodePointOp::LowSurrogate
            | CodePointOp::IsValid
            | CodePointOp::IsBmp
            | CodePointOp::IsSupplementary,
        ) => None,
        _ => Some("?"),
    }
}

/// The failures of the runtime that evaluating `x` itself (not its operands) may raise, by
/// standard class name, with what raises them in words.
fn runtime_failures(
    class: &Class,
    facts: &Facts,
    m: usize,
    x: &Expr,
    safe: &[String],
) -> Vec<(&'static str, &'static str, bool)> {
    const NPE: &str = "java.lang.NullPointerException";
    const INDEX: &str = "java.lang.ArrayIndexOutOfBoundsException";
    let tested = |e: &Expr| matches!(&e.kind, ExprKind::Var(v) if safe.contains(v));
    let null = |e: &Expr| {
        (m != usize::MAX && !tested(e) && maybe_null(facts, m, e)) || matches!(e.kind, ExprKind::Null)
    };
    let mut out = Vec::new();
    match &x.kind {
        ExprKind::Binary(BinOp::Div | BinOp::Rem, l, r)
            if l.ty.unboxed().is_integral() && r.ty.unboxed().is_integral() =>
        {
            let constant =
                matches!(r.kind, ExprKind::Int(v) if v != 0) || matches!(r.kind, ExprKind::Long(v) if v != 0);
            if !constant && !facts.safe_division.contains(&x.id) {
                out.push(("java.lang.ArithmeticException", "this division", true));
            }
        }
        ExprKind::Assign { op: Some(BinOp::Div | BinOp::Rem), value, .. }
            if x.ty.unboxed().is_integral() && value.ty.unboxed().is_integral() =>
        {
            let constant = matches!(value.kind, ExprKind::Int(v) if v != 0)
                || matches!(value.kind, ExprKind::Long(v) if v != 0);
            if !constant && !facts.safe_division.contains(&x.id) {
                out.push(("java.lang.ArithmeticException", "this division", true));
            }
        }
        ExprKind::Index(array, _) => {
            if null(array) {
                out.push((NPE, "this array access", false));
            }
            if !facts.in_bounds.contains(&x.id) {
                out.push((INDEX, "this array access", true));
            }
        }
        ExprKind::Length(array) if null(array) => out.push((NPE, "this length", false)),
        ExprKind::Field { object, .. } if null(object) => out.push((NPE, "this field access", false)),
        ExprKind::MethodCall { receiver, resolved: Some(r), is_super: false, .. }
            if !class.methods[*r].is_static && null(receiver) =>
        {
            out.push((NPE, "this call", false))
        }
        ExprKind::NewArray(sizes) => {
            if sizes.iter().any(|s| !matches!(s.kind, ExprKind::Int(v) if v >= 0)) {
                out.push(("java.lang.NegativeArraySizeException", "this array creation", false));
            }
        }
        ExprKind::Cast(to, value)
            if !to.is_reference() && matches!(value.ty, Type::Boxed(_)) && null(value) =>
        {
            out.push((NPE, "this unboxing", false))
        }
        ExprKind::Builtin(builtin, args) => match builtin {
            Builtin::Println
            | Builtin::Print
            | Builtin::MathAbs
            | Builtin::MathMax
            | Builtin::MathMin
            | Builtin::MathSqrt
            | Builtin::MathPow
            | Builtin::CharIsDigit
            | Builtin::CharIsLetter
            | Builtin::CharIsUpperCase
            | Builtin::CharIsLowerCase
            | Builtin::CharToUpperCase
            | Builtin::CharToLowerCase
            | Builtin::CharIsWhitespace
            | Builtin::CharIsSpaceChar
            | Builtin::CharIsLetterOrDigit
            | Builtin::CharIsAlphabetic
            | Builtin::CharIsSurrogate
            | Builtin::CharIsHighSurrogate
            | Builtin::CharIsLowSurrogate
            | Builtin::CharDigit
            | Builtin::CharForDigit
            | Builtin::CharNumericValue
            | Builtin::EnumOrdinal
            | Builtin::EnumName
            | Builtin::EnumCompareTo
            | Builtin::Exit
            | Builtin::ClassName { .. }
            // A null is a value to them, and an object's own method is a call of its own.
            | Builtin::ObjectsEquals
            | Builtin::ObjectsHashCode => {}
            Builtin::StringLength | Builtin::StringEquals => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", false));
                }
            }
            Builtin::StringCharAt => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", false));
                }
                if !facts.index_in_bounds.contains(&x.id) {
                    out.push(("java.lang.StringIndexOutOfBoundsException", "this charAt", true));
                }
            }
            Builtin::Boxed(BoxedOp::Parse | BoxedOp::ParseRadix, _) => {
                out.push(("java.lang.NumberFormatException", "this parse", true));
            }
            Builtin::Boxed(
                BoxedOp::Box
                | BoxedOp::ToString
                | BoxedOp::Compare
                | BoxedOp::HashCode
                | BoxedOp::Sum
                | BoxedOp::FloatTest(_)
                | BoxedOp::Logical(_),
                _,
            ) => {}
            Builtin::NullCheck => out.push((NPE, "this synchronized", false)),
            _ => {
                // A null argument throws, except where the method takes null (an `equals`, the
                // elements of `join`, the text of `Arrays.toString`).
                let takes_null = matches!(
                    builtin,
                    Builtin::String(StringOp::EqualsIgnoreCase | StringOp::EqualsOther)
                        | Builtin::ArraysToString
                        | Builtin::Arrays(ArraysOp::Equals | ArraysOp::HashCode | ArraysOp::DeepToString)
                        | Builtin::Boxed(BoxedOp::Equals, _)
                );
                let checked = if takes_null || matches!(builtin, Builtin::String(StringOp::Join)) {
                    &args[..args.len().min(1)]
                } else {
                    &args[..]
                };
                if !takes_null && checked.iter().any(null) {
                    out.push((NPE, "this call", false));
                }
                if let Some(failure) = library_failure(builtin)
                    && !constant_pattern(builtin, args)
                {
                    let checked = matches!(
                        builtin,
                        Builtin::String(StringOp::Substring)
                            | Builtin::Bits(BitsOp::ParseRadix, _)
                            | Builtin::EnumValueOf
                    );
                    out.push((failure, "this call of the library", checked));
                }
            }
        },
        _ => {}
    }
    if let ExprKind::Assign { target: Place::Index(array, _), .. }
    | ExprKind::IncDec { target: Place::Index(array, _), .. } = &x.kind
    {
        if null(array) {
            out.push((NPE, "this array access", false));
        }
        if !facts.in_bounds.contains(&x.id) {
            out.push((INDEX, "this array access", true));
        }
    }
    if let ExprKind::Assign { target: Place::Field { object, .. }, .. }
    | ExprKind::IncDec { target: Place::Field { object, .. }, .. } = &x.kind
        && null(object)
    {
        out.push((NPE, "this field access", false));
    }
    // A cast to a class that the value's type is not under may fail.
    if let ExprKind::Cast(to, value) = &x.kind
        && let (Some(t), Some(s)) = (class.class_index(*to), class.class_index(value.ty))
        && !class.hierarchy.is_subtype(s, t)
    {
        out.push(("java.lang.ClassCastException", "this cast", false));
    }
    // A box that may be null is unboxed by arithmetic and comparisons.
    if let ExprKind::Binary(_, l, r) = &x.kind {
        for operand in [l, r] {
            if matches!(operand.ty, Type::Boxed(_)) && null(operand) && !matches!(x.ty, Type::String) {
                out.push((NPE, "this unboxing", false));
            }
        }
    }
    out
}
