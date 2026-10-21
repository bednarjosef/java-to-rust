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

use super::object_values::{self, Flow, ObjectValues, STATIC};
use crate::ast::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

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
    let values = object_values::analyze_for_dispatch(class);
    let walk = Walk {
        class,
        values: &values,
        glue: object_values::runtime_called_for_dispatch(class).into_iter().collect(),
        supers: supers(class),
        index: class.types.iter().enumerate().map(|(t, d)| (d.name.as_str(), t)).collect(),
        state: RefCell::new(State {
            work: Vec::new(),
            seen: HashSet::new(),
            waiting: HashMap::new(),
            unknown: None,
            live: LiveMethods {
                methods: HashSet::new(),
                initialized: HashSet::new(),
                instantiated: HashSet::new(),
                constructed: HashSet::new(),
                touched: HashSet::new(),
                unresolved: Vec::new(),
            },
        }),
    };
    for (m, method) in class.methods.iter().enumerate() {
        if method.is_main {
            walk.reach(m, STATIC);
        }
    }
    for e in &class.raised {
        walk.code(STATIC, STATIC, e);
        walk.handed(STATIC, STATIC, e);
    }
    loop {
        let next = walk.state.borrow_mut().work.pop();
        let Some((m, ctx)) = next else { break };
        for s in &class.methods[m].body {
            visit_stmt(s, &mut |e| walk.code(m, ctx, e), &mut |s| walk.statement(m, ctx, s));
        }
    }
    let Walk { state, supers, .. } = walk;
    let State { unknown, mut live, .. } = state.into_inner();
    if let Some(unknown) = unknown {
        return Err(unknown);
    }
    live.constructed = live.instantiated.iter().flat_map(|&c| supers[c].iter().copied()).collect();
    let mut touched: HashSet<usize> = live.initialized.clone();
    touched.extend(live.methods.iter().map(|&m| class.methods[m].class));
    touched.extend(live.constructed.iter().copied());
    live.touched = touched;
    Ok(live)
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
fn never_runs(method: &Method) -> bool {
    matches!(
        method.body.as_slice(),
        [Stmt::Expr(Expr { kind: ExprKind::Builtin(Builtin::NoObject { .. }, _), .. })]
    )
}

/// The largest id of an expression of the program.
fn largest_id(class: &Class) -> usize {
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
fn supers(class: &Class) -> Vec<Vec<usize>> {
    (0..class.types.len())
        .map(|t| {
            let mut seen = HashSet::new();
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
    glue: Vec<usize>,
    supers: Vec<Vec<usize>>,
    index: HashMap<&'a str, usize>,
    state: RefCell<State>,
}

struct State {
    /// The methods to scan, each with the class of the object it runs on (`STATIC`: none).
    work: Vec<(usize, usize)>,
    seen: HashSet<(usize, usize)>,
    /// The instance methods called on objects of a class that no object of is created yet, by
    /// that class: they run once one is.
    waiting: HashMap<usize, Vec<usize>>,
    /// The first thing that the walk met that the facts do not answer.
    unknown: Option<String>,
    live: LiveMethods,
}

impl Walk<'_> {
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
            state.waiting.entry(ctx).or_default().push(m);
            return;
        }
        // The facts of a method are for the objects that the analysis followed it on.
        if !self.values.contexts[m].contains(&ctx) && state.unknown.is_none() {
            let on = self.class.types.get(ctx).map_or("no object", |d| d.name.as_str());
            let name = format!("{}.{}", self.class.types[method.class].name, method.name);
            state.unknown = Some(format!("`{name}` run on {on}"));
        }
        state.live.methods.insert(m);
        if state.seen.insert((m, ctx)) {
            state.work.push((m, ctx));
        }
    }

    /// The static initialization of type `t` and of every type above it runs.
    fn initialize(&self, t: usize) {
        for &t in &self.supers[t] {
            if !self.state.borrow_mut().live.initialized.insert(t) {
                continue;
            }
            let class = self.class;
            for (m, method) in class.methods.iter().enumerate() {
                if method.class == t && (method.name == "<clinit>" || class.builds_constants(m)) {
                    self.reach(m, STATIC);
                }
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
                for c in class.hierarchy.concretes(t) {
                    self.instantiate(c);
                }
            }
        }
    }

    /// An object of the concrete class `c` is created: the static initialization of its class,
    /// and the initializers of its instance fields and of the types above it.
    fn instantiate(&self, c: usize) {
        self.initialize(c);
        if !self.state.borrow_mut().live.instantiated.insert(c) {
            return;
        }
        let waiting = self.state.borrow_mut().waiting.remove(&c).unwrap_or_default();
        for m in waiting {
            self.reach(m, c);
        }
        for &t in &self.supers[c] {
            for init in self.class.types[t].fields.iter().filter_map(|f| f.init.as_ref()) {
                self.code(STATIC, c, init);
            }
        }
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
            ExprKind::Call { resolved: None, name, .. }
            | ExprKind::MethodCall { resolved: None, name, .. } => {
                let message = format!("a call of `{name}` that lowering left unresolved");
                self.state.borrow_mut().live.unresolved.push(Error { pos: x.pos, message });
            }
            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. } => {
                for (target, c) in self.values.targets(self.class, m, ctx, x) {
                    self.reach(target, c);
                }
            }
            ExprKind::New { class: c, ctor, .. } => {
                self.instantiate(*c);
                match ctor {
                    Some(r) => self.reach(*r, *c),
                    // Without a constructor of its own, it runs those of its superclasses.
                    None => {
                        let ancestors = self.class.hierarchy.ancestors(*c);
                        let class = self.class;
                        for (r, method) in class.methods.iter().enumerate() {
                            if method.is_ctor && ancestors.contains(&method.class) {
                                self.reach(r, *c);
                            }
                        }
                    }
                }
            }
            ExprKind::Static { class: c, .. } | ExprKind::EnumConstant { class: c, .. } => {
                self.initialize(*c)
            }
            ExprKind::Assign { target: Place::Static { class: c, .. }, .. }
            | ExprKind::IncDec { target: Place::Static { class: c, .. }, .. } => self.initialize(*c),
            ExprKind::Builtin(Builtin::InitializeClass { class: c }, _) => self.initialize(*c),
            _ => {}
        }
        // An enum's objects are its constants, which its static initialization makes.
        if let Type::Enum(name) | Type::Class(name, _) = x.ty
            && let Some(&t) = self.index.get(name)
            && self.class.types[t].kind == TypeKind::Enum
        {
            self.initialize(t);
        }
        for (flow, ty) in object_values::handed(self.values, self.class, m, ctx, x) {
            self.glue(flow, ty);
        }
    }

    /// The runtime is handed the value of `e`, or the elements of the innermost arrays of an
    /// array.
    fn handed(&self, m: usize, ctx: usize, e: &Expr) {
        let (flow, ty) = object_values::handed_by(self.values, self.class, m, ctx, e);
        self.glue(flow, ty);
    }

    /// The runtime may call the methods of `runtime_called` on the objects that `flow`, of type
    /// `ty`, holds.
    fn glue(&self, flow: Flow, ty: Type) {
        let classes: Vec<usize> =
            if flow.any { self.values.concretes_of(ty).to_vec() } else { flow.classes.iter().collect() };
        for c in classes {
            for &g in &self.glue {
                if let Some(target) = self.values.implementation(self.class, g, c) {
                    self.reach(target, c);
                }
            }
        }
    }
}
