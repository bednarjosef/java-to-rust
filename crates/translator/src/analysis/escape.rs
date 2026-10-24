//! Escape analysis: the `new`s whose objects never leave the method that creates them, so that
//! the object can live in a slot on that method's stack instead of the collected heap
//! (`java::Slot`, docs/collector.md, "Objects on the stack").
//!
//! An object escapes its method when it is stored into a field, an array element or a static,
//! returned, thrown, captured by a lambda or a local class (whose constructor stores it into a
//! field), handed to the runtime (a builtin, a string concatenation, which calls its
//! `toString`), or passed to a call whose callee lets that parameter escape, or to a call that
//! is not known. Anything the analysis does not model escapes.
//!
//! Each method gets a summary of its parameters, `this` first: whether each may escape, and
//! whether each may be what the method returns (`ListBuffer.append` returns `this`), so that a
//! call's result is an alias of those arguments in the caller. The summaries start empty and grow
//! until nothing changes, so recursion is covered. A virtual call takes the union over every
//! implementation it may run (`Class::callees`); after `live_methods`, those that cannot run
//! have bodies that never run, which let nothing escape. An abstract method has no body: when
//! no implementation of it exists, the call cannot run.
//!
//! Within a method the analysis is flow-insensitive: each variable (by name) holds the set of
//! parameters and `new`s that any assignment in the method gives it, and every use of the
//! variable is a use of all of them.
//!
//! **Cost.** Each method's code is walked once and reduced to its flows (`Compiled`): numbered
//! variables (the results of calls among them), what each assignment gives each, its calls with
//! their arguments, and what escapes or is returned. Solving a method again when a callee's
//! summary changes then only unions bitsets, and a call's targets are one shared list whose
//! combined summary is kept until one of them changes (a call on an `Object` may run thousands of
//! methods). On javac the analysis takes about half a second.
//!
//! **Loops.** The translation gives each `new` one slot, which a `new` in a loop reuses on every
//! iteration. That is sound only if the object of the previous iteration can no longer be reached
//! when the next one is made. It is required that every variable that may hold the object is
//! declared inside the body of the innermost loop around the `new` (a variable declared there is
//! a new one on each iteration, and Java does not let the next iteration read it before it
//! assigns it), and since the object is never stored anywhere else, nothing else holds it.
//!
//! **Factories.** A method that returns a new object of its own, which nothing else in it lets
//! escape (`List.iterator()`), is a factory. It makes that object at most once per call: the
//! `new` (or the call of another factory that makes it) is outside every loop, and constructors
//! are not factories. A call whose only target is a factory is then a source in its caller, like
//! a `new`, and the same rules decide whether its object stays there; when it does, the caller
//! gives the factory a slot of its own to make the object in. A factory may also return other
//! values (`emptyIterator()`): then the slot is left empty, and the caller's rules hold for the
//! other value too, which it treats as its own object. The set of factories grows with the
//! summaries known, a few rounds per method, and is then cut down until every method in it is
//! still a factory with the factories that remain.

use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

/// What the analysis found (see the module's comment).
#[derive(Default)]
pub struct StackObjects {
    /// The `new`s and the calls of factories (by expression id) whose objects never leave the
    /// method whose body holds them and, in a loop, are no longer reachable when the next
    /// iteration makes the next one.
    pub objects: HashSet<ExprId>,
    /// The method that each call of `objects` runs, a factory.
    pub calls: HashMap<ExprId, usize>,
    /// The factories: the methods that return a new object that does not otherwise leave them.
    pub factories: HashMap<usize, Product>,
}

/// The object that a factory returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Product {
    /// The expression that makes it: a `new`, or a call of another factory.
    pub made: ExprId,
    /// Its class.
    pub class: usize,
    /// The factory that the call runs, if it is made by one.
    pub call: Option<usize>,
}

/// The objects that may live on the stack (see the module's comment).
pub fn stack_objects(class: &Class) -> StackObjects {
    let n = class.methods.len();
    let mut lists = Lists::default();
    let field_inits = field_inits(class);
    let code: Vec<Compiled> = (0..n).map(|m| compile(class, m, &field_inits, &mut lists)).collect();
    lists.combined = vec![None; lists.targets.len()];
    let mut analysis = Analysis {
        code,
        lists,
        summaries: class.methods.iter().map(|m| Summary::empty(arity(m))).collect(),
        factories: FastMap::default(),
    };
    // Who calls each method, to solve them again when its summary or its being a factory changes.
    let mut callers: Vec<Vec<usize>> = vec![Vec::new(); n];
    for m in 0..n {
        let mut callees: Vec<usize> = Vec::new();
        for call in &analysis.code[m].calls {
            if let Some(l) = call.targets {
                callees.extend(analysis.lists.targets[l as usize].iter().copied());
            }
        }
        callees.sort_unstable();
        callees.dedup();
        for t in callees {
            callers[t].push(m);
        }
    }
    let mut queued = vec![true; n];
    let mut work: Vec<usize> = (0..n).rev().collect();
    while let Some(m) = work.pop() {
        queued[m] = false;
        let summary = analysis.solve(m).summary;
        if summary != analysis.summaries[m] {
            analysis.set_summary(m, summary);
            for &caller in &callers[m] {
                if !std::mem::replace(&mut queued[caller], true) {
                    work.push(caller);
                }
            }
        }
    }
    // The factories, with the summaries known: a method is one when what it returns of its own
    // objects is one object, made by a `new` or a call of another factory. Adding a factory can
    // take one away (a method that returned one new object and one factory's), so the set is
    // grown a few rounds and then cut down until every method in it is one.
    let mut queued = vec![true; n];
    let mut work: Vec<usize> = (0..n).rev().collect();
    let mut visits = vec![0u8; n];
    while let Some(m) = work.pop() {
        queued[m] = false;
        visits[m] = visits[m].saturating_add(1);
        // A method visited too often is no factory from then on, so the rounds end.
        let product = analysis.solve(m).product.filter(|_| visits[m] < 4);
        if product != analysis.factories.get(&m).copied() {
            match product {
                Some(p) => _ = analysis.factories.insert(m, p),
                None => _ = analysis.factories.remove(&m),
            }
            for &caller in &callers[m] {
                if !std::mem::replace(&mut queued[caller], true) {
                    work.push(caller);
                }
            }
        }
    }
    loop {
        let mut wrong: Vec<usize> = Vec::new();
        let factories: Vec<(usize, Product)> = analysis.factories.iter().map(|(&m, &p)| (m, p)).collect();
        for (m, p) in factories {
            if analysis.solve(m).product != Some(p) {
                wrong.push(m);
            }
        }
        if wrong.is_empty() {
            break;
        }
        for m in wrong {
            analysis.factories.remove(&m);
        }
    }
    let mut found = StackObjects::default();
    for m in 0..n {
        for (id, call) in analysis.solve(m).staying {
            found.objects.insert(id);
            if let Some(t) = call {
                found.calls.insert(id, t);
            }
        }
    }
    found.factories = analysis.factories.into_iter().collect();
    found
}

/// A hash of indices, which need no protection from chosen keys.
#[derive(Default)]
struct IndexHasher(u64);

impl Hasher for IndexHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_usize(b as usize);
        }
    }

    fn write_usize(&mut self, n: usize) {
        self.0 = (self.0.rotate_left(5) ^ n as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<IndexHasher>>;

/// The parameters of a method, `this` first for an instance method or a constructor.
fn arity(m: &Method) -> usize {
    m.params.len() + usize::from(!m.is_static)
}

/// What a method does with its parameters, `this` first.
#[derive(Clone, PartialEq, Eq)]
struct Summary {
    escapes: Vec<bool>,
    returned: Vec<bool>,
}

impl Summary {
    fn empty(arity: usize) -> Summary {
        Summary { escapes: vec![false; arity], returned: vec![false; arity] }
    }
}

/// What one analysis of a method finds with the summaries and factories known so far.
struct MethodFacts {
    summary: Summary,
    /// The `new`s and calls of factories whose objects stay in the method, with the factory that
    /// each call runs.
    staying: Vec<(ExprId, Option<usize>)>,
    /// What makes the one object of its own that the method returns, and the object's class,
    /// when the method is a factory.
    product: Option<Product>,
}

/// A set of the sources of one method's values: its parameters, then its `new`s and its calls
/// that may run a factory.
#[derive(Clone, Default, PartialEq, Eq)]
struct Sources(Vec<u64>);

impl Sources {
    fn insert(&mut self, i: usize) {
        if self.0.len() <= i / 64 {
            self.0.resize(i / 64 + 1, 0);
        }
        self.0[i / 64] |= 1 << (i % 64);
    }

    fn contains(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }

    /// Adds `other`; whether that added anything.
    fn union(&mut self, other: &Sources) -> bool {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        let mut grew = false;
        for (w, &o) in self.0.iter_mut().zip(&other.0) {
            grew |= o & !*w != 0;
            *w |= o;
        }
        grew
    }
}

/// The lists of methods that the program's calls may run, each kept once, and what each list's
/// methods together do with their parameters.
#[derive(Default)]
struct Lists {
    targets: Vec<Rc<[usize]>>,
    /// The list of each call, by resolved method and receiver class (`usize::MAX` for a call of
    /// one method).
    index: FastMap<(usize, usize), u32>,
    /// Whether every method of each list takes the same parameters: their number and whether
    /// they are static; `None` when they differ, and `Some(None)` for an empty list.
    shapes: Vec<Option<Option<(usize, bool)>>>,
    /// The lists that each method is in.
    lists_of: FastMap<usize, Vec<u32>>,
    /// The union of the summaries of each list's methods, when it is known.
    combined: Vec<Option<Rc<Summary>>>,
}

impl Lists {
    fn intern(&mut self, class: &Class, key: (usize, usize), make: impl FnOnce() -> Vec<usize>) -> u32 {
        if let Some(&l) = self.index.get(&key) {
            return l;
        }
        let targets = make();
        let l = self.targets.len() as u32;
        let mut shape = Some(None);
        for &t in &targets {
            let m = &class.methods[t];
            let this = Some((arity(m), m.is_static));
            shape = match shape {
                Some(None) => Some(this),
                Some(s) if s == this => Some(s),
                _ => None,
            };
            self.lists_of.entry(t).or_default().push(l);
        }
        self.shapes.push(shape);
        self.targets.push(targets.into());
        self.index.insert(key, l);
        l
    }

    /// What the methods of list `l` together do with their parameters.
    fn combined(&mut self, l: u32, summaries: &[Summary]) -> Rc<Summary> {
        if let Some(s) = &self.combined[l as usize] {
            return s.clone();
        }
        let mut all = Summary::empty(0);
        for &t in self.targets[l as usize].iter() {
            let s = &summaries[t];
            if all.escapes.len() < s.escapes.len() {
                all.escapes.resize(s.escapes.len(), false);
                all.returned.resize(s.returned.len(), false);
            }
            for k in 0..s.escapes.len() {
                all.escapes[k] |= s.escapes[k];
                all.returned[k] |= s.returned[k];
            }
        }
        let all = Rc::new(all);
        self.combined[l as usize] = Some(all.clone());
        all
    }
}

/// A value of a method's code: the sources that some variables hold, and some sources.
#[derive(Default)]
struct Value {
    vars: Vec<u32>,
    sources: Vec<u32>,
}

impl Value {
    fn is_empty(&self) -> bool {
        self.vars.is_empty() && self.sources.is_empty()
    }
}

/// A call (or `new`, `this(..)` or `super(..)`) of a method's code.
struct Call {
    /// The methods it may run, a list of `Lists`, or `None` when it is not a call of the
    /// program's code.
    targets: Option<u32>,
    /// Whether the arguments fit the parameters of every target, so that the call is followed.
    fits: bool,
    /// Its arguments, receiver first, when it is followed.
    args: Vec<Value>,
    /// What escapes when it is not followed: the receiver, `this` for a call on it, the call's
    /// own source and every argument.
    unknown: Value,
    /// The variable that holds its result.
    result: u32,
    /// Its own source: a `new`'s, or that of a call of one method, which is one when that method
    /// is a factory.
    site: Option<u32>,
}

/// A source of a method's own: a `new`, or a call of one method that may be a factory.
struct Site {
    id: ExprId,
    /// The class of a `new`'s object.
    class: usize,
    /// The method a call runs.
    call: Option<usize>,
    /// The loops around it, innermost last; `None` when it is not found in the body.
    loops: Option<Vec<usize>>,
}

/// A variable named in a method's code.
struct Named {
    /// Its variable.
    var: u32,
    /// Which parameter it is, if it is one (`this` not counted).
    param: Option<usize>,
    /// The loops around every declaration of it; `None` when it has none.
    loops: Option<Vec<usize>>,
}

/// A method's code, reduced once to the flows the analysis follows: the variables (named ones
/// first, then the results of calls), what each assignment gives each, the calls, and what
/// escapes and is returned. Solving it with the summaries known so far walks no expression.
#[derive(Default)]
struct Compiled {
    is_abstract: bool,
    is_ctor: bool,
    arity: usize,
    has_this: bool,
    named: Vec<Named>,
    vars: usize,
    assigns: Vec<(u32, Value)>,
    calls: Vec<Call>,
    escapes: Vec<Value>,
    returns: Vec<Value>,
    sites: Vec<Site>,
}

fn field_inits(class: &Class) -> Vec<Vec<&Expr>> {
    class.types.iter().map(|t| t.fields.iter().filter_map(|f| f.init.as_ref()).collect()).collect()
}

/// The state of compiling one method.
struct Compiling<'a, 'l> {
    class: &'a Class,
    lists: &'l mut Lists,
    code: Compiled,
    names: FastMap<&'a str, u32>,
    /// The names of `code.named`, in order.
    order: Vec<&'a str>,
    site_of: FastMap<ExprId, u32>,
    call_of: FastMap<ExprId, u32>,
}

/// Reduces method `m` to its flows (`Compiled`).
fn compile(class: &Class, m: usize, field_inits: &[Vec<&Expr>], lists: &mut Lists) -> Compiled {
    let method = &class.methods[m];
    if method.is_abstract {
        return Compiled { is_abstract: true, arity: arity(method), ..Compiled::default() };
    }
    let has_this = !method.is_static;
    let code = Compiled { is_ctor: method.is_ctor, arity: arity(method), has_this, ..Compiled::default() };
    let mut cx = Compiling {
        class,
        lists,
        code,
        names: FastMap::default(),
        order: Vec::new(),
        site_of: FastMap::default(),
        call_of: FastMap::default(),
    };
    for (i, p) in method.params.iter().enumerate() {
        cx.var(&p.name);
        cx.code.named[i].param = Some(i);
    }
    // The sources: the `new`s of the body, and its calls of one method.
    let mut made: Vec<&Expr> = Vec::new();
    for s in &method.body {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if matches!(
                        x.kind,
                        ExprKind::New { ctor: Some(_), .. }
                            | ExprKind::Call { .. }
                            | ExprKind::MethodCall { .. }
                    ) {
                        made.push(x)
                    }
                })
            },
            &mut |_| {},
        );
    }
    for x in made {
        let site = match x.kind {
            ExprKind::New { class: c, .. } => Some((c, None)),
            _ => match cx.targets(x) {
                Some(l) => match cx.lists.targets[l as usize][..] {
                    [t] => Some((0, Some(t))),
                    _ => None,
                },
                None => None,
            },
        };
        if let Some((c, call)) = site
            && !cx.site_of.contains_key(&x.id)
        {
            cx.site_of.insert(x.id, cx.code.sites.len() as u32);
            cx.code.sites.push(Site { id: x.id, class: c, call, loops: None });
        }
    }
    // The expressions the method runs: its body's, and for a constructor that does not delegate,
    // its class's field initializers.
    let mut exprs: Vec<&Expr> = Vec::new();
    for s in &method.body {
        visit_stmt(s, &mut |e| visit_expr(e, &mut |x| exprs.push(x)), &mut |_| {});
    }
    if method.is_ctor && !delegates(&method.body) {
        for init in &field_inits[method.class] {
            visit_expr(init, &mut |x| exprs.push(x));
        }
    }
    for x in exprs {
        match &x.kind {
            ExprKind::Assign { op: None, target: Place::Var(name), value }
            | ExprKind::InstanceOf { value, binding: Some(name), .. } => {
                let v = cx.var(name);
                let value = cx.value(value);
                cx.code.assigns.push((v, value));
            }
            _ => {}
        }
        let mut escaping = Value::default();
        match &x.kind {
            ExprKind::Assign {
                target: Place::Field { .. } | Place::Static { .. } | Place::Index(..),
                value,
                ..
            }
            | ExprKind::Assign { op: Some(_), value, .. } => cx.add(value, &mut escaping),
            ExprKind::ArrayLit(items) => items.iter().for_each(|i| cx.add(i, &mut escaping)),
            // A concatenation calls the `toString` of the objects it is given.
            ExprKind::Binary(_, l, r) if x.ty == Type::String => {
                cx.add(l, &mut escaping);
                cx.add(r, &mut escaping);
            }
            ExprKind::Builtin(builtin, args) if !only_looks(builtin) => {
                args.iter().for_each(|a| cx.add(a, &mut escaping))
            }
            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. } => _ = cx.call(x),
            _ => {}
        }
        if !escaping.is_empty() {
            cx.code.escapes.push(escaping);
        }
    }
    let mut stmts: Vec<&Stmt> = Vec::new();
    for s in &method.body {
        visit_stmt(s, &mut |_| {}, &mut |s| stmts.push(s));
    }
    for s in stmts {
        match s {
            Stmt::Local { name, init: Some(init), .. } => {
                let v = cx.var(name);
                let value = cx.value(init);
                cx.code.assigns.push((v, value));
            }
            Stmt::Return(Some(e), _) => {
                let value = cx.value(e);
                cx.code.returns.push(value);
            }
            Stmt::Throw(e) => {
                let value = cx.value(e);
                cx.code.escapes.push(value);
            }
            _ => {}
        }
    }
    // Where the sources and the declarations are, in the loops.
    let scopes = Scopes::of(method, &cx.site_of);
    for site in &mut cx.code.sites {
        site.loops = scopes.sites.get(&site.id).cloned();
    }
    for (named, name) in cx.code.named.iter_mut().zip(&cx.order) {
        named.loops = scopes.declarations.get(name).map(|ds| {
            let mut all = ds.first().cloned().unwrap_or_default();
            for d in &ds[1..] {
                all.retain(|l| d.contains(l));
            }
            all
        });
    }
    cx.code
}

impl<'a> Compiling<'a, '_> {
    /// The variable `name`.
    fn var(&mut self, name: &'a str) -> u32 {
        if let Some(&v) = self.names.get(name) {
            return v;
        }
        let v = self.new_var();
        self.names.insert(name, v);
        self.code.named.push(Named { var: v, param: None, loops: None });
        self.order.push(name);
        v
    }

    fn new_var(&mut self) -> u32 {
        self.code.vars += 1;
        (self.code.vars - 1) as u32
    }

    /// The list of methods that the call `x` may run, or `None` when `x` is not a call of the
    /// program's code. An empty list for a call of a method that has no implementation.
    fn targets(&mut self, x: &Expr) -> Option<u32> {
        let class = self.class;
        let (r, t) = match &x.kind {
            ExprKind::MethodCall { receiver, resolved: Some(r), is_super, .. } => {
                match class.class_index(receiver.ty) {
                    Some(t) if !*is_super && !class.methods[*r].is_static => (*r, t),
                    _ => (*r, usize::MAX),
                }
            }
            ExprKind::Call { resolved: Some(r), .. }
            | ExprKind::New { ctor: Some(r), .. }
            | ExprKind::ThisCall { resolved: Some(r), .. }
            | ExprKind::SuperInit { resolved: Some(r), .. } => (*r, usize::MAX),
            _ => return None,
        };
        Some(
            self.lists.intern(class, (r, t), || {
                if t == usize::MAX { vec![r] } else { class.hierarchy.targets(r, t) }
            }),
        )
    }

    /// The value of `e`.
    fn value(&mut self, e: &'a Expr) -> Value {
        let mut out = Value::default();
        self.add(e, &mut out);
        out
    }

    /// Adds to `out` the sources whose objects `e` may be.
    fn add(&mut self, e: &'a Expr, out: &mut Value) {
        match &e.kind {
            ExprKind::Var(name) => {
                let v = self.var(name);
                out.vars.push(v);
            }
            ExprKind::This if self.code.has_this => out.sources.push(0),
            ExprKind::Cast(_, value) | ExprKind::Assign { op: None, value, .. } => self.add(value, out),
            ExprKind::Conditional(_, a, b) => {
                self.add(a, out);
                self.add(b, out);
            }
            ExprKind::Builtin(Builtin::NullChecked, args) if !args.is_empty() => {
                self.add(&args[args.len() - 1], out)
            }
            ExprKind::Switch { cases, .. } => {
                for stmt in cases.iter().flat_map(|c| &c.body) {
                    let mut yields = Vec::new();
                    visit_stmt_outside_switches(stmt, &mut |s| {
                        if let Stmt::Yield(e) = s {
                            yields.push(e);
                        }
                    });
                    for e in yields {
                        self.add(e, out);
                    }
                }
            }
            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. } => {
                let c = self.call(e);
                out.vars.push(self.code.calls[c as usize].result);
            }
            _ => {}
        }
    }

    /// The call `x`, compiled once.
    fn call(&mut self, x: &'a Expr) -> u32 {
        if let Some(&c) = self.call_of.get(&x.id) {
            return c;
        }
        let class = self.class;
        let targets = self.targets(x);
        let site = self.site_of.get(&x.id).copied();
        let own = |site: Option<u32>, arity: usize| Value {
            vars: Vec::new(),
            sources: site.map(|s| arity as u32 + s).into_iter().collect(),
        };
        let this = if self.code.has_this { vec![0] } else { Vec::new() };
        let (receiver, args): (Option<Value>, &[Expr]) = match &x.kind {
            ExprKind::MethodCall { receiver, args, resolved: Some(r), .. } => {
                (if class.methods[*r].is_static { None } else { Some(self.value(receiver)) }, args)
            }
            ExprKind::MethodCall { args, .. } => (None, args),
            ExprKind::Call { args, resolved: Some(r), .. } if class.methods[*r].is_static => (None, args),
            ExprKind::Call { args, .. } => (None, args),
            ExprKind::New { args, ctor: Some(_), .. } => (Some(own(site, self.code.arity)), args),
            ExprKind::New { args, .. } => (None, args),
            ExprKind::ThisCall { args, .. } | ExprKind::SuperInit { args, .. } => {
                (Some(Value { vars: Vec::new(), sources: this.clone() }), args)
            }
            _ => unreachable!("a call"),
        };
        let followed = match &x.kind {
            ExprKind::MethodCall { resolved: Some(_), .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. } => true,
            ExprKind::Call { resolved: Some(r), .. } => class.methods[*r].is_static,
            _ => false,
        };
        let start = usize::from(receiver.is_some());
        let mut all: Vec<Value> = receiver.into_iter().collect();
        for a in args {
            let value = self.value(a);
            all.push(value);
        }
        let fits = followed
            && targets.is_some_and(|l| match self.lists.shapes[l as usize] {
                Some(None) => true,
                Some(Some((n, is_static))) => n == all.len() && is_static == (start == 0),
                None => false,
            });
        let mut unknown = Value::default();
        if !fits {
            // A call the analysis does not follow lets everything it is given escape.
            if let ExprKind::MethodCall { receiver, .. } = &x.kind {
                self.add(receiver, &mut unknown);
            }
            if matches!(
                x.kind,
                ExprKind::Call { .. } | ExprKind::ThisCall { .. } | ExprKind::SuperInit { .. }
            ) {
                unknown.sources.extend(&this);
            }
            unknown.sources.extend(own(site, self.code.arity).sources);
            for a in args {
                self.add(a, &mut unknown);
            }
            all.clear();
        }
        let result = self.new_var();
        let c = self.code.calls.len() as u32;
        self.code.calls.push(Call { targets, fits, args: all, unknown, result, site });
        self.call_of.insert(x.id, c);
        c
    }
}

struct Analysis {
    code: Vec<Compiled>,
    lists: Lists,
    summaries: Vec<Summary>,
    /// The factories known so far (see `StackObjects::factories`).
    factories: FastMap<usize, Product>,
}

impl Analysis {
    fn set_summary(&mut self, m: usize, summary: Summary) {
        self.summaries[m] = summary;
        if let Some(ls) = self.lists.lists_of.get(&m) {
            for &l in ls {
                self.lists.combined[l as usize] = None;
            }
        }
    }

    /// Whether site `s` of method `m` is a source: a `new`, or a call of a factory.
    fn active(&self, site: &Site) -> bool {
        site.call.is_none_or(|t| self.factories.contains_key(&t))
    }

    /// Solves method `m` with the summaries and factories known so far: its summary, the
    /// objects that stay in it, and what it makes and returns.
    fn solve(&mut self, m: usize) -> MethodFacts {
        let code = &self.code[m];
        if code.is_abstract {
            return MethodFacts { summary: Summary::empty(code.arity), staying: Vec::new(), product: None };
        }
        let combined: Vec<Option<Rc<Summary>>> = code
            .calls
            .iter()
            .map(|c| {
                c.fits.then(|| self.lists.combined(c.targets.expect("a followed call"), &self.summaries))
            })
            .collect();
        let active: Vec<bool> = code.sites.iter().map(|s| self.active(s)).collect();
        let first = code.arity;
        let mut vars: Vec<Sources> = vec![Sources::default(); code.vars];
        for named in &code.named {
            if let Some(i) = named.param {
                vars[named.var as usize].insert(i + usize::from(code.has_this));
            }
        }
        let eval = |vars: &[Sources], value: &Value| {
            let mut s = Sources::default();
            for &v in &value.vars {
                s.union(&vars[v as usize]);
            }
            for &i in &value.sources {
                s.insert(i as usize);
            }
            s
        };
        // The variables: what each assignment and call gives each, until nothing grows.
        loop {
            let mut grew = false;
            for (v, value) in &code.assigns {
                let s = eval(&vars, value);
                grew |= vars[*v as usize].union(&s);
            }
            for (call, combined) in code.calls.iter().zip(&combined) {
                let mut s = Sources::default();
                if let Some(site) = call.site
                    && active[site as usize]
                {
                    s.insert(first + site as usize);
                }
                if let Some(combined) = combined {
                    for (k, arg) in call.args.iter().enumerate() {
                        if combined.returned.get(k) == Some(&true) {
                            s.union(&eval(&vars, arg));
                        }
                    }
                }
                grew |= vars[call.result as usize].union(&s);
            }
            if !grew {
                break;
            }
        }
        // What escapes, and what is returned.
        let mut escaping = Sources::default();
        let mut returned = Sources::default();
        for value in &code.escapes {
            escaping.union(&eval(&vars, value));
        }
        for value in &code.returns {
            returned.union(&eval(&vars, value));
        }
        for (call, combined) in code.calls.iter().zip(&combined) {
            match combined {
                Some(combined) => {
                    for (k, arg) in call.args.iter().enumerate() {
                        if combined.escapes.get(k) == Some(&true) {
                            escaping.union(&eval(&vars, arg));
                        }
                    }
                }
                None => _ = escaping.union(&eval(&vars, &call.unknown)),
            }
        }
        let mut summary = Summary::empty(code.arity);
        for i in 0..code.arity {
            summary.escapes[i] = escaping.contains(i);
            summary.returned[i] = returned.contains(i);
        }
        let mut staying = Vec::new();
        let mut returned_sites = Vec::new();
        for (i, site) in code.sites.iter().enumerate() {
            let s = first + i;
            if !active[i] {
                continue;
            }
            if returned.contains(s) {
                returned_sites.push((i, s));
            } else if !escaping.contains(s) && dead_by_next_iteration(code, &vars, site, s) {
                staying.push((site.id, site.call));
            }
        }
        let mut product = None;
        // A factory makes the object it returns once per call, outside any loop, and lets it
        // leave only by being returned.
        if let [(i, s)] = returned_sites[..]
            && !escaping.contains(s)
            && code.sites[i].loops.as_ref().is_some_and(|loops| loops.is_empty())
            && !code.is_ctor
        {
            let site = &code.sites[i];
            let class = match site.call {
                Some(t) => self.factories[&t].class,
                None => site.class,
            };
            product = Some(Product { made: site.id, class, call: site.call });
        }
        MethodFacts { summary, staying, product }
    }
}

/// Whether the object of `site` (source `s`), if it is in a loop, can no longer be reached when
/// the next iteration makes the next one: every variable that may hold it is declared inside the
/// body of the innermost loop around it. (The results of calls are not kept across iterations.)
fn dead_by_next_iteration(code: &Compiled, vars: &[Sources], site: &Site, s: usize) -> bool {
    let Some(&innermost) = site.loops.as_ref().and_then(|loops| loops.last()) else { return true };
    code.named.iter().filter(|named| vars[named.var as usize].contains(s)).all(|named| {
        named.param.is_none() && named.loops.as_ref().is_some_and(|loops| loops.contains(&innermost))
    })
}

/// Whether `builtin` only looks at the objects it is given, keeping none: their class, their
/// identity hash, whether they are null.
fn only_looks(builtin: &Builtin) -> bool {
    matches!(
        builtin,
        Builtin::GetClass
            | Builtin::ClassName { .. }
            | Builtin::SameClass { .. }
            | Builtin::NullChecked
            | Builtin::Native(Native::IdentityHash | Native::SystemIdentityHash)
    )
}

/// Where a method's `new`s and variable declarations are, in its loops.
struct Scopes<'m> {
    /// How many loops have been numbered.
    count: usize,
    /// The `new`s and calls of factories to place.
    made: &'m FastMap<ExprId, u32>,
    /// The loops around each `new` and call of a factory, outermost first, as numbers of the
    /// loops.
    sites: HashMap<ExprId, Vec<usize>>,
    /// For each variable, the loops around each of its declarations.
    declarations: HashMap<&'m str, Vec<Vec<usize>>>,
}

impl<'m> Scopes<'m> {
    fn of(method: &'m Method, made: &'m FastMap<ExprId, u32>) -> Scopes<'m> {
        let mut scopes = Scopes { count: 0, made, sites: HashMap::new(), declarations: HashMap::new() };
        for p in &method.params {
            scopes.declarations.entry(p.name.as_str()).or_default().push(Vec::new());
        }
        for s in &method.body {
            scopes.stmt(s, &mut Vec::new());
        }
        scopes
    }

    fn declare(&mut self, name: &'m str, loops: &[usize]) {
        self.declarations.entry(name).or_default().push(loops.to_vec());
    }

    /// Notes the `new`s and declarations of the statement `s`, inside the loops `loops`.
    fn stmt(&mut self, s: &'m Stmt, loops: &mut Vec<usize>) {
        let in_loop =
            |scopes: &mut Scopes<'m>, loops: &mut Vec<usize>, body: &'m Stmt, exprs: &[&'m Expr]| {
                scopes.count += 1;
                loops.push(scopes.count);
                // A pattern variable of the condition may stay in scope after the loop: it is
                // declared outside it, while a `new` there runs on every iteration.
                for e in exprs {
                    scopes.expr(e, loops, Some(loops.len() - 1));
                }
                scopes.stmt(body, loops);
                loops.pop();
            };
        match s {
            Stmt::Local { name, init, .. } => {
                self.declare(name, loops);
                if let Some(e) = init {
                    self.expr(e, loops, None);
                }
            }
            Stmt::Expr(e) | Stmt::Return(Some(e), _) | Stmt::Yield(e) | Stmt::Throw(e) => {
                self.expr(e, loops, None)
            }
            Stmt::If { cond, then, otherwise } => {
                self.expr(cond, loops, None);
                self.stmt(then, loops);
                if let Some(o) = otherwise {
                    self.stmt(o, loops);
                }
            }
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => in_loop(self, loops, body, &[cond]),
            Stmt::For { init, cond, update, body } => {
                for s in init {
                    self.stmt(s, loops);
                }
                let exprs: Vec<&Expr> = cond.iter().chain(update).collect();
                in_loop(self, loops, body, &exprs);
            }
            Stmt::ForEach { name, array, body, .. } => {
                self.expr(array, loops, None);
                self.count += 1;
                loops.push(self.count);
                self.declare(name, loops);
                self.stmt(body, loops);
                loops.pop();
            }
            Stmt::Block(stmts) => stmts.iter().for_each(|s| self.stmt(s, loops)),
            Stmt::Switch { selector, cases } => {
                self.expr(selector, loops, None);
                for case in cases {
                    for label in &case.labels {
                        if let CaseLabel::Value(e) = label {
                            self.expr(e, loops, None);
                        }
                    }
                    case.body.iter().for_each(|s| self.stmt(s, loops));
                }
            }
            Stmt::Labeled { body, .. } => self.stmt(body, loops),
            Stmt::Try { body, catches, finally } => {
                body.iter().for_each(|s| self.stmt(s, loops));
                for catch in catches {
                    self.declare(&catch.name, loops);
                    catch.body.iter().for_each(|s| self.stmt(s, loops));
                }
                finally.iter().flatten().for_each(|s| self.stmt(s, loops));
            }
            Stmt::Return(None, _)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel(_)
            | Stmt::ContinueLabel(_) => {}
        }
    }

    /// Notes the `new`s and pattern variables of `e`, inside the loops `loops`; pattern variables
    /// inside only the first `bindings` of them when that is given. The statements of a switch
    /// expression are walked again with their own loops, and a `new` keeps the most loops it is
    /// found inside.
    fn expr(&mut self, e: &'m Expr, loops: &mut Vec<usize>, bindings: Option<usize>) {
        let mut switches = Vec::new();
        visit_expr(e, &mut |x| match &x.kind {
            ExprKind::New { .. } | ExprKind::Call { .. } | ExprKind::MethodCall { .. }
                if self.made.contains_key(&x.id) =>
            {
                let at = self.sites.entry(x.id).or_default();
                if at.len() <= loops.len() {
                    *at = loops.clone();
                }
            }
            ExprKind::InstanceOf { binding: Some(name), .. } => {
                let around = &loops[..bindings.unwrap_or(loops.len())];
                self.declarations.entry(name.as_str()).or_default().push(around.to_vec());
            }
            ExprKind::Switch { cases, .. } => switches.push(cases),
            _ => {}
        });
        for cases in switches {
            for s in cases.iter().flat_map(|c| &c.body) {
                self.stmt(s, loops);
            }
        }
    }
}
