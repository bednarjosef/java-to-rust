//! Which methods throw. A method returns `Result<_, Throwable>` exactly when some caller
//! observes an exception that may escape it: the call is inside a `try` whose `catch` matches the
//! exception, or in a method whose own callers observe it. Every other exception ends the program
//! where it is thrown, as Java's would once nothing catches it, so the methods that nothing
//! observes keep their plain signatures.
//!
//! Two fixpoints over the calls (every implementation a virtual call may run, constructors,
//! `this(...)` and `super(...)`):
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
//!
//! The `toString` that making an object's text runs (`"" + o`, `println(o)`, `append(o)`,
//! `String.valueOf(o)`) is not such a call: the generator writes it with `Display`, which has no
//! way to return an exception, so what it throws ends the program there, as an exception that
//! nothing catches does. Where a `toString` that the program wrote may throw what a `catch`
//! around the text, or around a call on the way to it, would catch, the program is rejected. The
//! JDK's own `toString`s throw only for bugs (a collection changed while it is shown) and are not
//! looked at. An explicit `o.toString()` is an ordinary call.

mod entry;
mod failures;
mod field_inits;
mod guards;

use super::Facts;
use super::hierarchy;
use super::nullness::{maybe_null, non_null_names};
use crate::ast::*;
use crate::lower::standard_subclass;
use failures::runtime_failures;
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
    /// The text of an object, which runs one of these `toString` methods; what they throw ends the
    /// program here.
    Text(Vec<usize>),
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
    /// For a call, the values of its arguments when some are known (see `guards`).
    known: Vec<Vec<Option<guards::Value>>>,
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
    let mut points: Vec<Vec<Point>> =
        (0..class.methods.len()).map(|m| method_points(class, facts, m)).collect();
    let matches = |k: &Kind, t: usize| match k {
        Kind::Class(c) => h.is_subtype(*c, t),
        Kind::Runtime(name) => {
            let decl = &class.types[t].name;
            crate::lower::is_standard(decl) && (*name == "?" || standard_subclass(name, decl))
        }
    };
    let caught = |k: &Kind, frames: &[Frame]| frames.iter().any(|f| f.catches.iter().any(|&t| matches(k, t)));
    let initializer: Vec<bool> = (0..class.methods.len()).map(|m| class.is_initializer(m)).collect();
    // A static initializer that the lowering rewrote keeps its failures as exceptions (see
    // `lower/clinit_errors.rs`): it is translated like a method, not collapsed.
    let guarded: Vec<bool> = class
        .methods
        .iter()
        .map(|m| m.name == "<clinit>" && crate::lower::rewritten_clinit(&class.types[m.class]))
        .collect();
    let raised = |p: &Point, escapes: &[BTreeSet<Kind>]| -> BTreeSet<Kind> {
        match &p.raise {
            Raise::Throw(t) => h.concretes(*t).into_iter().map(Kind::Class).collect(),
            Raise::Call(callees) => callees.iter().flat_map(|&c| escapes[c].iter().cloned()).collect(),
            Raise::Runtime { class, .. } => BTreeSet::from([Kind::Runtime(class)]),
            Raise::Text(_) => BTreeSet::new(),
        }
    };

    // What escapes each method.
    let escapes_of = |points: &[Vec<Point>]| {
        let mut escapes: Vec<BTreeSet<Kind>> = vec![BTreeSet::new(); class.methods.len()];
        loop {
            let mut changed = false;
            for m in 0..class.methods.len() {
                let mut out = BTreeSet::new();
                for p in &points[m] {
                    out.extend(raised(p, &escapes).into_iter().filter(|k| !caught(k, &p.frames)));
                }
                if initializer[m] && !guarded[m] && !out.is_empty() {
                    out = BTreeSet::from([Kind::Runtime(INITIALIZER_ERROR)]);
                }
                if out != escapes[m] {
                    escapes[m] = out;
                    changed = true;
                }
            }
            if !changed {
                return escapes;
            }
        }
    };
    let mut escapes = escapes_of(&points);
    // A call that raises nothing for the arguments it passes is no point (see `guards`). Leaving
    // one out only shrinks what escapes, so the calls are looked at again until none is left.
    loop {
        let mut dropped = Vec::new();
        for (m, ps) in points.iter().enumerate() {
            for (i, p) in ps.iter().enumerate() {
                let Raise::Call(callees) = &p.raise else { continue };
                if p.known.is_empty() || raised(p, &escapes).is_empty() {
                    continue;
                }
                let callee = callees[0];
                let quiet = p.known.iter().all(|args| {
                    let run = guards::run(class, callee, args);
                    points[callee].iter().all(|q| {
                        // The start of a method that initializes the caller's own class, which is
                        // initialized or being initialized, does nothing.
                        let own = matches!(&q.raise, Raise::Call(cs) if q.what == entry::START && cs.iter().all(|&c| class.methods[c].class == class.methods[m].class));
                        own || run.skips(q.pos, &q.raise) || raised(q, &escapes).iter().all(|k| caught(k, &q.frames))
                    })
                });
                if quiet {
                    dropped.push((m, i));
                }
            }
        }
        if dropped.is_empty() {
            break;
        }
        for &(m, i) in dropped.iter().rev() {
            points[m].remove(i);
        }
        escapes = escapes_of(&points);
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

    // An object's text whose `toString`, one the program wrote, may throw what a `catch` around
    // the text, or around a call on the way to it, would catch. The JDK's own `toString`s throw
    // only for bugs (a collection changed while it is shown) and are not looked at.
    for (m, p) in points.iter().enumerate().flat_map(|(m, ps)| ps.iter().map(move |p| (m, p))) {
        let Raise::Text(shown) = &p.raise else { continue };
        let program = shown.iter().filter(|&&c| !from_jdk(&class.types[class.methods[c].class]));
        if program.flat_map(|&c| &escapes[c]).any(|k| caught(k, &p.frames) || observed(k, &seen[m])) {
            return error(
                p.pos,
                "catching an exception that this toString may throw not supported yet".to_string(),
            );
        }
    }

    // A static initializer never returns a `Result`: what it raises ends the program (see `seen`).
    let mut throwing: HashSet<usize> = (0..class.methods.len())
        .filter(|&m| guarded[m] || (!initializer[m] && escapes[m].iter().any(|k| observed(k, &seen[m]))))
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
    // The `try`s whose `catch` gets a `NullPointerException` that the runtime raises, those of
    // them that get one without the checked form (whose message is the runtime's), and the
    // objects of `null_checked` with the `try` that catches their exception in their method, and
    // their method.
    let mut runtime_npe_tries = HashSet::new();
    let mut unchecked_npe_tries = HashSet::new();
    let mut npe_catchers: Vec<(ExprId, Option<usize>, usize)> = Vec::new();
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
                        npe_catchers.push((id, catcher.map(|f| f.id), m));
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
                        unchecked_npe_tries.insert(f.id);
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
    // dereference ("Cannot invoke ..."); the message of one that code creates is its own. Where
    // a `catch` reads it, the translation writes Java's message (`npe_messages`), for the checked
    // form only.
    let npe = Kind::Runtime("java.lang.NullPointerException");
    let mut reading_tries = HashSet::new();
    for method in &class.methods {
        let mut found = None;
        for s in &method.body {
            visit_stmt(s, &mut |_| {}, &mut |s| {
                let Stmt::Try { catches, .. } = s else { return };
                let id = s as *const Stmt as usize;
                // A `try` that a dereference of its own reaches, or any where one leaves its
                // method, which a call in the `try` may reach.
                if !runtime_npe_tries.contains(&id)
                    && npe_catchers.iter().all(|(_, catcher, _)| catcher.is_some())
                {
                    return;
                }
                for catch in catches.iter().filter(|c| c.types.iter().any(|&t| matches(&npe, t))) {
                    // Any use of the exception may read its message (`getMessage`, its text, a
                    // method it is passed to), except throwing it on.
                    let mut rethrown = HashSet::new();
                    for b in &catch.body {
                        visit_stmt(b, &mut |_| {}, &mut |s| {
                            if let Stmt::Throw(e) = s {
                                let thrown = if let ExprKind::Cast(_, inner) = &e.kind { inner } else { e };
                                rethrown.insert(thrown.id);
                            }
                        });
                    }
                    for b in &catch.body {
                        visit_stmt(
                            b,
                            &mut |e| {
                                visit_expr(e, &mut |x| {
                                    if matches!(&x.kind, ExprKind::Var(v) if *v == catch.name)
                                        && !rethrown.contains(&x.id)
                                    {
                                        reading_tries.insert(id);
                                        facts.npe_message_read.get_or_insert(x.pos);
                                        if unchecked_npe_tries.contains(&id) {
                                            found.get_or_insert(x.pos);
                                        }
                                    }
                                })
                            },
                            &mut |_| {},
                        );
                    }
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
    // A `catch` may read the message of a dereference that its `try` catches, or of one whose
    // exception leaves its method, in a method that a call in the `try` may run.
    let mut reached = HashSet::new();
    let mut calls: Vec<usize> = points
        .iter()
        .flatten()
        .filter(|p| p.frames.iter().any(|f| reading_tries.contains(&f.id) && !f.catches.is_empty()))
        .flat_map(|p| match &p.raise {
            Raise::Call(callees) => callees.clone(),
            _ => Vec::new(),
        })
        .collect();
    while let Some(m) = calls.pop() {
        if reached.insert(m) {
            calls.extend(points[m].iter().flat_map(|p| match &p.raise {
                Raise::Call(callees) => callees.clone(),
                _ => Vec::new(),
            }));
        }
    }
    facts.npe_message_needed = npe_catchers
        .iter()
        .filter(|(_, catcher, m)| match catcher {
            Some(t) => reading_tries.contains(t),
            None => reached.contains(m),
        })
        .map(|&(id, ..)| id)
        .collect();

    // A static initializer's exception reaches the program as `ExceptionInInitializerError`,
    // which ends it here: one that a `catch` (of `Error`, `LinkageError`, `Throwable`) or a
    // `finally` around a use of the class would see is not translated.
    let initializer_error = Kind::Runtime(INITIALIZER_ERROR);
    for (m, method) in class.methods.iter().enumerate() {
        if initializer[m] && !guarded[m] && !escapes[m].is_empty() && observed(&initializer_error, &seen[m]) {
            return error(method.pos, "a static initializer that throws inside a `try` not supported yet");
        }
    }
    // The initializers of instance fields run in each constructor, outside its `try`s: what they
    // raise escapes the constructor (see `field_inits`).
    let init_throwing = field_inits::throwing(class, facts, &raised, &escapes);
    // The initializers of enum constants run in the static initializer, so what they raise would
    // be `ExceptionInInitializerError`: not translated yet.
    let escapes_ref = &escapes;
    let initializer_raises = |e: &Expr, m: usize| {
        let mut out = Vec::new();
        expr_points(class, facts, m, e, &[], &[], &mut out);
        out.iter()
            .any(|p| raised(p, escapes_ref).iter().any(|k| matches!(k, Kind::Class(_))))
            .then_some(e.pos)
    };
    for decl in &class.types {
        for constant in decl.enum_constants.iter().flatten() {
            if let Some(pos) = constant.values.iter().find_map(|e| initializer_raises(e, usize::MAX)) {
                return error(pos, "an enum constant whose arguments throw not supported yet");
            }
        }
    }

    facts.throwing = throwing;
    facts.init_throwing = init_throwing;
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
    let mut out: Vec<Point> = entry::entry_point(class, facts, m).into_iter().collect();
    let mut frames = Vec::new();
    block_points(class, facts, m, &method.body, &mut frames, &[], &mut out);
    // A constructor runs the initializers of its class's fields, unless it starts with `this(..)`.
    if method.is_ctor && !delegates(&method.body) {
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
        // So does an assignment to a field, whose value is the assigned one (`(current = e).next`).
        ExprKind::Assign { op: None, target: Place::Field { .. } | Place::Static { .. }, value }
            if !matches!(e.ty, Type::Boxed(_)) =>
        {
            is_tested(value, safe)
        }
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
        out.push(Point {
            raise,
            pos: selector.pos,
            what: "this switch",
            frames: frames.clone(),
            known: Vec::new(),
        });
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
                out.push(Point {
                    raise,
                    pos: e.pos,
                    what: "this throw",
                    frames: frames.clone(),
                    known: Vec::new(),
                });
            } else if let Some(t) = class.class_index(thrown.ty) {
                out.push(Point {
                    raise: Raise::Throw(t),
                    pos: e.pos,
                    what: "this throw",
                    frames: frames.clone(),
                    known: Vec::new(),
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
        ExprKind::Builtin(Builtin::InitializeClass { class: t }, _) => classes.push(*t),
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
    // `main` starts by initializing its class (`statics::entry_inits`), so no later code runs
    // that class's `<clinit>` but the code it runs itself, where the call returns at once.
    let main_class = match class.methods.iter().filter(|c| c.is_main).collect::<Vec<_>>()[..] {
        [main] => Some(main.class),
        _ => None,
    };
    classes
        .into_iter()
        .filter_map(|t| {
            let init = facts.init_class(class, t)?;
            let initialized = Some(init) == main_class
                || current.is_some_and(|c| c == init || class.hierarchy.ancestors(c).contains(&init));
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
    let callees = class.callees(x);
    if !callees.is_empty() {
        let known = match callees[..] {
            [only] => Some(guards::known_args(class, x))
                .filter(|k| k.first().is_some_and(|args| class.methods[only].params.len() == args.len()))
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let raise = Raise::Call(callees);
        out.push(Point { raise, pos: x.pos, what: "this call", frames: frames.to_vec(), known });
    }
    let mut push = |raise: Raise, pos: Pos, what: &'static str| {
        out.push(Point { raise, pos, what, frames: frames.to_vec(), known: Vec::new() });
    };
    for init in static_inits(class, facts, m, x) {
        push(Raise::Call(vec![init]), x.pos, "this static use");
    }
    for object in text_objects(x) {
        let shown = to_string_impls(class, object.ty);
        if !shown.is_empty() {
            push(Raise::Text(shown), object.pos, "this toString");
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

/// Whether `decl` is a class of the JDK, which no program may declare.
pub(super) fn from_jdk(decl: &TypeDecl) -> bool {
    ["java", "javax", "jdk", "sun"]
        .iter()
        .any(|root| decl.package == *root || decl.package.starts_with(&format!("{root}.")))
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
