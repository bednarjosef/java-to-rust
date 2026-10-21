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

/// What a static initializer that throws raises: Java wraps the exception in this `Error`.
const INITIALIZER_ERROR: &str = "java.lang.ExceptionInInitializerError";

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
    let initializer: Vec<bool> = (0..class.methods.len()).map(|m| class.is_initializer(m)).collect();
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
            if initializer[m] && !out.is_empty() {
                out = BTreeSet::from([Kind::Runtime(INITIALIZER_ERROR)]);
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

    // A static initializer never returns a `Result`: what it raises ends the program (see `seen`).
    let mut throwing: HashSet<usize> = (0..class.methods.len())
        .filter(|&m| !initializer[m] && escapes[m].iter().any(|k| observed(k, &seen[m])))
        .collect();
    let groups: Vec<usize> = throwing.iter().flat_map(|&m| h.group(m)).collect();
    throwing.extend(groups);

    // The failures of the runtime that something would see, the `try`s that catch something,
    // and those that an exception of the program leaves.
    let mut live_tries = HashSet::new();
    let mut raising_tries = HashSet::new();
    let mut raising = HashSet::new();
    let mut null_checked = std::collections::HashMap::new();
    let mut catching = std::collections::HashMap::new();
    // The `try`s whose `catch` gets a `NullPointerException` that the runtime raises.
    let mut runtime_npe_tries = HashSet::new();
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
                    if name == "java.lang.NullPointerException" {
                        null_checked.insert(id, p.pos);
                        if let Some(f) = catcher {
                            runtime_npe_tries.insert(f.id);
                        }
                    } else {
                        raising.insert(id);
                    }
                    raised_classes.insert(c);
                } else if seen_here && let Some(c) = raised_class {
                    // No checked form of its own: the operation runs inside `java::catching`.
                    catching.insert(id, p.pos);
                    if let Some(f) = catcher
                        && name == "java.lang.NullPointerException"
                    {
                        runtime_npe_tries.insert(f.id);
                    }
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

    // Java's message of a `NullPointerException` that the runtime raises describes the
    // dereference ("Cannot invoke ..."); the message of one that code creates is its own.
    let npe = Kind::Runtime("java.lang.NullPointerException");
    for method in &class.methods {
        let mut found = None;
        for s in &method.body {
            visit_stmt(s, &mut |_| {}, &mut |s| {
                let Stmt::Try { catches, .. } = s else { return };
                if !runtime_npe_tries.contains(&(s as *const Stmt as usize)) {
                    return;
                }
                let Some(catch) = catches.iter().find(|c| c.types.iter().any(|&t| matches(&npe, t))) else {
                    return;
                };
                for b in &catch.body {
                    visit_stmt(
                        b,
                        &mut |e| {
                            visit_expr(e, &mut |x| {
                                if let ExprKind::MethodCall { receiver, name, .. } = &x.kind
                                    && matches!(name.as_str(), "getMessage" | "getLocalizedMessage")
                                    && matches!(&receiver.kind, ExprKind::Var(v) if *v == catch.name)
                                {
                                    found.get_or_insert(x.pos);
                                }
                            })
                        },
                        &mut |_| {},
                    );
                }
            });
        }
        if let Some(pos) = found {
            return error(
                pos,
                "the message of a `NullPointerException` that the runtime raises not supported yet",
            );
        }
    }

    // A static initializer's exception reaches the program as `ExceptionInInitializerError`, an
    // `Error` that a `try` here is not known to match, so one that observes it is not translated.
    for (m, method) in class.methods.iter().enumerate() {
        if initializer[m] && !escapes[m].is_empty() && (seen[m].all || !seen[m].types.is_empty()) {
            return error(method.pos, "a static initializer that throws inside a `try` not supported yet");
        }
    }
    // The initializers of instance fields and of enum constants run outside any `try` of the
    // program: Java would wrap what they throw in `ExceptionInInitializerError`.
    let escapes_ref = &escapes;
    let initializer_raises = |e: &Expr, m: usize| {
        let mut out = Vec::new();
        expr_points(class, facts, m, e, &[], &[], &mut out);
        out.iter()
            .any(|p| raised(p, escapes_ref).iter().any(|k| matches!(k, Kind::Class(_))))
            .then_some(e.pos)
    };
    for decl in &class.types {
        for f in &decl.fields {
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

    facts.throwing = throwing;
    facts.live_tries = live_tries;
    facts.raising_tries = raising_tries;
    facts.raising = raising;
    facts.null_checked = null_checked;
    facts.catching = catching;
    facts.raised_classes = raised_classes;
    Ok(())
}

/// Which methods may fail at all: throw, fail in the runtime (whether or not a handler would see
/// it), or call a method that may fail.
/// The points of method `m` that may raise an exception, with the `try`s around each.
fn method_points(class: &Class, facts: &Facts, m: usize) -> Vec<Point> {
    let method = &class.methods[m];
    let mut out = Vec::new();
    let mut frames = Vec::new();
    block_points(class, facts, m, &method.body, &mut frames, &[], &mut out);
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
    for (v, fact) in proved(cond, when) {
        if !assigned(v) {
            out.push(fact);
        }
    }
    out
}

/// What `safe` holds for a variable that is an instance of the class `name`, besides not null.
fn instance_fact(v: &str, name: &str) -> String {
    format!("{v} instanceof {name}")
}

/// The name of the class or interface that an `instanceof` or a cast tests: one of the program,
/// a runtime class (a string or a box) or an enum.
fn instance_name(ty: Type) -> Option<&'static str> {
    match ty {
        Type::Class(name, _) | Type::Enum(name) => Some(name),
        Type::String | Type::Boxed(_) => runtime_name(ty),
        _ => None,
    }
}

/// What `cond` proves when it is `when`, by variable: that it is not null (its null tests), and
/// for a top-level `instanceof` when true, that it is an instance of the class too; through a
/// `!`, what its operand proves.
fn proved(cond: &Expr, when: bool) -> Vec<(&str, String)> {
    if let ExprKind::Unary(UnOp::Not, inner) = &cond.kind {
        return proved(inner, !when);
    }
    let mut out: Vec<(&str, String)> =
        non_null_names(cond, when).into_iter().map(|v| (v, v.to_string())).collect();
    if when {
        let mut parts = Vec::new();
        super::nullness::conjuncts(cond, &mut parts);
        for (k, part) in parts.iter().enumerate() {
            if let ExprKind::InstanceOf { value, ty, .. } = &part.kind
                && let ExprKind::Var(v) = &value.kind
                && !parts[k + 1..].iter().any(|later| assigns(later, v))
            {
                out.push((v, v.clone()));
                if let Some(name) = instance_name(*ty) {
                    out.push((v, instance_fact(v, name)));
                }
            }
        }
    }
    out
}

/// Whether `e` is a variable that `safe` knows not null, or a cast of one.
fn is_tested(e: &Expr, safe: &[String]) -> bool {
    match &e.kind {
        ExprKind::Var(v) => safe.contains(v),
        // A cast of a reference keeps its null.
        ExprKind::Cast(to, value) if to.is_reference() => is_tested(value, safe),
        _ => false,
    }
}

/// Whether `s` never completes normally: it returns, throws, breaks or continues.
fn leaves(s: &Stmt) -> bool {
    match s {
        Stmt::Return(..) | Stmt::Throw(_) | Stmt::Break | Stmt::Continue => true,
        Stmt::BreakLabel(_) | Stmt::ContinueLabel(_) => true,
        Stmt::Block(stmts) => stmts.last().is_some_and(leaves),
        _ => false,
    }
}

/// The points of a block's statements: after `if (cond) return;`, what `cond` proves when false
/// holds for the statements that follow, until one of them assigns the variable.
fn block_points(
    class: &Class,
    facts: &Facts,
    m: usize,
    stmts: &[Stmt],
    frames: &mut Vec<Frame>,
    safe: &[String],
    out: &mut Vec<Point>,
) {
    let mut safe = safe.to_vec();
    for (k, s) in stmts.iter().enumerate() {
        stmt_points(class, facts, m, s, frames, &safe, out);
        let rest = &stmts[k + 1..];
        let assigned = |v: &str| rest.iter().any(|r| assigns_stmt(r, v));
        match s {
            Stmt::If { cond, then, otherwise: None } if leaves(then) => {
                safe = narrowed(&safe, cond, false, assigned);
            }
            // A local set to a value known not null, such as `Point p = (Point) o;`.
            Stmt::Local { name, init: Some(init), .. } if is_tested(init, &safe) && !assigned(name) => {
                safe.push(name.clone());
            }
            _ => {}
        }
    }
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
    // A switch on a string that is null throws, as its selector is dereferenced.
    if let Stmt::Switch { selector, .. } = s
        && selector.ty == Type::String
        && !is_tested(selector, safe)
        && maybe_null(facts, m, selector)
    {
        let raise =
            Raise::Runtime { class: "java.lang.NullPointerException", checked: true, id: selector.id };
        out.push(Point { raise, pos: selector.pos, what: "this switch", frames: frames.clone() });
    }
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
            // A value that may be null throws a `NullPointerException` instead, by `throw` itself.
            if matches!(thrown.kind, ExprKind::Null) || maybe_null(facts, m, thrown) {
                let raise =
                    Raise::Runtime { class: "java.lang.NullPointerException", checked: true, id: thrown.id };
                out.push(Point { raise, pos: e.pos, what: "this throw", frames: frames.clone() });
            } else if let Some(t) = class.class_index(thrown.ty) {
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
        Stmt::Block(stmts) => block_points(class, facts, m, stmts, frames, safe, out),
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

/// The `<clinit>`s that evaluating `x` runs: those of the classes whose static field it reads or
/// writes, or whose static method or constructor it calls, unless the method `m` is in such a class
/// or in a subclass of it (as `Gen::init_call` decides). A constant is inlined and runs nothing.
fn static_inits(class: &Class, facts: &Facts, m: usize, x: &Expr) -> Vec<usize> {
    let mut classes = Vec::new();
    match &x.kind {
        ExprKind::Static { class: t, name } => {
            if !class.types[*t].statics.iter().any(|f| f.name == *name && f.constant) {
                classes.push(*t);
            }
        }
        ExprKind::Assign { target: Place::Static { class: t, .. }, .. } => classes.push(*t),
        // The first use of an enum's constant initializes the enum.
        ExprKind::EnumConstant { class: t, .. } => classes.push(*t),
        ExprKind::Builtin(Builtin::EnumValueOf, _) => {
            if let Type::Enum(name) = x.ty
                && let Some(t) = class.type_index(name)
            {
                classes.push(t);
            }
        }
        _ => {}
    }
    for c in class.callees(x) {
        let callee = &class.methods[c];
        if callee.is_static || callee.is_ctor {
            classes.push(callee.class);
        }
    }
    let current = class.methods.get(m).map(|c| c.class);
    classes
        .into_iter()
        .filter_map(|t| {
            let init = facts.init_class(class, t)?;
            let initialized =
                current.is_some_and(|c| c == init || class.hierarchy.ancestors(c).contains(&init));
            if initialized {
                return None;
            }
            class.methods.iter().position(|c| c.class == init && c.name == "<clinit>")
        })
        .collect()
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
    for init in static_inits(class, facts, m, x) {
        push(Raise::Call(vec![init]), x.pos, "this static use");
    }
    for object in text_objects(x) {
        let shown = to_string_impls(class, object.ty);
        if !shown.is_empty() {
            push(Raise::Call(shown), object.pos, "this toString");
        }
    }
    for (name, what, checked, id) in runtime_failures(class, facts, m, x, safe) {
        push(Raise::Runtime { class: name, checked, id }, x.pos, what);
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
        // What a started thread runs raises nothing where the thread is started: an exception
        // that escapes it ends that thread (the runtime prints it), whatever catches the start.
        ExprKind::Builtin(Builtin::Native(Native::ThreadStart), args) => children.extend(&args[..4]),
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
        ExprKind::Builtin(Builtin::ClassName { .. } | Builtin::ClassLiteralName { .. }, _) => Vec::new(),
        // The class, identity or ordinal of an object, which no `toString` gives.
        ExprKind::Builtin(
            Builtin::GetClass
            | Builtin::SameClass { .. }
            | Builtin::ClassMethod(_)
            | Builtin::ComparableClassFor
            | Builtin::EnumOrdinal
            | Builtin::EnumName
            | Builtin::EnumCompareTo
            | Builtin::Native(Native::IdentityHash),
            _,
        ) => Vec::new(),
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

/// Whether the generator has the checked form of dereferencing `object`: the object of a call or
/// a field access is written by `Gen::object` and its relatives, which check it with `non_null`.
fn object_checked(object: &Expr) -> bool {
    matches!(object.ty, Type::Class(..))
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

/// An argument of the runtime's `format` of each argument's kind, as the code generator passes
/// it (`format_arg`), with any value.
fn format_samples(args: &[Expr]) -> Vec<java_runtime::format::Arg<'static>> {
    use java_runtime::format::Arg;
    args.iter()
        .map(|a| match a.ty.unboxed() {
            Type::Null => Arg::Null,
            Type::Boolean => Arg::Bool(false),
            Type::Char => Arg::Char(0),
            Type::Byte => Arg::Byte(0),
            Type::Short => Arg::Short(0),
            Type::Int => Arg::Int(0),
            Type::Long => Arg::Long(0),
            Type::Float => Arg::Float(0.0),
            Type::Double => Arg::Double(0.0),
            Type::String => Arg::Str(""),
            _ => Arg::Text(""),
        })
        .collect()
}

fn library_failure(builtin: &Builtin, args: &[Expr]) -> Option<&'static str> {
    match builtin {
        Builtin::String(op) => match op {
            StringOp::Substring => Some("java.lang.StringIndexOutOfBoundsException"),
            // `valueOf(chars)` copies the whole array; only an offset and a count can be out of range.
            StringOp::FromChars if args.len() == 3 => Some("java.lang.StringIndexOutOfBoundsException"),
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
        Builtin::Bits(BitsOp::ParseRadix | BitsOp::ParseRange, _) => Some("java.lang.NumberFormatException"),
        Builtin::Unsigned(UnsignedOp::Parse | UnsignedOp::ParseRange, _) => {
            Some("java.lang.NumberFormatException")
        }
        Builtin::EnumValueOf => Some("java.lang.IllegalArgumentException"),
        Builtin::Bits(..) => None,
        Builtin::Boxed(BoxedOp::Unbox | BoxedOp::Equals, _) => None,
        Builtin::Unsigned(UnsignedOp::Compare | UnsignedOp::ToString | UnsignedOp::Widen, _) => None,
        // Overflow, and bounds the wrong way round.
        Builtin::Native(Native::Math { name, .. }) if name.ends_with("Exact") => {
            Some("java.lang.ArithmeticException")
        }
        Builtin::Native(Native::Math { name: "clamp", .. }) => Some("java.lang.IllegalArgumentException"),
        // The others are functions on numbers, which never throw.
        Builtin::Native(Native::Math { .. }) => None,
        // A negative time. Nothing interrupts a thread of a translated program, so `sleep` never
        // throws `InterruptedException`.
        Builtin::Native(Native::Sleep { .. }) => Some("java.lang.IllegalArgumentException"),
        // `Thread`'s checks are in Java (`java/Threads.java`); its natives cannot fail.
        Builtin::Native(
            Native::OnSpinWait
            | Native::ThreadYield
            | Native::ThreadNumber
            | Native::ThreadId
            | Native::MainThreadId
            | Native::HasCurrentThread
            | Native::SetCurrentThread
            | Native::CurrentThread
            | Native::ThreadStart
            | Native::ThreadAlive
            | Native::ThreadJoin
            | Native::ThreadRename,
        ) => None,
        // A monitor that the thread does not hold (and for `wait` a negative timeout, which
        // `runtime_failures` adds). Nothing interrupts a thread, so `wait` never throws
        // `InterruptedException`.
        Builtin::Native(Native::MonitorWait | Native::MonitorNotify { .. }) => {
            Some("java.lang.IllegalMonitorStateException")
        }
        Builtin::Native(Native::HoldsLock) => None,
        Builtin::CodePoint(CodePointOp::ToCharsInto) => Some("java.lang.IllegalArgumentException"),
        Builtin::CodePoint(
            CodePointOp::CharCount
            | CodePointOp::ToCodePoint
            | CodePointOp::HighSurrogate
            | CodePointOp::LowSurrogate
            | CodePointOp::IsValid
            | CodePointOp::IsBmp
            | CodePointOp::IsSupplementary
            | CodePointOp::IsLetter
            | CodePointOp::IsDigit
            | CodePointOp::IsLetterOrDigit
            | CodePointOp::IsWhitespace
            | CodePointOp::IsSpaceChar
            | CodePointOp::IsUpperCase
            | CodePointOp::IsLowerCase
            | CodePointOp::IsAlphabetic
            | CodePointOp::ToUpperCase
            | CodePointOp::ToLowerCase
            | CodePointOp::NumericValue
            | CodePointOp::Digit,
        ) => None,
        // A name that is not a charset: `UnsupportedCharsetException`, `IllegalCharsetNameException`
        // or, for null, `IllegalArgumentException`. Not checked, so a catch of it is rejected.
        Builtin::Charset(CharsetOp::ForName | CharsetOp::IsSupported) => Some("?"),
        Builtin::Charset(_) => None,
        // What an object's `toString` throws is a point of its own (`text_objects`).
        Builtin::ObjectText => None,
        Builtin::ArraysCopyOf => Some("java.lang.NegativeArraySizeException"),
        Builtin::Native(Native::ArrayCopy) => Some("java.lang.ArrayIndexOutOfBoundsException"),
        // A literal format that fits its arguments' types never throws.
        Builtin::Format => match &args[0].kind {
            ExprKind::Str(format)
                if java_runtime::format::never_fails(format, &format_samples(&args[1..])) =>
            {
                None
            }
            _ => Some("?"),
        },
        _ => Some("?"),
    }
}

/// The failures of the runtime that evaluating `x` itself (not its operands) may raise, by
/// standard class name, with what raises them in words, whether the generator has their checked
/// form, and the expression that the checked form is written for: `x`, or for a
/// `NullPointerException` of a call or a field access, the object that is dereferenced.
fn runtime_failures(
    class: &Class,
    facts: &Facts,
    m: usize,
    x: &Expr,
    safe: &[String],
) -> Vec<(&'static str, &'static str, bool, ExprId)> {
    const NPE: &str = "java.lang.NullPointerException";
    const INDEX: &str = "java.lang.ArrayIndexOutOfBoundsException";
    let tested = |e: &Expr| is_tested(e, safe);
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
                out.push(("java.lang.ArithmeticException", "this division", true, x.id));
            }
        }
        ExprKind::Assign { op: Some(BinOp::Div | BinOp::Rem), value, .. }
            if x.ty.unboxed().is_integral() && value.ty.unboxed().is_integral() =>
        {
            let constant = matches!(value.kind, ExprKind::Int(v) if v != 0)
                || matches!(value.kind, ExprKind::Long(v) if v != 0);
            if !constant && !facts.safe_division.contains(&x.id) {
                out.push(("java.lang.ArithmeticException", "this division", true, x.id));
            }
        }
        ExprKind::Index(array, _) => {
            if null(array) {
                out.push((NPE, "this array access", true, array.id));
            }
            if !facts.in_bounds.contains(&x.id) {
                out.push((INDEX, "this array access", true, x.id));
            }
        }
        ExprKind::Length(array) if null(array) => out.push((NPE, "this length", true, array.id)),
        ExprKind::Switch { selector, .. } if selector.ty == Type::String && null(selector) => {
            out.push((NPE, "this switch", true, selector.id))
        }
        ExprKind::Field { object, .. } if null(object) => {
            out.push((NPE, "this field access", object_checked(object), object.id))
        }
        ExprKind::MethodCall { receiver, resolved: Some(r), is_super: false, .. }
            if !class.methods[*r].is_static && null(receiver) =>
        {
            out.push((NPE, "this call", object_checked(receiver), receiver.id))
        }
        ExprKind::NewArray(sizes) => {
            // A size the range analysis keeps non-negative cannot throw.
            let nonnegative = |s: &Expr| facts.index_ranges.get(&s.id).is_some_and(|&(lo, _)| lo >= 0);
            if sizes.iter().any(|s| !matches!(s.kind, ExprKind::Int(v) if v >= 0) && !nonnegative(s)) {
                out.push(("java.lang.NegativeArraySizeException", "this array creation", false, x.id));
            }
        }
        ExprKind::Cast(to, value)
            if !to.is_reference() && matches!(value.ty, Type::Boxed(_)) && null(value) =>
        {
            out.push((NPE, "this unboxing", false, x.id))
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
            | Builtin::NoObject { .. }
            | Builtin::UnknownClass
            | Builtin::ClassName { .. }
            | Builtin::ClassLiteralName { .. }
            | Builtin::SameClass { .. }
            | Builtin::GetClass
            | Builtin::ClassLiteral(_)
            | Builtin::ClassMethod(_)
            | Builtin::ComparableClassFor
            // A null is a value to them, and an object's own method is a call of its own.
            | Builtin::ObjectsEquals
            | Builtin::ObjectsHashCode => {}
            // The receiver, a string, is dereferenced with the checked form (`Facts::null_checked`).
            Builtin::StringLength | Builtin::StringEquals => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", true, args[0].id));
                }
            }
            Builtin::StringCharAt => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", true, args[0].id));
                }
                if !facts.index_in_bounds.contains(&x.id) {
                    out.push(("java.lang.StringIndexOutOfBoundsException", "this charAt", true, x.id));
                }
            }
            Builtin::Boxed(BoxedOp::Parse | BoxedOp::ParseRadix, _) => {
                out.push(("java.lang.NumberFormatException", "this parse", true, x.id));
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
            Builtin::MonitorEnter => out.push((NPE, "this synchronized", true, args[0].id)),
            Builtin::NullChecked | Builtin::Unmatched => {
                if null(&args[0]) {
                    out.push((NPE, "this null check", true, args[0].id));
                }
            }
            // A zero divisor throws, as in a division.
            Builtin::Native(Native::Math { name: "floorDiv" | "floorMod", .. }) => {
                let constant = matches!(args[1].kind, ExprKind::Int(v) if v != 0)
                    || matches!(args[1].kind, ExprKind::Long(v) if v != 0);
                if !constant {
                    out.push(("java.lang.ArithmeticException", "this division", true, x.id));
                }
            }
            _ => {
                // Writing past the end of the array throws, as `Character.toChars` does.
                if matches!(builtin, Builtin::CodePoint(CodePointOp::ToCharsInto)) {
                    out.push(("java.lang.ArrayIndexOutOfBoundsException", "this call of the library", true, x.id));
                }
                // A begin or end outside the string throws, as `parseInt(s, begin, end, radix)` does.
                if matches!(builtin, Builtin::Bits(BitsOp::ParseRange, _) | Builtin::Unsigned(UnsignedOp::ParseRange, _)) {
                    out.push(("java.lang.IndexOutOfBoundsException", "this call of the library", true, x.id));
                }
                // A null argument throws, except where the method takes null (an `equals`, the
                // elements of `join`, the text of `Arrays.toString`).
                let takes_null = matches!(
                    builtin,
                    Builtin::String(StringOp::EqualsIgnoreCase | StringOp::EqualsOther)
                        | Builtin::ArraysToString
                        | Builtin::Arrays(ArraysOp::Equals | ArraysOp::HashCode | ArraysOp::DeepToString)
                        | Builtin::Boxed(BoxedOp::Equals, _)
                        | Builtin::Charset(CharsetOp::ForName | CharsetOp::IsSupported)
                );
                let checked = if takes_null || matches!(builtin, Builtin::String(StringOp::Join) | Builtin::Format) {
                    &args[..args.len().min(1)]
                } else {
                    &args[..]
                };
                // Only the first may be the receiver, which is dereferenced with the checked
                // form; the code generator rejects a call that does not (`check_dereferences`).
                if !takes_null && checked.iter().any(null) {
                    let receiver = !checked[1..].iter().any(null);
                    out.push((NPE, "this call", receiver, if receiver { args[0].id } else { x.id }));
                }
                if let Some(failure) = library_failure(builtin, args)
                    && !constant_pattern(builtin, args)
                {
                    let checked = matches!(
                        builtin,
                        Builtin::String(StringOp::Substring | StringOp::FromChars)
                            | Builtin::Bits(BitsOp::ParseRadix | BitsOp::ParseRange, _)
                            | Builtin::Unsigned(UnsignedOp::Parse | UnsignedOp::ParseRange, _)
                            | Builtin::CodePoint(CodePointOp::ToCharsInto)
                            | Builtin::EnumValueOf
                            | Builtin::Native(Native::MonitorWait | Native::MonitorNotify { .. })
                    );
                    out.push((failure, "this call of the library", checked, x.id));
                }
                if let Builtin::Native(Native::MonitorWait) = builtin
                    && !matches!(args[1].kind, ExprKind::Long(n) if n >= 0)
                {
                    out.push(("java.lang.IllegalArgumentException", "this call of the library", true, x.id));
                }
            }
        },
        _ => {}
    }
    if let ExprKind::Assign { target: Place::Index(array, _), .. }
    | ExprKind::IncDec { target: Place::Index(array, _), .. } = &x.kind
    {
        if null(array) {
            out.push((NPE, "this array access", true, array.id));
        }
        if !facts.in_bounds.contains(&x.id) {
            out.push((INDEX, "this array access", true, x.id));
        }
    }
    if let ExprKind::Assign { target: Place::Field { object, .. }, .. }
    | ExprKind::IncDec { target: Place::Field { object, .. }, .. } = &x.kind
        && null(object)
    {
        out.push((NPE, "this field access", object_checked(object), object.id));
    }
    // A cast to a class that the value's type is not under may fail, unless an `instanceof`
    // tested the variable.
    // The variable was tested to be an instance of a class under `t`.
    let instance = |t: usize, value: &Expr| {
        let ExprKind::Var(v) = &value.kind else { return false };
        let prefix = instance_fact(v, "");
        safe.iter()
            .filter_map(|fact| fact.strip_prefix(&prefix))
            .any(|name| class.type_index(name).is_some_and(|tested| class.hierarchy.is_subtype(tested, t)))
    };
    // A cast that lowering inserted (at its value's position) is javac's conversion of an erased
    // type variable's value, as `Entry<K, V> e = i.next()`, which does not fail.
    if let ExprKind::Cast(to, value) = &x.kind
        && x.pos != value.pos
        && let (Some(t), Some(s)) = (class.class_index(*to), class.class_index(value.ty))
        && !class.hierarchy.is_subtype(s, t)
        && !instance(t, value)
    {
        out.push(("java.lang.ClassCastException", "this cast", true, x.id));
    }
    // So may a cast of an object to a string, a box or an enum, unless an `instanceof` tested it.
    if let ExprKind::Cast(to, value) = &x.kind
        && x.pos != value.pos
        && matches!(value.ty, Type::Class(..))
        && matches!(to, Type::String | Type::Boxed(_) | Type::Enum(_))
        && let Some(name) = instance_name(*to)
        && !matches!(&value.kind, ExprKind::Var(v) if safe.contains(&instance_fact(v, name)))
    {
        out.push(("java.lang.ClassCastException", "this cast", true, x.id));
    }
    // A box that may be null is unboxed by arithmetic and comparisons.
    if let ExprKind::Binary(_, l, r) = &x.kind {
        for operand in [l, r] {
            if matches!(operand.ty, Type::Boxed(_)) && null(operand) && !matches!(x.ty, Type::String) {
                out.push((NPE, "this unboxing", false, x.id));
            }
        }
    }
    out
}
