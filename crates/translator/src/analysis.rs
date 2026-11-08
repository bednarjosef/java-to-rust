//! Analyses that prove facts about the program, so that the code generator can write the Rust a
//! person would write wherever that behaves exactly like the Java:
//!
//! - The value ranges of `int` and `long` variables show which arithmetic cannot overflow and
//!   which divisions can neither divide by zero nor overflow; those use Rust's plain operators
//!   instead of `wrapping_add` and `java::div` (see `ranges`).
//! - Strings that are always ASCII have a `len()` equal to Java's UTF-16 `length()`.
//! - String parameters that a method never assigns are borrowed: `&str`, or `Option<&str>` when
//!   they can be null.
//! - Array locals that no other name can see are plain `Vec`s (see `arrays`).
//! - Classes whose objects never change after construction are shared without a `RefCell`
//!   (see `objects`), and methods that only reach their object through `this` borrow it as
//!   `&self` or `&mut self` (see `borrowing`).
//!
//! The facts form a table keyed by expression id. An expression without a fact gets the faithful
//! default translation, so a weaker analysis only makes the output plainer, never wrong.

mod alongside;
mod assigned_fields;
pub use alongside::one_after_another;
mod arrays;
mod borrowing;
mod boxing;
pub(crate) mod closure;
pub mod closures;
mod constants;
mod early_reads;
mod escape;
mod exceptions;
mod fixed_fields;
mod frozen_fields;
pub mod hierarchy;
mod indexing;
pub mod live_methods;
mod lock_values;
mod monitors;
mod moves;
pub(crate) mod npe_messages;
mod nullness;
mod object_values;
mod objects;
pub(crate) use objects::instantiated;
mod owned;
mod patterns;
mod proven_classes;
mod ranges;
mod reflection_table;
mod statics;
mod string_identity;
mod threads;
pub(crate) mod unboxing;
mod whole_objects;

use crate::ast::*;
use std::cell::RefCell;

pub use exceptions::{catches_by_unwinding, generator_text, jumps_out, text_objects};
pub(crate) use npe_messages::{
    ENUM_COMPARE_NULL, SEARCH_NULL_ELEMENT, SEARCH_NULL_KEY, compare_null_message,
};
pub use npe_messages::{KeptMessages, npe_messages, npe_messages_kept};

pub use arrays::{RowIndex, Slice, SlicedLoop};
pub use escape::{Guard, StackObjects};
pub use nullness::{conjuncts, maybe_null, non_null_names, null_test, nullable_type, nullable_var};
pub use string_identity::string_identity;
pub(crate) use string_identity::{Made, made, value_of_operand};
pub use threads::array_family;
pub use whole_objects::{whole_objects, whole_objects_all};

/// The types whose statics a thread that the program starts may touch (`Class::thread_statics`),
/// for a translation that does not keep only the methods that can run (a whole program's does,
/// and finds these on the way): `None` when the program starts no thread, or that is not known.
pub fn thread_statics(class: &Class) -> Option<std::collections::HashSet<usize>> {
    if !threads::threaded(class) {
        return None;
    }
    live_methods::live_methods(class).ok()?.thread_statics.map(|t| t.into_iter().collect())
}

#[derive(Clone, Default)]
pub struct Facts {
    /// Whether the program starts a thread (see `threads`).
    pub threaded: bool,
    /// Whether the program interrupts a thread, so that `sleep`, `join` and `wait` may throw
    /// `InterruptedException` (see `threads`).
    pub interrupts: bool,
    /// The classes, by type index, whose objects two threads may reach: `Arc`s, with a
    /// `java::Locked` unless immutable.
    pub thread_shared: FastSet<usize>,
    /// The types whose statics the started threads' code may touch (`Class::thread_statics`),
    /// `None` when not known; see `Facts::process_statics`.
    pub thread_statics: Option<std::collections::HashSet<usize>>,
    /// Whether two threads may reach a `StringBuilder`, whose builders are then all
    /// `java::SharedStringBuilder`s (see `threads::shares_builders`).
    pub shared_builders: bool,
    /// The families of the arrays that two threads may reach (`threads::array_family`), whose
    /// arrays are `java::SharedArray`s; every other array is a `java::Array`.
    pub shared_arrays: FastSet<Type>,
    /// The methods of those classes that must not borrow their object for the whole call, and
    /// the methods that use the kept monitor of `this` (see `monitors`).
    pub long_methods: FastSet<usize>,
    /// Whether the program waits, notifies or asks `Thread.holdsLock`, so that every monitor it
    /// uses is kept (see `monitors`).
    pub monitors: bool,
    /// The classes, by type index, whose class monitors are kept (see `monitors`).
    pub class_monitors: FastSet<usize>,
    /// Integer `+`, `-`, `*`, negation, `++`, `--`, compound assignments and `Math.abs` that
    /// never overflow.
    pub no_overflow: FastSet<ExprId>,
    /// The values that the index of each array indexing or element assignment takes, from the
    /// range analysis: (lowest, highest), empty when `lowest > highest`. Each size of a
    /// `new T[n]` has its values here too, keyed by the size expression.
    pub index_ranges: FastMap<ExprId, (i64, i64)>,
    /// The class type, by index, that a dominating test proved each read of a local holds, by
    /// the read's id, where it is narrower than the local's type (see `proven_classes`).
    pub proven_types: FastMap<ExprId, usize>,
    /// Reads of string, array and object variables that are last uses: the value is moved, not
    /// cloned (see `moves`).
    pub moves: FastSet<ExprId>,
    /// Integer `/`, `%` and their compound assignments whose divisor is never zero and that never
    /// compute `MIN / -1`.
    pub safe_division: FastSet<ExprId>,
    /// `String.length()`, `String.charAt` and `String.substring` calls on strings that are always
    /// ASCII.
    pub ascii_length: FastSet<ExprId>,
    /// `(char)` casts whose integer value the range analysis keeps within ASCII.
    pub ascii_chars: FastSet<ExprId>,
    /// `s.charAt(i)` calls whose index is always within the string.
    pub index_in_bounds: FastSet<ExprId>,
    /// String parameters, as (method, parameter) indexes, that are never assigned.
    pub borrowed_params: FastSet<(usize, usize)>,
    /// String parameters that are never assigned but can be null: `Option<&str>`.
    pub borrowed_optional_params: FastSet<(usize, usize)>,
    /// The borrowed string parameters (`borrowed_params`, `borrowed_optional_params`) that a
    /// method of their group reads by code unit index (`charAt`, `indexOf`, `substring`,
    /// `length`): a `java::Str` (an `Option` of one where it may be null), which keeps whether the
    /// string is ASCII, rather than a `&java::Wtf8`.
    pub lent_strings: FastSet<(usize, usize)>,
    /// Object parameters, as (method, parameter) indexes, that the method never assigns and only
    /// uses through: it reads and writes their fields, calls their methods, compares or prints
    /// them, passes them on to such parameters, and copies them into locals. They are `&Rc<..>`,
    /// which the caller lends.
    pub borrowed_objects: FastSet<(usize, usize)>,
    /// Object parameters that can be null and that the method uses the same way: they are
    /// `&Option<Rc<..>>`, which the caller lends.
    pub borrowed_optional_objects: FastSet<(usize, usize)>,
    /// String variables and parameters, as (method, name), that can hold null.
    pub nullable_vars: MethodLocals,
    /// Methods whose string or object return value can be null.
    pub nullable_returns: FastSet<usize>,
    /// String and object fields, as (class, field), that can hold null: `Option`s.
    pub nullable_fields: FastSet<(String, String)>,
    /// String and object fields, as (class, field), that are null only while their object is
    /// being built: every constructor sets them, to a value that is not null, before anything can
    /// read them (`assigned_fields.rs`). Their cells are `Option`s, which start empty, and a read
    /// takes the value without a null check (`java::assigned`).
    pub assigned_fields: FastSet<(String, String)>,
    /// String and object static fields, as (class index, field), that can hold null.
    pub nullable_statics: FastSet<(usize, String)>,
    /// Statics of object type, of a class initialized eagerly, that are never null once the class
    /// is initialized: their cell starts empty, as there is no object to start it with, so it is
    /// an `Option` that is written with `Some` and read with `java::non_null`.
    pub placeholder_statics: FastSet<(usize, String)>,
    /// The reads of a static that may run during its class's initialization, before the
    /// initializer sets it (see `early_reads`): a read of a static of `placeholder_statics` there
    /// may be null, and is its cell's `Option`.
    pub early_static_reads: FastSet<ExprId>,
    /// Statics that are never null where code reads them, though the nullness analysis lets
    /// them be: a static set nowhere else, which the first statements of its class's static
    /// initializer set to a value that is not null, as `ForkJoinPool`'s `U = Unsafe.getUnsafe()`.
    /// Reading it cannot throw (see `exceptions`); its cell is still an `Option`.
    pub initialized_statics: FastSet<(usize, String)>,
    /// The element types (strings, objects and arrays) whose arrays can hold null: their
    /// elements are `Option`s, in every array of that element type.
    pub nullable_elements: FastSet<Type>,
    /// Array locals and borrowed array parameters, as (method, name), that no other name can
    /// see: plain `Vec`s (of `Vec`s, for arrays of arrays). The flag says whether an element of
    /// the array, at any depth, is ever assigned.
    pub plain_arrays: FastMap<(usize, String), bool>,
    /// Array accesses (`a[i]` reads, element assignments and rows `m[i]` of `m[i][j]`) whose
    /// index is always in bounds (`arrays::bounds`).
    pub in_bounds: FastSet<ExprId>,
    /// Array parameters, as (method, parameter) indexes, that are plain at every call: `&[T]`, or
    /// `&mut [T]` when the flag says the elements are written.
    pub borrowed_arrays: FastMap<(usize, usize), bool>,
    /// The variables of for-each loops over plain arrays of arrays, as (method, name): each
    /// borrows the rows in turn (`for row in m.iter()`), as `&Vec<T>` or `&mut Vec<T>`.
    pub row_loops: FastSet<(usize, String)>,
    /// Innermost counting loops over plain arrays whose accesses are all in bounds when a check
    /// made before the loop passes: the loop then runs over slices, keyed by its condition.
    pub sliced_loops: FastMap<ExprId, SlicedLoop>,
    /// Methods that return arrays nobody else holds: they return the plain `Vec`.
    pub owned_returns: FastSet<usize>,
    /// Counting loops over a plain array whose body only reads `a[i]`, keyed by the loop
    /// condition: the reads that can become elements of `for x in a.iter()`.
    pub element_loops: FastMap<ExprId, Vec<ExprId>>,
    /// The classes whose objects never change after construction, by qualified name.
    pub immutable: FastSet<String>,
    /// The classes whose objects are never shared, so they are plain values, by qualified name.
    pub owned: FastSet<String>,
    /// The classes whose objects change and are shared, by qualified name: an object is an
    /// `Arc<C>` whose fields are cells, read and written by value, with no `RefCell` (see
    /// `objects::celled_classes`).
    pub celled: FastSet<String>,
    /// The celled classes whose fields are `java::SharedField`s, because threads share their
    /// objects or the objects keep a monitor (`monitored`).
    pub shared_cells: FastSet<String>,
    /// The celled classes whose objects keep a monitor, because a monitor site of the program may
    /// lock them (`monitors::kept`): an `Arc<java::Monitored<C>>`, where the other celled objects
    /// are an `Arc<C>`.
    pub monitored: FastSet<String>,
    /// For each type (index into `Class::types`), the concrete classes and enums whose values
    /// become values of that type (see `hierarchy::flows`).
    pub flows: Vec<FastSet<usize>>,
    /// The instance methods (indexes into `Class::methods`) that change their object.
    pub mutating: FastSet<usize>,
    /// The instance methods that borrow their object for the call: `&self` or `&mut self`.
    pub self_methods: FastSet<usize>,
    /// The classes whose constructors build a plain value, which `new` then shares.
    pub plain_constructors: FastSet<String>,
    /// For each other class, by index, the positions of the constructors and field initializers
    /// that keep it from building a plain value.
    pub not_plain: FastMap<usize, Vec<Pos>>,
    /// Methods and constructors that touch no object but their own, so they borrow no other.
    pub isolated: FastSet<usize>,
    /// Methods and constructors that may borrow a shared object mutably, through what they call.
    pub writers: FastSet<usize>,
    /// The classes, by index in `Class::types`, whose static initialization runs eagerly through
    /// an `init` function; the other classes initialize each static lazily on first access.
    pub eager_init: FastSet<usize>,
    /// The eager classes whose static initialization `main` runs first, before any thread starts
    /// (`statics::early`).
    pub early_init: FastSet<usize>,
    /// The statics, as (class index, name), that Java assigns once and only in their class's
    /// static initialization: declared `final`, not constants, and assigned by no other method.
    /// Where they are the process's, those of object, array or string type are `java::Final`
    /// cells, which a thread reads without a lock (see `codegen/statics.rs`).
    pub final_statics: FastSet<(usize, String)>,
    /// The final statics that are frozen arrays: one-dimensional arrays that only their class's
    /// static initialization makes, as new arrays, and that the program only indexes to read and
    /// measures (`statics::frozen`). Where they are `java::Final`s, they hold a `java::Frozen`,
    /// whose elements a thread reads with no atomic load.
    pub frozen_statics: FastSet<(usize, String)>,
    /// The instance array fields that only their class's constructors make and write, through
    /// `this`, and that the program otherwise only indexes to read and measures
    /// (`frozen_fields`): `java::Frozen`s, whose elements are read with no atomic load.
    pub frozen_fields: FastSet<(usize, String)>,
    /// The new arrays that frozen fields are given, made into `java::Frozen`s as they are made:
    /// with whether the field's arrays have a constant length (`frozen_lengths`).
    pub frozen_news: FastMap<ExprId, bool>,
    /// The constant length of every array a frozen static or field is given, where they all have
    /// the same one: the `java::Frozen`'s `N`.
    pub frozen_lengths: FastMap<(usize, String), usize>,
    /// The static methods and constructors that run their class's `init` when they start.
    pub entry_init: FastSet<usize>,
    /// The class of `main`, when the program has exactly one, which `main` initializes first.
    pub main_class: Option<usize>,
    /// The assignments of each static (see `exceptions::guards`).
    static_writes: exceptions::guards::StaticWrites,
    /// The boxed kinds (`Type::Boxed` of the primitive) whose identity `==` observes: their boxes
    /// are shared objects. Every other boxed kind is its primitive value.
    pub faithful_boxes: FastSet<Type>,
    /// The `==` and `!=` (by expression id) on two `Object` values that may both be strings or
    /// boxes, whose identity the translation does not keep: compared at run time, where values
    /// that are not equal are different objects and equal ones end the program
    /// (`java::unkept_identity`), as Java may or may not see one object in them.
    pub identity_checks: FastSet<ExprId>,
    /// The classes with subclasses (indexes into `Class::types`) whose fields are a struct of
    /// their own, `<Name>Fields`, that the structs of their subclasses embed as `base`.
    pub fields_structs: FastSet<usize>,
    /// The methods of classes with subclasses that live on the class's fields struct.
    pub field_level: FastSet<usize>,
    /// The class that declares each field visible in each class: (class, field) to class, by
    /// qualified name.
    pub field_owners: FastMap<(String, String), String>,
    /// The instance fields that only constructors write, through `this`, by declaring type and
    /// name: their reads borrow the field alone (see `fixed_fields`).
    pub fixed_fields: FastSet<(usize, String)>,
    /// The methods and constructors that return `Result<_, Throwable>`: some caller observes an
    /// exception that may escape them (see `exceptions`).
    pub throwing: FastSet<usize>,
    /// The methods of `throwing` that are there only for their override group: nothing that a
    /// caller observes escapes their own code. Their code is a function that returns its value
    /// plainly, and the function by the method's name wraps it in `Ok` (see `exceptions`).
    pub plain_bodies: FastSet<usize>,
    /// The classes whose instance field initializers raise, so that their `field_inits` returns a
    /// `Result` (see `exceptions::field_inits`).
    pub init_throwing: FastSet<usize>,
    /// The `try` statements, by address, whose `catch` clauses catch an exception that their body
    /// may throw. The others run their body as a plain block.
    pub live_tries: FastSet<usize>,
    /// The `try` statements, by address, whose body or `catch` clauses an exception of the
    /// program may leave or reach (which their `finally` must see).
    pub raising_tries: FastSet<usize>,
    /// The operations whose failure a `catch` or a `finally` may see, which are written with the
    /// runtime's checked form (see `analysis::exceptions`).
    pub raising: FastSet<ExprId>,
    /// The objects, by expression, whose dereference by a call or a field access may throw a
    /// `NullPointerException` that a `catch` or a `finally` may see: they are written with the
    /// runtime's checked `non_null`. With the position of the call or the field access.
    pub null_checked: FastMap<ExprId, Pos>,
    /// Where a `catch` first reads the message of a `NullPointerException` that the runtime
    /// raises (`getMessage`), which the translation then writes as Java's (see `npe_messages`).
    pub npe_message_read: Option<Pos>,
    /// The objects of `null_checked` whose message such a `catch` may read: those that its `try`
    /// catches itself, and those whose exception leaves their method. Each must have Java's
    /// message in `npe_messages`, or the program is not translated.
    pub npe_message_needed: FastSet<ExprId>,
    /// The dereferences outside `null_checked` whose `NullPointerException` may unwind to a `try`
    /// that catches by unwinding and reads it (`exceptions::catches_by_unwinding`): they raise
    /// Java's message where `npe_messages` finds it. With their places.
    pub npe_unwound: FastMap<ExprId, Pos>,
    /// The message of the `NullPointerException` of the dereference of each object of
    /// `null_checked` whose message is known: Java's text, or `None` for an exception without
    /// one (such as `Objects.requireNonNull`'s). Filled in only when `npe_message_read` is set.
    pub npe_messages: FastMap<ExprId, Option<String>>,
    /// The dereferences of `npe_message_needed` whose message is approximated
    /// (`Program::approximate_messages`), with the position of the method each is in (`None` for a
    /// field's initializer).
    pub npe_approximated: Vec<(Pos, Option<Pos>)>,
    /// The operations whose failure, which has no checked form of its own, a `catch` or a
    /// `finally` may see: they run inside `java::catching`. With their positions.
    pub catching: FastMap<ExprId, Pos>,
    /// The standard classes whose objects the runtime raises where the program may see them.
    pub raised_classes: std::collections::BTreeSet<usize>,
    /// Whether some `try` around a point that raises catches by unwinding
    /// (`exceptions::catches_by_unwinding`): the static initializations then hold the exceptions
    /// they raise (`java::exceptions_held`).
    pub unwinding_tries: bool,
    /// The functional interfaces whose values are closures, `Rc<dyn Fn(A) -> R>` (see
    /// `closures`).
    pub closure_types: FastSet<usize>,
    /// The locals, by method and name, that hold a closure only to call it: a plain Rust
    /// closure, `let f = |x| ..;` (see `closures::closure_locals`).
    pub closure_locals: FastSet<(usize, String)>,
    /// The `new`s and calls of factories whose objects never leave the method that creates
    /// them and, in a loop, can no longer be reached when the next iteration makes the next one:
    /// their objects may live in a slot on the method's stack (see `escape`). Empty when the
    /// translation is made with `JAVA_TO_RUST_HEAP_ONLY` set.
    pub stack_objects: StackObjects,
}

impl Facts {
    /// The class whose `init` function initializing class `t` runs: `t` when its initialization
    /// is eager, else its nearest superclass whose initialization is, which Java initializes
    /// first (its `init` starts by initializing its own superclass).
    pub fn init_class(&self, class: &Class, t: usize) -> Option<usize> {
        std::iter::once(t).chain(class.hierarchy.ancestors(t)).find(|c| self.eager_init.contains(c))
    }

    /// Whether a read of the field `name` of an object of type `ty` can give null.
    pub fn field_nullable(&self, ty: Type, name: &str) -> bool {
        self.field_key(ty, name).is_some_and(|key| self.nullable_fields.contains(&key))
    }

    /// Whether the field `name` of an object of type `ty` is an `Option`: it can hold null, or it
    /// does while its object is being built (`assigned_fields`).
    pub fn field_optional(&self, ty: Type, name: &str) -> bool {
        self.field_key(ty, name)
            .is_some_and(|key| self.nullable_fields.contains(&key) || self.assigned_fields.contains(&key))
    }

    /// Whether the field `name` of an object of type `ty` is an `Option` whose reads never find it
    /// empty (`assigned_fields`).
    pub fn field_assigned(&self, ty: Type, name: &str) -> bool {
        self.field_key(ty, name).is_some_and(|key| self.assigned_fields.contains(&key))
    }

    /// The key of the field `name` of an object of type `ty` in the field facts: its declaring
    /// class and its name.
    fn field_key(&self, ty: Type, name: &str) -> Option<(String, String)> {
        let Type::Class(c, _) = ty else { return None };
        let owner = self.field_owners.get(&(c.to_string(), name.to_string())).map_or(c, String::as_str);
        Some((owner.to_string(), name.to_string()))
    }
}

pub fn analyze(class: &Class) -> Result<Facts, Error> {
    analyze_all(class).map_err(|mut errors| errors.remove(0))
}

/// The facts, or the errors of the first analysis that rejects the program: all of them, where
/// that analysis finds more than one (a census stubs them together, `program/census.rs`).
pub fn analyze_all(class: &Class) -> Result<Facts, Vec<Error>> {
    analyze_tolerating(class, &[])
}

/// `analyze_all`, past the errors `tolerated` of the checks that only reject (the validation of
/// constructors and enums, of what threads share, of strings and boxes as `Object`s and of
/// monitors): a census goes on past those of them that no stub can take away, such as a field's
/// type (`program/census.rs`).
pub fn analyze_tolerating(class: &Class, tolerated: &[Error]) -> Result<Facts, Vec<Error>> {
    Ok(analyze_from(class, tolerated, &mut None)?.with_stack_objects())
}

/// The facts that depend on the program alone and not on the errors tolerated, which a census
/// that only tolerates more errors of the same program finds once (`program/census.rs`): those
/// that the analyses find before their first check, and the range analysis's, kept once the
/// analyses first get past the checks, none of which reads them.
pub struct Represented {
    facts: Facts,
    ranges: Option<Facts>,
}

/// The facts that the analyses find before their first check.
fn represented(class: &Class) -> Represented {
    use crate::timings::phase;
    let mut facts = phase("analysis: structs", || {
        let mut facts = Facts {
            fields_structs: hierarchy::fields_structs(class),
            field_owners: hierarchy::field_owners(class),
            closure_types: closures::closure_types(class),
            ..Facts::default()
        };
        facts.closure_locals = closures::closure_locals(class, &facts.closure_types);
        facts
    });
    phase("analysis: representation", || representation(class, &mut facts));
    Represented { facts, ranges: None }
}

/// The facts of `analyze_from`, and the objects that stay on the stack, found alongside them.
pub struct Analysed {
    pub facts: Facts,
    stack_objects: alongside::Done<Option<escape::StackObjects>>,
}

impl Analysed {
    /// The facts with the objects that stay on the stack (`Facts::stack_objects`), the last of
    /// the facts, which no check reads: a census takes them after its last check of the analyses,
    /// the messages of `NullPointerException`s (`program/census.rs`). None with
    /// `JAVA_TO_RUST_HEAP_ONLY` set.
    pub fn with_stack_objects(self) -> Facts {
        let mut facts = self.facts;
        if let Some(found) = self.stack_objects.take() {
            facts.stack_objects = found;
        }
        facts
    }
}

/// What the analyses find from the code alone, after the checks (`analyze_from`).
struct FromCode<'scope> {
    ranges: Facts,
    fixed_fields: FastSet<(usize, String)>,
    eager_init: FastSet<usize>,
    early_init: FastSet<usize>,
    final_statics: FastSet<(usize, String)>,
    frozen_statics: FastSet<(usize, String)>,
    entry_init: FastSet<usize>,
    early_static_reads: FastSet<ExprId>,
    proven_types: FastMap<ExprId, usize>,
    /// `Facts::ascii_length`, from the ranges' `ascii_chars`, on a thread of its own.
    ascii_length: alongside::Alongside<'scope, FastSet<ExprId>>,
}

/// `analyze_tolerating`, from the facts that `represented` found for `class` (which it finds
/// first when `found` has none), and the objects that stay on the stack.
///
/// The checks run first, in their order, each as soon as the facts it reads are known: the
/// validation reads the representation's facts, the pattern variables the code alone, strings
/// and boxes as `Object`s the flows of objects, what threads share the boxes compared by identity
/// (`boxing`, from the code), and the monitors the representation's flags. The facts that none of
/// them reads (ranges, fixed fields, statics, early reads, nullness) come after them, so a check
/// that rejects the program ends the analyses before them. Each step sees the same facts as when
/// the checks were among them: the checks read none of the facts found after them, and the steps
/// after them see the checks' own facts (`identity_checks`, `faithful_boxes`) only where they did
/// before. None of these steps changes the program or keeps state that another reads.
///
/// So the steps that read none of each other's facts run at once, on threads of their own
/// (`alongside`): from the start, alongside the representation and the checks, the ranges, fixed
/// fields, statics and early reads, which read the code and the ranges alone, with the ASCII
/// strings (`ascii_length`) on a thread of their own once the ranges are found, and the objects on
/// the stack, which read the code alone, until the caller takes them; once the representation and the pattern variables are checked, nullness, which reads the
/// representation's facts and the statics, and then the borrowed parameters, which read nullness
/// alone, alongside the object values and the checks after them. Nothing after them but the
/// generator reads the borrowed parameters and ASCII strings. Each step's result is taken where
/// the steps ran one after another, so the analyses find what they found then, and stop at the
/// same error or panic.
pub fn analyze_from(
    class: &Class,
    tolerated: &[Error],
    found: &mut Option<Represented>,
) -> Result<Analysed, Vec<Error>> {
    analyze_with(class, tolerated, found, None)
}

/// The errors of the checks of the analyses that read the code alone: the pattern variables, and
/// the monitors, whose facts (whether the program starts threads, whether it observes monitors)
/// are scans of the code. A census runs them before the analyses (`program::census`), so that
/// the code they reject is stubbed before the analyses first run; the analyses run them again
/// where they always do, and find nothing more.
pub fn code_checks(class: &Class) -> Vec<Error> {
    let mut errors = patterns::check(class).err().unwrap_or_default();
    let facts =
        Facts { threaded: threads::threaded(class), monitors: monitors::observed(class), ..Facts::default() };
    errors.extend(monitors::check(class, &facts).err().unwrap_or_default());
    errors
}

/// `analyze_from` for a census, which stubs the code of every error it is given at once: an
/// error of a check, or of the analysis of exceptions, does not end the analyses but is added to
/// `errors`, and the checks and analyses after it run as if it had passed, to find their errors
/// too. They then see code that the stubs of the errors before take away; an error they find in
/// it comes to nothing, as the stub that takes its code away was made first. A panic of a step
/// after an error may come of the code that the error rejects, and is the caller's to take as
/// the end of the analyses at that error.
pub fn analyze_collecting(
    class: &Class,
    tolerated: &[Error],
    found: &mut Option<Represented>,
    errors: &RefCell<Vec<Error>>,
) -> Result<Analysed, Vec<Error>> {
    analyze_with(class, tolerated, found, Some(errors))
}

fn analyze_with(
    class: &Class,
    tolerated: &[Error],
    found: &mut Option<Represented>,
    collect: Option<&RefCell<Vec<Error>>>,
) -> Result<Analysed, Vec<Error>> {
    let checked = |result: Result<(), Vec<Error>>| {
        let mut errors = result.err().unwrap_or_default();
        errors.retain(|e| !tolerated.iter().any(|t| t.pos == e.pos && t.message == e.message));
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    };
    // An error that ends the analyses, or is collected.
    let failed = |errors: Vec<Error>| match collect {
        Some(collected) => {
            collected.borrow_mut().extend(errors);
            Ok(())
        }
        None => Err(errors),
    };
    use crate::timings::phase;
    use alongside::Alongside;
    std::thread::scope(|scope| {
        let stack_objects = Alongside::start(scope, || {
            std::env::var_os("JAVA_TO_RUST_HEAP_ONLY")
                .is_none()
                .then(|| phase("analysis: stack objects", || escape::stack_objects(class)))
        });
        let ranges = found.as_mut().and_then(|found| found.ranges.take());
        let from_code = Alongside::start(scope, move || {
            let ranges = ranges.unwrap_or_else(|| {
                let mut ranges = Facts::default();
                phase("analysis: ranges", || ranges::analyze(class, &mut ranges));
                ranges
            });
            // Whether the program starts threads, as the representation finds it.
            let threaded = threads::threaded(class);
            let fixed_fields =
                phase("analysis: fixed fields", || fixed_fields::fixed_fields(class, threaded));
            let (eager_init, early_init, final_statics, frozen_statics, entry_init) =
                phase("analysis: statics", || {
                    let (eager, finals) = statics::eager_and_final(class);
                    let early = if threaded { statics::early(class, &eager) } else { FastSet::default() };
                    let frozen = statics::frozen(class, &finals);
                    let entry = statics::entry_inits(class, &eager);
                    (eager, early, finals, frozen, entry)
                });
            let ascii_chars = ranges.ascii_chars.clone();
            let ascii_length = Alongside::start(scope, move || {
                let mut facts = Facts { ascii_chars, ..Facts::default() };
                phase("analysis: ascii", || ascii_strings(class, &mut facts));
                facts.ascii_length
            });
            let early_static_reads =
                phase("analysis: early static reads", || early_reads::early_reads(class, &eager_init));
            FromCode {
                ranges,
                fixed_fields,
                eager_init,
                early_init,
                final_statics,
                frozen_statics,
                entry_init,
                early_static_reads,
                proven_types: phase("analysis: proven classes", || proven_classes::proven_types(class)),
                ascii_length,
            }
        });
        let found = found.get_or_insert_with(|| represented(class));
        // The representation finds no boxes compared by identity (`boxing`, below), and a panic
        // of a check may have left some.
        found.facts.faithful_boxes.clear();
        checked(phase("analysis: validate", || validate(class, &found.facts))).or_else(failed)?;
        patterns::check(class).or_else(failed)?;
        // The facts as the steps after the checks find them: the representation's, with those
        // found from the code; the checks' own (`faithful_boxes`) are set aside until nullness
        // is done.
        let mut facts = found.facts.clone();
        let nullness = Alongside::start(scope, move || {
            let FromCode {
                ranges,
                fixed_fields,
                eager_init,
                early_init,
                final_statics,
                frozen_statics,
                entry_init,
                early_static_reads,
                proven_types,
                ascii_length,
            } = from_code.join();
            facts.no_overflow = ranges.no_overflow.clone();
            facts.safe_division = ranges.safe_division.clone();
            facts.ascii_chars = ranges.ascii_chars.clone();
            facts.index_ranges = ranges.index_ranges.clone();
            facts.fixed_fields = fixed_fields;
            facts.eager_init = eager_init;
            facts.early_init = early_init;
            facts.final_statics = final_statics;
            facts.frozen_statics = frozen_statics;
            facts.entry_init = entry_init;
            facts.main_class = match class.methods.iter().filter(|c| c.is_main).collect::<Vec<_>>()[..] {
                [main] => Some(main.class),
                _ => None,
            };
            facts.early_static_reads = early_static_reads;
            facts.proven_types = proven_types;
            phase("analysis: nullness", || nullness::analyze(class, &mut facts));
            let borrowing = alongside::Done::of(|| {
                phase("analysis: borrowing", || {
                    borrowed_params(class, &mut facts);
                    (facts.borrowed_objects, facts.borrowed_optional_objects) =
                        borrowed_objects(class, &facts);
                })
            });
            (facts, ranges, borrowing, ascii_length)
        });
        let (identity_checks, object_values) =
            phase("analysis: object values", || patterns::check_object_values(class, &found.facts));
        checked(object_values).or_else(failed)?;
        // The representation's facts have no boxes compared by identity; what threads share reads
        // them.
        phase("analysis: boxing", || boxing::analyze(class, &mut found.facts));
        let checks = (|| {
            if found.facts.threaded {
                checked(phase("analysis: threads", || threads::check(class, &found.facts)))
                    .or_else(failed)?;
            }
            checked(phase("analysis: monitors", || monitors::check(class, &found.facts))).or_else(failed)
        })();
        let faithful_boxes = std::mem::take(&mut found.facts.faithful_boxes);
        checks?;
        let (mut facts, ranges, borrowing, ascii_length) = nullness.join();
        found.ranges = Some(ranges);
        facts.identity_checks = identity_checks;
        facts.faithful_boxes = faithful_boxes;
        facts.class_monitors = monitors::class_monitors(class, &facts);
        borrowing.take();
        facts.ascii_length = ascii_length.join();
        lent_strings(class, &mut facts);
        phase("analysis: frozen fields", || frozen_fields::analyze(class, &mut facts));
        phase("analysis: arrays", || arrays::analyze(class, &mut facts));
        phase("analysis: indexing", || indexing::analyze(class, &mut facts));
        phase("analysis: moves", || moves::analyze(class, &mut facts));
        phase("analysis: exceptions", || exceptions::analyze(class, &mut facts)).or_else(failed)?;
        // An uncaught-exception handler gets the failures of the runtime as objects of their classes.
        if threads::handles_uncaught(class) {
            for new in &class.raised {
                if let Some((c, _, _)) = class.raised_object(new) {
                    facts.raised_classes.insert(c);
                }
            }
        }
        // The exceptions that the runtime raises are values of `Throwable` too.
        if !facts.raised_classes.is_empty() {
            facts.flows = phase("analysis: raised flows", || {
                hierarchy::flows(class, &facts.field_level, &facts.raised_classes)
            });
        }
        Ok(Analysed { facts, stack_objects: stack_objects.finish() })
    })
}

/// How the objects of each class are represented and which methods borrow them: the facts from
/// `immutable` to `writers`, given `fields_structs`.
fn representation(class: &Class, facts: &mut Facts) {
    use crate::timings::phase;
    facts.threaded = threads::threaded(class);
    facts.interrupts = threads::interrupts(class);
    if facts.threaded {
        facts.thread_statics = class.thread_statics.clone();
        facts.thread_shared = phase("representation: thread shared", || threads::thread_shared(class, facts));
        facts.shared_builders =
            phase("representation: shared builders", || threads::shares_builders(class, facts));
        facts.shared_arrays = phase("representation: shared arrays", || threads::shared_arrays(class, facts));
        facts.long_methods =
            phase("representation: long methods", || threads::long_methods(class, &facts.thread_shared));
    }
    facts.monitors = phase("representation: monitors", || monitors::observed(class));
    let (monitor_classes, monitor_methods) =
        phase("representation: kept monitors", || monitors::kept(class, facts));
    facts.long_methods.extend(monitor_methods);
    let reach = phase("representation: reach", || borrowing::reach(class));
    let created = phase("representation: instantiated", || objects::instantiated(class));
    facts.immutable = phase("representation: immutable", || objects::immutable_classes(class));
    // An object whose monitor is kept has a lock, or an address that stays; so does every other
    // object of its class tree, which is immutable together or not at all.
    objects::remove_families(class, &mut facts.immutable, &monitor_classes);
    loop {
        facts.field_level = phase("representation: field level", || {
            hierarchy::field_level(
                class,
                &reach.reach,
                &facts.immutable,
                &facts.fields_structs,
                &facts.long_methods,
            )
        });
        facts.owned = phase("representation: owned", || {
            owned::owned_classes(class, &facts.immutable, &facts.field_level, &created)
        });
        facts.owned.retain(|c| !monitor_classes.contains(c));
        facts.flows = phase("representation: flows", || {
            hierarchy::flows(class, &facts.field_level, &Default::default())
        });
        let handled = phase("representation: handles", || objects::calls_needing_handles(class, facts));
        if handled.is_empty() {
            break;
        }
        for c in handled {
            for f in class.hierarchy.family(c) {
                facts.immutable.remove(&class.types[f].name);
            }
        }
    }
    // An exception is converted to `Throwable` when it is thrown, so it is never a plain value.
    if let Some(throwable) = class.type_index("java.lang.Throwable") {
        facts.owned.retain(|c| class.type_index(c).is_none_or(|t| !class.hierarchy.is_subtype(t, throwable)));
    }
    let borrowing = phase("representation: borrowing", || borrowing::borrowing(class, facts, &reach));
    facts.mutating = borrowing.mutating;
    facts.self_methods = borrowing.methods;
    facts.plain_constructors = borrowing.plain_constructors;
    facts.not_plain = borrowing.not_plain;
    facts.isolated = borrowing.isolated;
    facts.writers = borrowing.writers;
    (facts.celled, facts.shared_cells, facts.monitored) =
        phase("representation: celled", || objects::celled_classes(class, facts, &monitor_classes, &created));
    // A class without objects of a celled tree counts as celled, not immutable.
    facts.immutable.retain(|c| !facts.celled.contains(c));
}

/// The classes with subclasses whose constructors cannot build their fields struct as a plain
/// value: they use `this` as a value or call an overridable method. `whole_objects` gives them
/// the second constructor form.
fn whole_object_classes(class: &Class) -> FastSet<usize> {
    let mut facts = Facts { fields_structs: hierarchy::fields_structs(class), ..Facts::default() };
    representation(class, &mut facts);
    facts
        .fields_structs
        .iter()
        .copied()
        .filter(|&t| !facts.plain_constructors.contains(&class.types[t].name))
        .collect()
}

/// Rejects what the analyses show the translation cannot do: a superclass's constructor runs
/// before its subclass's object exists, building the fields struct as a plain value, so it may
/// use `this` only for that struct. Every class that does is reported, at each constructor and
/// field initializer that does, after one another.
fn validate(class: &Class, facts: &Facts) -> Result<(), Vec<Error>> {
    let mut errors = Vec::new();
    let mut structs: Vec<usize> = facts.fields_structs.iter().copied().collect();
    structs.sort_unstable();
    for t in structs {
        if facts.plain_constructors.contains(&class.types[t].name) {
            continue;
        }
        let first = || class.methods.iter().find(|m| m.is_ctor && m.class == t).map(|m| m.pos);
        let positions = match facts.not_plain.get(&t) {
            Some(positions) => positions.clone(),
            None => vec![first().unwrap_or_default()],
        };
        for pos in positions {
            errors.push(Error {
                pos,
                message: "constructors of a class with subclasses that use `this` as a value or call \
                          overridable methods not supported yet"
                    .to_string(),
            });
        }
    }
    // An enum may implement an interface's method with one that every enum has; the dispatcher
    // knows `name()`, `ordinal()` and `toString()`, and `Object`'s methods.
    for e in (0..class.types.len()).filter(|&e| class.types[e].kind == TypeKind::Enum) {
        for (m, method) in class.methods.iter().enumerate() {
            // `Object`'s `equals` and `hashCode` are the constant's identity, and `Comparable`'s
            // `compareTo` compares the ordinals.
            // An interface may declare `Object`'s `equals` again (as `Comparator` does).
            let builtin = matches!(method.name.as_str(), "name" | "ordinal" | "toString" | "hashCode")
                && method.params.is_empty()
                || method.name == "equals" && method.params.len() == 1
                || matches!(class.types[method.class].name.as_str(), OBJECT | COMPARABLE);
            if method.is_abstract
                && class.hierarchy.is_subtype(e, method.class)
                && class.hierarchy.implementation(m, e).is_none()
                && !builtin
            {
                errors.push(Error {
                    pos: method.pos,
                    message: format!(
                        "an enum's own `{}` implementing `{}.{}` not supported yet",
                        method.name, class.types[method.class].name, method.name
                    ),
                });
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Whether the field initializers of class `t` are written into the struct literal of each of
/// its constructors: its constructors build a plain value, and the initializers use `this` only
/// to read fields that earlier initializers set.
pub fn inline_initializers(class: &Class, t: usize, facts: &Facts) -> bool {
    let decl = &class.types[t];
    (facts.owned.contains(&decl.name) || facts.plain_constructors.contains(&decl.name))
        && decl
            .fields
            .iter()
            .enumerate()
            .all(|(i, f)| f.init.as_ref().is_none_or(|e| reads_earlier_fields(decl, i, e)))
}

/// Whether the initializer `e` of field `i` uses `this` only to read fields declared before it
/// that have initializers: those have their initial values then, which a constructor computes
/// into locals first.
pub fn reads_earlier_fields(decl: &TypeDecl, i: usize, e: &Expr) -> bool {
    let mut allowed = FastSet::default();
    let mut selves = Vec::new();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::This => selves.push(x.id),
        ExprKind::Field { object, name }
            if matches!(object.kind, ExprKind::This)
                && decl.fields[..i].iter().any(|f| &f.name == name && f.init.is_some()) =>
        {
            allowed.insert(object.id);
        }
        _ => {}
    });
    selves.iter().all(|id| allowed.contains(id))
}

/// The leading statements of a constructor that assign a field of `this` a value that uses `this`
/// only to read fields that earlier ones assigned, each field once: the constructor's struct
/// literal takes them in, as (field index, value), computing those earlier ones into locals first
/// (see `reads_leading_fields`). Empty when the class's initializers are not inlined, or the
/// constructor starts with `this(...)`. A field whose initializer has effects is never taken, as
/// the initializer must run.
pub fn leading_assignments<'c>(class: &Class, ctor: &'c Method, facts: &Facts) -> Vec<(usize, &'c Expr)> {
    let decl = &class.types[ctor.class];
    if delegates(&ctor.body) || !inline_initializers(class, ctor.class, facts) {
        return Vec::new();
    }
    let mut out: Vec<(usize, &Expr)> = Vec::new();
    // `super(...)` runs first, before the initializers: the struct literal's `base`. The
    // statements before it run before the literal is built.
    let (_, _, body) = split_explicit_call(&ctor.body);
    for stmt in body {
        let Stmt::Expr(Expr {
            kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
            ..
        }) = stmt
        else {
            break;
        };
        if !matches!(object.kind, ExprKind::This) || !reads_only(value, decl, &out) {
            break;
        }
        // A field of a superclass is in `base`, built by `super(...)`: it is assigned after it.
        let Some(i) = decl.fields.iter().position(|f| &f.name == name) else { break };
        if out.iter().any(|&(j, _)| j == i) || decl.fields[i].init.as_ref().is_some_and(has_effects) {
            break;
        }
        out.push((i, value));
    }
    out
}

/// Whether `value` uses `this` only to read fields of `decl` that the assignments `earlier` set.
fn reads_only(value: &Expr, decl: &TypeDecl, earlier: &[(usize, &Expr)]) -> bool {
    let mut allowed = FastSet::default();
    let mut selves = Vec::new();
    visit_expr(value, &mut |x| match &x.kind {
        ExprKind::This => selves.push(x.id),
        ExprKind::Field { object, name }
            if matches!(object.kind, ExprKind::This)
                && earlier.iter().any(|&(j, _)| decl.fields[j].name == *name) =>
        {
            allowed.insert(object.id);
        }
        _ => {}
    });
    selves.iter().all(|id| allowed.contains(id))
}

/// Whether a leading assignment of `leading` reads a field that an earlier one set: the struct
/// literal then computes them into locals, in order, first.
pub fn reads_leading_fields(decl: &TypeDecl, leading: &[(usize, &Expr)]) -> bool {
    leading.iter().any(|&(_, value)| {
        let mut found = false;
        visit_expr(value, &mut |x| {
            found |= matches!(&x.kind, ExprKind::Field { object, name }
                if matches!(object.kind, ExprKind::This)
                    && leading.iter().any(|&(j, _)| decl.fields[j].name == *name))
        });
        found
    })
}

fn borrowed_params(class: &Class, facts: &mut Facts) {
    // A shared string keeps its identity only as a `java::JStr`, which is passed as it is.
    if class.shared_strings {
        return;
    }
    for (m, method) in class.methods.iter().enumerate() {
        for (i, p) in method.params.iter().enumerate() {
            if p.ty != Type::String || assigns_any(&method.body, &p.name) {
                continue;
            }
            if nullable_var(facts, m, &p.name) {
                facts.borrowed_optional_params.insert((m, i));
            } else {
                facts.borrowed_params.insert((m, i));
            }
        }
    }
    align_groups(class, &mut facts.borrowed_params);
    align_groups(class, &mut facts.borrowed_optional_params);
}

/// `Facts::lent_strings`, once `borrowed_params` and `ascii_length` are known.
fn lent_strings(class: &Class, facts: &mut Facts) {
    for (m, method) in class.methods.iter().enumerate() {
        for (i, p) in method.params.iter().enumerate() {
            let borrowed =
                facts.borrowed_params.contains(&(m, i)) || facts.borrowed_optional_params.contains(&(m, i));
            if borrowed && indexes(&method.body, &p.name, &facts.ascii_length) {
                facts.lent_strings.insert((m, i));
            }
        }
    }
    // The methods of a group take their parameters alike: one that reads it by index makes it a
    // `Str` in all, which those that do not read it so use as its text.
    let lent: Vec<(usize, usize)> = facts.lent_strings.iter().copied().collect();
    for (m, i) in lent {
        for &g in class.hierarchy.group_members(m) {
            facts.lent_strings.insert((g, i));
        }
    }
}

/// Whether `body` reads the string variable `name` by code unit index where it is not proved
/// ASCII (`ascii`): `charAt`, `indexOf`, `lastIndexOf`, `substring` or `length` of it.
fn indexes(body: &[Stmt], name: &str, ascii: &FastSet<ExprId>) -> bool {
    let mut found = false;
    for s in body {
        visit_stmt(
            s,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    let ExprKind::Builtin(builtin, args) = &x.kind else { return };
                    let indexed = matches!(
                        builtin,
                        Builtin::StringCharAt
                            | Builtin::StringLength
                            | Builtin::String(
                                StringOp::Substring | StringOp::IndexOf | StringOp::LastIndexOf
                            )
                    );
                    let mut text = args.first();
                    while let Some(Expr { kind: ExprKind::Cast(_, inner), .. }) = text {
                        text = Some(inner);
                    }
                    found |= indexed
                        && !ascii.contains(&x.id)
                        && matches!(text, Some(Expr { kind: ExprKind::Var(v), .. }) if v == name);
                })
            },
            &mut |_| {},
        );
    }
    found
}

/// Keeps a (method, parameter) pair only when every method of the method's override group has
/// it: a call may run any of them, so they take their parameters alike. Says whether it removed
/// any.
fn align_groups(class: &Class, set: &mut FastSet<(usize, usize)>) -> bool {
    let unaligned: Vec<(usize, usize)> = set
        .iter()
        .copied()
        .filter(|&(m, i)| class.hierarchy.group(m).iter().any(|&g| !set.contains(&(g, i))))
        .collect();
    for key in &unaligned {
        set.remove(key);
    }
    !unaligned.is_empty()
}

/// Parameters, as (method, parameter) indexes.
type Params = FastSet<(usize, usize)>;

/// The object parameters that methods borrow: those that can never be null (`&Rc<..>`) and
/// those that can (`&Option<Rc<..>>`).
fn borrowed_objects(class: &Class, facts: &Facts) -> (Params, Params) {
    let mut borrowed = Params::default();
    for (m, method) in class.methods.iter().enumerate() {
        for (i, p) in method.params.iter().enumerate() {
            if matches!(p.ty, Type::Class(..)) && !assigns_any(&method.body, &p.name) {
                borrowed.insert((m, i));
            }
        }
    }
    // A parameter passed on to a parameter that is not borrowed is used as a value.
    loop {
        let losing: Vec<(usize, usize)> = borrowed
            .iter()
            .copied()
            .filter(|&(m, i)| {
                let method = &class.methods[m];
                used_as_value(&method.body, &method.params[i].name, &borrowed)
            })
            .collect();
        for p in &losing {
            borrowed.remove(p);
        }
        if !align_groups(class, &mut borrowed) && losing.is_empty() {
            break;
        }
    }
    let (mut nullable, mut plain): (Params, Params) =
        borrowed.into_iter().partition(|&(m, i)| nullable_var(facts, m, &class.methods[m].params[i].name));
    // The methods of a group take a parameter alike, so it is null in all of them or in none.
    align_groups(class, &mut plain);
    align_groups(class, &mut nullable);
    (plain, nullable)
}

/// The `Var` node inside `e` when `e` reads the variable `name`, through any casts.
fn var_read(e: &Expr, name: &str) -> Option<ExprId> {
    match &e.kind {
        ExprKind::Var(v) if v == name => Some(e.id),
        ExprKind::Cast(_, inner) => var_read(inner, name),
        _ => None,
    }
}

/// Whether `stmts` use the object variable `name` other than through it, or by a copy that a local
/// variable takes: as the value of an assignment to a field, an element or a static, an argument
/// of a parameter that is not borrowed, a returned or thrown value, or an operand of anything but
/// a field access, a call on it, `instanceof`, `==`, `!=` or a concatenation. A read that the
/// method needs as a value (a local's, a cast's) clones the borrowed handle there.
fn used_as_value(stmts: &[Stmt], name: &str, borrowed: &FastSet<(usize, usize)>) -> bool {
    let mut allowed = FastSet::default();
    let mut uses = Vec::new();
    let mut on_expr = |e: &Expr| {
        visit_expr(e, &mut |x| match &x.kind {
            ExprKind::Var(v) if v == name => uses.push(x.id),
            ExprKind::Field { object, .. }
            | ExprKind::Assign { target: Place::Field { object, .. }, .. }
            | ExprKind::IncDec { target: Place::Field { object, .. }, .. } => {
                allowed.extend(var_read(object, name));
            }
            ExprKind::Assign { target: Place::Var(_), value, .. } => allowed.extend(var_read(value, name)),
            ExprKind::MethodCall { receiver, .. } => allowed.extend(var_read(receiver, name)),
            ExprKind::InstanceOf { value, .. } => allowed.extend(var_read(value, name)),
            ExprKind::Binary(BinOp::Eq | BinOp::Ne | BinOp::Add, l, r) => {
                allowed.extend(var_read(l, name));
                allowed.extend(var_read(r, name));
            }
            ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
                allowed.extend(args.iter().filter_map(|a| var_read(a, name)));
            }
            _ => {}
        });
        visit_expr(e, &mut |x| match &x.kind {
            ExprKind::Call { args, resolved: Some(callee), .. }
            | ExprKind::MethodCall { args, resolved: Some(callee), .. }
            | ExprKind::New { args, ctor: Some(callee), .. }
            | ExprKind::ThisCall { args, resolved: Some(callee) }
            | ExprKind::SuperInit { args, resolved: Some(callee) } => {
                for (j, a) in args.iter().enumerate() {
                    if let ExprKind::Var(v) = &a.kind
                        && v == name
                        && borrowed.contains(&(*callee, j))
                    {
                        allowed.insert(a.id);
                    }
                }
            }
            _ => {}
        });
    };
    let mut locals = Vec::new();
    for stmt in stmts {
        visit_stmt(stmt, &mut on_expr, &mut |s| {
            if let Stmt::Local { init: Some(init), .. } = s {
                locals.extend(var_read(init, name));
            }
        });
    }
    allowed.extend(locals);
    uses.iter().any(|id| !allowed.contains(id))
}

/// Which string variables, parameters and return values can hold a non-ASCII string. Starts from
/// "all ASCII" and marks what a non-ASCII value can reach, until nothing changes.
#[derive(Default)]
struct NonAscii {
    vars: FastSet<(usize, String)>,
    params: FastSet<(usize, usize)>,
    /// Override groups (`Hierarchy::group_id`) of which a method may return non-ASCII text, and
    /// those with a method that does not return a `String` (a type variable), not followed.
    returns: FastSet<usize>,
    untyped: FastSet<usize>,
    /// Instance and static fields, by name in any class: a name is followed only when every
    /// field of that name is a `String` (`string_fields`), and then it is non-ASCII when any of
    /// them may be.
    fields: FastSet<String>,
    string_fields: FastSet<String>,
    /// The `(char)` casts that the range analysis proves ASCII.
    chars: FastSet<ExprId>,
}

/// The method index that `NonAscii::ascii` takes for an expression outside any method (a field
/// initializer), where every variable may be non-ASCII.
const NO_METHOD: usize = usize::MAX;

impl NonAscii {
    fn var(&self, class: &Class, m: usize, name: &str) -> bool {
        let Some(method) = class.methods.get(m) else { return true };
        let param = method.params.iter().position(|p| p.name == name);
        self.vars.contains(&(m, name.to_string())) || param.is_some_and(|i| self.params.contains(&(m, i)))
    }

    fn field(&self, name: &str) -> bool {
        !self.string_fields.contains(name) || self.fields.contains(name)
    }

    /// Whether the method `callee` (or an override of it that a call may run) may return a
    /// non-ASCII string. A method that returns a type variable is not followed.
    fn returns(&self, class: &Class, callee: usize) -> bool {
        let group = class.hierarchy.group_id(callee);
        self.returns.contains(&group) || self.untyped.contains(&group)
    }

    /// Whether `e`, evaluated in method `m`, is ASCII text (or not a string at all).
    fn ascii(&self, class: &Class, m: usize, e: &Expr) -> bool {
        if e.ty == Type::Char {
            // A char is ASCII when it is a constant, or a cast the range analysis keeps small.
            return match e.kind {
                ExprKind::Char(c) => c < 0x80,
                ExprKind::Cast(..) => self.chars.contains(&e.id),
                _ => false,
            };
        }
        if e.ty != Type::String {
            return true;
        }
        match &e.kind {
            ExprKind::Str(s) => s.is_ascii(),
            ExprKind::Var(name) => !self.var(class, m, name),
            ExprKind::Binary(_, l, r) => self.ascii(class, m, l) && self.ascii(class, m, r),
            ExprKind::Assign { op: None, value, .. } => self.ascii(class, m, value),
            ExprKind::Assign { target: Place::Var(target), value, .. } => {
                !self.var(class, m, target) && self.ascii(class, m, value)
            }
            ExprKind::Assign { target: Place::Field { name, .. }, value, .. } => {
                !self.field(name) && self.ascii(class, m, value)
            }
            ExprKind::Assign { target: Place::Static { name, .. }, value, .. } => {
                !self.field(name) && self.ascii(class, m, value)
            }
            ExprKind::Field { name, .. } | ExprKind::Static { name, .. } => !self.field(name),
            ExprKind::Call { resolved: Some(callee), .. }
            | ExprKind::MethodCall { resolved: Some(callee), is_super: false, .. } => {
                !self.returns(class, *callee)
            }
            ExprKind::Conditional(_, a, b) => self.ascii(class, m, a) && self.ascii(class, m, b),
            // Slicing, trimming, case mapping, repeating and replacing ASCII text gives ASCII.
            ExprKind::Builtin(
                Builtin::String(
                    StringOp::Substring
                    | StringOp::Trim
                    | StringOp::Strip
                    | StringOp::StripLeading
                    | StringOp::StripTrailing
                    | StringOp::ToUpperCase
                    | StringOp::ToLowerCase
                    | StringOp::Repeat
                    | StringOp::Concat
                    | StringOp::Replace
                    | StringOp::Identity
                    | StringOp::ToString
                    | StringOp::Intern,
                ),
                args,
            ) => args.iter().all(|a| self.ascii(class, m, a)),
            _ => false,
        }
    }
}

fn ascii_strings(class: &Class, facts: &mut Facts) {
    let mut non_ascii = NonAscii { chars: facts.ascii_chars.clone(), ..NonAscii::default() };
    let all_fields = || class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics));
    let other_fields: FastSet<&str> =
        all_fields().filter(|f| f.ty != Type::String).map(|f| f.name.as_str()).collect();
    non_ascii.string_fields = all_fields()
        .filter(|f| f.ty == Type::String && !other_fields.contains(f.name.as_str()))
        .map(|f| f.name.clone())
        .collect();
    // An enum's fields hold the values of its constants' arguments, which no assignment shows.
    for t in &class.types {
        let Some(constants) = &t.enum_constants else { continue };
        let literal = |v: &Expr| v.ty != Type::String || matches!(&v.kind, ExprKind::Str(s) if s.is_ascii());
        if !constants.iter().all(|c| c.values.iter().all(literal)) {
            non_ascii.fields.extend(t.fields.iter().filter(|f| f.ty == Type::String).map(|f| f.name.clone()));
        }
    }
    for (m, method) in class.methods.iter().enumerate() {
        if method.ret != Type::String {
            non_ascii.untyped.insert(class.hierarchy.group_id(m));
        }
    }
    // Field initializers are not walked below and have no method's variables to look up, so
    // every parameter that a call in one reaches may be non-ASCII; so are the parameters of the
    // constructors that build the exceptions the runtime raises, from the runtime's message.
    let initializers = all_fields().filter_map(|f| f.init.as_ref()).chain(&class.raised);
    for init in initializers {
        visit_expr(init, &mut |x| match &x.kind {
            ExprKind::Call { args, resolved: Some(callee), .. }
            | ExprKind::MethodCall { args, resolved: Some(callee), .. }
            | ExprKind::ThisCall { args, resolved: Some(callee) }
            | ExprKind::SuperInit { args, resolved: Some(callee) }
            | ExprKind::New { args, ctor: Some(callee), .. } => {
                for g in class.hierarchy.group(*callee) {
                    non_ascii.params.extend((0..args.len()).map(|i| (g, i)));
                }
            }
            _ => {}
        });
    }
    loop {
        let mut vars = Vec::new();
        let mut declared = Vec::new();
        let mut params = Vec::new();
        let mut returns = Vec::new();
        let mut fields: Vec<String> = all_fields()
            .filter(|f| f.init.as_ref().is_some_and(|init| !non_ascii.ascii(class, NO_METHOD, init)))
            .map(|f| f.name.clone())
            .collect();
        for (m, method) in class.methods.iter().enumerate() {
            for stmt in &method.body {
                let mut on_expr = |e: &Expr| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Assign { target: Place::Var(target), .. } if x.ty == Type::String => {
                            if !non_ascii.ascii(class, m, x) {
                                vars.push((m, target.clone()));
                            }
                        }
                        ExprKind::Assign {
                            target: Place::Field { name, .. } | Place::Static { name, .. },
                            ..
                        } if x.ty == Type::String => {
                            if !non_ascii.ascii(class, m, x) {
                                fields.push(name.clone());
                            }
                        }
                        // A string that a pattern binds is any string an `Object` holds.
                        ExprKind::InstanceOf { ty: Type::String, binding: Some(name), .. } => {
                            vars.push((m, name.clone()));
                        }
                        ExprKind::Call { args, resolved: Some(callee), .. }
                        | ExprKind::MethodCall { args, resolved: Some(callee), .. }
                        | ExprKind::ThisCall { args, resolved: Some(callee) }
                        | ExprKind::SuperInit { args, resolved: Some(callee) }
                        | ExprKind::New { args, ctor: Some(callee), .. } => {
                            for (i, arg) in args.iter().enumerate() {
                                if !non_ascii.ascii(class, m, arg) {
                                    params.extend(class.hierarchy.group(*callee).into_iter().map(|g| (g, i)));
                                }
                            }
                        }
                        _ => {}
                    })
                };
                let mut on_stmt = |s: &Stmt| match s {
                    Stmt::Local { ty: Type::String, name, init: Some(init), .. }
                        if !non_ascii.ascii(class, m, init) =>
                    {
                        declared.push((m, name.clone()))
                    }
                    // The elements of a string array are not followed: each may be non-ASCII.
                    Stmt::ForEach { ty: Type::String, name, .. } => declared.push((m, name.clone())),
                    Stmt::Return(Some(value), _) if !non_ascii.ascii(class, m, value) => {
                        returns.push(class.hierarchy.group_id(m))
                    }
                    _ => {}
                };
                visit_stmt(stmt, &mut on_expr, &mut on_stmt);
            }
        }
        let sizes = |n: &NonAscii| (n.vars.len(), n.params.len(), n.returns.len(), n.fields.len());
        let before = sizes(&non_ascii);
        non_ascii.vars.extend(vars);
        non_ascii.vars.extend(declared);
        non_ascii.params.extend(params);
        non_ascii.returns.extend(returns);
        non_ascii.fields.extend(fields);
        if before == sizes(&non_ascii) {
            break;
        }
    }
    for (m, method) in class.methods.iter().enumerate() {
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Builtin(
                            Builtin::StringLength
                            | Builtin::StringCharAt
                            | Builtin::String(StringOp::Substring),
                            args,
                        ) = &x.kind
                            && (args[0].ty == Type::String
                                || !matches!(x.kind, ExprKind::Builtin(Builtin::String(_), _)))
                            && non_ascii.ascii(class, m, &args[0])
                        {
                            facts.ascii_length.insert(x.id);
                        }
                    })
                },
                &mut |_| {},
            );
        }
    }
}

/// The value that `e` reads where it is a local that lowering holds a call's argument or receiver
/// in until the call (`lower/argument_order.rs`), in method `m`; else `e`.
pub(crate) fn held_value<'c>(class: &'c Class, m: usize, e: &'c Expr) -> &'c Expr {
    let ExprKind::Var(name) = &e.kind else { return e };
    if !crate::lower::held_local(name) {
        return e;
    }
    let Some(method) = class.methods.get(m) else { return e };
    let mut found = None;
    for stmt in &method.body {
        visit_stmt(stmt, &mut |_| {}, &mut |s| {
            if let Stmt::Local { name: local, init: Some(init), .. } = s
                && local == name
            {
                found = Some(init);
            }
        });
    }
    found.unwrap_or(e)
}

/// A hash of indices, which need no protection from chosen keys.
#[derive(Default)]
pub struct IndexHasher(u64);

impl std::hash::Hasher for IndexHasher {
    /// A name, eight bytes at a time.
    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<8>();
        for word in words {
            self.write_usize(u64::from_le_bytes(*word) as usize);
        }
        for &b in rest {
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

pub(crate) type FastMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<IndexHasher>>;
pub type FastSet<T> = std::collections::HashSet<T, std::hash::BuildHasherDefault<IndexHasher>>;

/// A set of locals and parameters, as (method, name), that is asked about without making a
/// `String` for the name.
#[derive(Clone, Default)]
pub struct MethodLocals {
    by_method: FastMap<usize, FastSet<String>>,
    len: usize,
}

impl MethodLocals {
    pub fn contains(&self, m: usize, name: &str) -> bool {
        self.by_method.get(&m).is_some_and(|names| names.contains(name))
    }

    pub fn insert(&mut self, m: usize, name: String) -> bool {
        let added = self.by_method.entry(m).or_default().insert(name);
        self.len += added as usize;
        added
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Extend<(usize, String)> for MethodLocals {
    fn extend<I: IntoIterator<Item = (usize, String)>>(&mut self, items: I) {
        for (m, name) in items {
            self.insert(m, name);
        }
    }
}
