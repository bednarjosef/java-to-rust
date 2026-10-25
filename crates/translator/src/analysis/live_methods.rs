//! The methods that can run: reachability over the lowered program from `main`, where a call
//! runs the implementations of the classes whose objects its receiver can be, by the points-to
//! facts of `object_values` (`analyze_for_dispatch`), instead of those of every class its type
//! allows. Reachability over javac's trees (`program::reach`) has no such facts: a call of
//! `toString`, `equals` or `hashCode` on an `Object` in library code reaches the override of every
//! class with objects, and so does every string conversion, which pulls in large parts of the JDK
//! that the program never runs. A whole program keeps the methods that can run, and the others
//! get a body that never runs (`prune`): what lowering rejected in them is no longer a reason to
//! reject the program.
//!
//! Why a method left out cannot run. The facts say, for every place, which classes its objects
//! can be, and where the analysis does not follow a value (a library call's result, a caught
//! exception, `this` in a static initializer) that it may be an object of any class its type
//! allows. They are drawn from the code of every method that lowering kept, which includes the
//! code of every method that this walk finds: a method that lowering rejected and the walk finds
//! rejects the program. So every value that running code can hold is in the facts, and every call
//! that running code makes goes to a method that the walk finds, by induction over the run. A call
//! that lowering left unresolved has no method to go to: where it can run, it rejects the program.
//! Where the walk meets a method entered on an object that the analysis did not follow it for, the
//! facts do not tell, and nothing is left out (`live_methods` is an error).
//!
//! An instance method runs on an object, so where the facts allow a class that no running code
//! creates an object of, its methods wait until some code does. The objects of the program's
//! classes come from a `new` (reflection's `newInstance` is lowered to one, see `lower/by_name.rs`),
//! from the runtime raising an exception (`Class::raised`, which are `new`s too), from an enum's
//! static initialization (its constants), or from a copy of an object that exists (`clone`).
//!
//! What runs besides the calls in the code:
//!
//! - A type's static initialization, with that of every type above it, once a static method of
//!   it or a constructor is called, a static field of it is read or written, an expression of an
//!   enum type is evaluated (`valueOf` makes its constants), or `Builtin::InitializeClass` names
//!   it; with the constants of an enum and the methods that build them
//!   (`Class::builds_constants`), and the initializers of its static fields.
//! - The initializers of the instance fields of a class and of the types above it, when an object
//!   of it is created, and the constructors of its superclasses when it has none of its own.
//! - What the runtime may call itself on an object that it holds (`object_values::runtime_called`:
//!   `toString` to make its text, `equals`, `hashCode`, `compareTo` and the methods of
//!   `CharSequence` and `Number`). It holds every object given to a builtin (but those that only
//!   lock, wait, name, test, hash by identity, copy or fill), every operand of a string
//!   concatenation, every exception thrown or raised by the runtime, and every element of an
//!   array given to a builtin (`object_values::handed`).
//! - The exceptions that the runtime raises (`Class::raised`).
//!
//! What does not run of a method that does: the side of a branch whose condition has a value
//! that the walk knows, such as a parameter that every call passes the same constant (`known`).
//! The walk learns those values as it goes, and walks a method again when one it relied on
//! changes. Nor does a `catch` clause that catches no class with objects: what it catches is an
//! object, and objects come only from where the walk sees them come from (above), the runtime's
//! exceptions among them (`Class::raised`), so a clause whose classes have none yet waits, and the
//! method is walked again once one of them does.

use super::FastMap as HashMap;
use super::FastSet as HashSet;
use super::object_values::{self, ClassSet, Flow, ObjectValues, STATIC};
use crate::ast::*;
use std::cell::RefCell;
use std::rc::Rc;

mod known;

/// What can run.
pub struct LiveMethods {
    /// The methods that can run, by index into `Class::methods`.
    pub methods: HashSet<usize>,
    /// The types whose static initialization can run.
    pub initialized: HashSet<usize>,
    /// The concrete classes that can have objects.
    pub instantiated: HashSet<usize>,
    /// The types above a class that can have objects, itself included: those whose instance
    /// field initializers can run.
    pub constructed: HashSet<usize>,
    /// The types any of whose code can run: those initialized, those with a method that can run,
    /// and those above a class that can have objects.
    pub touched: HashSet<usize>,
    /// The calls that lowering left unresolved in code that can run: each rejects the program.
    pub unresolved: Vec<Error>,
    /// The types whose statics code that a started thread runs may touch (`Class::thread_statics`):
    /// `None` when the program starts no thread, or the facts do not tell.
    pub thread_statics: Option<HashSet<usize>>,
    /// The classes whose objects the receiver of each instance call that can run may be, over
    /// every object of every class that runs the calling method (`Class::receivers`).
    pub receivers: HashMap<ExprId, ClassSet>,
}

impl LiveMethods {
    /// Whether code that lowering rejected, of type `ty`, can run: when it is part of the body
    /// of `method` alone, whether that method can; otherwise whether any code of the type can.
    pub fn runs(&self, ty: usize, method: Option<usize>) -> bool {
        match method {
            Some(m) => self.methods.contains(&m),
            None => self.touched.contains(&ty),
        }
    }
}

/// The methods that can run in `class` (see the module's comment), or what the facts do not
/// tell, when they do not.
pub fn live_methods(class: &Class) -> Result<LiveMethods, String> {
    Ok(live_methods_unless(class, &[])?.expect("a walk with nothing to stop it runs to its end"))
}

/// `live_methods`, which stops as soon as it finds that some piece of `code` can run: `None`
/// then. Each piece is a type and a method or none, as in `LiveMethods::runs`. What the walk
/// finds only grows, so a piece that runs when it stops runs at its end too.
pub fn live_methods_unless(
    class: &Class,
    code: &[(usize, Option<usize>)],
) -> Result<Option<LiveMethods>, String> {
    let values = object_values::analyze_for_dispatch(class);
    if crate::timings::enabled() {
        eprintln!("timing: points-to for dispatch done{}", crate::timings::memory());
    }
    let found = crate::timings::phase("live walk", || walk(class, code, &values, true));
    if std::env::var_os("JAVA_TO_RUST_LIVE_CHECK").is_some() {
        let slow =
            crate::timings::phase("live walk that skips nothing", || walk(class, code, &values, false));
        check(&found, &slow);
    }
    found
}

/// The walk of `live_methods_unless` over the points-to facts `values`, which skips the work
/// that it knows does nothing when `fast` (`Walk::fast`).
fn walk(
    class: &Class,
    code: &[(usize, Option<usize>)],
    values: &ObjectValues,
    fast: bool,
) -> Result<Option<LiveMethods>, String> {
    let walk = Walk {
        class,
        values,
        fast,
        glue: object_values::runtime_called_for_dispatch(class).into_iter().collect(),
        glue_declared: glue_declared(class),
        supers: supers(class),
        initializers: methods_by_type(class, |m| class.is_initializer(m)),
        constructors: methods_by_type(class, |m| class.methods[m].is_ctor),
        index: class.types.iter().enumerate().map(|(t, d)| (d.name.as_str(), t)).collect(),
        stop: Stop::new(class, code),
        state: RefCell::new(State {
            work: Vec::new(),
            queued: HashSet::default(),
            seen: HashSet::default(),
            rescans: [0; 4],
            waiting: HashMap::default(),
            waited: HashSet::default(),
            contexts: HashMap::default(),
            catching: HashMap::default(),
            glued: HashSet::default(),
            known: known::Knowledge::default(),
            unknown: None,
            live: LiveMethods {
                methods: HashSet::default(),
                initialized: HashSet::default(),
                instantiated: HashSet::default(),
                constructed: HashSet::default(),
                touched: HashSet::default(),
                unresolved: Vec::new(),
                thread_statics: None,
                receivers: HashMap::default(),
            },
            built: HashSet::default(),
            initialized_above: HashSet::default(),
            prepared: HashMap::default(),
            glued_classes: ClassSet::default(),
            glued_any: HashSet::default(),
            stopped: false,
        }),
        memos: RefCell::default(),
    };
    walk.captured_fields();
    for (m, method) in class.methods.iter().enumerate() {
        if method.is_main {
            walk.enter(m, STATIC, STATIC, None);
        }
    }
    for e in &class.raised {
        walk.code(STATIC, STATIC, e);
        walk.handed(STATIC, STATIC, e);
    }
    walk.run();
    let thread_statics = {
        let state = walk.state.borrow();
        if state.unknown.is_none() && !state.stopped { walk.thread_statics() } else { None }
    };
    let Walk { state, .. } = walk;
    let State { unknown, mut live, stopped, .. } = state.into_inner();
    if let Some(unknown) = unknown {
        return Err(unknown);
    }
    if stopped {
        return Ok(None);
    }
    let mut touched: HashSet<usize> = live.initialized.clone();
    touched.extend(live.methods.iter().map(|&m| class.methods[m].class));
    touched.extend(live.constructed.iter().copied());
    live.touched = touched;
    live.thread_statics = thread_statics;
    Ok(Some(live))
}

/// With `JAVA_TO_RUST_LIVE_CHECK`, that the walk found what a walk that skips nothing finds:
/// the same sets, the same unresolved calls in the same order, and the same receivers.
fn check(fast: &Result<Option<LiveMethods>, String>, slow: &Result<Option<LiveMethods>, String>) {
    let differ = |what: &str| panic!("live check: the walk that skips work found other {what}");
    let (fast, slow) = match (fast, slow) {
        (Err(a), Err(b)) if a == b => return eprintln!("live check: the same error"),
        (Ok(None), Ok(None)) => return eprintln!("live check: both stopped"),
        (Ok(Some(a)), Ok(Some(b))) => (a, b),
        _ => return differ("outcome"),
    };
    let sets = [
        ("methods", &fast.methods, &slow.methods),
        ("initialized types", &fast.initialized, &slow.initialized),
        ("instantiated classes", &fast.instantiated, &slow.instantiated),
        ("constructed types", &fast.constructed, &slow.constructed),
        ("touched types", &fast.touched, &slow.touched),
    ];
    for (what, a, b) in sets {
        if a != b {
            differ(what);
        }
    }
    if format!("{:?}", fast.unresolved) != format!("{:?}", slow.unresolved) {
        differ("unresolved calls");
    }
    if fast.thread_statics != slow.thread_statics {
        differ("thread statics");
    }
    let classes = |set: &ClassSet| set.iter().collect::<Vec<usize>>();
    if fast.receivers.len() != slow.receivers.len()
        || fast.receivers.iter().any(|(x, a)| slow.receivers.get(x).map(classes) != Some(classes(a)))
    {
        differ("receivers");
    }
    eprintln!(
        "live check: the same {} methods, {} classes with objects and {} receivers",
        fast.methods.len(),
        fast.instantiated.len(),
        fast.receivers.len()
    );
}

impl Walk<'_> {
    /// Walks the methods on the work list, and those they reach, until none is left, or until
    /// the walk can stop: it met what the facts do not tell (`State::unknown`, which makes the
    /// walk an error whatever else it finds), or code that stops it can run (`Stop`).
    fn run(&self) {
        let class = self.class;
        let timings = crate::timings::enabled();
        let mut walks = 0usize;
        loop {
            let next = {
                let mut state = self.state.borrow_mut();
                let next = state.work.pop();
                if let Some(pair) = next {
                    state.queued.remove(&pair);
                }
                next
            };
            let Some((m, ctx)) = next else { break };
            walks += 1;
            {
                let mut state = self.state.borrow_mut();
                // Each method that runs is on the work list once at least (`reach`).
                if state.unknown.is_some()
                    || self.stop.methods.contains(&m)
                    || walks.is_multiple_of(1024) && self.stop.types_run(&state.live)
                {
                    state.stopped = true;
                    if timings {
                        eprintln!("timing: live walk stopped after {walks} walks");
                    }
                    break;
                }
            }
            if timings && walks.is_multiple_of(10_000) {
                let state = self.state.borrow();
                let waiting: usize = state.waiting.values().map(Vec::len).sum();
                let words: usize = state.live.receivers.values().map(ClassSet::words).sum();
                let [parameter, field, object, catch] = state.rescans;
                eprintln!(
                    "timing: live walk: {walks} walks, {} to go, {} seen, {waiting} waiting, {} receivers in \
                     {words} words; walked again for a parameter {parameter}, a field {field}, an object \
                     {object}, a catch {catch}{}",
                    state.work.len(),
                    state.seen.len(),
                    state.live.receivers.len(),
                    crate::timings::memory()
                );
            }
            let dead = self.prepare(m);
            let code = |e: &Expr| {
                visit_expr(e, &mut |x| {
                    if dead.contains(&x.id) { self.unresolved(x) } else { self.expr(m, ctx, x) }
                })
            };
            let statement = |s: &Stmt| match s {
                Stmt::Throw(e) if dead.contains(&e.id) => {}
                _ => self.statement(m, ctx, s),
            };
            for s in &class.methods[m].body {
                visit_stmt(s, &mut |e| code(e), &mut |s| statement(s));
            }
            self.state.borrow_mut().known.env = Rc::default();
        }
    }

    /// Makes the parameters and locals of method `m` those that the walk knows (`env`), and
    /// returns the expressions of it that cannot run (`dead`, `dead_catches`). A method walked on
    /// more than one object gets the same on each until something that they relied on changes,
    /// which walks it again (`rescan`): so they are remembered until then (`State::prepared`).
    fn prepare(&self, m: usize) -> Rc<HashSet<ExprId>> {
        let prepared = if self.fast { self.state.borrow().prepared.get(&m).cloned() } else { None };
        if let Some((env, dead)) = prepared {
            self.state.borrow_mut().known.env = env;
            return dead;
        }
        let env = Rc::new(self.env(m));
        self.state.borrow_mut().known.env = env.clone();
        let mut dead = self.dead(m);
        dead.extend(self.dead_catches(m));
        let dead = Rc::new(dead);
        let mut state = self.state.borrow_mut();
        if self.fast && state.contexts.get(&m).is_some_and(|c| c.len() > 1) {
            state.prepared.insert(m, (env, dead.clone()));
        }
        dead
    }

    /// The types whose statics code that a started thread runs may read or write, or whose
    /// static initialization it may run: a second walk, once this one is done, from what the
    /// runtime runs on a thread that `Thread.start()` starts (the call of `run` in
    /// `Native::ThreadStart`), from the dispatch of an exception that ends a thread
    /// (`Native::InstallUncaught`) and from the exceptions that the runtime raises. `None` when
    /// the program starts no thread, or when the facts do not tell what that walk needs.
    ///
    /// Why the second walk is sound. It starts from this walk's knowledge, which holds for every
    /// call and write of running code, the started threads' included (`known`), and with the
    /// classes that this walk found objects of, of which a thread may be handed any: so it
    /// follows every call that a thread's code can make. It starts with no type initialized, as a
    /// thread may be the first to touch a class: so it runs the static initialization of every
    /// type that the thread's code reads or writes a static of, calls a static method or
    /// constructor of, or names an enum constant of; and it runs the field initializers of every
    /// class that the thread's code creates an object of (`State::built`).
    fn thread_statics(&self) -> Option<HashSet<usize>> {
        let class = self.class;
        let main = self.state.borrow();
        let mut roots: Vec<(usize, &Expr)> = Vec::new();
        let mut starts = false;
        for &m in &main.live.methods {
            for s in &class.methods[m].body {
                visit_stmt(
                    s,
                    &mut |e| {
                        visit_expr(e, &mut |x| match &x.kind {
                            ExprKind::Builtin(Builtin::Native(Native::ThreadStart), args) => {
                                starts = true;
                                roots.extend(args.last().map(|run| (m, run)));
                            }
                            ExprKind::Builtin(Builtin::Native(Native::InstallUncaught), args) => {
                                roots.extend(args.iter().map(|a| (m, a)))
                            }
                            _ => {}
                        })
                    },
                    &mut |_| {},
                );
            }
        }
        if !starts {
            return None;
        }
        let thread = Walk {
            class,
            values: self.values,
            fast: self.fast,
            glue: self.glue.clone(),
            glue_declared: self.glue_declared.clone(),
            supers: self.supers.clone(),
            initializers: self.initializers.clone(),
            constructors: self.constructors.clone(),
            index: self.index.clone(),
            stop: Stop::default(),
            state: RefCell::new(State {
                work: Vec::new(),
                queued: HashSet::default(),
                seen: HashSet::default(),
                rescans: [0; 4],
                waiting: HashMap::default(),
                waited: HashSet::default(),
                contexts: HashMap::default(),
                catching: HashMap::default(),
                glued: HashSet::default(),
                known: main.known.clone(),
                unknown: None,
                live: LiveMethods {
                    methods: HashSet::default(),
                    initialized: HashSet::default(),
                    instantiated: main.live.instantiated.clone(),
                    constructed: main.live.constructed.clone(),
                    touched: HashSet::default(),
                    unresolved: Vec::new(),
                    thread_statics: None,
                    receivers: HashMap::default(),
                },
                built: HashSet::default(),
                initialized_above: HashSet::default(),
                prepared: HashMap::default(),
                glued_classes: ClassSet::default(),
                glued_any: HashSet::default(),
                stopped: false,
            }),
            memos: RefCell::default(),
        };
        for e in &class.raised {
            thread.code(STATIC, STATIC, e);
            thread.handed(STATIC, STATIC, e);
        }
        for (m, root) in roots {
            for &ctx in main.contexts.get(&m).into_iter().flatten() {
                let env = Rc::new(thread.env(m));
                thread.state.borrow_mut().known.env = env;
                thread.code(m, ctx, root);
                thread.state.borrow_mut().known.env = Rc::default();
            }
        }
        thread.run();
        let state = thread.state.into_inner();
        state.unknown.is_none().then_some(state.live.initialized)
    }
}

/// Gives every method that cannot run a body that never runs (`Builtin::NoObject`), and returns
/// how many it changed. A `<clinit>` that cannot run is among them: no code reads or writes the
/// statics of its class, so nothing calls its `init`.
///
/// The instance field initializers of a class that no object can be created of, nor of a class
/// below it, never run either, and are dropped: the fields get their defaults in a constructor
/// that never runs. Such a class may still have a struct, when its type names a value that is
/// always null, and its initializers may use what is no longer emitted. An enum's objects are its
/// constants, which its static initialization makes (`EnumConstant::values`), not a `new`: enums
/// and their constants' classes keep theirs.
pub fn prune(class: &mut Class, live: &LiveMethods) -> usize {
    let mut next_id = largest_id(class) + 1;
    let mut pruned = 0;
    for (m, method) in class.methods.iter_mut().enumerate() {
        if method.is_abstract || live.methods.contains(&m) || never_runs(method) {
            continue;
        }
        let never = ExprKind::Builtin(Builtin::NoObject { class: method.class }, Vec::new());
        method.body = vec![Stmt::Expr(Expr { kind: never, pos: method.pos, ty: Type::Void, id: next_id })];
        next_id += 1;
        pruned += 1;
    }
    let is_enum = |t: usize| class.types[t].kind == TypeKind::Enum;
    let unconstructed: Vec<usize> = (0..class.types.len())
        .filter(|&t| !live.constructed.contains(&t))
        .filter(|&t| !is_enum(t) && !class.hierarchy.ancestors(t).into_iter().any(is_enum))
        .collect();
    for t in unconstructed {
        for field in &mut class.types[t].fields {
            pruned += usize::from(field.init.take().is_some());
        }
    }
    pruned
}

/// Whether the body of `method` already never runs: a stub's, or a dormant method's.
pub(super) fn never_runs(method: &Method) -> bool {
    matches!(
        method.body.as_slice(),
        [Stmt::Expr(Expr { kind: ExprKind::Builtin(Builtin::NoObject { .. }, _), .. })]
    )
}

/// The largest id of an expression of the program.
pub(super) fn largest_id(class: &Class) -> usize {
    let mut max = 0;
    let mut see = |e: &Expr| visit_expr(e, &mut |x| max = max.max(x.id));
    for method in &class.methods {
        method.body.iter().for_each(|s| visit_stmt(s, &mut see, &mut |_| {}));
    }
    for decl in &class.types {
        decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()).for_each(&mut see);
        decl.enum_constants.iter().flatten().flat_map(|c| &c.values).for_each(&mut see);
    }
    class.raised.iter().for_each(&mut see);
    max
}

/// Every type above each type, itself included: its superclasses and its interfaces.
/// The groups that the runtime may call (`runtime_called_for_dispatch`) that each type declares a
/// method of.
fn glue_declared(class: &Class) -> Vec<Vec<usize>> {
    let glue = object_values::runtime_called_for_dispatch(class);
    let mut declared = vec![Vec::new(); class.types.len()];
    for (m, method) in class.methods.iter().enumerate() {
        let group = class.hierarchy.group_id(m);
        if glue.contains(&group) {
            declared[method.class].push(group);
        }
    }
    declared
}

/// The methods of each type for which `keep` holds, in index order.
fn methods_by_type(class: &Class, keep: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); class.types.len()];
    for m in (0..class.methods.len()).filter(|&m| keep(m)) {
        out[class.methods[m].class].push(m);
    }
    out
}

fn supers(class: &Class) -> Vec<Vec<usize>> {
    (0..class.types.len())
        .map(|t| {
            let mut seen = HashSet::default();
            let mut work = vec![t];
            while let Some(s) = work.pop() {
                if seen.insert(s) {
                    let decl = &class.types[s];
                    work.extend(decl.extends.iter().chain(&decl.implements));
                }
            }
            let mut out: Vec<usize> = seen.into_iter().collect();
            out.sort_unstable();
            out
        })
        .collect()
}

struct Walk<'a> {
    class: &'a Class,
    values: &'a ObjectValues,
    /// Whether the walk skips work that it knows does nothing or finds what it found before:
    /// running the methods of a call again with arguments known to hold what they held when it
    /// last ran them (`Walk::call`), handing the runtime objects that it has been handed
    /// (`Memos::handed`, `State::glued_classes`), initializing a type again
    /// (`State::initialized_above`), finding the values and dead code of a method again while
    /// nothing they relied on changed (`State::prepared`), and asking whether the classes with
    /// objects return the same literal while no other has objects (`same_literal`).
    /// `JAVA_TO_RUST_LIVE_CHECK` checks that the walk finds the same without.
    fast: bool,
    glue: Vec<usize>,
    /// The groups of `glue` that each type declares a method of (`glue_declared`).
    glue_declared: Vec<Vec<usize>>,
    supers: Vec<Vec<usize>>,
    /// The methods of each type's static initialization (`Class::is_initializer`), in index order.
    initializers: Vec<Vec<usize>>,
    /// The constructors of each type, in index order.
    constructors: Vec<Vec<usize>>,
    index: HashMap<&'a str, usize>,
    stop: Stop,
    state: RefCell<State>,
    /// What the points-to facts, which do not change during the walk, answer: the methods that
    /// each call runs on each object, and the concrete classes below each type. A method is walked
    /// again whenever what it relies on changes, and a call on a receiver that may be any class
    /// runs thousands of methods.
    memos: RefCell<Memos>,
}

/// The code whose running stops a walk (`live_methods_unless`).
#[derive(Default)]
struct Stop {
    /// The methods of the code, and every method of a type whose any code is.
    methods: HashSet<usize>,
    /// The types any of whose code is: one of them initialized or constructed stops the walk too.
    types: Vec<usize>,
}

impl Stop {
    fn new(class: &Class, code: &[(usize, Option<usize>)]) -> Stop {
        let mut stop = Stop::default();
        for &(ty, method) in code {
            match method {
                Some(m) => {
                    stop.methods.insert(m);
                }
                None => stop.types.push(ty),
            }
        }
        if !stop.types.is_empty() {
            let types: HashSet<usize> = stop.types.iter().copied().collect();
            stop.methods
                .extend((0..class.methods.len()).filter(|&m| types.contains(&class.methods[m].class)));
        }
        stop
    }

    fn types_run(&self, live: &LiveMethods) -> bool {
        self.types.iter().any(|t| live.initialized.contains(t) || live.constructed.contains(t))
    }
}

/// The methods that a call runs, each with the class of the object it runs on.
type Targets = Rc<[(usize, usize)]>;

#[derive(Default)]
struct Memos {
    targets: HashMap<(ExprId, usize), Targets>,
    /// Each list of targets once.
    lists: HashSet<Targets>,
    concretes: HashMap<usize, Rc<[usize]>>,
    /// What each call passed when it last ran all its targets, by call and list of targets
    /// (`Walk::call`).
    passed: HashMap<(ExprId, usize), Passed>,
    /// The expressions whose objects the runtime has been handed, each with its method and the
    /// object it runs on (`object_values::handed`), and the thrown expressions so (`handed_by`):
    /// handing them again hands the runtime nothing new.
    handed: HashSet<(usize, ExprId, usize)>,
    thrown: HashSet<(usize, ExprId, usize)>,
    /// What `same_literal` found, by method and type, with how many classes had objects then.
    literals: HashMap<(usize, usize), (usize, Option<i128>)>,
}

/// What a call passed the methods it runs (`Walk::call`): what was known of the argument at
/// each position that some method tracks the parameter of, in the order that `pass_known` found
/// them, and `found` as it left it.
struct Passed {
    order: Rc<[usize]>,
    found: Rc<[Option<known::Known>]>,
}

/// Why a method is walked again (`rescan`): a parameter, a field or an `instanceof` or call that it
/// reads is known to hold more, or a class that one of its `catch` clauses catches has objects.
#[derive(Clone, Copy)]
enum Rescan {
    Parameter,
    Field,
    Object,
    Catch,
}

struct State {
    /// The methods to scan, each with the class of the object it runs on (`STATIC`: none).
    work: Vec<(usize, usize)>,
    /// Those on the work list: one already on it is not added again, as its walk reads what the
    /// walk knows by then.
    queued: HashSet<(usize, usize)>,
    seen: HashSet<(usize, usize)>,
    /// How many times a method was put on the work list again, by `Rescan`.
    rescans: [usize; 4],
    /// The instance methods called on objects of a class that no object of is created yet, by
    /// that class: they run once one is. Each once, in the order first called: a call on a
    /// receiver that may be any class calls the method on thousands of them, each time the
    /// calling method is walked.
    waiting: HashMap<usize, Vec<usize>>,
    waited: HashSet<(usize, usize)>,
    /// The objects that each method has been walked on, by method.
    contexts: HashMap<usize, Vec<usize>>,
    /// The methods with a `catch` clause that catches no class with objects yet, by each type
    /// that the clause catches: they are walked again once an object of a class below it is
    /// created.
    catching: HashMap<usize, HashSet<usize>>,
    /// The classes whose objects the runtime has been handed (`glue`).
    glued: HashSet<usize>,
    /// The values that the walk knows (`known`).
    known: known::Knowledge,
    /// The first thing that the walk met that the facts do not answer.
    unknown: Option<String>,
    /// Whether the walk stopped before its end (`Walk::run`).
    stopped: bool,
    live: LiveMethods,
    /// The classes whose field initializers the walk has run: those of `LiveMethods::instantiated`,
    /// except in the walk of the started threads (`thread_statics`), which starts with the
    /// objects of the first walk and runs the initializers of those that its code creates.
    built: HashSet<usize>,
    /// The types whose static initialization has run with that of every type above them
    /// (`Walk::initialize`).
    initialized_above: HashSet<usize>,
    /// What `prepare` found for each method walked on more than one object, until a change to
    /// what it relied on walks the method again (`rescan`), or makes a field unknown wherever it
    /// is (`Knowledge::unplaced`).
    prepared: HashMap<usize, (Rc<known::Env>, Rc<HashSet<ExprId>>)>,
    /// `glued`, and the types that the runtime has been handed objects of any class of
    /// (`Walk::glue`).
    glued_classes: ClassSet,
    glued_any: HashSet<Type>,
}

impl Walk<'_> {
    /// The methods that the call `x` in method `m` runs on an object of class `ctx`, each with the
    /// class of the object (`ObjectValues::targets`, remembered).
    fn targets(&self, m: usize, ctx: usize, x: &Expr) -> Targets {
        if let Some(found) = self.memos.borrow().targets.get(&(x.id, ctx)) {
            return found.clone();
        }
        let targets = self.values.targets(self.class, m, ctx, x);
        let mut memos = self.memos.borrow_mut();
        let list = match memos.lists.get(targets.as_slice()) {
            Some(list) => list.clone(),
            None => {
                let list: Targets = targets.into();
                memos.lists.insert(list.clone());
                list
            }
        };
        memos.targets.insert((x.id, ctx), list.clone());
        list
    }

    /// The concrete classes below type `t`, itself included (`Hierarchy::concretes`, remembered).
    fn concretes(&self, t: usize) -> Rc<[usize]> {
        if let Some(found) = self.memos.borrow().concretes.get(&t) {
            return found.clone();
        }
        let found: Rc<[usize]> = self.class.hierarchy.concretes(t).into();
        self.memos.borrow_mut().concretes.insert(t, found.clone());
        found
    }

    /// Method `m` is entered on an object of class `ctx` with the arguments `args` of method
    /// `caller` (`None`: arguments that the walk does not see).
    fn enter(&self, m: usize, ctx: usize, caller: usize, args: Option<&[Expr]>) {
        self.pass(caller, m, args);
        self.reach(m, ctx);
    }

    /// Method `m` runs on an object of class `ctx`.
    fn reach(&self, m: usize, ctx: usize) {
        let method = &self.class.methods[m];
        if method.is_abstract {
            return;
        }
        if method.is_static || method.is_ctor {
            self.initialize(method.class);
        }
        let ctx = if method.is_static { STATIC } else { ctx };
        let mut state = self.state.borrow_mut();
        // A method runs on an object only once one is created.
        if ctx != STATIC && !state.live.instantiated.contains(&ctx) {
            if state.waited.insert((ctx, m)) {
                state.waiting.entry(ctx).or_default().push(m);
            }
            return;
        }
        // The facts of a method are for the objects that the analysis followed it on.
        if !self.values.contexts[m].contains(&ctx) && state.unknown.is_none() {
            let on = self.class.types.get(ctx).map_or("no object", |d| d.name.as_str());
            let name = format!("{}.{}", self.class.types[method.class].name, method.name);
            state.unknown = Some(format!("`{name}` run on {on}"));
        }
        state.live.methods.insert(m);
        if state.seen.insert((m, ctx)) && state.queued.insert((m, ctx)) {
            state.work.push((m, ctx));
            state.contexts.entry(m).or_default().push(ctx);
        }
    }

    /// The static initialization of type `t` and of every type above it runs.
    fn initialize(&self, t: usize) {
        if self.fast && self.state.borrow().initialized_above.contains(&t) {
            return;
        }
        for &t in &self.supers[t] {
            if !self.state.borrow_mut().live.initialized.insert(t) {
                continue;
            }
            let class = self.class;
            for &m in &self.initializers[t] {
                self.enter(m, STATIC, STATIC, None);
            }
            let decl = &class.types[t];
            for init in decl.statics.iter().filter_map(|f| f.init.as_ref()) {
                self.code(STATIC, STATIC, init);
            }
            for value in decl.enum_constants.iter().flatten().flat_map(|c| &c.values) {
                self.code(STATIC, STATIC, value);
            }
            // An enum's static initialization creates its constants, objects of the enum and
            // of the classes of their bodies.
            if decl.kind == TypeKind::Enum {
                for &c in self.concretes(t).iter() {
                    self.instantiate(c);
                }
            }
        }
        // Each type above is initialized now, so initializing `t` again does nothing.
        if self.fast {
            self.state.borrow_mut().initialized_above.insert(t);
        }
    }

    /// An object of the concrete class `c` is created: the static initialization of its class,
    /// and the initializers of its instance fields and of the types above it.
    fn instantiate(&self, c: usize) {
        self.initialize(c);
        let (first, built) = {
            let mut state = self.state.borrow_mut();
            (state.live.instantiated.insert(c), state.built.insert(c))
        };
        if first {
            self.first_object(c);
        }
        if !built {
            return;
        }
        for &t in &self.supers[c] {
            for field in &self.class.types[t].fields {
                if let Some(init) = &field.init {
                    self.code(STATIC, c, init);
                    self.initialize_field(t, &field.name, init);
                }
            }
        }
    }

    /// The first object of the concrete class `c` is created: what waited for one can run.
    fn first_object(&self, c: usize) {
        // The `catch` clauses of a type above it can run now.
        let woken: Vec<usize> = {
            let mut state = self.state.borrow_mut();
            let mut woken = Vec::new();
            for &t in &self.supers[c] {
                if state.live.constructed.insert(t) {
                    woken.extend(state.catching.remove(&t).unwrap_or_default());
                }
            }
            woken
        };
        for m in woken {
            self.rescan(m, Rescan::Catch);
        }
        self.created(c);
        let waiting = self.state.borrow_mut().waiting.remove(&c).unwrap_or_default();
        for m in waiting {
            self.reach(m, c);
        }
    }

    /// The expressions of the `catch` clauses of method `m` that catch no class with objects yet
    /// (see the module's comment), which cannot run; the method waits for each type they catch.
    fn dead_catches(&self, m: usize) -> HashSet<ExprId> {
        let mut dead = HashSet::default();
        let mut waiting = Vec::new();
        {
            let state = self.state.borrow();
            let constructed = &state.live.constructed;
            for s in &self.class.methods[m].body {
                visit_stmt(s, &mut |_| {}, &mut |s| {
                    let Stmt::Try { catches, .. } = s else { return };
                    for catch in catches.iter().filter(|c| !c.types.iter().any(|t| constructed.contains(t))) {
                        waiting.extend(catch.types.iter().copied());
                        for b in &catch.body {
                            visit_stmt(
                                b,
                                &mut |e| visit_expr(e, &mut |x| _ = dead.insert(x.id)),
                                &mut |_| {},
                            );
                        }
                    }
                });
            }
        }
        let mut state = self.state.borrow_mut();
        for t in waiting {
            state.catching.entry(t).or_default().insert(m);
        }
        dead
    }

    /// The statements of method `m` that the runtime sees an object in: a `throw`.
    fn statement(&self, m: usize, ctx: usize, s: &Stmt) {
        if let Stmt::Throw(e) = s {
            self.handed(m, ctx, e);
        }
    }

    /// Everything that the expression `e`, in method `m` on an object of class `ctx` (`STATIC`
    /// for both in an initializer), runs.
    fn code(&self, m: usize, ctx: usize, e: &Expr) {
        visit_expr(e, &mut |x| self.expr(m, ctx, x));
    }

    /// What the expression `x` itself runs, not counting the expressions inside it.
    fn expr(&self, m: usize, ctx: usize, x: &Expr) {
        match &x.kind {
            ExprKind::Call { resolved: None, .. } | ExprKind::MethodCall { resolved: None, .. } => {
                self.unresolved(x)
            }
            ExprKind::Call { args, .. }
            | ExprKind::MethodCall { args, .. }
            | ExprKind::ThisCall { args, .. }
            | ExprKind::SuperInit { args, .. } => {
                let targets = self.targets(m, ctx, x);
                if targets.len() > 100_000 && crate::timings::enabled() {
                    let name = format!(
                        "{}.{}",
                        self.class.types[self.class.methods[m].class].name, self.class.methods[m].name
                    );
                    eprintln!(
                        "timing: live walk: a call in {name} on {ctx} runs {} methods{}",
                        targets.len(),
                        crate::timings::memory()
                    );
                }
                self.call(m, x, args, &targets);
            }
            ExprKind::New { class: c, ctor, args } => {
                self.instantiate(*c);
                match ctor {
                    Some(r) => self.enter(*r, *c, m, Some(args)),
                    // Without a constructor of its own, it runs those of its superclasses.
                    None => {
                        let mut ctors: Vec<usize> = (self.class.hierarchy.ancestors(*c).iter())
                            .flat_map(|&a| self.constructors[a].iter().copied())
                            .collect();
                        ctors.sort_unstable();
                        for r in ctors {
                            self.enter(r, *c, m, None);
                        }
                    }
                }
            }
            ExprKind::Static { class: c, .. } | ExprKind::EnumConstant { class: c, .. } => {
                self.initialize(*c)
            }
            ExprKind::Assign { target: Place::Static { class: c, .. }, .. }
            | ExprKind::IncDec { target: Place::Static { class: c, .. }, .. } => self.initialize(*c),
            ExprKind::Assign { op, target: Place::Field { object, name }, value } if known::tracked(x.ty) => {
                self.write_field(m, object.ty, name, op.is_none().then_some(&**value))
            }
            ExprKind::IncDec { target: Place::Field { object, name }, .. } if known::tracked(x.ty) => {
                self.write_field(m, object.ty, name, None)
            }
            ExprKind::Builtin(Builtin::InitializeClass { class: c }, _) => self.initialize(*c),
            ExprKind::Builtin(Builtin::Native(Native::Missing { what }), _) => {
                let message = format!("native method `{what}` not supported yet");
                let unresolved = &mut self.state.borrow_mut().live.unresolved;
                if !unresolved.iter().any(|e| e.pos == x.pos && e.message == message) {
                    unresolved.push(Error { pos: x.pos, message });
                }
            }
            _ => {}
        }
        // An enum's objects are its constants, which its static initialization makes.
        if let Type::Enum(name) | Type::Class(name, _) = x.ty
            && let Some(&t) = self.index.get(name)
            && self.class.types[t].kind == TypeKind::Enum
        {
            self.initialize(t);
        }
        if object_values::hands(x) && (!self.fast || self.memos.borrow_mut().handed.insert((m, x.id, ctx))) {
            for (flow, ty) in object_values::handed(self.values, self.class, m, ctx, x) {
                self.glue(flow, ty);
            }
        }
    }

    /// The call `x` in method `m`, with the arguments `args`, runs `targets`, each on an object
    /// of the class given with it.
    ///
    /// A method is walked many times, and a call in it runs the same targets on each walk on the
    /// same object, thousands of them on a receiver that may be any class. Where what is known of
    /// the arguments is what it was when the call last ran them all, running them again does
    /// nothing: their parameters already hold those values, they already run, and what the walk
    /// knows only grows. So the walk finds what is known of the arguments first, as running the
    /// targets would, without recording what it relies on (`probe`); when nothing changed, it
    /// records that, as running them would have, and runs nothing.
    fn call(&self, m: usize, x: &Expr, args: &[Expr], targets: &Targets) {
        let key = (x.id, Rc::as_ptr(targets).cast::<()>() as usize);
        let passed = self
            .fast
            .then(|| self.memos.borrow().passed.get(&key).map(|p| (p.order.clone(), p.found.clone())));
        let order = match passed.flatten() {
            Some((order, found)) => {
                let (now, relied) = self.probe(m, args, &order);
                if *now == *found {
                    let known = &mut self.state.borrow_mut().known;
                    for (on, m) in relied {
                        known.relied(on, m);
                    }
                    return;
                }
                Some(order)
            }
            None => None,
        };
        if let ExprKind::MethodCall { receiver, is_super: false, resolved: Some(r), .. } = &x.kind
            && !self.class.methods[*r].is_static
        {
            let mut state = self.state.borrow_mut();
            let classes = state.live.receivers.entry(receiver.id).or_default();
            targets.iter().for_each(|&(_, c)| classes.insert(c));
        }
        let mut found = vec![None; args.len()];
        for &(target, c) in targets.iter() {
            self.pass_known(m, target, Some(args), &mut found);
            self.reach(target, c);
        }
        if self.fast {
            // The positions in the order that `pass_known` finds them.
            let order = order.unwrap_or_else(|| {
                let mut order = Vec::new();
                for &(target, _) in targets.iter() {
                    let params = &self.class.methods[target].params;
                    if params.len() == args.len() {
                        for (i, p) in params.iter().enumerate() {
                            if known::tracked(p.ty) && !order.contains(&i) {
                                order.push(i);
                            }
                        }
                    }
                }
                order.into()
            });
            self.memos.borrow_mut().passed.insert(key, Passed { order, found: found.into() });
        }
    }

    /// A call that lowering left unresolved: where it can run, it rejects the program. One in code
    /// that cannot run is reported too, as the translation of its method still has it.
    fn unresolved(&self, x: &Expr) {
        if let ExprKind::Call { resolved: None, name, .. }
        | ExprKind::MethodCall { resolved: None, name, .. } = &x.kind
        {
            let message = format!("a call of `{name}` that lowering left unresolved");
            let unresolved = &mut self.state.borrow_mut().live.unresolved;
            if !unresolved.iter().any(|e| e.pos == x.pos && e.message == message) {
                unresolved.push(Error { pos: x.pos, message });
            }
        }
    }

    /// The runtime is handed the value of `e`, or the elements of the innermost arrays of an
    /// array.
    fn handed(&self, m: usize, ctx: usize, e: &Expr) {
        if self.fast && !self.memos.borrow_mut().thrown.insert((m, e.id, ctx)) {
            return;
        }
        let (flow, ty) = object_values::handed_by(self.values, self.class, m, ctx, e);
        self.glue(flow, ty);
    }

    /// The runtime may call the methods of `runtime_called` on the objects that `flow`, of type
    /// `ty`, holds.
    fn glue(&self, flow: Flow, ty: Type) {
        if self.fast {
            let state = self.state.borrow();
            let glued = if flow.any {
                state.glued_any.contains(&ty)
            } else {
                state.glued_classes.holds(&flow.classes)
            };
            if glued {
                return;
            }
        }
        let classes: Vec<usize> =
            if flow.any { self.values.concretes_of(ty).to_vec() } else { flow.classes.iter().collect() };
        for c in classes {
            // Handing the runtime an object of a class again calls nothing new.
            {
                let mut state = self.state.borrow_mut();
                if !state.glued.insert(c) {
                    continue;
                }
                if self.fast {
                    state.glued_classes.insert(c);
                }
            }
            // A class implements only the groups that a type above it declares a method of: asking
            // for the others, of thousands of `toString`s and `equals`es, remembered an answer of
            // none for each class, which ran the Minecraft server's census out of memory.
            let above: HashSet<usize> =
                self.supers[c].iter().flat_map(|&t| &self.glue_declared[t]).copied().collect();
            for &g in self.glue.iter().filter(|g| above.contains(g)) {
                if let Some(target) = self.values.implementation(self.class, g, c) {
                    self.enter(target, c, STATIC, None);
                }
            }
        }
        if self.fast && flow.any {
            self.state.borrow_mut().glued_any.insert(ty);
        }
    }
}
