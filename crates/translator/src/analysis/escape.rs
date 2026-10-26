//! Escape analysis: the `new`s whose objects never leave the method that creates them, so that
//! the object can live in a slot on that method's stack instead of the collected heap
//! (`java::Slot`, docs/collector.md, "Objects on the stack").
//!
//! An object escapes its method when it is stored into a field (but see **Contents**), an array
//! element or a static, returned, thrown, handed to the runtime (a builtin, a string
//! concatenation, which calls its `toString`), or passed to a call whose callee lets that
//! parameter escape, or to a call that is not known. Anything the analysis does not model
//! escapes.
//!
//! Each method gets a summary of its parameters, `this` first: whether each may escape, and
//! whether each may be what the method returns (`ListBuffer.append` returns `this`), so that a
//! call's result is an alias of those arguments in the caller. The summaries start empty and grow
//! until nothing changes, so recursion is covered. A virtual call takes the union over every
//! implementation it may run: the implementations for the classes whose objects its receiver may
//! hold, when the whole program is known (`Class::receivers`, from `live_methods`), or else
//! every override (`Class::callees`); after `live_methods`, those that cannot run have bodies
//! that never run, which let nothing escape. An abstract method has no body: when no
//! implementation of it exists, the call cannot run.
//!
//! **Receivers.** Whether the receiver of a virtual call escapes depends on which class it is,
//! so a parameter's summary says it escapes either always or *via* methods: when it is the
//! receiver of a call of one of them (directly or in a callee) and what its class runs for that
//! method lets its `this` escape. A visitor or a lambda passed down to code that calls it then
//! escapes only if its own class's methods let it, not because some other class's do. At an
//! object of the method's own, whose class is known, those conditions are checked; a method
//! that is being checked already counts as not letting it escape (the others decide).
//!
//! **Contents.** A lambda or an anonymous class keeps what it captures in fields of its own, and
//! a list's iterator keeps the list. A field that the program reads only on `this` (`this.f` or
//! `f`, never `x.f`: `own_fields`) is read only by its object's own methods, so what is stored
//! in it need not escape: it is held by that object. Each method's summary says which
//! parameters it stores in which such fields of `this` (a constructor its captures), and what it
//! does with what `this` holds in each (`Summary::contents`, always or via methods, like a
//! parameter); fields are kept apart, so a lambda that leaks one capture keeps the others. In a
//! method, a load of such a field of `this` is one more source, and what a `new` stores in its
//! object, or `this(..)` and `super(..)` in `this`, is held by that object (what a method called
//! on any other object stores escapes). What an object holds escapes when it escapes or is
//! returned, or as what its class runs for the methods called on it lets it. A held object in a
//! loop is unreachable by the next iteration only if the objects holding it are made in that
//! loop and are too. A method's summary then depends on the summaries of the methods its
//! objects' classes run, which are solved first again when they change.
//!
//! `JAVA_TO_RUST_ESCAPE_WHY=name,name` writes on standard error, for every method whose name
//! contains one of the names, its summary, and for every object it makes whether it stays, and
//! if not, what holds it and the chain of calls that lets it escape.
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
//! methods). On javac the analysis takes about a second.
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

use super::FastMap;
use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// What the analysis found (see the module's comment).
#[derive(Clone, Default)]
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
    let own_fields = own_fields(class);
    let code: Vec<Compiled> =
        (0..n).map(|m| compile(class, m, &field_inits, &own_fields, &mut lists)).collect();
    lists.combined = vec![None; lists.targets.len()];
    let mut analysis = Analysis {
        hierarchy: &class.hierarchy,
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
    // Callees are solved before their callers, so that a caller is mostly solved once, with its
    // callees' summaries final: the order of the strongly connected components of the calls, each
    // after those it calls. The least fixpoint is the same in any order.
    let callees: Vec<Vec<usize>> = {
        let mut callees = vec![Vec::new(); n];
        for (t, cs) in callers.iter().enumerate() {
            for &c in cs {
                callees[c].push(t);
            }
        }
        callees
    };
    let (rank, _) = super::closure::components(&callees);
    let mut edges: HashSet<(usize, usize)> = HashSet::new();
    let mut queued = vec![true; n];
    let mut work: std::collections::BinaryHeap<std::cmp::Reverse<(usize, usize)>> =
        (0..n).map(|m| std::cmp::Reverse((rank[m], m))).collect();
    let mut solves = 0;
    // Once the summaries are final, what solving a method finds depends on the factories only
    // through those that its calls run (`active`, `site_class`): each method's last facts are
    // kept with the factories they were found with, and found again only when one changed.
    let mut kept: Vec<Option<Kept>> = (0..n).map(|_| None).collect();
    // The time each method's solves took, for --timings.
    let mut spent: Vec<(f64, u32)> = if crate::timings::enabled() { vec![(0.0, 0); n] } else { Vec::new() };
    while let Some(std::cmp::Reverse((_, m))) = work.pop() {
        queued[m] = false;
        solves += 1;
        let start = (!spent.is_empty()).then(std::time::Instant::now);
        let MethodFacts { summary, read, staying, product } = analysis.solve(m);
        kept[m] = Some(Kept { key: analysis.factory_key(m), staying, product });
        if let Some(start) = start {
            spent[m].0 += start.elapsed().as_secs_f64();
            spent[m].1 += 1;
        }
        for t in read {
            if edges.insert((t, m)) {
                callers[t].push(m);
            }
        }
        if summary != analysis.summaries[m] {
            analysis.set_summary(m, summary);
            for &caller in &callers[m] {
                if !std::mem::replace(&mut queued[caller], true) {
                    work.push(std::cmp::Reverse((rank[caller], caller)));
                }
            }
        }
    }
    if crate::timings::enabled() {
        eprintln!("timing: stack objects: {solves} solves of {n} methods");
        let mut costly: Vec<usize> = (0..n).collect();
        costly.sort_by(|&a, &b| spent[b].0.total_cmp(&spent[a].0));
        for &m in costly.iter().take(12) {
            let method = &class.methods[m];
            let code = &analysis.code[m];
            eprintln!(
                "timing: stack objects: {:.3} s in {} solves of {}.{} ({} variables, {} steps, {} objects)",
                spent[m].0,
                spent[m].1,
                class.types[method.class].name,
                method.name,
                code.vars,
                code.assigns.len() + code.calls.len(),
                code.sites.len()
            );
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
        let product = analysis.settled(m, &mut kept).product.filter(|_| visits[m] < 4);
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
            if analysis.settled(m, &mut kept).product != Some(p) {
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
        for &(id, call) in &analysis.settled(m, &mut kept).staying {
            found.objects.insert(id);
            if let Some(t) = call {
                found.calls.insert(id, t);
            }
        }
    }
    if let Ok(wanted) = std::env::var("JAVA_TO_RUST_ESCAPE_WHY") {
        analysis.explain_all(class, &wanted);
    }
    found.factories = analysis.factories.into_iter().collect();
    found
}

/// The parameters of a method, `this` first for an instance method or a constructor.
fn arity(m: &Method) -> usize {
    m.params.len() + usize::from(!m.is_static)
}

/// What a method does with its parameters, `this` first.
#[derive(Clone, PartialEq, Eq)]
struct Summary {
    escapes: Vec<Escape>,
    returned: Vec<bool>,
    /// The fields of `this` that only its own methods read (see `Contents`) that each parameter
    /// may be stored in.
    stored: Vec<Vec<FieldKey>>,
    /// What the method does with what `this` holds in each of those fields, by field.
    contents: Vec<(FieldKey, Escape)>,
}

impl Summary {
    fn empty(arity: usize) -> Summary {
        Summary {
            escapes: vec![Escape::default(); arity],
            returned: vec![false; arity],
            stored: vec![Vec::new(); arity],
            contents: Vec::new(),
        }
    }

    /// Adds what `other` does to what this does, as the summary of calls that may run either.
    fn add(&mut self, other: &Summary) {
        if self.escapes.len() < other.escapes.len() {
            self.escapes.resize(other.escapes.len(), Escape::default());
            self.returned.resize(other.returned.len(), false);
            self.stored.resize(other.stored.len(), Vec::new());
        }
        for k in 0..other.escapes.len() {
            self.escapes[k].add(&other.escapes[k]);
            self.returned[k] |= other.returned[k];
            for f in &other.stored[k] {
                if let Err(at) = self.stored[k].binary_search(f) {
                    self.stored[k].insert(at, *f);
                }
            }
        }
        add_fields(&mut self.contents, &other.contents);
    }

    /// Whether this does at least what `other` does with each parameter.
    fn covers(&self, other: &Summary) -> bool {
        let covers_fields =
            |mine: &[FieldKey], theirs: &[FieldKey]| theirs.iter().all(|f| mine.binary_search(f).is_ok());
        self.escapes.len() >= other.escapes.len()
            && (0..other.escapes.len()).all(|k| {
                self.escapes[k].covers(&other.escapes[k])
                    && (self.returned[k] || !other.returned[k])
                    && covers_fields(&self.stored[k], &other.stored[k])
            })
            && other.contents.iter().all(|(f, e)| {
                self.contents
                    .binary_search_by_key(f, |&(g, _)| g)
                    .is_ok_and(|at| self.contents[at].1.covers(e))
            })
    }
}

/// Whether a parameter escapes: always, or when the object is of a class whose implementation of
/// one of the methods `via` (called on it as the receiver) lets its `this` escape.
#[derive(Clone, Default, PartialEq, Eq)]
struct Escape {
    always: bool,
    /// Sorted.
    via: Vec<usize>,
}

/// Adds what `other` lets escape of each field to `into`, both by field.
fn add_fields(into: &mut Vec<(FieldKey, Escape)>, other: &[(FieldKey, Escape)]) {
    for (f, e) in other {
        match into.binary_search_by_key(f, |&(g, _)| g) {
            Ok(at) => into[at].1.add(e),
            Err(at) => into.insert(at, (*f, e.clone())),
        }
    }
}

impl Escape {
    fn covers(&self, other: &Escape) -> bool {
        (self.always || !other.always) && other.via.iter().all(|r| self.via.binary_search(r).is_ok())
    }

    fn add(&mut self, other: &Escape) {
        self.always |= other.always;
        if !other.via.iter().all(|r| self.via.binary_search(r).is_ok()) {
            self.via.extend(&other.via);
            self.via.sort_unstable();
            self.via.dedup();
        }
    }
}

/// What solving a method found once the summaries were final, and the factories it found it with
/// (`Analysis::factory_key`).
struct Kept {
    key: Vec<Option<Product>>,
    staying: Vec<(ExprId, Option<usize>)>,
    product: Option<Product>,
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
    /// The methods outside its calls' targets whose summaries decided it.
    read: Vec<usize>,
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

    fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }

    fn of(i: usize) -> Sources {
        let mut s = Sources::default();
        s.insert(i);
        s
    }

    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(w, &bits)| {
            let mut rest = bits;
            std::iter::from_fn(move || {
                let b = rest.trailing_zeros() as usize;
                (rest != 0).then(|| {
                    rest &= rest - 1;
                    w * 64 + b
                })
            })
        })
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
    /// one method), for a call whose receiver's classes are not known.
    index: FastMap<(usize, usize), u32>,
    /// Each list, by its methods.
    by_methods: FastMap<Rc<[usize]>, u32>,
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
        let l = self.list(class, make());
        self.index.insert(key, l);
        l
    }

    /// The list of the methods `targets`.
    fn list(&mut self, class: &Class, mut targets: Vec<usize>) -> u32 {
        targets.sort_unstable();
        targets.dedup();
        let targets: Rc<[usize]> = targets.into();
        if let Some(&l) = self.by_methods.get(&targets) {
            return l;
        }
        let l = self.targets.len() as u32;
        let mut shape = Some(None);
        for &t in targets.iter() {
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
        self.by_methods.insert(targets.clone(), l);
        self.targets.push(targets);
        l
    }

    /// What the methods of list `l` together do with their parameters.
    fn combined(&mut self, l: u32, summaries: &[Summary]) -> Rc<Summary> {
        if let Some(s) = &self.combined[l as usize] {
            return s.clone();
        }
        let mut all = Summary::empty(0);
        for &t in self.targets[l as usize].iter() {
            all.add(&summaries[t]);
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
    /// The method it names.
    method: Option<usize>,
    /// Whether it runs what the class of its receiver (its first argument) runs for `method`.
    dispatched: bool,
    /// Whether its first argument is the receiver.
    has_receiver: bool,
    /// Whether the receiver is exactly the object it names: a `new`'s own, or `this` for
    /// `this(..)` and `super(..)`, so that what the callee stores in its `this` is stored there.
    exact_receiver: bool,
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
    /// What each of `escapes` is, for the explanations.
    labels: Vec<&'static str>,
    returns: Vec<Value>,
    sites: Vec<Site>,
    /// What is stored in each field of `this` that only its own methods read (`own_fields`).
    stores: Vec<(FieldKey, Value)>,
    /// The fields of `this` that only its own methods read that the method reads: what each
    /// holds is a source, after the sites.
    held: Vec<FieldKey>,
}

/// The initializers of each class's instance fields, with the fields' places.
fn field_inits(class: &Class) -> Vec<Vec<(usize, &Expr)>> {
    class
        .types
        .iter()
        .map(|t| t.fields.iter().enumerate().filter_map(|(i, f)| Some((i, f.init.as_ref()?))).collect())
        .collect()
}

/// An instance field: the class that declares it and its place among that class's fields.
type FieldKey = (usize, usize);

/// The instance field `name` of the objects of class `t`: declared in it or a superclass.
fn field_of(class: &Class, t: usize, name: &str) -> Option<FieldKey> {
    let mut at = Some(t);
    while let Some(c) = at {
        if let Some(i) = class.types[c].fields.iter().position(|f| f.name == name) {
            return Some((c, i));
        }
        at = class.types[c].extends;
    }
    None
}

/// The instance fields that the program reads only on `this` (`this.f`, or `f` alone): what an
/// object holds in them is read only by its own methods (see `Contents`). A read on another
/// object whose class is not known counts for every field of its name.
fn own_fields(class: &Class) -> HashSet<FieldKey> {
    let mut fields: HashSet<FieldKey> = class
        .types
        .iter()
        .enumerate()
        .flat_map(|(t, decl)| (0..decl.fields.len()).map(move |i| (t, i)))
        .collect();
    let mut on_others = |x: &Expr| match &x.kind {
        ExprKind::Field { object, name }
        | ExprKind::Assign { op: Some(_), target: Place::Field { object, name }, .. }
        | ExprKind::IncDec { target: Place::Field { object, name }, .. }
            if !matches!(object.kind, ExprKind::This) =>
        {
            match class.class_index(object.ty).and_then(|t| field_of(class, t, name)) {
                Some(key) => _ = fields.remove(&key),
                None => fields.retain(|&(t, i)| class.types[t].fields[i].name != *name),
            }
        }
        _ => {}
    };
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut on_others), &mut |_| {});
        }
    }
    for t in &class.types {
        for init in t.fields.iter().chain(&t.statics).filter_map(|f| f.init.as_ref()) {
            visit_expr(init, &mut on_others);
        }
    }
    fields
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
    own_fields: &'a HashSet<FieldKey>,
    /// The method being compiled.
    method: usize,
}

/// Reduces method `m` to its flows (`Compiled`).
fn compile<'a>(
    class: &'a Class,
    m: usize,
    field_inits: &[Vec<(usize, &'a Expr)>],
    own_fields: &'a HashSet<FieldKey>,
    lists: &mut Lists,
) -> Compiled {
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
        own_fields,
        method: m,
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
        for &(i, init) in &field_inits[method.class] {
            visit_expr(init, &mut |x| exprs.push(x));
            let value = cx.value(init);
            if own_fields.contains(&(method.class, i)) {
                cx.code.stores.push(((method.class, i), value));
            } else {
                cx.code.escapes.push(value);
                cx.code.labels.push("stored in a field, a static or an array");
            }
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
        let label = match &x.kind {
            ExprKind::Assign { op: None, target: Place::Field { object, name }, value }
                if has_this
                    && matches!(object.kind, ExprKind::This)
                    && let Some(key) = field_of(class, method.class, name)
                    && own_fields.contains(&key) =>
            {
                let value = cx.value(value);
                cx.code.stores.push((key, value));
                ""
            }
            ExprKind::Assign {
                target: Place::Field { .. } | Place::Static { .. } | Place::Index(..),
                value,
                ..
            }
            | ExprKind::Assign { op: Some(_), value, .. } => {
                cx.add(value, &mut escaping);
                "stored in a field, a static or an array"
            }
            ExprKind::ArrayLit(items) => {
                items.iter().for_each(|i| cx.add(i, &mut escaping));
                "put in an array"
            }
            // A concatenation calls the `toString` of the objects it is given.
            ExprKind::Binary(_, l, r) if x.ty == Type::String => {
                cx.add(l, &mut escaping);
                cx.add(r, &mut escaping);
                "concatenated to a string"
            }
            ExprKind::Builtin(builtin, args) if !only_looks(builtin) => {
                args.iter().for_each(|a| cx.add(a, &mut escaping));
                "given to the runtime"
            }
            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::New { .. }
            | ExprKind::ThisCall { .. }
            | ExprKind::SuperInit { .. } => {
                cx.call(x);
                ""
            }
            _ => "",
        };
        if !escaping.is_empty() {
            cx.code.escapes.push(escaping);
            cx.code.labels.push(label);
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
                cx.code.labels.push("thrown");
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
                // The classes whose objects the receiver may be, where the points-to facts tell
                // (`Class::receivers`): the call runs what those classes run for it.
                if !*is_super
                    && !class.methods[*r].is_static
                    && let Some(classes) = class.receivers.get(&receiver.id)
                {
                    let targets: Option<Vec<usize>> =
                        classes.iter().map(|&c| class.hierarchy.implementation(*r, c)).collect();
                    if let Some(targets) = targets {
                        return Some(self.lists.list(class, targets));
                    }
                }
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
            ExprKind::Field { object, name }
                if self.code.has_this
                    && matches!(object.kind, ExprKind::This)
                    && let Some(key) = field_of(self.class, self.class.methods[self.method].class, name)
                    && self.own_fields.contains(&key) =>
            {
                let j = match self.code.held.iter().position(|&k| k == key) {
                    Some(j) => j,
                    None => {
                        self.code.held.push(key);
                        self.code.held.len() - 1
                    }
                };
                out.sources.push((self.code.arity + self.code.sites.len() + j) as u32)
            }
            ExprKind::Cast(_, value) | ExprKind::Assign { op: None, value, .. } => self.add(value, out),
            ExprKind::Conditional(_, a, b) => {
                self.add(a, out);
                self.add(b, out);
            }
            ExprKind::Builtin(Builtin::NullChecked(_), args) if !args.is_empty() => {
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
        let method = match &x.kind {
            ExprKind::MethodCall { resolved, .. }
            | ExprKind::Call { resolved, .. }
            | ExprKind::ThisCall { resolved, .. }
            | ExprKind::SuperInit { resolved, .. } => *resolved,
            ExprKind::New { ctor, .. } => *ctor,
            _ => None,
        };
        let dispatched = matches!(&x.kind, ExprKind::MethodCall { is_super: false, resolved: Some(r), .. }
            if !class.methods[*r].is_static);
        let exact_receiver = matches!(
            x.kind,
            ExprKind::New { ctor: Some(_), .. } | ExprKind::ThisCall { .. } | ExprKind::SuperInit { .. }
        ) && start == 1;
        self.code.calls.push(Call {
            targets,
            fits,
            args: all,
            unknown,
            result,
            site,
            method,
            dispatched,
            has_receiver: start == 1,
            exact_receiver,
        });
        self.call_of.insert(x.id, c);
        c
    }
}

struct Analysis<'a> {
    hierarchy: &'a Hierarchy,
    code: Vec<Compiled>,
    lists: Lists,
    summaries: Vec<Summary>,
    /// The factories known so far (see `StackObjects::factories`).
    factories: FastMap<usize, Product>,
}

impl Analysis<'_> {
    /// Sets the summary of method `m`. The combined summary of each list with `m` in it is kept
    /// when the new summary only adds to the old, with the new one added (what the list's
    /// methods do together is then the same as found again), and is found again otherwise: a
    /// list can have thousands of methods (the overrides of `Object.equals`), whose summaries
    /// change one at a time.
    fn set_summary(&mut self, m: usize, summary: Summary) {
        let grows = summary.covers(&self.summaries[m]);
        if let Some(ls) = self.lists.lists_of.get(&m) {
            for &l in ls {
                let combined = &mut self.lists.combined[l as usize];
                match combined {
                    Some(all) if grows => Rc::make_mut(all).add(&summary),
                    _ => *combined = None,
                }
            }
        }
        self.summaries[m] = summary;
    }

    /// The class of the object of an active site.
    fn site_class(&self, site: &Site) -> usize {
        match site.call {
            Some(t) => self.factories[&t].class,
            None => site.class,
        }
    }

    /// Whether what an object of class `c` runs for method `r` (called on it) lets its `this`
    /// escape, with the summaries known so far; `seen` holds the methods already being followed,
    /// which add nothing. The methods whose summaries it reads are added to `read`.
    fn this_escapes(&self, r: usize, c: usize, seen: &mut Vec<usize>, read: &mut Vec<usize>) -> bool {
        let Some(t) = self.hierarchy.implementation(r, c) else { return true };
        if seen.contains(&t) {
            return false;
        }
        seen.push(t);
        read.push(t);
        let Some(e) = self.summaries[t].escapes.first() else { return true };
        e.always || e.via.iter().any(|&r| self.this_escapes(r, c, seen, read))
    }

    /// What what an object of class `c` runs for method `r` (called on it) does with what the
    /// object holds in each of its own fields (see `Contents`), by field, with the summaries
    /// known so far; `None` when it lets all of it escape.
    fn contents_escape(
        &self,
        r: usize,
        c: usize,
        seen: &mut Vec<usize>,
        read: &mut Vec<usize>,
    ) -> Option<Vec<(FieldKey, Escape)>> {
        let t = self.hierarchy.implementation(r, c)?;
        if seen.contains(&t) {
            return Some(Vec::new());
        }
        seen.push(t);
        read.push(t);
        let summary = &self.summaries[t];
        let this = summary.escapes.first()?;
        if this.always {
            return None;
        }
        let mut all = summary.contents.clone();
        for &r in &this.via {
            add_fields(&mut all, &self.contents_escape(r, c, seen, read)?);
        }
        Some(all)
    }

    /// What the factories are of the methods that the calls among the sources of method `m` run,
    /// which is all that solving it reads of them.
    fn factory_key(&self, m: usize) -> Vec<Option<Product>> {
        self.code[m].sites.iter().filter_map(|s| s.call).map(|t| self.factories.get(&t).copied()).collect()
    }

    /// The facts of method `m` with the final summaries and the factories known so far: those
    /// kept, when they were found with the same factories, or else found again and kept.
    fn settled<'k>(&mut self, m: usize, kept: &'k mut [Option<Kept>]) -> &'k Kept {
        let key = self.factory_key(m);
        if kept[m].as_ref().is_none_or(|k| k.key != key) {
            let MethodFacts { staying, product, .. } = self.solve(m);
            kept[m] = Some(Kept { key, staying, product });
        }
        kept[m].as_ref().expect("kept above")
    }

    /// Whether site `s` of method `m` is a source: a `new`, or a call of a factory.
    fn active(&self, site: &Site) -> bool {
        site.call.is_none_or(|t| self.factories.contains_key(&t))
    }

    /// The sources that each variable of method `m` holds, what each of its objects and `this`
    /// hold in their own fields, what escapes and what is returned, with the summaries and
    /// factories known so far.
    fn flows(&mut self, m: usize) -> Flows {
        let code = &self.code[m];
        let combined: Vec<Option<Rc<Summary>>> = code
            .calls
            .iter()
            .map(|c| {
                c.fits.then(|| self.lists.combined(c.targets.expect("a followed call"), &self.summaries))
            })
            .collect();
        let code = &self.code[m];
        let active: Vec<bool> = code.sites.iter().map(|s| self.active(s)).collect();
        let first = code.arity;
        let sites = first..first + code.sites.len();
        // What `this` holds in each of its own fields that the method reads, as sources of its
        // own.
        let held = sites.end;
        let n = held + code.held.len();
        let mut vars: Vec<Sources> = vec![Sources::default(); code.vars];
        for named in &code.named {
            if let Some(i) = named.param {
                vars[named.var as usize].insert(i + usize::from(code.has_this));
            }
        }
        // The variables: what each assignment and call gives each, until nothing grows. They only
        // grow, by unions, so the order of the steps does not change where they end: a step is
        // taken again only when a variable it reads grew (a method of the Minecraft server has
        // thousands of them, which each pass over all of them took again).
        let assigns = code.assigns.len();
        let mut readers: Vec<Vec<u32>> = vec![Vec::new(); code.vars];
        for (i, (_, value)) in code.assigns.iter().enumerate() {
            for &v in &value.vars {
                readers[v as usize].push(i as u32);
            }
        }
        for (j, (call, combined)) in code.calls.iter().zip(&combined).enumerate() {
            let Some(combined) = combined else { continue };
            for (k, arg) in call.args.iter().enumerate() {
                if combined.returned.get(k) == Some(&true) {
                    for &v in &arg.vars {
                        readers[v as usize].push((assigns + j) as u32);
                    }
                }
            }
        }
        let steps = assigns + code.calls.len();
        let mut queued = vec![true; steps];
        let mut queue: std::collections::VecDeque<u32> = (0..steps as u32).collect();
        while let Some(i) = queue.pop_front() {
            let i = i as usize;
            queued[i] = false;
            let (target, s) = if i < assigns {
                let (v, value) = &code.assigns[i];
                (*v, eval(&vars, value))
            } else {
                let (call, combined) = (&code.calls[i - assigns], &combined[i - assigns]);
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
                (call.result, s)
            };
            if vars[target as usize].union(&s) {
                for &r in &readers[target as usize] {
                    if !std::mem::replace(&mut queued[r as usize], true) {
                        queue.push_back(r);
                    }
                }
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
        // What each source holds in its own fields: `this` what the method and the constructors
        // it delegates to store, a `new`'s object what its constructor stores. What a method
        // called on any other object stores in it escapes.
        let mut by_field: Vec<Vec<(FieldKey, Sources)>> = vec![Vec::new(); n];
        if code.has_this {
            for (f, value) in &code.stores {
                field(&mut by_field[0], *f).union(&eval(&vars, value));
            }
        }
        for (call, combined) in code.calls.iter().zip(&combined) {
            let Some(combined) = combined else { continue };
            if !call.has_receiver {
                continue;
            }
            for (k, arg) in call.args.iter().enumerate().skip(1) {
                let Some(fields) = combined.stored.get(k).filter(|fields| !fields.is_empty()) else {
                    continue;
                };
                let stored = eval(&vars, arg);
                if call.exact_receiver {
                    for x in eval(&vars, &call.args[0]).iter() {
                        for &f in fields {
                            field(&mut by_field[x], f).union(&stored);
                        }
                    }
                } else {
                    escaping.union(&stored);
                }
            }
        }
        let contents: Vec<Sources> = by_field
            .iter()
            .map(|fields| {
                let mut all = Sources::default();
                for (_, s) in fields {
                    all.union(s);
                }
                all
            })
            .collect();
        // What the method's own fields of `this` hold cannot be returned (a call's result is
        // not followed into them); the objects of its own stored in `this` outlive it; what
        // other objects hold is not followed; what a returned object holds is not either.
        for j in held..n {
            if returned.contains(j) {
                escaping.insert(j);
            }
        }
        for (x, held_by) in contents.iter().enumerate() {
            let own = sites.contains(&x) || (code.has_this && x == 0);
            if !own || (sites.contains(&x) && returned.contains(x)) {
                escaping.union(held_by);
            }
        }
        if code.has_this {
            for x in contents[0].iter() {
                if sites.contains(&x) {
                    escaping.insert(x);
                }
            }
        }
        // The sources that the fields of each of `sources` hold (of `this`, what the method
        // reads of them), all of them or those of field `f`.
        let holds = |sources: &Sources, f: Option<FieldKey>| {
            let mut out = Sources::default();
            for x in sources.iter() {
                if code.has_this && x == 0 {
                    for (j, &g) in code.held.iter().enumerate() {
                        if f.is_none_or(|f| f == g) {
                            out.insert(held + j);
                        }
                    }
                } else if sites.contains(&x) {
                    match f {
                        None => _ = out.union(&contents[x]),
                        Some(f) => {
                            if let Some((_, s)) = by_field[x].iter().find(|&&(g, _)| g == f) {
                                out.union(s);
                            }
                        }
                    }
                }
            }
            out
        };
        let mut via: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (call, combined) in code.calls.iter().zip(&combined) {
            match combined {
                Some(combined) => {
                    for (k, arg) in call.args.iter().enumerate() {
                        // The receiver of a dispatched call escapes as what its class runs lets
                        // its `this` and what it holds escape.
                        if k == 0 && call.dispatched {
                            let method = call.method.expect("a dispatched call names its method");
                            add_via(&mut via, &eval(&vars, arg), &[method]);
                            continue;
                        }
                        let Some(e) = combined.escapes.get(k) else { continue };
                        apply(e, &eval(&vars, arg), &mut escaping, &mut via);
                    }
                    if call.has_receiver && !call.dispatched {
                        let receivers = eval(&vars, &call.args[0]);
                        for (f, e) in &combined.contents {
                            apply(e, &holds(&receivers, Some(*f)), &mut escaping, &mut via);
                        }
                    }
                }
                None => _ = escaping.union(&eval(&vars, &call.unknown)),
            }
        }
        // An object of the method's own is of a known class, so what its methods do with their
        // `this` and its fields decides whether it and what it holds escape when methods are
        // called on it. What an object that escapes holds escapes with it.
        let mut read = Vec::new();
        let mut resolved = vec![0; n];
        // What each source holds is the same throughout, so each that escapes lets it escape
        // once.
        let mut spread = vec![false; held];
        loop {
            let mut grew = false;
            for x in 0..held {
                if escaping.contains(x) && !std::mem::replace(&mut spread[x], true) {
                    grew |= escaping.union(&holds(&Sources::of(x), None));
                }
            }
            for (i, site) in code.sites.iter().enumerate() {
                let s = first + i;
                if !active[i] || escaping.contains(s) || via[s].len() == resolved[s] {
                    continue;
                }
                resolved[s] = via[s].len();
                let c = self.site_class(site);
                if via[s].iter().any(|&r| self.this_escapes(r, c, &mut Vec::new(), &mut read)) {
                    escaping.insert(s);
                    grew = true;
                    continue;
                }
                if contents[s].is_empty() {
                    continue;
                }
                let mut all = Some(Vec::new());
                for &r in &via[s] {
                    all = all.and_then(|mut all| {
                        add_fields(&mut all, &self.contents_escape(r, c, &mut Vec::new(), &mut read)?);
                        Some(all)
                    });
                }
                match all {
                    None => grew |= escaping.union(&contents[s]),
                    Some(all) => {
                        for (f, e) in &all {
                            grew |= apply(e, &holds(&Sources::of(s), Some(*f)), &mut escaping, &mut via);
                        }
                    }
                }
            }
            if !grew {
                break;
            }
        }
        read.sort_unstable();
        read.dedup();
        Flows { vars, combined, active, escaping, returned, via, contents, by_field, read }
    }

    /// Solves method `m` with the summaries and factories known so far: its summary, the
    /// objects that stay in it, and what it makes and returns.
    fn solve(&mut self, m: usize) -> MethodFacts {
        if self.code[m].is_abstract {
            return MethodFacts {
                summary: Summary::empty(self.code[m].arity),
                staying: Vec::new(),
                product: None,
                read: Vec::new(),
            };
        }
        let Flows { vars, escaping, returned, via, contents, by_field, read, active, .. } = self.flows(m);
        let code = &self.code[m];
        let first = code.arity;
        let held = first + code.sites.len();
        let mut summary = Summary::empty(first);
        for (i, at) in via.iter().enumerate().take(first) {
            summary.escapes[i] = Escape { always: escaping.contains(i), via: at.clone() };
            summary.returned[i] = returned.contains(i);
            if code.has_this && i > 0 {
                let mut fields: Vec<FieldKey> =
                    by_field[0].iter().filter(|(_, s)| s.contains(i)).map(|&(f, _)| f).collect();
                fields.sort_unstable();
                summary.stored[i] = fields;
            }
        }
        let mut held_escapes: Vec<(FieldKey, Escape)> = code
            .held
            .iter()
            .enumerate()
            .map(|(j, &f)| (f, Escape { always: escaping.contains(held + j), via: via[held + j].clone() }))
            .collect();
        held_escapes.sort_unstable_by_key(|&(f, _)| f);
        summary.contents = held_escapes;
        let mut staying = Vec::new();
        let mut returned_sites = Vec::new();
        for (i, site) in code.sites.iter().enumerate() {
            let s = first + i;
            if !active[i] {
                continue;
            }
            if returned.contains(s) {
                returned_sites.push((i, s));
            } else if !escaping.contains(s)
                && dead_by_next_iteration(code, &vars, &contents, s, &mut Vec::new())
            {
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
            product = Some(Product { made: site.id, class: self.site_class(site), call: site.call });
        }
        MethodFacts { summary, staying, product, read }
    }

    /// Writes on standard error why each object made in the methods whose names contain one of
    /// `wanted` (`JAVA_TO_RUST_ESCAPE_WHY`, comma-separated) does not stay in its method.
    fn explain_all(&mut self, class: &Class, wanted: &str) {
        let wanted: Vec<&str> = wanted.split(',').filter(|w| !w.is_empty()).collect();
        for m in 0..self.code.len() {
            let name = method_name(class, m);
            if self.code[m].is_abstract || !wanted.iter().any(|w| name.contains(w)) {
                continue;
            }
            let flows = self.flows(m);
            let first = self.code[m].arity;
            let summary = &self.summaries[m];
            let show = |e: &Escape| {
                let via: Vec<String> = e.via.iter().map(|&r| method_name(class, r)).collect();
                if e.always { "always".to_string() } else { format!("via {via:?}") }
            };
            let params: Vec<String> = summary.escapes.iter().map(show).collect();
            let field_name = |&(t, i): &FieldKey| class.types[t].fields[i].name.clone();
            let stored: Vec<Vec<String>> =
                summary.stored.iter().map(|fs| fs.iter().map(field_name).collect()).collect();
            let held: Vec<String> =
                summary.contents.iter().map(|(f, e)| format!("{} {}", field_name(f), show(e))).collect();
            eprintln!(
                "[escape] {name}: parameters {params:?}, stored in {stored:?}, fields of `this` {held:?}"
            );
            for i in 0..self.code[m].sites.len() {
                if !flows.active[i] {
                    continue;
                }
                let s = first + i;
                let site = &self.code[m].sites[i];
                let c = self.site_class(site);
                let made = match site.call {
                    Some(t) => format!("call of {} ({})", method_name(class, t), class.types[c].name),
                    None => format!("new {}", class.types[c].name),
                };
                let mut out = format!("[escape] {name}: {made} (id {})", site.id);
                let escapes = flows.escaping.contains(s);
                if flows.returned.contains(s) {
                    out.push_str(" is returned");
                }
                if escapes {
                    out.push_str(" escapes:\n");
                    let holders: Vec<usize> = (0..first + self.code[m].sites.len())
                        .filter(|&t| t != s && flows.contents[t].contains(s))
                        .collect();
                    if !holders.is_empty() {
                        out.push_str(&format!(
                            "  held by sources {holders:?} (parameters first, `this` 0)\n"
                        ));
                    }
                    let mut seen = Vec::new();
                    if let Some(path) = flows.via[s].iter().find_map(|&r| self.escape_path(r, c, &mut seen)) {
                        let names: Vec<String> = path.iter().map(|&t| method_name(class, t)).collect();
                        out.push_str(&format!("  as the receiver of {}:\n", names.join(" -> ")));
                        let last = *path.last().expect("a path has a method");
                        let mut seen = HashSet::new();
                        self.explain(class, last, 0, Some(c), 2, &mut seen, &mut out);
                    }
                    let mut seen = HashSet::new();
                    self.explain(class, m, s, Some(c), 1, &mut seen, &mut out);
                } else if !flows.returned.contains(s) {
                    if dead_by_next_iteration(&self.code[m], &flows.vars, &flows.contents, s, &mut Vec::new())
                    {
                        out.push_str(" stays");
                    } else {
                        out.push_str(" is held past its loop's iteration");
                    }
                }
                eprintln!("{}", out.trim_end());
            }
        }
    }

    /// The methods that an object of class `c` runs from method `r` (called on it) to one that
    /// lets its `this` escape always, if there is one.
    fn escape_path(&self, r: usize, c: usize, seen: &mut Vec<usize>) -> Option<Vec<usize>> {
        let Some(t) = self.hierarchy.implementation(r, c) else { return Some(vec![r]) };
        if seen.contains(&t) {
            return None;
        }
        seen.push(t);
        let e = self.summaries[t].escapes.first()?;
        if e.always {
            return Some(vec![t]);
        }
        e.via.iter().find_map(|&r| self.escape_path(r, c, seen)).map(|mut path| {
            path.insert(0, t);
            path
        })
    }

    /// Whether a parameter with `e` escapes for an object of class `c`, if known.
    fn escapes_as(&self, e: &Escape, c: Option<usize>) -> bool {
        match c {
            None => e.always || !e.via.is_empty(),
            Some(c) => {
                e.always || e.via.iter().any(|&r| self.this_escapes(r, c, &mut Vec::new(), &mut Vec::new()))
            }
        }
    }

    /// Adds to `out` why source `s` of method `m`, an object of class `c` if known, escapes,
    /// following the callees that let it.
    #[allow(clippy::too_many_arguments)]
    fn explain(
        &mut self,
        class: &Class,
        m: usize,
        s: usize,
        c: Option<usize>,
        depth: usize,
        seen: &mut HashSet<(usize, usize)>,
        out: &mut String,
    ) {
        let indent = "  ".repeat(depth);
        if depth > 8 || !seen.insert((m, s)) {
            out.push_str(&format!("{indent}(followed already)\n"));
            return;
        }
        if self.code[m].is_abstract {
            return;
        }
        let flows = self.flows(m);
        let code = &self.code[m];
        for (value, label) in code.escapes.iter().zip(&code.labels) {
            if eval(&flows.vars, value).contains(s) {
                out.push_str(&format!("{indent}{label}\n"));
            }
        }
        let mut next: Vec<(usize, usize)> = Vec::new();
        for (call, combined) in code.calls.iter().zip(&flows.combined) {
            let called = call.method.map_or("?".to_string(), |r| method_name(class, r));
            match combined {
                Some(combined) => {
                    for (k, arg) in call.args.iter().enumerate() {
                        if !eval(&flows.vars, arg).contains(s) {
                            continue;
                        }
                        if k == 0 && call.dispatched {
                            let r = call.method.expect("a dispatched call names its method");
                            match c {
                                Some(c) if self.this_escapes(r, c, &mut Vec::new(), &mut Vec::new()) => {
                                    let t = self.hierarchy.implementation(r, c);
                                    out.push_str(&format!("{indent}the receiver of a call of {called}\n"));
                                    next.extend(t.map(|t| (t, 0)));
                                }
                                Some(_) => {}
                                None => out.push_str(&format!(
                                    "{indent}the receiver of a call of {called}, by class\n"
                                )),
                            }
                            continue;
                        }
                        let Some(e) = combined.escapes.get(k) else { continue };
                        if self.escapes_as(e, c) {
                            let targets =
                                &self.lists.targets[call.targets.expect("a followed call") as usize];
                            out.push_str(&format!(
                                "{indent}argument {k} of a call of {called} ({} targets)\n",
                                targets.len()
                            ));
                            let targets: Vec<usize> = targets.to_vec();
                            next.extend(
                                targets
                                    .into_iter()
                                    .filter(|&t| {
                                        self.summaries[t]
                                            .escapes
                                            .get(k)
                                            .is_some_and(|e| self.escapes_as(e, c))
                                    })
                                    .take(3)
                                    .map(|t| (t, k)),
                            );
                        }
                    }
                }
                None => {
                    if eval(&flows.vars, &call.unknown).contains(s) {
                        out.push_str(&format!("{indent}given to a call that is not followed: {called}\n"));
                    }
                }
            }
        }
        for (t, k) in next {
            out.push_str(&format!("{indent}in {}:\n", method_name(class, t)));
            self.explain(class, t, k, c, depth + 1, seen, out);
        }
    }
}

/// What solving a method finds before it is summed up (`Analysis::flows`).
struct Flows {
    vars: Vec<Sources>,
    combined: Vec<Option<Rc<Summary>>>,
    active: Vec<bool>,
    escaping: Sources,
    returned: Sources,
    /// For each source, the methods called on it as the receiver (or that callees call on it):
    /// it escapes if what its class runs for one of them lets its `this` escape.
    via: Vec<Vec<usize>>,
    /// What each source holds in its own fields (see `Contents`), in all of them and by field;
    /// the last sources are what `this` holds in the fields the method reads.
    contents: Vec<Sources>,
    by_field: Vec<Vec<(FieldKey, Sources)>>,
    /// The methods outside the calls' targets whose summaries were read.
    read: Vec<usize>,
}

/// What `fields` holds in field `f`.
fn field(fields: &mut Vec<(FieldKey, Sources)>, f: FieldKey) -> &mut Sources {
    let at = match fields.iter().position(|&(g, _)| g == f) {
        Some(at) => at,
        None => {
            fields.push((f, Sources::default()));
            fields.len() - 1
        }
    };
    &mut fields[at].1
}

/// Adds the methods `rs` to the `via` of each of `sources`; whether that added any.
fn add_via(via: &mut [Vec<usize>], sources: &Sources, rs: &[usize]) -> bool {
    let mut grew = false;
    for i in sources.iter() {
        for &r in rs {
            if let Err(at) = via[i].binary_search(&r) {
                via[i].insert(at, r);
                grew = true;
            }
        }
    }
    grew
}

/// Lets `sources` escape as `e` says; whether that added anything.
fn apply(e: &Escape, sources: &Sources, escaping: &mut Sources, via: &mut [Vec<usize>]) -> bool {
    if e.always { escaping.union(sources) } else { add_via(via, sources, &e.via) }
}

/// The sources that `value` may be, with the variables holding `vars`.
fn eval(vars: &[Sources], value: &Value) -> Sources {
    let mut s = Sources::default();
    for &v in &value.vars {
        s.union(&vars[v as usize]);
    }
    for &i in &value.sources {
        s.insert(i as usize);
    }
    s
}

/// `Class.method`, for the explanations.
fn method_name(class: &Class, m: usize) -> String {
    let method = &class.methods[m];
    format!("{}.{}", class.types[method.class].name, method.name)
}

/// Whether the object of `site` (source `s`), if it is in a loop, can no longer be reached when
/// the next iteration makes the next one: every variable that may hold it is declared inside the
/// body of the innermost loop around it. (The results of calls are not kept across iterations.)
/// An object held in the fields of another of the method's objects is held as long as that one:
/// the other must be made in the same loop and be unreachable too by the next iteration.
/// `seen` holds the sources being checked, which count as not.
fn dead_by_next_iteration(
    code: &Compiled,
    vars: &[Sources],
    contents: &[Sources],
    s: usize,
    seen: &mut Vec<usize>,
) -> bool {
    let first = code.arity;
    let site = &code.sites[s - first];
    let Some(&innermost) = site.loops.as_ref().and_then(|loops| loops.last()) else { return true };
    if seen.contains(&s) {
        return false;
    }
    seen.push(s);
    code.named.iter().filter(|named| vars[named.var as usize].contains(s)).all(|named| {
        named.param.is_none() && named.loops.as_ref().is_some_and(|loops| loops.contains(&innermost))
    }) && (first..first + code.sites.len()).filter(|&t| t != s && contents[t].contains(s)).all(|t| {
        code.sites[t - first].loops.as_ref().is_some_and(|loops| loops.contains(&innermost))
            && dead_by_next_iteration(code, vars, contents, t, seen)
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
            | Builtin::NullChecked(_)
            | Builtin::MemoryAccessWarning(_)
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
    /// expression are walked on their own, with the loops in them, once: walking them here too
    /// would walk the switches nested in them once more at each level, twice as many times for
    /// each (the decompiled Minecraft server nests them deep enough for gigabytes).
    fn expr(&mut self, e: &'m Expr, loops: &mut Vec<usize>, bindings: Option<usize>) {
        // The expressions in the statements of `e`'s switch expressions, at any depth.
        let mut inner: HashSet<*const Expr> = HashSet::new();
        visit_expr(e, &mut |x| {
            // An outer switch comes first, and holds the ones nested in its statements.
            if let ExprKind::Switch { cases, .. } = &x.kind
                && !inner.contains(&(x as *const Expr))
            {
                for s in cases.iter().flat_map(|c| &c.body) {
                    visit_stmt_exprs(s, &mut |y| _ = inner.insert(y));
                }
            }
        });
        let mut switches = Vec::new();
        visit_expr(e, &mut |x| match &x.kind {
            _ if inner.contains(&(x as *const Expr)) => {}
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
