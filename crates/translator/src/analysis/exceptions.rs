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
//! The `toString` that making an object's text runs is such a call where the generator makes the
//! text (`"" + o`, `println(o)`, `String.valueOf(o)`, `append(o)`, `String.format`): it calls the
//! `toString` of the program, or the `hashCode` of the program that `Object`'s `toString` shows,
//! and what that throws goes on from there. Elsewhere (the text of an enum's constant, the message
//! of an exception, `printf`, the objects that other library calls show) the text is made through
//! `Display`, which has no way to return an exception, so what it throws ends the program there, as an
//! exception that nothing catches does; where a `toString` that the program wrote may throw what
//! a `catch` around such a text, or around a call on the way to it, would catch, the program is
//! rejected. The JDK's own `toString`s throw only for bugs (a collection changed while it is
//! shown) and are not looked at. An explicit `o.toString()` is an ordinary call, except that one of the JDK's own
//! `toString`s lets out only what the program's code that it runs throws, and the `catch` clauses
//! around the call are not looked for in its own code. The JDK's static initializers run on the
//! JDK's own constants and flags, and so none of the program's code: one lets out nothing.

mod comparable;
mod conditions;
mod entry;
mod failures;
use conditions::condition_points;
use failures::format_may_fail;
mod field_inits;
pub(super) mod guards;

use super::Facts;
use super::closure::{self, Bits};
use super::hierarchy;
use super::nullness::{maybe_null, non_null_names};
use crate::ast::*;
use crate::lower::standard_subclass;
use failures::runtime_failures;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// What a static initializer that throws raises: Java wraps the exception in this `Error`.
const INITIALIZER_ERROR: &str = "java.lang.ExceptionInInitializerError";

/// An exception that may be raised: an object of a concrete class of the program (or a standard
/// class it creates), or a failure of the runtime, by its standard class's name (`"?"` when the
/// operation's failures are not known exactly).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

/// Rejects a `printf` or `format` of a `PrintStream` object whose format may fail. The text is
/// formatted before the stream's `formattedText` prints it (`lower::strings`), so a specifier
/// that fails as it prints would leave nothing printed, where Java's `Formatter` has written the
/// text before it to the stream. (`System.out.printf` prints that text, see
/// `java::print_format`.)
fn printf_of_streams(class: &Class, reject: &mut impl FnMut(Pos, &str)) {
    let (owner, name, _) = crate::program::PRINT_FORMATTED;
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::MethodCall { resolved: Some(r), args, .. } = &x.kind
                            && class.methods[*r].name == name
                            && class.types[class.methods[*r].class].name == owner
                            && let ExprKind::Builtin(Builtin::Format, format) = &args[0].kind
                            && format_may_fail(class, format)
                        {
                            reject(
                                x.pos,
                                "`printf` of a `PrintStream` object with a format that may fail not supported yet",
                            );
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
}

/// Every error found, in the order of the code; the first is the one a translation reports.
pub(super) fn analyze(class: &Class, facts: &mut Facts) -> Result<(), Vec<Error>> {
    if class.type_index("java.lang.Throwable").is_none() {
        return Ok(());
    }
    facts.static_writes = guards::static_writes(class);
    facts.initialized_statics = initialized_statics(class, facts);
    let mut errors = Vec::new();
    let mut reject = |pos: Pos, message: &str| errors.push(Error { pos, message: message.to_string() });
    printf_of_streams(class, &mut reject);
    let h = &class.hierarchy;
    let mut points: Vec<Vec<Point>> =
        (0..class.methods.len()).map(|m| method_points(class, facts, m)).collect();
    let matches = |k: &Kind, t: usize| match k {
        Kind::Class(c) => h.is_subtype(*c, t),
        // A failure not known exactly may be of any class of the JDK; one that is known is of
        // its class, a standard one or one that `standard_subclass` knows the superclass of.
        Kind::Runtime("?") => from_jdk(&class.types[t]),
        Kind::Runtime(name) => {
            let decl = &class.types[t].name;
            decl == name || crate::lower::is_standard(decl) && standard_subclass(name, decl)
        }
    };
    let caught = |k: &Kind, frames: &[Frame]| frames.iter().any(|f| f.catches.iter().any(|&t| matches(k, t)));
    let initializer: Vec<bool> = (0..class.methods.len()).map(|m| class.is_initializer(m)).collect();
    // A static initializer that the lowering rewrote keeps its failures as exceptions (see
    // `lower/clinit_errors.rs`): it is translated like a method, not collapsed. So do the
    // functions that build the constants of its enum (`lower/enum_tables.rs`), which only it
    // calls, inside the `try` whose `catch` makes their failure the class's.
    let guarded: Vec<bool> = (0..class.methods.len())
        .map(|m| {
            let method = &class.methods[m];
            (method.name == "<clinit>" || class.builds_constants(m))
                && crate::lower::rewritten_clinit(&class.types[method.class])
        })
        .collect();
    // The classes whose objects a `throw` of a value of type `t` throws: those under `t` that are
    // `Throwable`s, since a value of a type variable or of `Object` (as the JDK's generic code
    // throws, `throw (X) e`) is one all the same.
    let throwable = class.type_index("java.lang.Throwable");
    let thrown = |t: usize| -> Vec<usize> {
        h.concretes(t).into_iter().filter(|&c| throwable.is_some_and(|w| h.is_subtype(c, w))).collect()
    };
    let raised = |p: &Point, escapes: &[BTreeSet<Kind>]| -> BTreeSet<Kind> {
        match &p.raise {
            Raise::Throw(t) => thrown(*t).into_iter().map(Kind::Class).collect(),
            Raise::Call(callees) => callees.iter().flat_map(|&c| escapes[c].iter().cloned()).collect(),
            Raise::Runtime { class, .. } => BTreeSet::from([Kind::Runtime(class)]),
            Raise::Text(_) => BTreeSet::new(),
        }
    };

    // The JDK's own `toString`s throw only for bugs, as for an object's text (see the module's
    // comment): a call of one lets out only what the program's code that it runs throws, and
    // what the caller's `catch` clauses catch is not looked for in its own code. So do the JDK's
    // static initializers, which run on the JDK's own constants and flags and so run none of the
    // program's code: one lets out nothing, although the calls of interfaces in it reach the
    // program's implementations too (`Function.apply` reaches each lambda of the program).
    // So do the constructors of the JDK's exceptions, which test only their arguments
    // (`URISyntaxException(input, reason, index)` throws for a null input or an index below -1),
    // and which the JDK's code that throws them never gives such arguments.
    let jdk: Vec<bool> = class.methods.iter().map(|m| from_jdk(&class.types[m.class])).collect();
    let bugs_only: Vec<bool> = class
        .methods
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let to_string = m.name == "toString" && m.params.is_empty() && !m.is_static;
            let exception_ctor =
                m.is_ctor && throwable.is_some_and(|t| class.hierarchy.is_subtype(m.class, t));
            jdk[i] && (to_string || exception_ctor || initializer[i] && !guarded[i])
        })
        .collect();

    // What escapes each method, and of that, what the program's code throws: all of it for a
    // method of the program, and for one of the JDK what escapes the program's methods it calls.
    // Methods are looked at callees first, and again whenever what escapes a method they call
    // grows (in calls that go round), until nothing does; what escapes only grows, so the order
    // does not change the outcome. The kinds are numbered, and each point's own kinds and those
    // its `try`s catch are sets of those numbers, worked out once.
    let escapes_of = |points: &[Vec<Point>]| {
        let n = class.methods.len();
        let mut kinds: Vec<Kind> = Vec::new();
        let mut numbers: HashMap<Kind, usize> = HashMap::new();
        let mut number = |k: Kind| {
            *numbers.entry(k.clone()).or_insert_with(|| {
                kinds.push(k);
                kinds.len() - 1
            })
        };
        let initializer_error = number(Kind::Runtime(INITIALIZER_ERROR));
        let own: Vec<Vec<Bits>> = points
            .iter()
            .map(|ps| {
                ps.iter()
                    .map(|p| match &p.raise {
                        Raise::Throw(t) => thrown(*t).into_iter().map(|c| number(Kind::Class(c))).collect(),
                        Raise::Runtime { class, .. } => Bits::from_iter([number(Kind::Runtime(class))]),
                        Raise::Call(_) | Raise::Text(_) => Bits::default(),
                    })
                    .collect()
            })
            .collect();
        // What the `try`s around each point catch, shared by the points with the same `try`s.
        let mut masks: Vec<Bits> = vec![Bits::default()];
        // Each `try` by its id, what it catches and whether it has a `finally`.
        type Tries<'a> = Vec<(usize, &'a [usize], bool)>;
        let mut mask_of: HashMap<Tries, usize> = HashMap::new();
        let caught_by: Vec<Vec<usize>> = points
            .iter()
            .map(|ps| {
                ps.iter()
                    .map(|p| {
                        if p.frames.is_empty() {
                            return 0;
                        }
                        let key = p.frames.iter().map(|f| (f.id, &f.catches[..], f.finally)).collect();
                        *mask_of.entry(key).or_insert_with(|| {
                            masks.push((0..kinds.len()).filter(|&k| caught(&kinds[k], &p.frames)).collect());
                            masks.len() - 1
                        })
                    })
                    .collect()
            })
            .collect();
        let mut escapes: Vec<Bits> = vec![Bits::default(); n];
        let mut from_program: Vec<Bits> = vec![Bits::default(); n];
        let mut callers: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut callees: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (m, ps) in points.iter().enumerate() {
            for p in ps {
                if let Raise::Call(called) = &p.raise {
                    for &c in called {
                        if callers[c].last() != Some(&m) {
                            callers[c].push(m);
                            callees[m].push(c);
                        }
                    }
                }
            }
        }
        let mut queued = vec![true; n];
        let mut queue: VecDeque<usize> = closure::components(&callees).1.into_iter().flatten().collect();
        while let Some(m) = queue.pop_front() {
            queued[m] = false;
            let mut out = Bits::default();
            let mut program = Bits::default();
            for (i, p) in points[m].iter().enumerate() {
                let caught = &masks[caught_by[m][i]];
                out.union_except(&own[m][i], caught);
                if let Raise::Call(called) = &p.raise {
                    for &c in called {
                        out.union_except(&escapes[c], caught);
                        program.union_except(&from_program[c], caught);
                    }
                }
            }
            if !jdk[m] {
                program = out.clone();
            }
            // A static initializer of the JDK runs none of the program's code (see `bugs_only`),
            // also one that keeps its failures as exceptions (`guarded`): the program's exceptions
            // that its calls seem to reach through interfaces never escape it.
            if jdk[m] && initializer[m] {
                program = Bits::default();
            }
            if bugs_only[m] {
                if initializer[m] {
                    program = Bits::default();
                }
                out = program.clone();
            }
            if initializer[m] && !guarded[m] {
                for set in [&mut out, &mut program] {
                    if !set.is_empty() {
                        *set = Bits::from_iter([initializer_error]);
                    }
                }
            }
            if out != escapes[m] || program != from_program[m] {
                escapes[m] = out;
                from_program[m] = program;
                for &c in &callers[m] {
                    if !queued[c] {
                        queued[c] = true;
                        queue.push_back(c);
                    }
                }
            }
        }
        escapes
            .iter()
            .map(|set| set.iter().map(|k| kinds[k].clone()).collect())
            .collect::<Vec<BTreeSet<Kind>>>()
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
                    let run = guards::run(class, &facts.static_writes, callee, args);
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

    // What the callers of each method observe: what the callers of its callers observe, and
    // what the `try`s around its calls catch. A call may run any method of an override group, so
    // the methods of a group observe the same. A method is looked at again whenever what it
    // observes grows, until nothing does; that only grows, so the order does not change the
    // outcome.
    let mut seen: Vec<Seen> = vec![Seen::default(); class.methods.len()];
    let mut queued = vec![true; class.methods.len()];
    let mut queue: VecDeque<usize> = (0..class.methods.len()).collect();
    let grow = |seen: &mut Seen, more: &Seen| {
        let before = (seen.types.len(), seen.all);
        seen.types.extend(more.types.iter().copied());
        seen.all |= more.all;
        before != (seen.types.len(), seen.all)
    };
    while let Some(m) = queue.pop_front() {
        queued[m] = false;
        let mut grown = Vec::new();
        for p in &points[m] {
            let Raise::Call(callees) = &p.raise else { continue };
            let mut here = if bugs_only[m] { Seen::default() } else { seen[m].clone() };
            for f in &p.frames {
                here.types.extend(f.catches.iter().copied());
                here.all |= f.finally;
            }
            for &c in callees {
                if grow(&mut seen[c], &here) {
                    grown.push(c);
                }
            }
        }
        let own = seen[m].clone();
        for &g in h.group_members(m) {
            if g != m && grow(&mut seen[g], &own) {
                grown.push(g);
            }
        }
        for c in grown {
            if !queued[c] {
                queued[c] = true;
                queue.push_back(c);
            }
        }
    }
    let observed = |k: &Kind, s: &Seen| s.all || s.types.iter().any(|&t| matches(k, t));

    // An object's text whose `toString`, one the program wrote, may throw what a `catch` around
    // the text, or around a call on the way to it, would catch. The JDK's own `toString`s throw
    // only for bugs (a collection changed while it is shown) and are not looked at.
    for (m, p) in points.iter().enumerate().flat_map(|(m, ps)| ps.iter().map(move |p| (m, p))) {
        let Raise::Text(shown) = &p.raise else { continue };
        let program = shown.iter().filter(|&&c| !from_jdk(&class.types[class.methods[c].class]));
        if program.flat_map(|&c| &escapes[c]).any(|k| caught(k, &p.frames) || observed(k, &seen[m])) {
            reject(p.pos, "catching an exception that this toString may throw not supported yet");
        }
    }

    // A static initializer never returns a `Result`: what it raises ends the program (see `seen`).
    let mut throwing: HashSet<usize> = (0..class.methods.len())
        .filter(|&m| guarded[m] || (!initializer[m] && escapes[m].iter().any(|k| observed(k, &seen[m]))))
        .collect();
    // The methods that throw themselves, not only through an override.
    let own = throwing.clone();
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
                    || (!bugs_only[m] && observed(&Kind::Runtime(name), &seen[m]));
                // Only a standard class has the constructor from a message that the runtime
                // builds its objects with (`lower::exceptions`).
                let raised_class = class
                    .type_index(name)
                    .filter(|&c| class.hierarchy.is_concrete(c) && crate::lower::is_standard(name));
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
                    // The null argument of a raw `compareTo` is raised with Java's message by the
                    // receiver's arm (`Gen::dispatcher`), so a `catch` may read it.
                    if let Some(f) = catcher
                        && name == "java.lang.NullPointerException"
                    {
                        runtime_npe_tries.insert(f.id);
                        if p.what != "this compareTo" {
                            unchecked_npe_tries.insert(f.id);
                        }
                    }
                    raised_classes.insert(c);
                } else if seen_here {
                    let simple = name.rsplit('.').next().unwrap_or_default();
                    let exception =
                        if name == "?" { "an exception".to_string() } else { format!("the `{simple}`") };
                    reject(
                        p.pos,
                        &format!("catching {exception} that {} may throw not supported yet", p.what),
                    );
                }
            }
        }
        if !throwing.contains(&m) {
            continue;
        }
        if method.is_main {
            reject(method.pos, "a call of `main` that is caught not supported yet");
            continue;
        }
        // An enum constant is built where the enum is initialized, outside any `try` of the
        // program, and an exception that the runtime raises by the `From` impl that turns its
        // failure into an object (`codegen::exceptions::raised_impl`), which cannot fail. (An
        // exception that the program builds passes its constructor's failure on from the `new`.)
        let decl = &class.types[method.class];
        let raised_with = |new: &Expr| matches!(new.kind, ExprKind::New { ctor: Some(c), .. } if c == m);
        if method.is_ctor && decl.enum_constants.is_some() {
            reject(method.pos, "a constructor of an enum that throws not supported yet");
        } else if method.is_ctor && class.raised.iter().any(raised_with) {
            reject(
                method.pos,
                "a constructor that throws, of an exception the runtime raises, not supported yet",
            );
        }
        // The text of an enum's constant is its `Display`, which cannot throw; that of an object is
        // a call of its `toString` where the generator makes it (`generator_text`).
        let of_enum = std::iter::once(method.class)
            .chain(h.ancestors(method.class))
            .any(|t| class.types[t].kind == TypeKind::Enum);
        if method.name == "toString" && method.params.is_empty() && of_enum {
            // At the override that throws, which may not be the first of its group.
            let thrower = h.group(m).into_iter().find(|g| own.contains(g)).unwrap_or(m);
            let pos = class.methods[thrower].pos;
            reject(pos, "a toString that throws an exception that is caught not supported yet");
        }
    }

    // Java's message of a `NullPointerException` that the runtime raises describes the
    // dereference ("Cannot invoke ..."); the message of one that code creates is its own. Where
    // a `catch` reads it, the translation writes Java's message (`npe_messages`), for the checked
    // form only.
    let npe = Kind::Runtime("java.lang.NullPointerException");
    let mut reading_tries = HashSet::new();
    // A suppressed exception is read only through `getSuppressed`, and printed by a
    // `printStackTrace` to a stream or a writer, which may be standard output (standard error, where
    // `printStackTrace()` and uncaught exceptions print, is not compared). So adding one to the
    // exception that is thrown on (a try-with-resources' `closing` catch) reads neither, unless
    // the program gets them back or prints them so.
    let mut suppressed_read = false;
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        suppressed_read |= matches!(
                            &x.kind,
                            ExprKind::MethodCall { name, args, .. }
                                if name == "getSuppressed" || name == "printStackTrace" && args.len() == 1
                        )
                    })
                },
                &mut |_| {},
            );
        }
    }
    for (m, method) in class.methods.iter().enumerate() {
        // A static initializer of the JDK throws only for bugs (see `bugs_only`): the `catch`
        // that a rewritten one has (section 0.9), which gives what it catches as the cause of
        // its `ExceptionInInitializerError`, sees no `NullPointerException`.
        if jdk[m] && initializer[m] {
            continue;
        }
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
                    // method it is passed to), except throwing it on and suppressing (above).
                    let mut rethrown = HashSet::new();
                    let mut suppressing = HashSet::new();
                    for b in &catch.body {
                        visit_stmt(
                            b,
                            &mut |e| {
                                visit_expr(e, &mut |x| {
                                    if let ExprKind::MethodCall { name, receiver, args, .. } = &x.kind
                                        && name == "addSuppressed"
                                    {
                                        suppressing.insert(receiver.id);
                                        if !suppressed_read {
                                            suppressing.extend(args.iter().map(|a| a.id));
                                        }
                                    }
                                })
                            },
                            &mut |s| {
                                if let Stmt::Throw(e) = s {
                                    let thrown =
                                        if let ExprKind::Cast(_, inner) = &e.kind { inner } else { e };
                                    rethrown.insert(thrown.id);
                                }
                            },
                        );
                    }
                    rethrown.extend(suppressing);
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
            reject(pos, "the message of a `NullPointerException` that the runtime raises not supported yet");
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

    // A static initializer's exception reaches the program as `ExceptionInInitializerError`, and
    // a later use of the class as `NoClassDefFoundError`: a `try` that may see either (a `catch`
    // of one of them or of an `Error` above them, or a `finally`) is not translated. A `catch`
    // of other classes, such as `ClassCastException`, never sees them.
    let linkage = [Kind::Runtime(INITIALIZER_ERROR), Kind::Runtime("java.lang.NoClassDefFoundError")];
    for (m, method) in class.methods.iter().enumerate() {
        if initializer[m]
            && !guarded[m]
            && !escapes[m].is_empty()
            && linkage.iter().any(|k| observed(k, &seen[m]))
        {
            reject(method.pos, "a static initializer that throws inside a `try` not supported yet");
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
                reject(pos, "an enum constant whose arguments throw not supported yet");
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
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
        Type::JavaClass => Some(CLASS_VALUE),
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
    let mut parts = Vec::new();
    super::nullness::conjuncts(cond, &mut parts);
    // A test of an assignment's value, `(k = e.getKey()) != null`, tests the variable too.
    match &parts[..] {
        [test] if !when => {
            if let Some((v, false)) = assigned_null_test(test) {
                out.push((v, v.to_string()));
            }
        }
        _ if when => {
            for (k, part) in parts.iter().enumerate() {
                if let Some((v, true)) = assigned_null_test(part)
                    && !parts[k + 1..].iter().any(|later| assigns(later, v))
                {
                    out.push((v, v.to_string()));
                }
            }
        }
        _ => {}
    }
    if when {
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

/// `(v = e) == null` or `(v = e) != null`: the variable and whether the test is true when it is
/// not null.
fn assigned_null_test(e: &Expr) -> Option<(&str, bool)> {
    let ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), l, r) = &e.kind else { return None };
    let assigned = match (&l.kind, &r.kind) {
        (ExprKind::Assign { op: None, target: Place::Var(v), .. }, ExprKind::Null)
        | (ExprKind::Null, ExprKind::Assign { op: None, target: Place::Var(v), .. }) => v,
        _ => return None,
    };
    Some((assigned, *op == BinOp::Ne))
}

/// The statics that are never null where code reads them (`Facts::initialized_statics`): each
/// one that the leading assignments of its class's `<clinit>` set to a value that is not null and
/// that nothing else assigns. Code of another thread that reads it waits until the class is
/// initialized; code of the initializing thread runs after that assignment, or else during the
/// values of it and the assignments before it, which `reads_static` shows never read it.
fn initialized_statics(class: &Class, facts: &Facts) -> HashSet<(usize, String)> {
    let mut leading = Vec::new();
    for (m, method) in class.methods.iter().enumerate() {
        if method.name != "<clinit>" {
            continue;
        }
        let mut before: Vec<&Expr> = Vec::new();
        let mut first = Vec::new();
        // A `<clinit>` that keeps the state of its class's initialization (`lower::clinit_errors`)
        // runs its own statements in a `try` after it: a use of the class after a failure throws
        // before it reads the class's statics.
        let body = match &method.body[..] {
            [Stmt::If { .. }, Stmt::If { .. }, Stmt::Expr(_), Stmt::Try { body, .. }]
                if crate::lower::rewritten_clinit(&class.types[method.class]) =>
            {
                body
            }
            body => body,
        };
        leading_statements(body, &mut first);
        for s in first {
            let (c, name, value) = match s {
                Stmt::Expr(Expr {
                    kind: ExprKind::Assign { op: None, target: Place::Static { class: c, name }, value },
                    ..
                }) => (*c, name, value),
                // Other expressions and locals' values, which run before what follows.
                Stmt::Local { init: Some(e), .. } | Stmt::Expr(e) | Stmt::If { cond: e, .. } => {
                    before.push(e);
                    continue;
                }
                _ => continue,
            };
            before.push(value);
            if c == method.class && !maybe_null(facts, m, value) && !reads_static(class, &before, c, name) {
                leading.push((c, name.clone()));
            }
        }
    }
    let mut writes: std::collections::HashMap<(usize, String), usize> = std::collections::HashMap::new();
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(
                s,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Assign { target: Place::Static { class: c, name }, .. }
                        | ExprKind::IncDec { target: Place::Static { class: c, name }, .. } = &x.kind
                        {
                            *writes.entry((*c, name.clone())).or_default() += 1;
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
    leading.into_iter().filter(|key| writes.get(key) == Some(&1)).collect()
}

/// The statements that start `stmts` and, unless one throws, all run one after the other, up to
/// the first other statement: expressions, declarations of locals, `if (..) throw ..;`, and the
/// statements of blocks and of `try` blocks whose `catch`es all throw. Whether all of `stmts` are
/// such. A static initializer that throws leaves its class not initialized, and its statics unread.
fn leading_statements<'s>(stmts: &'s [Stmt], out: &mut Vec<&'s Stmt>) -> bool {
    fn throws(s: &Stmt) -> bool {
        match s {
            Stmt::Throw(_) => true,
            Stmt::Block(stmts) => stmts.last().is_some_and(throws),
            _ => false,
        }
    }
    for s in stmts {
        match s {
            Stmt::Block(inner) | Stmt::Try { body: inner, finally: None, .. } if !matches!(s, Stmt::Try { catches, .. } if !catches.iter().all(|c| c.body.last().is_some_and(throws))) => {
                if !leading_statements(inner, out) {
                    return false;
                }
            }
            Stmt::Expr(_) | Stmt::Local { .. } => out.push(s),
            Stmt::If { then, otherwise: None, .. } if throws(then) => out.push(s),
            _ => return false,
        }
    }
    true
}

/// Whether evaluating `roots` may read the static `name` of class `c`: whether it, or a method
/// or static initializer that it may run, transitively, reads it. A walk too long to follow says
/// it may.
fn reads_static(class: &Class, roots: &[&Expr], c: usize, name: &str) -> bool {
    const LIMIT: usize = 2000;
    let clinits: std::collections::HashMap<usize, usize> = (class.methods.iter().enumerate())
        .filter(|(_, m)| m.name == "<clinit>")
        .map(|(i, m)| (m.class, i))
        .collect();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut work: Vec<usize> = Vec::new();
    let mut reads = false;
    let mut look = |e: &Expr, work: &mut Vec<usize>| {
        visit_expr(e, &mut |x| {
            if let ExprKind::Static { class: d, name: n } = &x.kind
                && *d == c
                && n == name
            {
                reads = true;
            }
            work.extend(class.callees(x));
            // The first use of another class runs its static initializer, and its superclasses'.
            let mut used = match &x.kind {
                ExprKind::Static { class: d, .. } | ExprKind::New { class: d, .. } => Some(*d),
                ExprKind::Call { resolved: Some(r), .. } => Some(class.methods[*r].class),
                _ => None,
            };
            while let Some(d) = used {
                work.extend(clinits.get(&d));
                used = class.types[d].extends;
            }
        })
    };
    for root in roots {
        look(root, &mut work);
    }
    while let Some(m) = work.pop() {
        if !seen.insert(m) {
            continue;
        }
        if seen.len() > LIMIT {
            return true;
        }
        for s in &class.methods[m].body {
            let mut found = Vec::new();
            visit_stmt(s, &mut |e| look(e, &mut found), &mut |_| {});
            work.extend(found);
        }
    }
    reads
}

/// Whether `e` is a variable that `safe` knows not null, or a cast of one.
fn is_tested(e: &Expr, safe: &[String]) -> bool {
    match &e.kind {
        ExprKind::Var(v) => safe.contains(v),
        // A cast of a reference keeps its null, to an enum too.
        ExprKind::Cast(to, value) if to.is_reference() || matches!(to, Type::Enum(_)) => {
            is_tested(value, safe)
        }
        // So does an assignment, whose value is the assigned one (`(current = e).next`).
        ExprKind::Assign {
            op: None,
            target: Place::Field { .. } | Place::Static { .. } | Place::Var(_),
            value,
        } if !matches!(e.ty, Type::Boxed(_)) => is_tested(value, safe),
        _ => false,
    }
}

/// Whether `value` is not null: the nullness analysis knows it, or `safe` does.
fn non_null(facts: &Facts, m: usize, value: &Expr, safe: &[String]) -> bool {
    !maybe_null(facts, m, value) || is_tested(value, safe)
}

/// Whether the statement `s`, when it completes, has last set `v` to a value that is not null:
/// it ends with `v = value;`, or it never completes.
fn sets_non_null(facts: &Facts, m: usize, s: &Stmt, v: &str, safe: &[String]) -> bool {
    match s {
        Stmt::Block(stmts) => stmts.last().is_some_and(|last| sets_non_null(facts, m, last, v, safe)),
        Stmt::Expr(Expr { kind: ExprKind::Assign { op: None, target: Place::Var(t), value }, .. }) => {
            t == v && non_null(facts, m, value, safe)
        }
        s => leaves(s),
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
            // `v = value;`, a value that is not null.
            Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Var(v), value }, ..
            }) if non_null(facts, m, value, &safe) && !assigned(v) => {
                safe.push(v.clone());
            }
            // `if (v == null) v = value;`, which leaves `v` not null either way.
            Stmt::If { cond, then, otherwise: None } => {
                for (v, fact) in proved(cond, false) {
                    if !assigned(v) && sets_non_null(facts, m, then, v, &safe) {
                        safe.push(fact);
                    }
                }
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
    // A switch on a string or an enum that is null throws, as its selector is dereferenced.
    if let Stmt::Switch { selector, .. } = s
        && matches!(selector.ty, Type::String | Type::Enum(_))
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
            condition_points(facts, m, cond, frames, safe, out);
            // A branch that a known condition never takes raises nothing (see `guards`).
            let known = guards::known_condition(class, &facts.static_writes, cond);
            let then_safe = narrowed(safe, cond, true, |v| assigns_stmt(then, v));
            if known != Some(false) {
                stmt_points(class, facts, m, then, frames, &then_safe, out);
            }
            if let Some(o) = otherwise
                && known != Some(true)
            {
                let else_safe = narrowed(safe, cond, false, |v| assigns_stmt(o, v));
                stmt_points(class, facts, m, o, frames, &else_safe, out);
            }
        }
        Stmt::While { cond, body } => {
            expr_points(class, facts, m, cond, frames, safe, out);
            condition_points(facts, m, cond, frames, safe, out);
            let body_safe = narrowed(safe, cond, true, |v| assigns_stmt(body, v));
            stmt_points(class, facts, m, body, frames, &body_safe, out);
        }
        Stmt::DoWhile { body, cond } => {
            stmt_points(class, facts, m, body, frames, safe, out);
            expr_points(class, facts, m, cond, frames, safe, out);
            condition_points(facts, m, cond, frames, safe, out);
        }
        Stmt::For { init, cond, update, body } => {
            for i in init {
                stmt_points(class, facts, m, i, frames, safe, out);
            }
            let body_safe = match cond {
                Some(c) => {
                    expr_points(class, facts, m, c, frames, safe, out);
                    condition_points(facts, m, c, frames, safe, out);
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
            // A value that may be null throws a `NullPointerException` instead when it is null, by
            // `throw` itself, unless an enclosing condition tested it; `null` itself, only that.
            let null = matches!(thrown.kind, ExprKind::Null);
            if null || (maybe_null(facts, m, thrown) && !is_tested(thrown, safe)) {
                let raise =
                    Raise::Runtime { class: "java.lang.NullPointerException", checked: true, id: thrown.id };
                out.push(Point {
                    raise,
                    pos: e.pos,
                    what: "this throw",
                    frames: frames.clone(),
                    known: Vec::new(),
                });
            }
            if !null && let Some(t) = class.class_index(thrown.ty) {
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
            // Each block narrows as it goes, as any block does (`block_points`).
            block_points(class, facts, m, body, frames, safe, out);
            frames.pop();
            // What a `catch` clause throws leaves the `try`, through its `finally`.
            frames.push(Frame {
                id: s as *const Stmt as usize,
                catches: Vec::new(),
                finally: finally.is_some(),
            });
            for catch in catches {
                block_points(class, facts, m, &catch.body, frames, safe, out);
            }
            frames.pop();
            if let Some(finally) = finally {
                block_points(class, facts, m, finally, frames, safe, out);
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
    let main_class = facts.main_class;
    debug_assert_eq!(class.hierarchy.overriders.len(), class.methods.len(), "the hierarchy is out of date");
    classes
        .into_iter()
        .filter_map(|t| {
            let init = facts.init_class(class, t)?;
            let initialized = Some(init) == main_class
                || current.is_some_and(|c| c == init || class.hierarchy.ancestors(c).contains(&init));
            if initialized {
                return None;
            }
            let methods = class.hierarchy.methods_of(init).iter().copied();
            methods
                .into_iter()
                .find(|&c| class.methods[c].class == init && class.methods[c].name == "<clinit>")
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
            [only] => Some(guards::known_args(class, &facts.static_writes, x))
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
    let calls_text = generator_text(x);
    for object in text_objects(x) {
        let shown = to_string_impls(class, object.ty);
        // Where the generator makes the text itself, a `toString` of the program, or the
        // `hashCode` of the program that `Object`'s `toString` shows, is a call, whose exception
        // goes on from there; the text that the runtime makes goes through `Display`.
        let (calls, shown): (Vec<usize>, Vec<usize>) = shown.into_iter().partition(|&i| {
            calls_text
                && matches!(class.methods[i].name.as_str(), "toString" | "hashCode")
                && !from_jdk(&class.types[class.methods[i].class])
        });
        if !calls.is_empty() {
            push(Raise::Call(calls), object.pos, "this toString");
        }
        if !shown.is_empty() {
            push(Raise::Text(shown), object.pos, "this toString");
        }
    }
    // `printf` makes all its text before it prints (`lower::strings::printf_call`), where Java
    // prints the text before each argument first: a `catch` would see less printed.
    if let ExprKind::Builtin(Builtin::Print, args) | ExprKind::MethodCall { args, .. } = &x.kind
        && let [format] = args.as_slice()
        && matches!(format.kind, ExprKind::Builtin(Builtin::Format, _))
        && format.pos == x.pos
    {
        for object in text_objects(format) {
            let shown = to_string_impls(class, object.ty);
            if !shown.is_empty() {
                push(Raise::Text(shown), object.pos, "this toString");
            }
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
            block_points(class, facts, m, &case.body, &mut nested, safe, out);
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
            condition_points(facts, m, c, frames, safe, out);
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
        // The runtime runs the dispatch of an uncaught exception later, on the thread it ends, and
        // makes the text of its stack trace there.
        ExprKind::Builtin(Builtin::Native(Native::InstallUncaught | Native::InstallTrace), _) => {}
        // The store of an atomic update of an element through `Unsafe` checks neither the array
        // nor the index, as the JVM's does not (see `failures::library_failure`): its operands
        // are evaluated, and the store raises nothing.
        ExprKind::Builtin(Builtin::ElementUpdate(_), args) => {
            for a in args {
                match &a.kind {
                    ExprKind::Assign { target: Place::Index(array, index), value, .. } => {
                        children.extend([&**array, &**index, &**value])
                    }
                    _ => children.push(a),
                }
            }
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

/// Whether `decl` is a class of the JDK, which no program may declare. The classes that lowering
/// provides (the standard exceptions, `Throwable`, `Thread`) have no package, but their JDK name.
pub(super) fn from_jdk(decl: &TypeDecl) -> bool {
    let in_jdk = |name: &str| {
        ["java", "javax", "jdk", "sun"]
            .iter()
            .any(|root| name.strip_prefix(root).is_some_and(|rest| rest.is_empty() || rest.starts_with('.')))
    };
    in_jdk(&decl.package) || decl.package.is_empty() && in_jdk(&decl.name)
}

/// The objects whose text an expression makes, which runs their `toString`.
pub fn text_objects(x: &Expr) -> Vec<&Expr> {
    let is_object = |a: &&Expr| matches!(a.ty, Type::Class(..) | Type::Enum(_));
    match &x.kind {
        ExprKind::Builtin(Builtin::ClassName { .. } | Builtin::ClassLiteralName { .. }, _) => Vec::new(),
        // The class, identity or ordinal of an object, which no `toString` gives, its monitor, and
        // the `Thread` that a thread is given or starts with.
        ExprKind::Builtin(
            Builtin::MonitorEnter
            | Builtin::Native(Native::MonitorWait | Native::MonitorNotify { .. } | Native::HoldsLock)
            | Builtin::Native(Native::SetCurrentThread | Native::ThreadStart)
            | Builtin::GetClass
            | Builtin::SameClass { .. }
            | Builtin::ClassMethod(_)
            | Builtin::ClassIsInstance
            | Builtin::ClassCheck
            | Builtin::ComparableClassFor
            | Builtin::EnumOrdinal
            | Builtin::EnumName
            | Builtin::EnumCompareTo
            | Builtin::Native(Native::IdentityHash | Native::SystemIdentityHash),
            _,
        ) => Vec::new(),
        // A copy, comparison, hash, clone or null check of an object, a fill or sort of an array,
        // `Unsafe`'s memory and atomic updates: none of them makes the object's text. Of the
        // natives, only `System.err`'s `print` does.
        ExprKind::Builtin(
            Builtin::ObjectClone
            | Builtin::ObjectsEquals
            | Builtin::ObjectsHashCode
            | Builtin::NullChecked
            | Builtin::Unmatched
            | Builtin::RawMemory { .. }
            | Builtin::ElementUpdate(_)
            | Builtin::ByteView { .. }
            | Builtin::ArraysFill
            | Builtin::ArraysSort,
            _,
        ) => Vec::new(),
        ExprKind::Builtin(Builtin::Native(native), _) if !matches!(native, Native::ErrPrint { .. }) => {
            Vec::new()
        }
        // A range of a `CharSequence` that a builder appends is read with its `length` and
        // `charAt` (`codegen::char_sequence`), or is a string's or a builder's.
        ExprKind::Builtin(Builtin::Builder(BuilderOp::Append), args) if args.len() == 4 => Vec::new(),
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

/// Whether the generator makes the text of the objects that `x` shows (`text_objects`) with a
/// call of their `toString` (`codegen::objects::object_text`): a concatenation, a print (to
/// `System.err` too), `String.valueOf` (which a builder's `append` of an object calls) and
/// `String.format` with its arguments one by one.
pub fn generator_text(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Builtin(
            Builtin::Println
            | Builtin::Print
            | Builtin::ObjectText
            | Builtin::Format
            | Builtin::Native(Native::ErrPrint { .. }),
            _,
        ) => true,
        ExprKind::Binary(BinOp::Add, ..) | ExprKind::Assign { op: Some(BinOp::Add), .. } => {
            x.ty == Type::String
        }
        _ => false,
    }
}

/// The `toString` methods that showing a value of type `ty` may run, and the `hashCode`s that
/// `Object`'s `toString` shows.
fn to_string_impls(class: &Class, ty: Type) -> Vec<usize> {
    // A plain enum's value shows the `toString` of its constant, or `Enum`'s, which shows its name.
    if let Type::Enum(name) = ty {
        let Some(t) = class.type_index(name) else { return Vec::new() };
        let mut out = Vec::new();
        for c in std::iter::once(t).chain(class.hierarchy.concretes(t)) {
            if let Some(i) = hierarchy::to_string_impl(class, c)
                && !out.contains(&i)
            {
                out.push(i);
            }
        }
        return out;
    }
    let Some(t) = class.class_index(ty) else { return Vec::new() };
    let mut out = Vec::new();
    for c in class.hierarchy.concretes(t) {
        if let Some(i) = hierarchy::text_impl(class, c)
            && !out.contains(&i)
        {
            out.push(i);
        }
    }
    out
}
