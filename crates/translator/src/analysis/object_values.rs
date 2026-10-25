//! Which expressions of the program may be a string or a box held as an object: a value of
//! `Object` (or of `Comparable`, `Number` or `CharSequence`) that came from a string, a box or a
//! primitive. A string or a box in an `Object` is a plain value whose identity the translation
//! does not keep, so `==` on two such values is rejected (`patterns::check_object_values`);
//! where this analysis shows that one side cannot be one, `==` is plain identity.
//!
//! The analysis is a points-to analysis that follows two things together until nothing changes:
//! whether a place may hold a string or a box, and which of the program's classes its objects
//! may be of. It follows them through variables, parameters, return values, fields, statics and
//! array elements (by element type). The classes decide which implementations a call runs, and
//! each instance method is followed once for every class whose objects run it, with `this` of
//! that class: an `IdentitySet` that inherits `Set`'s `add` and overrides its `contains` then
//! gets only what is added to identity sets, though other sets hold boxes. Anything it does not
//! follow (a library call's result, a variable bound by a construct it does not model, a
//! parameter of a method the runtime itself may call) may be anything.
//!
//! An expression of one of the program's own classes is never a string or a box, whatever it
//! reads: the translation's type for it holds none (a value read through an erased type variable
//! is cast to the class first).
//!
//! For the calls that can run (`live_methods`), the classes must cover every object that an
//! expression can be, as a call that misses one would run a method that was taken out. That
//! analysis (`analyze_for_dispatch`) follows more:
//!
//! - An array element is read as any element of the program's arrays whose class fits the type,
//!   since an array is written through one type and read through another (`Object[]` and
//!   `String[]`, `T[]` and its argument's).
//! - An instance method is followed for every concrete class under its own, as `super.m()` runs
//!   it on objects of a class that overrides it, and so is an instance field's initializer.
//! - The constants of an enum set its instance fields.
//! - The parameters of a method that the runtime may call itself (`runtime_called`) hold what
//!   calls in the code pass them and what the runtime holds (`handed`): the objects it was given.
//! - "Any class its type allows" is spelled out as those classes where the type is not open
//!   (`Object`, a type variable), so that it stays within the type wherever the value goes. A
//!   compound assignment stores a number or a string; a switch expression is one of the values it
//!   yields; a value checked for null or copied by `clone()` is the value given; what ends the
//!   program has none.

use super::Facts;
use crate::ast::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

/// The context of code that has no `this`: a static method, or a method that no class runs.
pub(super) const STATIC: usize = usize::MAX;

/// The types whose values include strings or boxes: `Object` and the interfaces and classes of
/// the JDK that strings or boxes implement.
const HOLDING_VALUES: [&str; 10] = [
    OBJECT,
    COMPARABLE,
    NUMBER,
    CHAR_SEQUENCE,
    "java.lang.Object",
    "java.lang.Comparable",
    "java.lang.Number",
    "java.lang.CharSequence",
    "java.io.Serializable",
    "java.lang.constant.Constable",
];

/// What a place or an expression may hold.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub(super) struct Flow {
    /// A string, a box or a primitive held as an object.
    value: bool,
    /// Objects of any class its type allows; `classes` lists them otherwise.
    pub any: bool,
    pub classes: ClassSet,
}

impl Flow {
    fn top() -> Flow {
        Flow { value: true, any: true, classes: ClassSet::default() }
    }

    fn of(class: usize) -> Flow {
        let mut classes = ClassSet::default();
        classes.insert(class);
        Flow { classes, ..Flow::default() }
    }

    /// Adds what `other` may hold; whether that changed anything.
    fn join(&mut self, other: &Flow) -> bool {
        let before = (self.value, self.any);
        self.value |= other.value;
        self.any |= other.any;
        let grew = !self.any && self.classes.union(&other.classes);
        grew || before != (self.value, self.any)
    }
}

/// A set of classes, by index into `Class::types`: a bit for each, from the word of the lowest
/// class in it to that of the highest, so that a set of a few classes far into the program's types
/// takes a few words rather than one for every 64 types before them. Copies share their words
/// until one of them changes: a set of every class of a type is copied into a great many places,
/// and on the Minecraft server it is thousands of words.
#[derive(Clone, Debug)]
pub struct ClassSet {
    /// The index of the first word in `words`.
    start: usize,
    words: Rc<Vec<u64>>,
}

impl Default for ClassSet {
    /// The empty set, whose words all empty sets share, so that making one allocates nothing.
    fn default() -> ClassSet {
        thread_local! {
            static EMPTY: Rc<Vec<u64>> = Rc::new(Vec::new());
        }
        ClassSet { start: 0, words: EMPTY.with(Rc::clone) }
    }
}

impl ClassSet {
    /// Widens the words to cover words `from..to`.
    fn cover(&mut self, from: usize, to: usize) {
        if self.words.is_empty() {
            self.start = from;
            self.words = Rc::new(vec![0; to - from]);
            return;
        }
        if from < self.start {
            Rc::make_mut(&mut self.words).splice(0..0, std::iter::repeat_n(0, self.start - from));
            self.start = from;
        }
        if self.start + self.words.len() < to {
            Rc::make_mut(&mut self.words).resize(to - self.start, 0);
        }
    }

    pub(super) fn insert(&mut self, c: usize) {
        self.cover(c / 64, c / 64 + 1);
        Rc::make_mut(&mut self.words)[c / 64 - self.start] |= 1 << (c % 64);
    }

    /// The word at index `i`, of all of them.
    fn word(&self, i: usize) -> u64 {
        i.checked_sub(self.start).and_then(|i| self.words.get(i)).copied().unwrap_or(0)
    }

    /// Adds the classes of `other`; whether that added any.
    fn union(&mut self, other: &ClassSet) -> bool {
        // A copy of this set (copies share their words) adds nothing.
        if self.start == other.start && Rc::ptr_eq(&self.words, &other.words) {
            return false;
        }
        let Some(from) = other.words.iter().position(|&w| w != 0) else { return false };
        let to = other.words.iter().rposition(|&w| w != 0).unwrap_or(from) + 1;
        let (from, to) = (other.start + from, other.start + to);
        if (from..to).all(|i| other.word(i) & !self.word(i) == 0) {
            return false;
        }
        // Where `other` holds every class of this set, this set becomes a copy of it.
        if (0..self.words.len()).all(|i| self.words[i] & !other.word(self.start + i) == 0) {
            *self = other.clone();
            return true;
        }
        self.cover(from, to);
        let words = Rc::make_mut(&mut self.words);
        for i in from..to {
            words[i - self.start] |= other.word(i);
        }
        true
    }

    /// Keeps the classes that are also in `other`.
    fn intersect(&mut self, other: &ClassSet) {
        let start = self.start;
        if self.words.iter().enumerate().all(|(i, &word)| word & !other.word(start + i) == 0) {
            return;
        }
        for (i, word) in Rc::make_mut(&mut self.words).iter_mut().enumerate() {
            *word &= other.word(start + i);
        }
    }

    /// How many words the set holds, for the timings.
    pub(super) fn words(&self) -> usize {
        self.words.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(move |(i, &word)| {
            let base = (self.start + i) * 64;
            // Each bit set, lowest first: the word with the bits below it cleared.
            std::iter::successors((word != 0).then_some(word), |&w| (w & (w - 1) != 0).then(|| w & (w - 1)))
                .map(move |w| base + w.trailing_zeros() as usize)
        })
    }
}

impl FromIterator<usize> for ClassSet {
    fn from_iter<I: IntoIterator<Item = usize>>(classes: I) -> ClassSet {
        let classes: Vec<usize> = classes.into_iter().collect();
        let mut set = ClassSet::default();
        if let (Some(&low), Some(&high)) = (classes.iter().min(), classes.iter().max()) {
            set.cover(low / 64, high / 64 + 1);
        }
        classes.into_iter().for_each(|c| set.insert(c));
        set
    }
}

impl PartialEq for ClassSet {
    fn eq(&self, other: &ClassSet) -> bool {
        let from = self.start.min(other.start);
        let to = (self.start + self.words.len()).max(other.start + other.words.len());
        (from..to).all(|i| self.word(i) == other.word(i))
    }
}

impl Eq for ClassSet {}

/// What the analysis found: the places that may hold a string or a box, and the classes of
/// their objects.
pub struct ObjectValues {
    /// The variables of each method in each of its contexts, by their slots in `followed`.
    vars: super::FastMap<(usize, usize), Vec<Flow>>,
    returns: super::FastMap<(usize, usize), Flow>,
    /// Instance fields, by the class that declares them.
    fields: HashMap<(usize, String), Flow>,
    statics: HashMap<(usize, String), Flow>,
    elements: HashMap<Type, Flow>,
    /// The variables of each method that the analysis follows, each with its slot in `vars`: its
    /// parameters (unless the runtime may call it) and the variables it declares, loops over or
    /// binds with a pattern.
    followed: Vec<super::FastMap<String, usize>>,
    /// For dispatch, the parameters of each method that the runtime may call itself
    /// (`runtime_called`), which hold what the runtime holds besides what calls pass them.
    runtime_params: Vec<Vec<String>>,
    /// For dispatch, the objects that the runtime holds: those given to a builtin
    /// (`hands_objects`), the operands of a string concatenation, and the exceptions thrown.
    handed: Flow,
    /// The classes whose objects run each method, `STATIC` for one without `this`.
    pub(super) contexts: Vec<Vec<usize>>,
    types: Types,
    /// The analysis for the calls that can run (see the module's comment).
    dispatch: bool,
    /// While the analysis runs, the places that the current visit has read.
    reads: RefCell<Option<super::FastSet<Read>>>,
}

/// A place whose flow a visit reads, so that the visit runs again when the place changes.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Read {
    /// The variables of a method in a context, which only the visit of that method in that
    /// context reads (so they have no readers, see `analyze_with`).
    Vars(usize, usize),
    Return(usize, usize),
    Field(usize, String),
    Static(usize, String),
    Element(Type),
    /// The elements of every array (for dispatch, see `elements_of`).
    Elements,
    Handed,
}

/// What the analysis needs to know of the program's types.
struct Types {
    index: super::FastMap<String, usize>,
    /// Types no string or box is a value of.
    excluded: Vec<bool>,
    /// Types that objects of every class may be values of: `Object` and its kin, and type
    /// variables.
    open: Vec<bool>,
    /// The concrete classes that are each type or its subtypes.
    concretes: Vec<Vec<usize>>,
    /// The same, as sets.
    concrete_sets: Vec<ClassSet>,
    /// The types that are each type or its subtypes.
    subtypes: Vec<ClassSet>,
    all_concretes: Vec<usize>,
    /// Whether "any class its type allows" is spelled out as those classes for the types that
    /// are not open, so that it does not spread past them (the analysis for dispatch).
    bounded: bool,
    /// The method with a body that each type declares for an override group.
    declared: super::FastMap<(usize, usize), usize>,
    implementations: RefCell<super::FastMap<(usize, usize), Option<usize>>>,
}

impl Types {
    fn new(class: &Class, bounded: bool) -> Types {
        let n = class.types.len();
        let index = class.types.iter().enumerate().map(|(t, d)| (d.name.clone(), t)).collect();
        let open: Vec<bool> = class
            .types
            .iter()
            .map(|d| d.kind == TypeKind::Param || HOLDING_VALUES.contains(&d.name.as_str()))
            .collect();
        let excluded = class
            .types
            .iter()
            .enumerate()
            .map(|(t, d)| {
                !open[t] && matches!(d.kind, TypeKind::Class | TypeKind::Interface | TypeKind::Enum)
            })
            .collect();
        let mut supers = vec![HashSet::new(); n];
        for (t, set) in supers.iter_mut().enumerate() {
            let mut work = vec![t];
            while let Some(s) = work.pop() {
                if set.insert(s) {
                    let d = &class.types[s];
                    work.extend(d.extends.iter().chain(&d.implements));
                }
            }
        }
        let all_concretes: Vec<usize> = (0..n).filter(|&t| class.hierarchy.is_concrete(t)).collect();
        let mut concretes = vec![Vec::new(); n];
        for &c in &all_concretes {
            for &s in &supers[c] {
                concretes[s].push(c);
            }
        }
        let mut declared = super::FastMap::default();
        for (m, method) in class.methods.iter().enumerate() {
            if !method.is_abstract && !method.is_static && !method.is_ctor {
                declared.insert((method.class, class.hierarchy.group_id(m)), m);
            }
        }
        let concrete_sets = concretes.iter().map(|c| c.iter().copied().collect()).collect();
        let mut subtypes = vec![ClassSet::default(); n];
        for (c, set) in supers.iter().enumerate() {
            for &s in set {
                subtypes[s].insert(c);
            }
        }
        Types {
            index,
            excluded,
            open,
            concretes,
            concrete_sets,
            subtypes,
            all_concretes,
            declared,
            implementations: RefCell::default(),
            bounded,
        }
    }

    /// The implementation that an object of class `c` runs for a call of a method of override
    /// group `group`: the nearest declaration in its superclass chain, then in its interfaces
    /// (as `Hierarchy::implementation`, remembered).
    fn implementation(&self, class: &Class, group: usize, c: usize) -> Option<usize> {
        if let Some(&found) = self.implementations.borrow().get(&(group, c)) {
            return found;
        }
        let mut chain = vec![c];
        chain.extend(class.hierarchy.ancestors(c));
        let mut found = chain.iter().find_map(|&t| self.declared.get(&(t, group)).copied());
        let mut work: Vec<usize> =
            chain.iter().flat_map(|&t| class.types[t].implements.iter().copied()).collect();
        let mut seen = HashSet::new();
        while found.is_none() && !work.is_empty() {
            let mut next = Vec::new();
            for i in work {
                if seen.insert(i) {
                    found = found.or_else(|| self.declared.get(&(i, group)).copied());
                    next.extend(class.types[i].implements.iter().copied());
                }
            }
            work = next;
        }
        self.implementations.borrow_mut().insert((group, c), found);
        found
    }

    /// The concrete classes whose objects a value of type `ty` may be.
    fn concretes_of(&self, ty: Type) -> &[usize] {
        match ty {
            Type::Class(name, _) | Type::Enum(name) => match self.index.get(name) {
                Some(&t) if !self.open[t] => &self.concretes[t],
                _ => &self.all_concretes,
            },
            _ => &[],
        }
    }

    /// What a value of type `ty` may hold, of what `flow` says.
    fn filter(&self, mut flow: Flow, ty: Type) -> Flow {
        match ty {
            Type::Class(name, _) => {
                if let Some(&t) = self.index.get(name) {
                    if self.excluded[t] {
                        flow.value = false;
                    }
                    if !self.open[t] && self.bounded && flow.any {
                        flow.any = false;
                        flow.classes = self.concrete_sets[t].clone();
                    } else if !self.open[t] {
                        flow.classes.intersect(&self.subtypes[t]);
                    }
                }
                flow
            }
            Type::Enum(name) if self.bounded => match self.index.get(name) {
                Some(&t) => Flow { classes: self.concrete_sets[t].clone(), ..Flow::default() },
                None => Flow { any: true, ..Flow::default() },
            },
            Type::Enum(_) => Flow { any: true, ..Flow::default() },
            Type::Null | Type::Void | Type::Array(_) | Type::StringBuilder | Type::JavaClass => {
                Flow::default()
            }
            _ => Flow { value: true, ..Flow::default() },
        }
    }
}

impl ObjectValues {
    /// Whether `l` and `r`, the sides of a comparison in method `m`, may both be strings or boxes
    /// held as objects, on an object of some class that runs `m`.
    pub fn may_both_be_values(&self, class: &Class, m: usize, l: &Expr, r: &Expr) -> bool {
        self.contexts[m]
            .iter()
            .any(|&ctx| self.eval(class, m, ctx, l).value && self.eval(class, m, ctx, r).value)
    }

    /// How many words the class sets of the places hold, and in how many distinct buffers they
    /// are, for the timings.
    fn class_set_words(&self) -> (usize, usize) {
        let mut seen = HashSet::new();
        let (mut words, mut held) = (0, 0);
        let flows = self.vars.values().flatten().chain(self.returns.values()).chain(self.fields.values());
        for flow in flows.chain(self.statics.values()).chain(self.elements.values()) {
            words += flow.classes.words.len();
            if seen.insert(Rc::as_ptr(&flow.classes.words)) {
                held += flow.classes.words.len();
            }
        }
        (words, held)
    }

    /// The update of variable `name` of method `m` in context `ctx` with `flow`, when the analysis
    /// follows the variable: one it does not follow holds anything (`var`).
    fn var_update(&self, m: usize, ctx: usize, name: &str, flow: Flow) -> Option<Update> {
        let &slot = self.followed.get(m)?.get(name)?;
        Some(Update::Var(m, ctx, slot, flow))
    }

    /// Notes that the running visit reads `place`, each place once: an expression is evaluated
    /// again for each expression around it, and a call on a receiver that may be any class reads
    /// the returns of thousands of methods.
    fn read(&self, place: impl FnOnce() -> Read) {
        if let Some(reads) = self.reads.borrow_mut().as_mut() {
            reads.insert(place());
        }
    }

    fn var(&self, m: usize, ctx: usize, name: &str) -> Flow {
        let Some(&slot) = self.followed.get(m).and_then(|f| f.get(name)) else {
            return Flow::top();
        };
        if !self.runtime_params[m].is_empty() {
            self.read(|| Read::Handed);
        }
        let mut flow = self.vars.get(&(m, ctx)).map(|v| v[slot].clone()).unwrap_or_default();
        if self.runtime_params[m].iter().any(|p| p == name) {
            flow.join(&self.handed);
        }
        flow
    }

    /// The elements of arrays of element type `element` that a read may give: in the analysis
    /// for dispatch, those of every array whose class fits the type (see the module's comment).
    pub(super) fn elements_of(&self, element: Type) -> Flow {
        if !self.dispatch {
            self.read(|| Read::Element(element));
            return self.elements.get(&element).cloned().unwrap_or_default();
        }
        self.read(|| Read::Elements);
        let mut flow = Flow::default();
        for stored in self.elements.values() {
            flow.join(stored);
        }
        self.types.filter(flow, element)
    }

    /// The concrete classes whose objects a value of type `ty` may be, whatever it holds.
    pub(super) fn concretes_of(&self, ty: Type) -> &[usize] {
        self.types.concretes_of(ty)
    }

    /// The implementation that an object of class `c` runs for a call of a method of override
    /// group `group`.
    pub(super) fn implementation(&self, class: &Class, group: usize, c: usize) -> Option<usize> {
        self.types.implementation(class, group, c)
    }

    /// What `e`, evaluated in method `m` on an object of class `ctx`, may be.
    pub(super) fn eval(&self, class: &Class, m: usize, ctx: usize, e: &Expr) -> Flow {
        let flow = match &e.kind {
            ExprKind::Null | ExprKind::NewArray(_) | ExprKind::ArrayLit(_) => Flow::default(),
            ExprKind::This if ctx == STATIC => Flow::top(),
            ExprKind::This => Flow::of(ctx),
            ExprKind::New { class: c, .. } => Flow::of(*c),
            ExprKind::Var(name) => self.var(m, ctx, name),
            ExprKind::Assign { op: None, value, .. } | ExprKind::Cast(_, value) => {
                self.eval(class, m, ctx, value)
            }
            ExprKind::Conditional(_, a, b) => {
                let mut flow = self.eval(class, m, ctx, a);
                flow.join(&self.eval(class, m, ctx, b));
                flow
            }
            ExprKind::Field { object, name } => match class.field_owner(object.ty, name) {
                Some(owner) => {
                    self.read(|| Read::Field(owner, name.clone()));
                    self.fields.get(&(owner, name.clone())).cloned().unwrap_or_default()
                }
                None => Flow::top(),
            },
            ExprKind::Static { class: c, name } => {
                self.read(|| Read::Static(*c, name.clone()));
                self.statics.get(&(*c, name.clone())).cloned().unwrap_or_default()
            }
            ExprKind::Index(array, _) => self.elements_of(array.ty.element()),
            ExprKind::Call { resolved: Some(_), .. } | ExprKind::MethodCall { resolved: Some(_), .. } => {
                let mut flow = Flow::default();
                for target in self.targets(class, m, ctx, e) {
                    self.read(|| Read::Return(target.0, target.1));
                    flow.join(&self.returns.get(&target).cloned().unwrap_or_default());
                }
                flow
            }
            // For dispatch: what ends the program has no value, a checked value or a copy is the
            // value it is given.
            ExprKind::Builtin(
                Builtin::UnknownClass
                | Builtin::UpdaterRefused
                | Builtin::MissingResource
                | Builtin::NoObject { .. },
                _,
            ) if self.dispatch => Flow::default(),
            ExprKind::Builtin(Builtin::NullChecked(_) | Builtin::ObjectClone, args)
                if self.dispatch && matches!(args.len(), 1 | 2) =>
            {
                self.eval(class, m, ctx, &args[args.len() - 1])
            }
            // For dispatch, a switch expression is one of the values it yields.
            ExprKind::Switch { cases, .. } if self.dispatch => {
                let mut flow = Flow::default();
                for s in cases.iter().flat_map(|c| &c.body) {
                    visit_stmt_outside_switches(s, &mut |s| {
                        if let Stmt::Yield(e) = s {
                            flow.join(&self.eval(class, m, ctx, e));
                        }
                    });
                }
                flow
            }
            _ => Flow::top(),
        };
        self.types.filter(flow, e.ty)
    }

    /// The methods that the call `e` in method `m` on an object of class `ctx` may run, each with
    /// the class of the object it runs on.
    pub(super) fn targets(&self, class: &Class, m: usize, ctx: usize, e: &Expr) -> Vec<(usize, usize)> {
        let context = |r: usize, ctx: usize| if class.methods[r].is_static { STATIC } else { ctx };
        match &e.kind {
            ExprKind::Call { resolved: Some(r), .. } => vec![(*r, context(*r, ctx))],
            ExprKind::New { class: c, ctor: Some(r), .. } => vec![(*r, *c)],
            ExprKind::ThisCall { resolved: Some(r), .. } | ExprKind::SuperInit { resolved: Some(r), .. } => {
                vec![(*r, ctx)]
            }
            ExprKind::MethodCall { resolved: Some(r), is_super, receiver, .. } => {
                if class.methods[*r].is_static || *is_super && matches!(receiver.kind, ExprKind::This) {
                    return vec![(*r, context(*r, ctx))];
                }
                let flow = self.eval(class, m, ctx, receiver);
                let classes: Vec<usize> = if flow.any {
                    self.types.concretes_of(receiver.ty).to_vec()
                } else {
                    flow.classes.iter().collect()
                };
                // A `super` call in a lambda's code is on the object that the lambda captured,
                // and runs the method it names on it.
                if *is_super {
                    return classes.into_iter().map(|c| (*r, c)).collect();
                }
                let group = class.hierarchy.group_id(*r);
                classes
                    .into_iter()
                    .filter_map(|c| Some((self.types.implementation(class, group, c)?, c)))
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

/// What one visit of a method learns about a place.
enum Update {
    /// A variable of a method in a context, by its slot.
    Var(usize, usize, usize, Flow),
    Return(usize, usize, Flow),
    Field(usize, String, Flow),
    Static(usize, String, Flow),
    Element(Type, Flow),
    /// Objects that the runtime is handed (see `handed`).
    Handed(Flow),
}

/// The override groups of the methods that the runtime may call itself on an object it holds,
/// such as `equals` from a library's map or `toString` to print it: those of `Object` and of the
/// JDK interfaces and classes that strings and boxes implement.
pub(super) fn runtime_called(class: &Class) -> HashSet<usize> {
    class
        .methods
        .iter()
        .enumerate()
        .filter(|(_, method)| HOLDING_VALUES.contains(&class.types[method.class].name.as_str()))
        .map(|(m, _)| class.hierarchy.group_id(m))
        .collect()
}

/// `runtime_called` for dispatch: with the groups of every `toString()`, `hashCode()`,
/// `equals(Object)` and `compareTo(T)`, which override methods of `Object` and `Comparable` that
/// the program may not declare (the translation's `Object` has none).
pub(super) fn runtime_called_for_dispatch(class: &Class) -> HashSet<usize> {
    let mut groups = runtime_called(class);
    for (m, method) in class.methods.iter().enumerate() {
        let shape = (method.name.as_str(), method.params.len());
        if !method.is_static
            && !method.is_ctor
            && matches!(shape, ("toString" | "hashCode", 0) | ("equals" | "compareTo", 1))
        {
            groups.insert(class.hierarchy.group_id(m));
        }
    }
    groups
}

/// Whether the runtime may hold an object given to `builtin`, and so call a method of it
/// (`runtime_called`): all builtins but those that only lock, wait, name, test, hash by identity,
/// copy or fill.
pub(super) fn hands_objects(builtin: &Builtin) -> bool {
    !matches!(
        builtin,
        Builtin::MonitorEnter
            | Builtin::MemoryAccessWarning(_)
            | Builtin::GetClass
            | Builtin::ClassName { .. }
            | Builtin::SameClass { .. }
            | Builtin::NullChecked(_)
            | Builtin::NoObject { .. }
            | Builtin::InitializeClass { .. }
            | Builtin::ClassLiteral(_)
            | Builtin::ClassLiteralName { .. }
            | Builtin::EnumOrdinal
            | Builtin::ArraysCopyOf
            | Builtin::ArraysFill
            | Builtin::StringArrayOf
            | Builtin::ObjectArrayOf
            | Builtin::Native(
                Native::IdentityHash
                    | Native::SystemIdentityHash
                    | Native::MonitorWait
                    | Native::MonitorNotify { .. }
                    | Native::HoldsLock
                    | Native::ArrayCopy
            )
    )
}

/// What the runtime is handed by `e`, the argument of a builtin: its value, or the elements of
/// the innermost arrays of an array.
pub(super) fn handed_by(
    values: &ObjectValues,
    class: &Class,
    m: usize,
    ctx: usize,
    e: &Expr,
) -> (Flow, Type) {
    let mut ty = e.ty;
    while let Type::Array(element) = ty {
        ty = *element;
    }
    match e.ty {
        Type::Array(_) => (values.elements_of(ty), ty),
        _ => (values.eval(class, m, ctx, e), ty),
    }
}

/// Follows the program's strings, boxes and objects through its places.
pub fn analyze(class: &Class, _facts: &Facts) -> ObjectValues {
    analyze_with(class, false)
}

/// The analysis for the calls that can run (see the module's comment).
pub(super) fn analyze_for_dispatch(class: &Class) -> ObjectValues {
    analyze_with(class, true)
}

fn analyze_with(class: &Class, dispatch: bool) -> ObjectValues {
    let types = Types::new(class, dispatch);
    // Methods the runtime may call itself, such as `equals` from a library's map: their
    // parameters may be anything. For dispatch, they may be anything that the runtime holds.
    let runtime_called = if dispatch { runtime_called_for_dispatch(class) } else { runtime_called(class) };
    let by_runtime = |m: usize| runtime_called.contains(&class.hierarchy.group_id(m));
    let followed = class
        .methods
        .iter()
        .enumerate()
        .map(|(m, method)| followed(method, dispatch || !by_runtime(m)))
        .collect();
    let runtime_params = class
        .methods
        .iter()
        .enumerate()
        .map(|(m, method)| match dispatch && by_runtime(m) {
            true => method.params.iter().map(|p| p.name.clone()).collect(),
            false => Vec::new(),
        })
        .collect();
    let contexts = class
        .methods
        .iter()
        .enumerate()
        .map(|(m, method)| {
            let found: Vec<usize> = if method.is_static {
                Vec::new()
            } else if method.is_ctor || dispatch {
                types.concretes[method.class].clone()
            } else {
                let group = class.hierarchy.group_id(m);
                types.concretes[method.class]
                    .iter()
                    .copied()
                    .filter(|&c| types.implementation(class, group, c) == Some(m))
                    .collect()
            };
            if found.is_empty() { vec![STATIC] } else { found }
        })
        .collect();
    let mut values = ObjectValues {
        vars: super::FastMap::default(),
        returns: super::FastMap::default(),
        fields: HashMap::new(),
        statics: HashMap::new(),
        elements: HashMap::new(),
        followed,
        runtime_params,
        handed: Flow::default(),
        contexts,
        types,
        dispatch,
        reads: RefCell::new(Some(super::FastSet::default())),
    };
    // The visits, each of a method in one of its contexts or (`usize::MAX`) of the code outside
    // methods, run again whenever a place that they read changes, until none does. As every
    // visit only adds to what places hold, the order does not change what they end up holding.
    let mut visits: Vec<(usize, usize)> =
        (0..class.methods.len()).flat_map(|m| values.contexts[m].iter().map(move |&ctx| (m, ctx))).collect();
    visits.push((usize::MAX, STATIC));
    let timings = crate::timings::enabled();
    if timings {
        let slots: usize =
            visits.iter().filter(|&&(m, _)| m != usize::MAX).map(|&(m, _)| values.followed[m].len()).sum();
        eprintln!(
            "timing: points-to{}: {} visits of {} methods, {slots} variable slots{}",
            if dispatch { " for dispatch" } else { "" },
            visits.len(),
            class.methods.len(),
            crate::timings::memory()
        );
    }
    let mut runs = 0usize;
    let mut queued = vec![true; visits.len()];
    let mut queue: VecDeque<usize> = (0..visits.len()).collect();
    let mut readers: super::FastMap<Read, super::FastSet<usize>> = super::FastMap::default();
    // Only the visit of a method in a context reads its variables there (`var`), so a change to
    // them runs that visit again, with no readers kept for them.
    let visit_of: super::FastMap<(usize, usize), usize> =
        visits.iter().enumerate().map(|(v, &mc)| (mc, v)).collect();
    while let Some(v) = queue.pop_front() {
        queued[v] = false;
        runs += 1;
        if timings && runs.is_multiple_of(100_000) {
            let entries: usize = readers.values().map(|r| r.len()).sum();
            let (words, held) = values.class_set_words();
            eprintln!(
                "timing: points-to: {runs} runs, {} queued, {} read places with {entries} readers, \
                 class sets of {words} words in {held}, {} variable lists, {} implementations known{}",
                queue.len(),
                readers.len(),
                values.vars.len(),
                values.types.implementations.borrow().len(),
                crate::timings::memory()
            );
        }
        let (m, ctx) = visits[v];
        let updates = match m {
            usize::MAX => visit_outside_methods(&values, class),
            _ => visit(&values, class, m, ctx, &class.methods[m].body),
        };
        let reads = values.reads.borrow_mut().replace(super::FastSet::default()).unwrap_or_default();
        if timings && (updates.len() > 200_000 || reads.len() > 200_000) {
            let name = match m {
                usize::MAX => "the code outside methods".to_string(),
                _ => format!("{}.{}", class.types[class.methods[m].class].name, class.methods[m].name),
            };
            eprintln!(
                "timing: points-to: a visit of {name} on {ctx} made {} updates and {} reads{}",
                updates.len(),
                reads.len(),
                crate::timings::memory()
            );
        }
        for read in reads {
            readers.entry(read).or_default().insert(v);
        }
        for changed in apply(&mut values, updates) {
            let own = match changed {
                Read::Vars(m, ctx) => visit_of.get(&(m, ctx)).copied(),
                _ => None,
            };
            for &r in own.iter().chain(readers.get(&changed).into_iter().flatten()) {
                if !queued[r] {
                    queued[r] = true;
                    queue.push_back(r);
                }
            }
        }
    }
    *values.reads.borrow_mut() = None;
    values
}

/// What the code outside methods stores: field initializers, the exceptions that the runtime
/// raises and, for dispatch, the constants of enums.
fn visit_outside_methods(values: &ObjectValues, class: &Class) -> Vec<Update> {
    let mut updates = Vec::new();
    for (t, decl) in class.types.iter().enumerate() {
        for (f, is_static) in
            decl.fields.iter().map(|f| (f, false)).chain(decl.statics.iter().map(|f| (f, true)))
        {
            let Some(init) = &f.init else { continue };
            // For dispatch, an instance field's initializer runs on each object that has it.
            let contexts = match is_static || !values.dispatch || values.types.concretes[t].is_empty() {
                true => vec![STATIC],
                false => values.types.concretes[t].clone(),
            };
            for ctx in contexts {
                let flow = values.types.filter(values.eval(class, usize::MAX, ctx, init), f.ty);
                updates.push(match is_static {
                    true => Update::Static(t, f.name.clone(), flow),
                    false => Update::Field(t, f.name.clone(), flow),
                });
                updates.extend(visit_value(values, class, usize::MAX, ctx, init));
            }
        }
    }
    for e in &class.raised {
        updates.extend(visit_value(values, class, usize::MAX, STATIC, e));
        if values.dispatch {
            updates.push(Update::Handed(values.eval(class, usize::MAX, STATIC, e)));
        }
    }
    // The constants of an enum set its instance fields, in declaration order.
    for (t, decl) in class.types.iter().enumerate().filter(|_| values.dispatch) {
        for constant in decl.enum_constants.iter().flatten() {
            for (field, value) in decl.fields.iter().zip(&constant.values) {
                let flow = values.types.filter(values.eval(class, usize::MAX, STATIC, value), field.ty);
                updates.push(Update::Field(t, field.name.clone(), flow));
                updates.extend(visit_value(values, class, usize::MAX, STATIC, value));
            }
        }
    }
    updates
}

/// Adds the updates to the places; the places that changed.
fn apply(values: &mut ObjectValues, updates: Vec<Update>) -> Vec<Read> {
    let mut changed = Vec::new();
    for update in updates {
        match update {
            Update::Var(m, ctx, slot, flow) => {
                let slots = values.followed[m].len();
                let vars = values.vars.entry((m, ctx)).or_insert_with(|| vec![Flow::default(); slots]);
                if vars[slot].join(&flow) {
                    changed.push(Read::Vars(m, ctx));
                }
            }
            Update::Return(m, ctx, flow) => {
                if values.returns.entry((m, ctx)).or_default().join(&flow) {
                    changed.push(Read::Return(m, ctx));
                }
            }
            Update::Field(t, name, flow) => {
                if values.fields.entry((t, name.clone())).or_default().join(&flow) {
                    changed.push(Read::Field(t, name));
                }
            }
            Update::Static(t, name, flow) => {
                if values.statics.entry((t, name.clone())).or_default().join(&flow) {
                    changed.push(Read::Static(t, name));
                }
            }
            Update::Element(ty, flow) => {
                if values.elements.entry(ty).or_default().join(&flow) {
                    changed.extend([Read::Element(ty), Read::Elements]);
                }
            }
            Update::Handed(flow) => {
                if values.handed.join(&flow) {
                    changed.push(Read::Handed);
                }
            }
        }
    }
    changed
}

/// What the statements `body` of method `m`, run on an object of class `ctx`, store.
fn visit(values: &ObjectValues, class: &Class, m: usize, ctx: usize, body: &[Stmt]) -> Vec<Update> {
    let mut out = Vec::new();
    let mut declared = Vec::new();
    for s in body {
        visit_stmt(s, &mut |e| out.extend(visit_value(values, class, m, ctx, e)), &mut |s| match s {
            Stmt::Local { ty, name, init: Some(init), .. } => {
                let flow = values.types.filter(values.eval(class, m, ctx, init), *ty);
                declared.extend(values.var_update(m, ctx, name, flow));
            }
            Stmt::ForEach { ty, name, array, .. } => {
                let flow = values.elements_of(array.ty.element());
                declared.extend(values.var_update(m, ctx, name, values.types.filter(flow, *ty)));
            }
            Stmt::Return(Some(e), _) => declared.push(Update::Return(m, ctx, values.eval(class, m, ctx, e))),
            Stmt::Throw(e) if values.dispatch => declared.push(Update::Handed(values.eval(class, m, ctx, e))),
            _ => {}
        });
    }
    out.extend(declared);
    out
}

/// What the expression `e` and the expressions in it store, in method `m` on an object of class
/// `ctx`.
fn visit_value(values: &ObjectValues, class: &Class, m: usize, ctx: usize, e: &Expr) -> Vec<Update> {
    let eval = |e: &Expr| values.eval(class, m, ctx, e);
    let mut out = Vec::new();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Assign { op, target, value } => {
            // A compound assignment stores a number or a string, whatever the place is.
            let flow = match op {
                Some(_) if values.dispatch => values.types.filter(Flow::top(), x.ty),
                Some(_) => Flow::top(),
                None => eval(value),
            };
            match target {
                Place::Var(name) => out.extend(values.var_update(m, ctx, name, flow)),
                Place::Field { object, name } => match class.field_owner(object.ty, name) {
                    Some(owner) => out.push(Update::Field(owner, name.clone(), flow)),
                    // A field the analysis cannot place: every field of that name.
                    None => {
                        for (t, d) in class.types.iter().enumerate() {
                            if d.fields.iter().any(|f| &f.name == name) {
                                out.push(Update::Field(t, name.clone(), flow.clone()));
                            }
                        }
                    }
                },
                Place::Static { class: c, name } => out.push(Update::Static(*c, name.clone(), flow)),
                Place::Index(array, _) => out.push(Update::Element(array.ty.element(), flow)),
            }
        }
        ExprKind::ArrayLit(items) => {
            for item in items {
                out.push(Update::Element(x.ty.element(), eval(item)));
            }
        }
        ExprKind::InstanceOf { value, ty, binding: Some(name) } => {
            out.extend(values.var_update(m, ctx, name, values.types.filter(eval(value), *ty)));
        }
        ExprKind::Call { args, .. }
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. }
        | ExprKind::MethodCall { args, .. } => {
            let flows: Vec<Flow> = args.iter().map(eval).collect();
            // What each parameter of a target method gets, by its slot: the same for each class
            // that the method runs on.
            let mut passed: super::FastMap<usize, Vec<(usize, Flow)>> = super::FastMap::default();
            for (t, c) in values.targets(class, m, ctx, x) {
                let params = passed.entry(t).or_insert_with(|| {
                    let followed = &values.followed[t];
                    let params = class.methods[t].params.iter().zip(&flows);
                    params
                        .filter_map(|(p, flow)| {
                            Some((*followed.get(&p.name)?, values.types.filter(flow.clone(), p.ty)))
                        })
                        .collect()
                });
                out.extend(params.iter().map(|(slot, flow)| Update::Var(t, c, *slot, flow.clone())));
            }
        }
        // A library function may store its arguments, or the elements of an array it is given,
        // into the arrays it is given.
        ExprKind::Builtin(_, args) if args.iter().any(|a| matches!(a.ty, Type::Array(_))) => {
            let mut flow = Flow::default();
            for a in args {
                match a.ty {
                    Type::Array(element) => flow.join(&values.elements_of(*element)),
                    _ => flow.join(&eval(a)),
                };
            }
            for a in args {
                if let Type::Array(element) = a.ty {
                    out.push(Update::Element(*element, flow.clone()));
                }
            }
        }
        _ => {}
    });
    if values.dispatch {
        visit_expr(e, &mut |x| {
            out.extend(handed(values, class, m, ctx, x).into_iter().map(|(f, _)| Update::Handed(f)))
        });
    }
    out
}

/// What the runtime is handed by the expression `x` itself, not counting the expressions inside
/// it: the arguments of a builtin that may hold them (`hands_objects`), and the operands of a
/// string concatenation.
pub(super) fn handed(
    values: &ObjectValues,
    class: &Class,
    m: usize,
    ctx: usize,
    x: &Expr,
) -> Vec<(Flow, Type)> {
    let operands: Vec<&Expr> = match &x.kind {
        ExprKind::Builtin(builtin, args) if hands_objects(builtin) => args.iter().collect(),
        ExprKind::Binary(_, l, r) if x.ty == Type::String => vec![l, r],
        ExprKind::Assign { op: Some(_), value, .. } if x.ty == Type::String => vec![value],
        _ => Vec::new(),
    };
    operands.into_iter().map(|a| handed_by(values, class, m, ctx, a)).collect()
}

/// The variables of `method` that the analysis follows, its parameters among them when `params`.
/// A name that a catch clause binds anywhere in the method is left to be anything.
fn followed(method: &Method, params: bool) -> super::FastMap<String, usize> {
    let mut names: HashSet<String> =
        if params { method.params.iter().map(|p| p.name.clone()).collect() } else { HashSet::new() };
    let mut bindings = Vec::new();
    let mut caught = Vec::new();
    for s in &method.body {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::InstanceOf { binding: Some(name), .. } = &x.kind {
                        bindings.push(name.clone());
                    }
                })
            },
            &mut |s| match s {
                Stmt::Local { name, .. } | Stmt::ForEach { name, .. } => _ = names.insert(name.clone()),
                Stmt::Try { catches, .. } => caught.extend(catches.iter().map(|c| c.name.clone())),
                _ => {}
            },
        );
    }
    names.extend(bindings);
    for name in caught {
        names.remove(&name);
    }
    let mut names: Vec<String> = names.into_iter().collect();
    names.sort();
    names.into_iter().enumerate().map(|(slot, name)| (name, slot)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_sets_far_apart_join_and_compare() {
        let mut far = ClassSet::from_iter([20_000]);
        assert_eq!(far.words.len(), 1);
        assert!(far.union(&ClassSet::from_iter([3, 20_001])));
        assert!(!far.union(&ClassSet::from_iter([3])));
        assert!(!far.union(&ClassSet::default()));
        assert_eq!(far.iter().collect::<Vec<_>>(), vec![3, 20_000, 20_001]);
        assert_eq!(far, ClassSet::from_iter([20_001, 3, 20_000]));
        far.intersect(&ClassSet::from_iter([20_000, 70]));
        assert_eq!(far, ClassSet::from_iter([20_000]));
        assert_ne!(far, ClassSet::default());
    }
}
