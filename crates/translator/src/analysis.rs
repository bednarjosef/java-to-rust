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

mod arrays;
mod borrowing;
mod boxing;
pub mod closures;
mod constants;
mod exceptions;
pub mod hierarchy;
mod indexing;
mod moves;
mod nullness;
mod objects;
mod owned;
mod patterns;
mod ranges;
mod statics;
mod threads;
mod whole_objects;

use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub use arrays::{RowIndex, Slice, SlicedLoop};
pub use nullness::{conjuncts, maybe_null, non_null_names, null_test, nullable_type, nullable_var};
pub use whole_objects::whole_objects;

#[derive(Default)]
pub struct Facts {
    /// Whether the program starts a thread (see `threads`).
    pub threaded: bool,
    /// The classes, by type index, whose objects two threads may reach: `Arc`s, with a
    /// `java::Locked` unless immutable.
    pub thread_shared: HashSet<usize>,
    /// The methods of those classes that must not borrow their object for the whole call.
    pub long_methods: HashSet<usize>,
    /// Integer `+`, `-`, `*`, negation, `++`, `--`, compound assignments and `Math.abs` that
    /// never overflow.
    pub no_overflow: HashSet<ExprId>,
    /// The values that the index of each array indexing or element assignment takes, from the
    /// range analysis: (lowest, highest), empty when `lowest > highest`. Each size of a
    /// `new T[n]` has its values here too, keyed by the size expression.
    pub index_ranges: HashMap<ExprId, (i64, i64)>,
    /// Reads of string, array and object variables that are last uses: the value is moved, not
    /// cloned (see `moves`).
    pub moves: HashSet<ExprId>,
    /// Integer `/`, `%` and their compound assignments whose divisor is never zero and that never
    /// compute `MIN / -1`.
    pub safe_division: HashSet<ExprId>,
    /// `String.length()` and `String.charAt` calls on strings that are always ASCII.
    pub ascii_length: HashSet<ExprId>,
    /// `(char)` casts whose integer value the range analysis keeps within ASCII.
    pub ascii_chars: HashSet<ExprId>,
    /// `s.charAt(i)` calls whose index is always within the string.
    pub index_in_bounds: HashSet<ExprId>,
    /// String parameters, as (method, parameter) indexes, that are never assigned.
    pub borrowed_params: HashSet<(usize, usize)>,
    /// String parameters that are never assigned but can be null: `Option<&str>`.
    pub borrowed_optional_params: HashSet<(usize, usize)>,
    /// Object parameters, as (method, parameter) indexes, that the method only uses through: it
    /// reads and writes their fields, calls their methods, compares or prints them, and passes
    /// them on to such parameters. They are `&Rc<..>`, which the caller lends.
    pub borrowed_objects: HashSet<(usize, usize)>,
    /// String variables and parameters, as (method, name), that can hold null.
    pub nullable_vars: HashSet<(usize, String)>,
    /// Methods whose string or object return value can be null.
    pub nullable_returns: HashSet<usize>,
    /// String and object fields, as (class, field), that can hold null: `Option`s.
    pub nullable_fields: HashSet<(String, String)>,
    /// String and object static fields, as (class index, field), that can hold null.
    pub nullable_statics: HashSet<(usize, String)>,
    /// Statics of object type, of a class initialized eagerly, that are never null once the class
    /// is initialized: their cell starts empty, as there is no object to start it with, so it is
    /// an `Option` that is written with `Some` and read with `java::non_null`.
    pub placeholder_statics: HashSet<(usize, String)>,
    /// The element types (strings, objects and arrays) whose arrays can hold null: their
    /// elements are `Option`s, in every array of that element type.
    pub nullable_elements: HashSet<Type>,
    /// Array locals and borrowed array parameters, as (method, name), that no other name can
    /// see: plain `Vec`s (of `Vec`s, for arrays of arrays). The flag says whether an element of
    /// the array, at any depth, is ever assigned.
    pub plain_arrays: HashMap<(usize, String), bool>,
    /// Accesses of plain arrays (`a[i]` reads, element assignments and rows `m[i]` of `m[i][j]`)
    /// whose index is always in bounds.
    pub in_bounds: HashSet<ExprId>,
    /// Array parameters, as (method, parameter) indexes, that are plain at every call: `&[T]`, or
    /// `&mut [T]` when the flag says the elements are written.
    pub borrowed_arrays: HashMap<(usize, usize), bool>,
    /// The variables of for-each loops over plain arrays of arrays, as (method, name): each
    /// borrows the rows in turn (`for row in m.iter()`), as `&Vec<T>` or `&mut Vec<T>`.
    pub row_loops: HashSet<(usize, String)>,
    /// Innermost counting loops over plain arrays whose accesses are all in bounds when a check
    /// made before the loop passes: the loop then runs over slices, keyed by its condition.
    pub sliced_loops: HashMap<ExprId, SlicedLoop>,
    /// Methods that return arrays nobody else holds: they return the plain `Vec`.
    pub owned_returns: HashSet<usize>,
    /// Counting loops over a plain array whose body only reads `a[i]`, keyed by the loop
    /// condition: the reads that can become elements of `for x in a.iter()`.
    pub element_loops: HashMap<ExprId, Vec<ExprId>>,
    /// The classes whose objects never change after construction, by qualified name.
    pub immutable: HashSet<String>,
    /// The classes whose objects are never shared, so they are plain values, by qualified name.
    pub owned: HashSet<String>,
    /// For each type (index into `Class::types`), the concrete classes and enums whose values
    /// become values of that type (see `hierarchy::flows`).
    pub flows: Vec<HashSet<usize>>,
    /// The instance methods (indexes into `Class::methods`) that change their object.
    pub mutating: HashSet<usize>,
    /// The instance methods that borrow their object for the call: `&self` or `&mut self`.
    pub self_methods: HashSet<usize>,
    /// The classes whose constructors build a plain value, which `new` then shares.
    pub plain_constructors: HashSet<String>,
    /// Methods and constructors that touch no object but their own, so they borrow no other.
    pub isolated: HashSet<usize>,
    /// Methods and constructors that may borrow a shared object mutably, through what they call.
    pub writers: HashSet<usize>,
    /// The classes, by index in `Class::types`, whose static initialization runs eagerly through
    /// an `init` function; the other classes initialize each static lazily on first access.
    pub eager_init: HashSet<usize>,
    /// The static methods and constructors that run their class's `init` when they start.
    pub entry_init: HashSet<usize>,
    /// The boxed kinds (`Type::Boxed` of the primitive) whose identity `==` observes: their boxes
    /// are shared objects. Every other boxed kind is its primitive value.
    pub faithful_boxes: HashSet<Type>,
    /// The classes with subclasses (indexes into `Class::types`) whose fields are a struct of
    /// their own, `<Name>Fields`, that the structs of their subclasses embed as `base`.
    pub fields_structs: HashSet<usize>,
    /// The methods of classes with subclasses that live on the class's fields struct.
    pub field_level: HashSet<usize>,
    /// The class that declares each field visible in each class: (class, field) to class, by
    /// qualified name.
    pub field_owners: HashMap<(String, String), String>,
    /// The methods and constructors that return `Result<_, Throwable>`: some caller observes an
    /// exception that may escape them (see `exceptions`).
    pub throwing: HashSet<usize>,
    /// The `try` statements, by address, whose `catch` clauses catch an exception that their body
    /// may throw. The others run their body as a plain block.
    pub live_tries: HashSet<usize>,
    /// The `try` statements, by address, whose body or `catch` clauses an exception of the
    /// program may leave or reach (which their `finally` must see).
    pub raising_tries: HashSet<usize>,
    /// The operations whose failure a `catch` or a `finally` may see, which are written with the
    /// runtime's checked form (see `analysis::exceptions`).
    pub raising: HashSet<ExprId>,
    /// The objects, by expression, whose dereference by a call or a field access may throw a
    /// `NullPointerException` that a `catch` or a `finally` may see: they are written with the
    /// runtime's checked `non_null`. With the position of the call or the field access.
    pub null_checked: HashMap<ExprId, Pos>,
    /// The standard classes whose objects the runtime raises where the program may see them.
    pub raised_classes: std::collections::BTreeSet<usize>,
    /// The functional interfaces whose values are closures, `Rc<dyn Fn(A) -> R>` (see
    /// `closures`).
    pub closure_types: HashSet<usize>,
}

impl Facts {
    /// The class whose `init` function initializing class `t` runs: `t` when its initialization
    /// is eager, else its nearest superclass whose initialization is, which Java initializes
    /// first (its `init` starts by initializing its own superclass).
    pub fn init_class(&self, class: &Class, t: usize) -> Option<usize> {
        std::iter::once(t).chain(class.hierarchy.ancestors(t)).find(|c| self.eager_init.contains(c))
    }

    /// Whether the field `name` of an object of type `ty` can hold null.
    pub fn field_nullable(&self, ty: Type, name: &str) -> bool {
        let Type::Class(c, _) = ty else { return false };
        let owner = self.field_owners.get(&(c.to_string(), name.to_string())).map_or(c, String::as_str);
        self.nullable_fields.contains(&(owner.to_string(), name.to_string()))
    }
}

pub fn analyze(class: &Class) -> Result<Facts, Error> {
    let mut facts = Facts {
        fields_structs: hierarchy::fields_structs(class),
        field_owners: hierarchy::field_owners(class),
        closure_types: closures::closure_types(class),
        ..Facts::default()
    };
    ranges::analyze(class, &mut facts);
    representation(class, &mut facts);
    validate(class, &facts)?;
    patterns::check(class)?;
    facts.eager_init = (0..class.types.len()).filter(|&t| statics::eager(class, t)).collect();
    facts.entry_init = statics::entry_inits(class, &facts.eager_init);
    nullness::analyze(class, &mut facts);
    patterns::check_object_values(class, &facts)?;
    boxing::analyze(class, &mut facts);
    if facts.threaded {
        threads::check(class, &facts)?;
    }
    borrowed_params(class, &mut facts);
    facts.borrowed_objects = borrowed_objects(class, &facts);
    ascii_strings(class, &mut facts);
    arrays::analyze(class, &mut facts);
    indexing::analyze(class, &mut facts);
    moves::analyze(class, &mut facts);
    exceptions::analyze(class, &mut facts)?;
    // The exceptions that the runtime raises are values of `Throwable` too.
    if !facts.raised_classes.is_empty() {
        facts.flows = hierarchy::flows(class, &facts.field_level, &facts.raised_classes);
    }
    Ok(facts)
}

/// How the objects of each class are represented and which methods borrow them: the facts from
/// `immutable` to `writers`, given `fields_structs`.
fn representation(class: &Class, facts: &mut Facts) {
    facts.threaded = threads::threaded(class);
    if facts.threaded {
        facts.thread_shared = threads::thread_shared(class);
        facts.long_methods = threads::long_methods(class, &facts.thread_shared);
    }
    let reach = borrowing::reach(class);
    facts.immutable = objects::immutable_classes(class);
    facts.field_level = hierarchy::field_level(
        class,
        &reach.reach,
        &facts.immutable,
        &facts.fields_structs,
        &facts.long_methods,
    );
    facts.owned = owned::owned_classes(class, &facts.immutable, &facts.field_level);
    facts.flows = hierarchy::flows(class, &facts.field_level, &Default::default());
    // An exception is converted to `Throwable` when it is thrown, so it is never a plain value.
    if let Some(throwable) = class.type_index("java.lang.Throwable") {
        facts.owned.retain(|c| class.type_index(c).is_none_or(|t| !class.hierarchy.is_subtype(t, throwable)));
    }
    let borrowing = borrowing::borrowing(class, facts, &reach);
    facts.mutating = borrowing.mutating;
    facts.self_methods = borrowing.methods;
    facts.plain_constructors = borrowing.plain_constructors;
    facts.isolated = borrowing.isolated;
    facts.writers = borrowing.writers;
}

/// The classes with subclasses whose constructors cannot build their fields struct as a plain
/// value: they use `this` as a value or call an overridable method. `whole_objects` gives them
/// the second constructor form.
fn whole_object_classes(class: &Class) -> HashSet<usize> {
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
/// use `this` only for that struct.
fn validate(class: &Class, facts: &Facts) -> Result<(), Error> {
    for &t in &facts.fields_structs {
        if facts.plain_constructors.contains(&class.types[t].name) {
            continue;
        }
        let pos = class.methods.iter().find(|m| m.is_ctor && m.class == t).map(|m| m.pos).unwrap_or_default();
        return Err(Error {
            pos,
            message:
                "constructors of a class with subclasses that use `this` as a value or call overridable \
                      methods not supported yet"
                    .to_string(),
        });
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
                return Err(Error {
                    pos: method.pos,
                    message: format!(
                        "an enum's own `{}` implementing `{}.{}` not supported yet",
                        method.name, class.types[method.class].name, method.name
                    ),
                });
            }
        }
    }
    Ok(())
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
    let mut allowed = HashSet::new();
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

/// The leading statements of a constructor that assign a field of `this` a value that does not
/// read `this`, each field once: the constructor's struct literal takes them in, as (field index,
/// value). Empty when the class's initializers are not inlined, or the constructor starts with
/// `this(...)`. A field whose initializer has effects is never taken, as the initializer must run.
pub fn leading_assignments<'c>(class: &Class, ctor: &'c Method, facts: &Facts) -> Vec<(usize, &'c Expr)> {
    let decl = &class.types[ctor.class];
    let delegates =
        matches!(ctor.body.first(), Some(Stmt::Expr(Expr { kind: ExprKind::ThisCall { .. }, .. })));
    if delegates || !inline_initializers(class, ctor.class, facts) {
        return Vec::new();
    }
    let mut out: Vec<(usize, &Expr)> = Vec::new();
    // `super(...)` runs first, before the initializers: the struct literal's `base`.
    let body = match ctor.body.first() {
        Some(Stmt::Expr(Expr { kind: ExprKind::SuperInit { .. }, .. })) => &ctor.body[1..],
        _ => &ctor.body[..],
    };
    for stmt in body {
        let Stmt::Expr(Expr {
            kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
            ..
        }) = stmt
        else {
            break;
        };
        if !matches!(object.kind, ExprKind::This) || mentions_this(value) {
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

fn borrowed_params(class: &Class, facts: &mut Facts) {
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

/// Keeps a (method, parameter) pair only when every method of the method's override group has
/// it: a call may run any of them, so they take their parameters alike. Says whether it removed
/// any.
fn align_groups(class: &Class, set: &mut HashSet<(usize, usize)>) -> bool {
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

fn borrowed_objects(class: &Class, facts: &Facts) -> HashSet<(usize, usize)> {
    let mut borrowed: HashSet<(usize, usize)> = HashSet::new();
    for (m, method) in class.methods.iter().enumerate() {
        for (i, p) in method.params.iter().enumerate() {
            if matches!(p.ty, Type::Class(..))
                && !nullable_var(facts, m, &p.name)
                && !assigns_any(&method.body, &p.name)
            {
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
                let name = &method.params[i].name;
                let mut value = false;
                for stmt in &method.body {
                    visit_stmt(stmt, &mut |e| value |= used_as_value(e, name, &borrowed), &mut |_| {});
                }
                value
            })
            .collect();
        for p in &losing {
            borrowed.remove(p);
        }
        if !align_groups(class, &mut borrowed) && losing.is_empty() {
            return borrowed;
        }
    }
}

/// Whether `e` uses the object variable `name` other than through it: as the value of an
/// assignment, an argument of a parameter that is not borrowed, or an operand of anything but a
/// field access, a call on it, `==`, `!=` or a concatenation.
fn used_as_value(e: &Expr, name: &str, borrowed: &HashSet<(usize, usize)>) -> bool {
    let is_name = |x: &Expr| matches!(&x.kind, ExprKind::Var(v) if v == name);
    let mut allowed = HashSet::new();
    let mut uses = Vec::new();
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Var(v) if v == name => uses.push(x.id),
        ExprKind::Field { object, .. }
        | ExprKind::Assign { target: Place::Field { object, .. }, .. }
        | ExprKind::IncDec { target: Place::Field { object, .. }, .. }
            if is_name(object) =>
        {
            allowed.insert(object.id);
        }
        ExprKind::MethodCall { receiver, .. } if is_name(receiver) => {
            allowed.insert(receiver.id);
        }
        ExprKind::InstanceOf { value, .. } if is_name(value) => {
            allowed.insert(value.id);
        }
        ExprKind::Binary(BinOp::Eq | BinOp::Ne | BinOp::Add, l, r) => {
            allowed.extend([l, r].into_iter().filter(|o| is_name(o)).map(|o| o.id));
        }
        ExprKind::Builtin(Builtin::Print | Builtin::Println, args) => {
            allowed.extend(args.iter().filter(|a| is_name(a)).map(|a| a.id));
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
                if is_name(a) && borrowed.contains(&(*callee, j)) {
                    allowed.insert(a.id);
                }
            }
        }
        _ => {}
    });
    uses.iter().any(|id| !allowed.contains(id))
}

/// Which string variables, parameters and return values can hold a non-ASCII string. Starts from
/// "all ASCII" and marks what a non-ASCII value can reach, until nothing changes.
#[derive(Default)]
struct NonAscii {
    vars: HashSet<(usize, String)>,
    params: HashSet<(usize, usize)>,
    returns: HashSet<usize>,
    /// The `(char)` casts that the range analysis proves ASCII.
    chars: HashSet<ExprId>,
}

impl NonAscii {
    fn var(&self, class: &Class, m: usize, name: &str) -> bool {
        let param = class.methods[m].params.iter().position(|p| p.name == name);
        self.vars.contains(&(m, name.to_string())) || param.is_some_and(|i| self.params.contains(&(m, i)))
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
            ExprKind::Call { resolved: Some(callee), .. } => !self.returns.contains(callee),
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
                    | StringOp::Identity,
                ),
                args,
            ) => args.iter().all(|a| self.ascii(class, m, a)),
            _ => false,
        }
    }
}

fn ascii_strings(class: &Class, facts: &mut Facts) {
    let mut non_ascii = NonAscii { chars: facts.ascii_chars.clone(), ..NonAscii::default() };
    // Field initializers are not walked below and have no method's variables to look up, so
    // every parameter that a call in one reaches may be non-ASCII.
    let initializers = class.types.iter().flat_map(|t| t.fields.iter().chain(&t.statics));
    for init in initializers.filter_map(|f| f.init.as_ref()) {
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
        for (m, method) in class.methods.iter().enumerate() {
            for stmt in &method.body {
                let mut on_expr = |e: &Expr| {
                    visit_expr(e, &mut |x| match &x.kind {
                        ExprKind::Assign { target: Place::Var(target), .. } if x.ty == Type::String => {
                            if !non_ascii.ascii(class, m, x) {
                                vars.push((m, target.clone()));
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
                    Stmt::Return(Some(value), _) if !non_ascii.ascii(class, m, value) => returns.push(m),
                    _ => {}
                };
                visit_stmt(stmt, &mut on_expr, &mut on_stmt);
            }
        }
        let before = (non_ascii.vars.len(), non_ascii.params.len(), non_ascii.returns.len());
        non_ascii.vars.extend(vars);
        non_ascii.vars.extend(declared);
        non_ascii.params.extend(params);
        non_ascii.returns.extend(returns);
        if before == (non_ascii.vars.len(), non_ascii.params.len(), non_ascii.returns.len()) {
            break;
        }
    }
    for (m, method) in class.methods.iter().enumerate() {
        for stmt in &method.body {
            visit_stmt(
                stmt,
                &mut |e| {
                    visit_expr(e, &mut |x| {
                        if let ExprKind::Builtin(Builtin::StringLength | Builtin::StringCharAt, args) =
                            &x.kind
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
