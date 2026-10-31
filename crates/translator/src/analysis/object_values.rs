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
//!   `String[]`, `T[]` and its argument's). The `Class` values in arrays are kept apart by where
//!   each array was made (`sites`): an array that may hold them (`Class[]`, `Type[]`,
//!   `Object[]`, `Class[][]` and the arrays inside it) is followed as a value from the expression
//!   that makes it, a copy of one (`clone()`, `Arrays.copyOf`) as the array itself, and a read of
//!   an element of an array known to be made at some places gives the `Class` values stored in
//!   arrays made there. An array that the analysis does not follow (a library's result, a
//!   variable it does not follow) may have been made anywhere: a read of it gives every `Class`
//!   value stored in any array, and what is stored through it (`loose`) goes to every array that
//!   such code may hold (`escaped`): those that the runtime or an unresolved call is given, and
//!   the arrays in those. As for objects, the runtime is taken to reach no array but those it is
//!   given and the arrays in them, not those in the fields of the objects it is given.
//! - An instance method is followed for every concrete class under its own, as `super.m()` runs
//!   it on objects of a class that overrides it, and so is an instance field's initializer.
//! - The constants of an enum set its instance fields.
//! - The parameters of a method that the runtime may call itself (`runtime_called`) hold what
//!   calls in the code pass them and what the runtime holds (`handed`): the objects it was given.
//! - `Class` values: those of class literals of the program's types, `getClass()` of the objects
//!   followed, and `getSuperclass()`, `getEnclosingClass()` and `getDeclaringClass()` of the
//!   classes followed; anything else may be the `Class` of any class. They give the classes that
//!   the lookups of the reflection table look in (`live_slot`), and the code that the table
//!   gives only gives what a member that a lookup may find gives: the classes in its part's
//!   data that those members name, and the cases of the `switch`es over its slots of those
//!   members (`analysis::reflection_table`).
//! - "Any class its type allows" is spelled out as those classes where the type is not open
//!   (`Object`, a type variable), so that it stays within the type wherever the value goes. A
//!   compound assignment stores a number or a string; a switch expression is one of the values it
//!   yields; a value checked for null or copied by `clone()` is the value given; what ends the
//!   program has none.

use super::Facts;
use super::{FastMap, FastSet};
use crate::ast::*;
use std::cell::RefCell;
use std::collections::VecDeque;
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

/// The types of the program that an array is a value of.
const ARRAY_SUPERTYPES: [&str; 4] =
    [OBJECT, "java.lang.Object", "java.lang.Cloneable", "java.io.Serializable"];

/// What a place or an expression may hold.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub(super) struct Flow {
    /// A string, a box or a primitive held as an object.
    value: bool,
    /// Objects of any class its type allows; `classes` lists them otherwise.
    pub any: bool,
    pub classes: ClassSet,
    /// `Class` values of any class; `class_values` lists them otherwise, by type index. The
    /// `Class` of a type that is not one of the program's (a primitive type, an array, a class of
    /// the runtime) is in neither: the reflection table has no members of it.
    pub any_class: bool,
    pub class_values: ClassSet,
    /// For dispatch, arrays that may hold `Class` values (`Types::holds_class_values`) made
    /// anywhere; `arrays` lists them otherwise, by their places (`ObjectValues::sites`).
    pub any_array: bool,
    pub arrays: ClassSet,
}

impl Flow {
    fn top() -> Flow {
        Flow { value: true, any: true, any_class: true, any_array: true, ..Flow::default() }
    }

    /// The array made at the place `site`.
    fn array(site: usize) -> Flow {
        let mut arrays = ClassSet::default();
        arrays.insert(site);
        Flow { arrays, ..Flow::default() }
    }

    /// Only the `Class` values and the arrays of this flow, what the places of arrays made at a
    /// place keep (`ObjectValues::site_elements`).
    fn class_part(&self) -> Flow {
        Flow {
            any_class: self.any_class,
            class_values: self.class_values.clone(),
            any_array: self.any_array,
            arrays: self.arrays.clone(),
            ..Flow::default()
        }
    }

    /// This flow without its `Class` values and arrays.
    fn object_part(self) -> Flow {
        Flow { value: self.value, any: self.any, classes: self.classes, ..Flow::default() }
    }

    /// The `Class` values of `class`.
    fn class_value(class: usize) -> Flow {
        let mut class_values = ClassSet::default();
        class_values.insert(class);
        Flow { class_values, ..Flow::default() }
    }

    /// Only the `Class` values of this flow.
    fn class_values(&self) -> Flow {
        Flow { any_class: self.any_class, class_values: self.class_values.clone(), ..Flow::default() }
    }

    fn of(class: usize) -> Flow {
        let mut classes = ClassSet::default();
        classes.insert(class);
        Flow { classes, ..Flow::default() }
    }

    /// Adds what `other` may hold; whether that changed anything.
    fn join(&mut self, other: &Flow) -> bool {
        let before = (self.value, self.any, self.any_class, self.any_array);
        self.value |= other.value;
        self.any |= other.any;
        self.any_class |= other.any_class;
        self.any_array |= other.any_array;
        let grew = !self.any && self.classes.union(&other.classes);
        let grew_classes = !self.any_class && self.class_values.union(&other.class_values);
        let grew_arrays = !self.any_array && self.arrays.union(&other.arrays);
        grew || grew_classes
            || grew_arrays
            || before != (self.value, self.any, self.any_class, self.any_array)
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

/// Whether every bit of `words` is also in `of`, a slice as long: a block of words at a time,
/// without a branch inside a block, so that the check vectorizes. The sets of every class of a
/// type are thousands of words, and most unions add nothing.
fn within(words: &[u64], of: &[u64]) -> bool {
    let ((blocks, tail), (of_blocks, of_tail)) = (words.as_chunks::<8>(), of.as_chunks::<8>());
    tail.iter().zip(of_tail).all(|(w, o)| w & !o == 0)
        && blocks
            .iter()
            .zip(of_blocks)
            .all(|(w, o)| w.iter().zip(o).fold(0, |outside, (w, o)| outside | w & !o) == 0)
}

/// Whether `words` are all zero, a block at a time as `within`.
fn empty(words: &[u64]) -> bool {
    let (blocks, tail) = words.as_chunks::<8>();
    tail.iter().all(|&w| w == 0) && blocks.iter().all(|b| b.iter().fold(0, |any, w| any | w) == 0)
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

    /// Whether class `c` is in the set.
    pub(super) fn contains(&self, c: usize) -> bool {
        self.word(c / 64) >> (c % 64) & 1 != 0
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
        let Some(first) = other.words.iter().position(|&w| w != 0) else { return false };
        let last = other.words.iter().rposition(|&w| w != 0).unwrap_or(first) + 1;
        // The words of `other` from its first class to its last, at `from..to` of all words: both
        // ends hold a class, so where this set does not cover them it lacks one.
        let theirs = &other.words[first..last];
        let (from, to) = (other.start + first, other.start + last);
        let (start, end) = (self.start, self.start + self.words.len());
        if start <= from && to <= end && within(theirs, &self.words[from - start..to - start]) {
            return false;
        }
        // Where `other` holds every class of this set, this set becomes a copy of it.
        if self.within(other) {
            *self = other.clone();
            return true;
        }
        self.cover(from, to);
        let start = self.start;
        let words = Rc::make_mut(&mut self.words);
        for (m, t) in words[from - start..to - start].iter_mut().zip(theirs) {
            *m |= t;
        }
        true
    }

    /// Whether every class of `other` is in this set.
    pub(super) fn holds(&self, other: &ClassSet) -> bool {
        other.within(self)
    }

    /// Whether every class of this set is in `other`.
    fn within(&self, other: &ClassSet) -> bool {
        let (start, end) = (self.start, self.start + self.words.len());
        let (low, high) = (start.max(other.start), end.min(other.start + other.words.len()));
        if low >= high {
            return empty(&self.words);
        }
        empty(&self.words[..low - start])
            && empty(&self.words[high - start..])
            && within(
                &self.words[low - start..high - start],
                &other.words[low - other.start..high - other.start],
            )
    }

    /// Keeps the classes that are also in `other`.
    fn intersect(&mut self, other: &ClassSet) {
        if self.within(other) {
            return;
        }
        let start = self.start;
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
        if self.start == other.start && Rc::ptr_eq(&self.words, &other.words) {
            return true;
        }
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
    vars: FastMap<(usize, usize), Vec<Flow>>,
    returns: FastMap<(usize, usize), Flow>,
    /// Instance fields, by the class that declares them.
    fields: FastMap<(usize, String), Flow>,
    statics: FastMap<(usize, String), Flow>,
    elements: FastMap<Type, Flow>,
    /// What the elements of every array hold together, for dispatch (`elements_of`), joined as
    /// they grow: a read of an array element would otherwise join every place of `elements`,
    /// thousands on the Minecraft server, and a great many visits read one.
    all_elements: Flow,
    /// For dispatch, the place of each array that may hold `Class` values
    /// (`Types::holds_class_values`), by the expression that makes it and, for the arrays inside
    /// a `new T[a][b]`, how deep it is (0 for the outer one).
    sites: FastMap<(ExprId, usize), usize>,
    /// The `Class` values and the arrays that code stores in the arrays made at each place
    /// (`Flow::class_part`): a read of an element of those arrays gives these alone, with
    /// `loose`, where the analysis knows where its array was made (`element_of`).
    site_elements: Vec<Flow>,
    /// The `Class` values and arrays stored in arrays that may have been made anywhere: through
    /// an array that the analysis does not follow, and by the runtime into the arrays it is
    /// given.
    loose: Flow,
    /// The places of the arrays that code the analysis does not follow may hold (`escape`): those
    /// given to the runtime or to an unresolved call, the arrays in them, and those stored in
    /// arrays that may have been made anywhere. Only these may be an array that the analysis
    /// does not know where it was made, so only their elements may hold what is stored through
    /// one (`loose`).
    escaped: ClassSet,
    /// The variables of each method that the analysis follows, each with its slot in `vars`: its
    /// parameters (unless the runtime may call it) and the variables it declares, loops over or
    /// binds with a pattern.
    followed: Vec<FastMap<String, usize>>,
    /// The slot in `vars` of each parameter of each method, where the analysis follows it.
    param_slots: Vec<Vec<Option<usize>>>,
    /// For dispatch, the parameters of each method that the runtime may call itself
    /// (`runtime_called`), which hold what the runtime holds besides what calls pass them.
    runtime_params: Vec<Vec<String>>,
    /// For dispatch, the objects that the runtime holds: those given to a builtin
    /// (`hands_objects`), the operands of a string concatenation, and the exceptions thrown.
    handed: Flow,
    /// The classes whose objects run each method, `STATIC` for one without `this`.
    pub(super) contexts: Vec<Vec<usize>>,
    /// For dispatch, the `Class` values that the lookups of the reflection table get, by kind
    /// (`table_kind`): the classes they look members up in (`ReflectionTable::lookups`).
    receivers: [Flow; 3],
    /// For dispatch, the kind of each lookup of the reflection table, by its method and the slot
    /// of its `Class` parameter.
    lookup_params: FastMap<(usize, usize), TableKind>,
    types: Types,
    /// The analysis for the calls that can run (see the module's comment).
    dispatch: bool,
    /// While the analysis runs, the places that the current visit has read.
    reads: RefCell<Option<FastSet<Read>>>,
    /// While the analysis runs, what each call on a receiver of several classes in each context
    /// was last found to run, and what it passed them (`Passed`).
    calls: RefCell<Option<FastMap<(ExprId, usize), Passed>>>,
    /// While the analysis runs, the returns of the targets of calls that run many methods.
    joined: RefCell<Joined>,
}

/// What a call on a receiver of several classes, in a context, was last found to run: the
/// targets that `targets` found for the receiver's flow, and, once a visit passed them
/// arguments, the flows it passed. A visit runs again whenever any place it read grows, and most
/// of its calls then have the receiver and the arguments they had: the call runs the same
/// targets, and passing its targets what they were passed before changes nothing, as places
/// only grow. On the Minecraft server a call may run thousands of methods.
struct Passed {
    receiver: Flow,
    targets: Targets,
    args: Option<Vec<Flow>>,
    /// Once the receiver grew, the targets that a visit last passed arguments, and those
    /// arguments: they need only what changed since.
    before: Option<(Targets, Vec<Flow>)>,
    /// The number of the join of the targets' returns (`Joined`), once a visit evaluated the call.
    joined: Option<u32>,
}

/// The fewest targets for which a call's are kept (`Passed`): for fewer, finding them again
/// costs about as much as comparing the flows.
const KEPT_TARGETS: usize = 4;

/// The returns of the targets of calls that run many methods, each list of targets joined into
/// a place of its own, kept up as the returns grow (`apply`). A call that runs thousands of
/// methods is evaluated in a great many visits, each of which would read and join the return of
/// every target, and run again whenever any of them grew; it reads the join instead, and runs
/// again only when the join grows.
#[derive(Default)]
struct Joined {
    /// The number of each list of targets that has a join, by the list's content.
    lists: FastMap<Targets, u32>,
    flows: Vec<Flow>,
    /// The joins that each target's return is in.
    of: FastMap<(usize, usize), Vec<u32>>,
}

/// The fewest targets whose returns are joined (`Joined`).
const JOINED_TARGETS: usize = 16;

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
    /// The `Class` values and arrays in the arrays made at a place (`ObjectValues::site_elements`).
    Site(usize),
    /// Those stored in arrays made anywhere (`ObjectValues::loose`).
    Loose,
    /// The `Class` values that the lookups of a kind get (`ObjectValues::receivers`).
    Receivers(TableKind),
    Handed,
    /// The returns of a list of targets, by the number of their join (`Joined`).
    Joined(u32),
}

/// What the analysis needs to know of the program's types.
struct Types {
    index: FastMap<String, usize>,
    /// Types no string or box is a value of.
    excluded: Vec<bool>,
    /// Types that objects of every class may be values of: `Object` and its kin, and type
    /// variables.
    open: Vec<bool>,
    /// Types that a `Class` value may be a value of: the open ones and every interface, as
    /// `Class` implements `Type`, `GenericDeclaration`, `AnnotatedElement` and more.
    holds_class: Vec<bool>,
    /// Types that an array may be a value of: `Object`, `Cloneable`, `Serializable` and type
    /// variables.
    holds_array: Vec<bool>,
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
    declared: FastMap<(usize, usize), usize>,
    implementations: RefCell<FastMap<(usize, usize), Option<usize>>>,
    /// The targets of a call of each override group on every class of a list of `concretes_of`,
    /// by group and the list's address: a call on a value that may be of any class its type
    /// allows is visited many times, and the list may be every class of the program.
    any_targets: RefCell<FastMap<(usize, usize), Targets>>,
}

/// The implementations that a call runs, each with the class of its object.
type Targets = Rc<[(usize, usize)]>;

impl Types {
    fn new(class: &Class, bounded: bool) -> Types {
        let n = class.types.len();
        let index = class.types.iter().enumerate().map(|(t, d)| (d.name.clone(), t)).collect();
        let open: Vec<bool> = class
            .types
            .iter()
            .map(|d| d.kind == TypeKind::Param || HOLDING_VALUES.contains(&d.name.as_str()))
            .collect();
        let holds_class =
            class.types.iter().enumerate().map(|(t, d)| open[t] || d.kind == TypeKind::Interface).collect();
        let holds_array = class
            .types
            .iter()
            .map(|d| d.kind == TypeKind::Param || ARRAY_SUPERTYPES.contains(&d.name.as_str()))
            .collect();
        let excluded = class
            .types
            .iter()
            .enumerate()
            .map(|(t, d)| {
                !open[t] && matches!(d.kind, TypeKind::Class | TypeKind::Interface | TypeKind::Enum)
            })
            .collect();
        let mut supers = vec![FastSet::default(); n];
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
        let mut declared = FastMap::default();
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
            holds_class,
            holds_array,
            concretes,
            concrete_sets,
            subtypes,
            all_concretes,
            declared,
            implementations: RefCell::default(),
            any_targets: RefCell::default(),
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
        let mut seen = FastSet::default();
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
    /// The implementations, each with its class, that the classes of `concretes`, a list of
    /// `concretes_of`, run for a call of a method of override group `group`.
    fn targets_on(&self, class: &Class, group: usize, concretes: &[usize]) -> Targets {
        let key = (group, concretes.as_ptr() as usize);
        if let Some(found) = self.any_targets.borrow().get(&key) {
            return found.clone();
        }
        let found: Targets =
            concretes.iter().filter_map(|&c| Some((self.implementation(class, group, c)?, c))).collect();
        self.any_targets.borrow_mut().insert(key, found.clone());
        found
    }

    fn concretes_of(&self, ty: Type) -> &[usize] {
        match ty {
            Type::Class(name, _) | Type::Enum(name) => match self.index.get(name) {
                Some(&t) if !self.open[t] => &self.concretes[t],
                _ => &self.all_concretes,
            },
            _ => &[],
        }
    }

    /// Whether a value of type `ty` may be a `Class`, or an array of those (`Class[][]` too):
    /// for dispatch, the arrays of such a type have places (`ObjectValues::sites`).
    fn holds_class_values(&self, ty: Type) -> bool {
        match ty {
            Type::JavaClass => true,
            Type::Class(name, _) => self.index.get(name).is_none_or(|&t| self.holds_class[t]),
            Type::Array(element) => self.holds_class_values(*element),
            _ => false,
        }
    }

    /// Whether a value of type `ty` may be an array that may hold `Class` values.
    fn may_be_array(&self, ty: Type) -> bool {
        match ty {
            Type::Array(element) => self.bounded && self.holds_class_values(*element),
            Type::Class(name, _) => self.index.get(name).is_none_or(|&t| self.holds_array[t]),
            _ => false,
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
                    if !self.holds_class[t] {
                        flow.any_class = false;
                        flow.class_values = ClassSet::default();
                    }
                    if !self.holds_array[t] {
                        flow.any_array = false;
                        flow.arrays = ClassSet::default();
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
            Type::JavaClass => flow.class_values(),
            Type::Array(element) if self.bounded && self.holds_class_values(*element) => {
                Flow { any_array: flow.any_array, arrays: flow.arrays, ..Flow::default() }
            }
            Type::Null | Type::Void | Type::Array(_) | Type::StringBuilder => Flow::default(),
            _ => Flow { value: true, ..Flow::default() },
        }
    }
}

/// The index of the table of `kind` in `ObjectValues::receivers`.
fn table_kind(kind: TableKind) -> usize {
    match kind {
        TableKind::Method => 0,
        TableKind::Field => 1,
        TableKind::Constructor => 2,
    }
}

impl ObjectValues {
    /// Whether a lookup may find the member in `slot` of the reflection table of `kind`: one
    /// that no class finds, or one whose class finds it and is among the `Class` values that the
    /// lookups of its kind get. In the analysis for dispatch only; otherwise every member.
    pub(super) fn live_slot(&self, class: &Class, kind: TableKind, slot: i32) -> bool {
        let Some(&Some(found_on)) = class.reflection_table.found_on.get(&(kind, slot)) else { return true };
        if !self.dispatch {
            return true;
        }
        self.read(|| Read::Receivers(kind));
        let receivers = &self.receivers[table_kind(kind)];
        receivers.any_class || receivers.class_values.contains(found_on)
    }

    /// Whether the case `body` of a `switch` may run: not one of a slot of the reflection table
    /// that no lookup finds (`live_slot`).
    pub(super) fn live_case(&self, class: &Class, body: &[Stmt]) -> bool {
        match body.last() {
            Some(Stmt::Yield(last)) => match class.reflection_table.cases.get(&last.id) {
                Some(&(kind, slot)) => self.live_slot(class, kind, slot),
                None => true,
            },
            _ => true,
        }
    }

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
        let mut seen = FastSet::default();
        let (mut words, mut held) = (0, 0);
        let flows = self.vars.values().flatten().chain(self.returns.values()).chain(self.fields.values());
        for flow in flows.chain(self.statics.values()).chain(self.elements.values()) {
            words += flow.classes.words.len();
            if seen.insert(Rc::as_ptr(&flow.classes.words) as usize) {
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
        self.types.filter(self.all_elements.clone(), element)
    }

    /// What a read of an element of `array`, evaluated in method `m` on an object of class
    /// `ctx`, may give: `elements_of` its type, but for dispatch, where the analysis knows where
    /// the array was made, its `Class` values and arrays are only those stored in arrays made
    /// there (`site_elements`) or in arrays that may have been made anywhere (`loose`).
    pub(super) fn element_of(&self, class: &Class, m: usize, ctx: usize, array: &Expr) -> Flow {
        let element = array.ty.element();
        let all = self.elements_of(element);
        if !self.dispatch || !self.types.holds_class_values(element) {
            return all;
        }
        self.elements_in(&self.eval(class, m, ctx, array), element, all)
    }

    /// `element_of` an array that may be what `made` holds, of element type `element`, whose
    /// elements of that type hold `all` (`elements_of`).
    fn elements_in(&self, made: &Flow, element: Type, all: Flow) -> Flow {
        if made.any_array {
            return all;
        }
        let mut flow = all.object_part();
        let mut escaped = false;
        for site in made.arrays.iter() {
            self.read(|| Read::Site(site));
            flow.join(&self.site_elements[site]);
            escaped |= self.escaped.contains(site);
        }
        if escaped {
            self.read(|| Read::Loose);
            flow.join(&self.loose);
        }
        self.types.filter(flow, element)
    }

    /// The updates that a store of `flow` into an element of `array`, evaluated in method `m` on
    /// an object of class `ctx`, makes: to the elements of its type and, for dispatch, of the
    /// `Class` values and arrays in it, to the arrays that the analysis knows it was made at, or
    /// to `loose` where it may have been made anywhere.
    fn element_updates(&self, class: &Class, m: usize, ctx: usize, array: &Expr, flow: Flow) -> Vec<Update> {
        let element = array.ty.element();
        let mut out = Vec::new();
        if self.dispatch && self.types.holds_class_values(element) {
            out = stores_into(&self.eval(class, m, ctx, array), &flow);
        }
        out.push(Update::Element(element, flow));
        out
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
            // An array that may hold `Class` values without a place (none in the code that
            // `array_sites` goes over) may be any.
            ExprKind::NewArray(_) | ExprKind::ArrayLit(_) => match self.sites.get(&(e.id, 0)) {
                Some(&site) => Flow::array(site),
                None => Flow { any_array: true, ..Flow::default() },
            },
            ExprKind::Null => Flow::default(),
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
            ExprKind::Index(array, _) => self.element_of(class, m, ctx, array),
            ExprKind::Call { resolved: Some(_), .. } | ExprKind::MethodCall { resolved: Some(_), .. } => {
                let targets = self.call_targets(class, m, ctx, e);
                if let Some(id) = self.joined_returns(ctx, e, &targets) {
                    self.read(|| Read::Joined(id));
                    return self.types.filter(self.joined.borrow().flows[id as usize].clone(), e.ty);
                }
                let mut flow = Flow::default();
                for target in targets.iter() {
                    self.read(|| Read::Return(target.0, target.1));
                    flow.join(self.returns.get(target).unwrap_or(&Flow::default()));
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
            // A copy of an array holds what the array holds, as if it were the array.
            ExprKind::Builtin(
                Builtin::ArraysCopyOf | Builtin::Arrays(ArraysOp::CopyOfRange | ArraysOp::Clone),
                args,
            ) if self.dispatch && !args.is_empty() => self.eval(class, m, ctx, &args[0]),
            ExprKind::Builtin(Builtin::ClassLiteral(ty), _) => match ty {
                Type::Class(name, _) | Type::Enum(name) => {
                    self.types.index.get(*name).map_or(Flow::default(), |&t| Flow::class_value(t))
                }
                _ => Flow::default(),
            },
            // The classes of the objects, or any class where those may be of any.
            ExprKind::Builtin(Builtin::GetClass, args) if args.len() == 1 => {
                let of = self.eval(class, m, ctx, &args[0]);
                Flow { any_class: of.any, class_values: of.classes, ..Flow::default() }
            }
            // An enum's class is a type of the program, a constant's body's class not.
            ExprKind::Builtin(Builtin::ClassMethod(ClassOp::EnumDeclaringClass), args) => {
                self.eval(class, m, ctx, &args[0])
            }
            ExprKind::Builtin(
                Builtin::ClassMethod(
                    op @ (ClassOp::GetSuperclass | ClassOp::GetEnclosingClass | ClassOp::GetDeclaringClass),
                ),
                args,
            ) if !args.is_empty() => {
                let of = self.eval(class, m, ctx, &args[0]);
                let mut class_values = ClassSet::default();
                for t in of.class_values.iter() {
                    let next = match op {
                        ClassOp::GetSuperclass => class.types[t].extends,
                        _ => class.types[t].enclosing,
                    };
                    if let Some(next) = next {
                        class_values.insert(next);
                    }
                }
                Flow { any_class: of.any_class, class_values, ..Flow::default() }
            }
            // For dispatch, a switch expression is one of the values it yields, but a case of a
            // member of the reflection table that no lookup finds (`live_slot`).
            ExprKind::Switch { cases, .. } if self.dispatch => {
                let mut flow = Flow::default();
                let live = cases.iter().filter(|c| self.live_case(class, &c.body));
                for s in live.flat_map(|c| &c.body) {
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
    pub(super) fn targets(&self, class: &Class, m: usize, ctx: usize, e: &Expr) -> Targets {
        match self.receiver(class, m, ctx, e) {
            Some(flow) => self.receiver_targets(class, e, &flow),
            None => self.fixed_targets(class, ctx, e),
        }
    }

    /// `targets`, while the analysis runs: those kept for the call (`Passed`) while its receiver
    /// holds what it held when they were found.
    fn call_targets(&self, class: &Class, m: usize, ctx: usize, e: &Expr) -> Targets {
        let Some(flow) = self.receiver(class, m, ctx, e) else { return self.fixed_targets(class, ctx, e) };
        if let Some(kept) = self.calls.borrow().as_ref().and_then(|calls| calls.get(&(e.id, ctx)))
            && kept.receiver == flow
        {
            return kept.targets.clone();
        }
        let targets = self.receiver_targets(class, e, &flow);
        if targets.len() >= KEPT_TARGETS
            && let Some(calls) = self.calls.borrow_mut().as_mut()
        {
            let before = calls.remove(&(e.id, ctx)).and_then(|old| match old.args {
                Some(args) => Some((old.targets, args)),
                None => old.before,
            });
            let kept = Passed { receiver: flow, targets: targets.clone(), args: None, before, joined: None };
            calls.insert((e.id, ctx), kept);
        }
        targets
    }

    /// While the analysis runs, the number of the join of the returns of `targets`, those of the
    /// call `e` on an object of class `ctx`, where it has one (`Joined`).
    fn joined_returns(&self, ctx: usize, e: &Expr, targets: &Targets) -> Option<u32> {
        if targets.len() < JOINED_TARGETS {
            return None;
        }
        let mut calls = self.calls.borrow_mut();
        let kept = calls.as_mut()?.get_mut(&(e.id, ctx)).filter(|kept| Rc::ptr_eq(&kept.targets, targets))?;
        if let Some(id) = kept.joined {
            return Some(id);
        }
        let mut joined = self.joined.borrow_mut();
        let id = match joined.lists.get(targets) {
            Some(&id) => id,
            None => {
                let id = joined.flows.len() as u32;
                let mut flow = Flow::default();
                for target in targets.iter() {
                    flow.join(self.returns.get(target).unwrap_or(&Flow::default()));
                    joined.of.entry(*target).or_default().push(id);
                }
                joined.flows.push(flow);
                joined.lists.insert(targets.clone(), id);
                id
            }
        };
        kept.joined = Some(id);
        Some(id)
    }

    /// What the receiver of the call `e` in method `m` on an object of class `ctx` may be, where
    /// the call runs the method of the receiver's class: `None` for a call whose target is fixed.
    fn receiver(&self, class: &Class, m: usize, ctx: usize, e: &Expr) -> Option<Flow> {
        let ExprKind::MethodCall { resolved: Some(r), is_super, receiver, .. } = &e.kind else { return None };
        if class.methods[*r].is_static || *is_super && matches!(receiver.kind, ExprKind::This) {
            return None;
        }
        Some(self.eval(class, m, ctx, receiver))
    }

    /// The method that the call `e` on an object of class `ctx` runs, where it does not depend on
    /// a receiver (`receiver`), with the class of the object it runs on.
    fn fixed_targets(&self, class: &Class, ctx: usize, e: &Expr) -> Targets {
        let context = |r: usize, ctx: usize| if class.methods[r].is_static { STATIC } else { ctx };
        match &e.kind {
            ExprKind::Call { resolved: Some(r), .. } => Rc::new([(*r, context(*r, ctx))]),
            ExprKind::New { class: c, ctor: Some(r), .. } => Rc::new([(*r, *c)]),
            ExprKind::ThisCall { resolved: Some(r), .. } | ExprKind::SuperInit { resolved: Some(r), .. } => {
                Rc::new([(*r, ctx)])
            }
            ExprKind::MethodCall { resolved: Some(r), .. } => Rc::new([(*r, context(*r, ctx))]),
            _ => Rc::new([]),
        }
    }

    /// The methods that the call `e` runs on a receiver that may be what `flow` holds.
    fn receiver_targets(&self, class: &Class, e: &Expr, flow: &Flow) -> Targets {
        let ExprKind::MethodCall { resolved: Some(r), is_super, receiver, .. } = &e.kind else {
            unreachable!("a call on a receiver")
        };
        if flow.any && !*is_super {
            let concretes = self.types.concretes_of(receiver.ty);
            return self.types.targets_on(class, class.hierarchy.group_id(*r), concretes);
        }
        let classes: Vec<usize> = if flow.any {
            self.types.concretes_of(receiver.ty).to_vec()
        } else {
            flow.classes.iter().collect()
        };
        // A `super` call in a lambda's code is on the object that the lambda captured, and runs
        // the method it names on it.
        if *is_super {
            return classes.into_iter().map(|c| (*r, c)).collect();
        }
        let group = class.hierarchy.group_id(*r);
        classes.into_iter().filter_map(|c| Some((self.types.implementation(class, group, c)?, c))).collect()
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
    /// The `Class` values and arrays stored in the arrays made at a place (`site_elements`).
    Site(usize, Flow),
    /// Those stored in arrays that may have been made anywhere (`loose`).
    Loose(Flow),
    /// The arrays of a flow, which code that the analysis does not follow is given (`escaped`).
    Escape(ClassSet),
    /// Objects that the runtime is handed (see `handed`).
    Handed(Flow),
}

/// The override groups of the methods that the runtime may call itself on an object it holds,
/// such as `equals` from a library's map or `toString` to print it: those of `Object` and of the
/// JDK interfaces and classes that strings and boxes implement.
pub(super) fn runtime_called(class: &Class) -> FastSet<usize> {
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
pub(super) fn runtime_called_for_dispatch(class: &Class) -> FastSet<usize> {
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
        Type::Array(_) => {
            // The runtime may also hand back the array itself, or the arrays in it.
            let mut flow = values.elements_of(ty);
            flow.any_array |= values.dispatch && values.types.holds_class_values(e.ty);
            (flow, ty)
        }
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
    let followed: Vec<FastMap<String, usize>> = class
        .methods
        .iter()
        .enumerate()
        .map(|(m, method)| followed(method, dispatch || !by_runtime(m)))
        .collect();
    let param_slots = class
        .methods
        .iter()
        .zip(&followed)
        .map(|(method, followed)| method.params.iter().map(|p| followed.get(&p.name).copied()).collect())
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
    let sites = if dispatch { array_sites(class, &types) } else { FastMap::default() };
    let mut values = ObjectValues {
        vars: FastMap::default(),
        returns: FastMap::default(),
        fields: FastMap::default(),
        statics: FastMap::default(),
        elements: FastMap::default(),
        all_elements: Flow::default(),
        site_elements: vec![Flow::default(); sites.len()],
        sites,
        loose: Flow::default(),
        escaped: ClassSet::default(),
        param_slots,
        followed,
        runtime_params,
        handed: Flow::default(),
        contexts,
        receivers: Default::default(),
        lookup_params: FastMap::default(),
        types,
        dispatch,
        reads: RefCell::new(Some(FastSet::default())),
        calls: RefCell::new(Some(FastMap::default())),
        joined: RefCell::default(),
    };
    // The lookups of the reflection table get what their `Class` parameter holds, and anything
    // where the analysis does not follow it.
    for &(kind, m) in class.reflection_table.lookups.iter().filter(|_| dispatch) {
        match values.param_slots[m].first() {
            Some(&Some(slot)) if values.runtime_params[m].is_empty() => {
                values.lookup_params.insert((m, slot), kind);
            }
            _ => values.receivers[table_kind(kind)].any_class = true,
        }
    }
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
    // The visits whose last run was huge (reflection's `Method.invoke` runs every method of the
    // table), queued apart and run again only when no other visit is queued: each run then takes
    // in what many others changed, and the order does not change what the places end up holding.
    let mut huge = vec![false; visits.len()];
    let mut later: VecDeque<usize> = VecDeque::new();
    // The visits that read each place, by the place's number (`read_ids`), and the places that
    // each visit is noted as reading: a run notes only what it reads anew, in its own small set,
    // rather than every place it reads in the set of that place's readers.
    let mut read_ids: FastMap<Read, u32> = FastMap::default();
    let mut readers: Vec<Vec<u32>> = Vec::new();
    let mut registered: Vec<FastSet<u32>> = vec![FastSet::default(); visits.len()];
    // Only the visit of a method in a context reads its variables there (`var`), so a change to
    // them runs that visit again, with no readers kept for them.
    let visit_of: FastMap<(usize, usize), usize> =
        visits.iter().enumerate().map(|(v, &mc)| (mc, v)).collect();
    while let Some(v) = queue.pop_front().or_else(|| later.pop_front()) {
        queued[v] = false;
        runs += 1;
        if timings && runs.is_multiple_of(100_000) {
            let entries: usize = readers.iter().map(|r| r.len()).sum();
            // The class sets are counted on every tenth line only: that goes over every place, as
            // long as a few hundred thousand runs on the Minecraft server.
            let sets = match runs.is_multiple_of(1_000_000) {
                true => {
                    let (words, held) = values.class_set_words();
                    format!("class sets of {words} words in {held}, ")
                }
                false => String::new(),
            };
            eprintln!(
                "timing: points-to: {runs} runs, {} queued, {} read places with {entries} readers, \
                 {sets}{} variable lists, {} implementations known{}",
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
        let mut reads = values.reads.borrow_mut().take().unwrap_or_default();
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
        huge[v] = updates.len() > HUGE_VISIT;
        for read in reads.drain() {
            let next = read_ids.len() as u32;
            let id = *read_ids.entry(read).or_insert(next);
            if id == next {
                readers.push(Vec::new());
            }
            if registered[v].insert(id) {
                readers[id as usize].push(v as u32);
            }
        }
        // The set, empty, is kept for the next run, with the room it grew to.
        *values.reads.borrow_mut() = Some(reads);
        for changed in apply(&mut values, updates) {
            let own = match changed {
                Read::Vars(m, ctx) => visit_of.get(&(m, ctx)).copied(),
                _ => None,
            };
            let read_by = read_ids.get(&changed).map_or(&[][..], |&id| &readers[id as usize][..]);
            for r in own.into_iter().chain(read_by.iter().map(|&r| r as usize)) {
                if !queued[r] {
                    queued[r] = true;
                    if huge[r] { later.push_back(r) } else { queue.push_back(r) }
                }
            }
        }
    }
    if timings && dispatch {
        let any = values.site_elements.iter().filter(|f| f.any_class).count();
        let escaped = values.escaped.iter().count();
        let loose = match values.loose.any_class {
            true => "any class".to_string(),
            false => format!("{} classes", values.loose.class_values.iter().count()),
        };
        eprintln!(
            "timing: points-to for dispatch: {runs} runs; {} places of arrays that may hold classes, \
             {any} of them any class, {escaped} given to code not followed; stored in arrays made \
             anywhere: {loose}",
            values.sites.len()
        );
    }
    *values.reads.borrow_mut() = None;
    *values.calls.get_mut() = None;
    *values.joined.get_mut() = Joined::default();
    values
}

/// The number of updates past which a visit runs again only once no other is queued.
const HUGE_VISIT: usize = 100_000;

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
                if let Some(&kind) = values.lookup_params.get(&(m, slot))
                    && values.receivers[table_kind(kind)].join(&flow.class_values())
                {
                    changed.push(Read::Receivers(kind));
                }
            }
            Update::Return(m, ctx, flow) => {
                if values.returns.entry((m, ctx)).or_default().join(&flow) {
                    changed.push(Read::Return(m, ctx));
                    let joined = values.joined.get_mut();
                    for &id in joined.of.get(&(m, ctx)).into_iter().flatten() {
                        if joined.flows[id as usize].join(&flow) {
                            changed.push(Read::Joined(id));
                        }
                    }
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
                    values.all_elements.join(&flow);
                    changed.extend([Read::Element(ty), Read::Elements]);
                }
            }
            Update::Site(site, flow) => {
                if values.site_elements[site].join(&flow) {
                    changed.push(Read::Site(site));
                    if values.escaped.contains(site) {
                        escape(values, &flow.arrays, &mut changed);
                    }
                }
            }
            Update::Loose(flow) => {
                if values.loose.join(&flow) {
                    changed.push(Read::Loose);
                    escape(values, &flow.arrays, &mut changed);
                }
            }
            Update::Escape(arrays) => escape(values, &arrays, &mut changed),
            Update::Handed(flow) => {
                if values.handed.join(&flow) {
                    changed.push(Read::Handed);
                }
            }
        }
    }
    changed
}

/// The updates of the places of the arrays that may be what `made` holds with the `Class` values
/// and arrays of `flow`, stored into their elements: `loose` where they may have been made
/// anywhere (`ObjectValues::element_updates`).
fn stores_into(made: &Flow, flow: &Flow) -> Vec<Update> {
    let part = flow.class_part();
    match made.any_array {
        true => vec![Update::Loose(part)],
        false => made.arrays.iter().map(|site| Update::Site(site, part.clone())).collect(),
    }
}

/// Adds the places `arrays` to those of the arrays that code the analysis does not follow may
/// hold (`ObjectValues::escaped`), with the arrays in them.
fn escape(values: &mut ObjectValues, arrays: &ClassSet, changed: &mut Vec<Read>) {
    let mut work: Vec<usize> = arrays.iter().filter(|&site| !values.escaped.contains(site)).collect();
    while let Some(site) = work.pop() {
        if values.escaped.contains(site) {
            continue;
        }
        values.escaped.insert(site);
        // A read of its elements now gives `loose` too.
        changed.push(Read::Site(site));
        work.extend(
            values.site_elements[site].arrays.iter().filter(|&inner| !values.escaped.contains(inner)),
        );
    }
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
                let flow = values.element_of(class, m, ctx, array);
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
                Place::Index(array, _) => out.extend(values.element_updates(class, m, ctx, array, flow)),
            }
        }
        ExprKind::ArrayLit(items) => {
            // The classes of a part of the reflection table's data hold only those that the
            // members that a lookup may find name (`ObjectValues::live_slot`).
            let named_by = items
                .first()
                .and_then(|first| class.reflection_table.data.get(&first.id))
                .filter(|data| values.dispatch && !data.named_by.is_empty());
            for (i, item) in items.iter().enumerate() {
                let named = |data: &TableData| match data.named_by.get(i) {
                    Some(slots) => slots.iter().any(|&slot| values.live_slot(class, data.kind, slot)),
                    None => true,
                };
                if named_by.is_none_or(named) {
                    let flow = eval(item);
                    if let Some(&site) = values.sites.get(&(x.id, 0)) {
                        out.push(Update::Site(site, flow.class_part()));
                    }
                    out.push(Update::Element(x.ty.element(), flow));
                }
            }
        }
        // The arrays inside a `new T[a][b]` have places of their own, held by those outside them.
        ExprKind::NewArray(sizes) if sizes.len() > 1 => {
            let mut ty = x.ty;
            for depth in 0..sizes.len() - 1 {
                ty = ty.element();
                let (Some(&outer), Some(&inner)) =
                    (values.sites.get(&(x.id, depth)), values.sites.get(&(x.id, depth + 1)))
                else {
                    break;
                };
                out.push(Update::Site(outer, Flow::array(inner)));
                out.push(Update::Element(ty, Flow::array(inner)));
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
            let targets = values.call_targets(class, m, ctx, x);
            // Passing the targets what they were passed before changes nothing (`Passed`): a
            // target that was passed the arguments before gets only those that changed since.
            let mut passed_before: Option<(Option<Targets>, Vec<bool>)> = None;
            if let Some(kept) =
                values.calls.borrow_mut().as_mut().and_then(|calls| calls.get_mut(&(x.id, ctx)))
                && Rc::ptr_eq(&kept.targets, &targets)
            {
                if kept.args.as_ref() == Some(&flows) {
                    return;
                }
                let same = |before: &[Flow]| before.iter().zip(&flows).map(|(b, f)| b == f).collect();
                passed_before = match (&kept.args, kept.before.take()) {
                    (Some(args), _) => Some((None, same(args))),
                    (None, Some((targets, args))) => Some((Some(targets), same(&args))),
                    (None, None) => None,
                };
                kept.args = Some(flows.clone());
            }
            // The targets passed the arguments before, where those are not all of them.
            let before: Option<FastSet<(usize, usize)>> = match &passed_before {
                Some((Some(targets), _)) => Some(targets.iter().copied().collect()),
                _ => None,
            };
            let unchanged = |i: usize, target: &(usize, usize)| match &passed_before {
                Some((_, same)) => same[i] && before.as_ref().is_none_or(|b| b.contains(target)),
                None => false,
            };
            // What each parameter of a target method gets, by its slot: the same for each class
            // that the method runs on.
            let mut passed: FastMap<usize, Vec<(usize, usize, Flow)>> = FastMap::default();
            for target @ &(t, c) in targets.iter() {
                let params = passed.entry(t).or_insert_with(|| {
                    let slots = values.param_slots[t].iter().zip(&class.methods[t].params).zip(&flows);
                    slots
                        .enumerate()
                        .filter_map(|(i, ((slot, p), flow))| {
                            Some((i, (*slot)?, values.types.filter(flow.clone(), p.ty)))
                        })
                        .collect()
                });
                let changed = params.iter().filter(|(i, ..)| !unchanged(*i, target));
                out.extend(changed.map(|(_, slot, flow)| Update::Var(t, c, *slot, flow.clone())));
            }
        }
        // A library function may store its arguments, or the elements of an array it is given,
        // into the arrays it is given. The `Class` values and arrays that an array's elements
        // hold are in it already: only those of the other arguments are stored into it, so
        // that sorting an array that may have been made anywhere stores nothing new (`loose`).
        // For dispatch, an argument of another type (`Object`, as the natives of
        // `java.lang.reflect.Array` take theirs) that holds arrays is an array given too, for
        // the `Class` values and arrays stored into it.
        ExprKind::Builtin(_, args)
            if args.iter().any(|a| {
                matches!(a.ty, Type::Array(_)) || values.dispatch && values.types.may_be_array(a.ty)
            }) =>
        {
            let flows: Vec<Flow> = args.iter().map(eval).collect();
            let typed = |i: usize| matches!(args[i].ty, Type::Array(_));
            let held = |i: usize| {
                values.dispatch
                    && values.types.may_be_array(args[i].ty)
                    && (flows[i].any_array || !flows[i].arrays.words.is_empty())
            };
            let arrays: Vec<usize> = (0..args.len()).filter(|&i| typed(i) || held(i)).collect();
            if arrays.is_empty() {
                return;
            }
            let given: Vec<Flow> = (0..args.len())
                .map(|i| match (typed(i), arrays.contains(&i)) {
                    (true, _) => values.element_of(class, m, ctx, &args[i]),
                    (false, true) => {
                        let all = values.elements_of(args[i].ty);
                        values.elements_in(&flows[i], args[i].ty, all).class_part()
                    }
                    (false, false) => flows[i].clone(),
                })
                .collect();
            let mut flow = Flow::default();
            given.iter().for_each(|f| _ = flow.join(f));
            for &i in &arrays {
                let mut stored = flow.clone().object_part();
                for (_, f) in given.iter().enumerate().filter(|&(j, _)| j != i) {
                    stored.join(&f.class_part());
                }
                match (typed(i), values.dispatch) {
                    (true, true) => out.extend(values.element_updates(class, m, ctx, &args[i], stored)),
                    (true, false) => out.push(Update::Element(args[i].ty.element(), flow.clone())),
                    (false, _) => {
                        let part = stored.class_part();
                        out.extend(stores_into(&flows[i], &part));
                        out.push(Update::Element(args[i].ty, part));
                    }
                }
            }
        }
        _ => {}
    });
    if values.dispatch {
        visit_expr(e, &mut |x| {
            out.extend(handed(values, class, m, ctx, x).into_iter().map(|(f, _)| Update::Handed(f)));
            // The arrays that the runtime or an unresolved call is given may be anywhere then.
            let given: Vec<&Expr> = match &x.kind {
                _ if hands(x) => match &x.kind {
                    ExprKind::Builtin(_, args) => args.iter().collect(),
                    ExprKind::Binary(_, l, r) => vec![l, r],
                    ExprKind::Assign { value, .. } => vec![value],
                    _ => Vec::new(),
                },
                ExprKind::Call { resolved: None, args, .. } => args.iter().collect(),
                ExprKind::MethodCall { resolved: None, receiver, args, .. } => {
                    std::iter::once(&**receiver).chain(args).collect()
                }
                _ => Vec::new(),
            };
            for a in given.into_iter().filter(|a| values.types.may_be_array(a.ty)) {
                let flow = values.eval(class, m, ctx, a);
                if !flow.arrays.words.is_empty() {
                    out.push(Update::Escape(flow.arrays));
                }
            }
        });
    }
    out
}

/// Whether the runtime is handed objects by `x` itself (`handed`): by a builtin that holds them,
/// or a string concatenation.
pub(super) fn hands(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Builtin(builtin, _) => hands_objects(builtin),
        ExprKind::Binary(_, _, _) | ExprKind::Assign { op: Some(_), .. } => x.ty == Type::String,
        _ => false,
    }
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
    if !hands(x) {
        return Vec::new();
    }
    let operands: Vec<&Expr> = match &x.kind {
        ExprKind::Builtin(_, args) => args.iter().collect(),
        ExprKind::Binary(_, l, r) if x.ty == Type::String => vec![l, r],
        ExprKind::Assign { op: Some(_), value, .. } if x.ty == Type::String => vec![value],
        _ => Vec::new(),
    };
    operands.into_iter().map(|a| handed_by(values, class, m, ctx, a)).collect()
}

/// The places of the arrays that may hold `Class` values (`ObjectValues::sites`), numbered in the
/// order of the program's code.
fn array_sites(class: &Class, types: &Types) -> FastMap<(ExprId, usize), usize> {
    let mut sites = FastMap::default();
    let mut note = |e: &Expr| {
        visit_expr(e, &mut |x| {
            let depths = match &x.kind {
                ExprKind::NewArray(sizes) => sizes.len().max(1),
                ExprKind::ArrayLit(_) => 1,
                _ => return,
            };
            let mut ty = x.ty;
            for depth in 0..depths {
                let Type::Array(element) = ty else { break };
                if !types.holds_class_values(*element) {
                    break;
                }
                let next = sites.len();
                sites.insert((x.id, depth), next);
                ty = *element;
            }
        })
    };
    for method in &class.methods {
        for s in &method.body {
            visit_stmt(s, &mut note, &mut |_| {});
        }
    }
    for decl in &class.types {
        for init in decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()) {
            note(init);
        }
        for value in decl.enum_constants.iter().flatten().flat_map(|c| &c.values) {
            note(value);
        }
    }
    for e in &class.raised {
        note(e);
    }
    sites
}

/// The variables of `method` that the analysis follows, its parameters among them when `params`.
/// A name that a catch clause binds anywhere in the method is left to be anything.
fn followed(method: &Method, params: bool) -> FastMap<String, usize> {
    let mut names: FastSet<String> =
        if params { method.params.iter().map(|p| p.name.clone()).collect() } else { FastSet::default() };
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
