//! Lowers the syntax tree that javac printed (see `java/JavaTree.java`) into the typed syntax
//! tree of `ast`. javac has already checked the program, given every expression its type and
//! resolved every name and call, so lowering only maps them onto the supported subset. Anything
//! outside it is reported as an error naming the construct.

mod abstract_sequence;
mod code_point;
mod covariant;
mod element_reads;
mod enum_tables;
mod enums;
mod exceptions;
mod file_channels;
mod foreach;
mod functional;
mod generic_args;
mod generic_types;
mod generics;
mod handle_values;
mod handles;
mod hierarchy;
mod lambdas;
mod local_names;
mod object;
mod object_clone;
mod objects;
mod real_generics;
mod references;
mod scans;
mod statics;
mod strings;
mod stubs;
mod suppress;
mod unsigned;

use crate::ast::*;
use crate::value::Value;
use objects::{MethodInfo, TypeInfo};
use std::cell::{Cell, RefCell};

mod annotation_objects;
mod annotation_presence;
mod annotation_values;
mod argument_order;
mod arraycopy_classes;
mod arrays;
mod boxing;
mod by_name;
mod callers;
mod captures;
mod census;
mod channels;
mod charsets;
mod classes;
mod clinit_errors;
mod constant_bodies;
mod initializers;
mod inner;
mod java_arrays;
mod java_objects;
mod libraries;
mod local_types;
mod modules;
mod natives;
mod negated_patterns;
mod packages;
mod patterns;
mod primitives;
mod processors;
mod program;
mod prologues;
mod proxies;
mod recreate;
mod reflection;
mod regex;
mod sun_unsafe;
mod switch;
mod sync;
pub(crate) mod system;
mod varargs;
pub use annotation_values::is_member_annotation_call;
pub(crate) use argument_order::{ARGUMENT, RECEIVER, held_local, ordered_call};
pub use arrays::runtime_arrays_call;
use arrays::{is_array_length, reads_name};
pub use captures::raw as raw_type;
pub use census::{panic_carried, panic_message, panic_place, quiet_panics};
pub use charsets::{Lookup as CharsetLookup, lookups as charset_lookups};
pub(crate) use classes::ENUM_CONSTANTS;
pub(crate) use clinit_errors::{STATE, rewritten_clinit};
pub(crate) use enum_tables::{ORDINAL as TABLE_ORDINAL, tabled_field};
pub use exceptions::{
    is_standard, standard_field_type, standard_local_name, standard_names, standard_subclass,
};
pub use functional::is_functional;
pub(crate) use handle_values::{UNFOLLOWED, UNFOLLOWED_MESSAGE};
pub(crate) use handles::UNRESOLVED_OFFSET;
pub(crate) use inner::OUTER;
pub use lambdas::computed_receiver;
pub use primitives::runtime_character_method;
pub use program::{Lowered, Rejected, census_lower_program, lower_program, tolerant_lower_program};

/// The UTF-16 code units of a string that `JavaTree` wrote: a JSON string, or the array of its code
/// units where it has a lone surrogate, which JSON cannot hold (`JavaTree.string`).
fn string_units(value: &Value) -> Option<Vec<u16>> {
    if let Some(text) = value.as_str() {
        return Some(text.encode_utf16().collect());
    }
    value.as_array()?.iter().map(|u| u.as_u64().and_then(|u| u16::try_from(u).ok())).collect()
}

/// The literal of the string of code units `units`: a `Str` where they are well-formed UTF-16, else
/// `Units`.
fn string_literal(units: Vec<u16>) -> ExprKind {
    match String::from_utf16(&units) {
        Ok(text) => ExprKind::Str(text),
        Err(_) => ExprKind::Units(units),
    }
}

pub fn lower(unit: &Value) -> Result<Class, Error> {
    let unit = Node(unit);
    charsets::check(unit)?;
    // A `;` at top level and an annotation type (`@interface`) do nothing when the program runs.
    let decls: Vec<Node> = unit
        .list("typeDecls")
        .filter(|n| !matches!(n.kind(), "EMPTY_STATEMENT" | "ANNOTATION_TYPE"))
        .collect();
    let Some(first) = decls.first().copied() else { return error(unit.pos(), "no class found") };
    let scans = std::rc::Rc::default();
    real_generics::retry(|real, covariant| {
        let mut lowering = Lowering::declare(&decls)?;
        lowering.scans = std::rc::Rc::clone(&scans);
        let annotations = unit.list("typeDecls").filter(|n| n.kind() == "ANNOTATION_TYPE");
        lowering.annotation_types.extend(annotations.map(|n| (crate::program::class_name(n.0), n)));
        lowering.covariant = covariant.clone();
        let trees: Vec<&Value> = decls.iter().map(|d| d.0).collect();
        lowering.standard = lowering.scans.standard(&trees);
        lowering.declare_standard()?;
        let declared = lowering.types.iter().map(|t| t.name.clone()).collect();
        // The standard classes may name interfaces too (`Thread` holds a `Runnable`).
        let trees: Vec<&Value> =
            trees.into_iter().chain(lowering.standard.classes.iter().map(|(_, d)| *d)).collect();
        lowering.functional = lowering.scans.functional(&trees, &declared);
        lowering.declare_functional()?;
        lowering.declare_plain_object(decls.iter().map(|d| d.0))?;
        lowering.name_nested();
        lowering.plan_captures().map_err(|(_, e)| e)?;
        lowering.real = real.cloned().unwrap_or_else(|| lowering.candidates());
        let planned: Vec<Node> = decls.iter().copied().chain(lowering.functional_decls()).collect();
        lowering.plan_object(&planned);
        let mut types: Vec<TypeDecl> =
            (0..lowering.types.len()).map(|t| lowering.type_decl(t)).collect::<Result<_, _>>()?;
        let mut methods: Vec<Method> =
            (0..lowering.methods.len()).map(|m| lowering.method(m)).collect::<Result<_, _>>()?;
        lowering.suppress_checks(&mut methods);
        lowering.init_cause_checks(&mut methods);
        lowering.object_methods(&mut methods)?;
        lowering.finish_types(&mut types, &mut methods).map_err(|(_, e)| e)?;
        lowering.object_types(&mut types);
        lowering.param_decls(&mut types);
        let failed = lowering.verify(&types, &methods);
        let hierarchy = Hierarchy::new(&types, &methods);
        let raised = lowering.raised_objects(&types, &methods);
        let name = first.str("simpleName").to_string();
        let class_file = lowering.class_file_names(&types);
        let notes = lowering.notes.take();
        let annotation_classes = lowering.annotation_classes.take();
        let module_labels = Default::default();
        let class = Class {
            name,
            types,
            methods,
            hierarchy,
            raised,
            class_file,
            notes,
            annotation_classes,
            shared_strings: false,
            module_labels,
            thread_statics: None,
            element_classes: lowering.element_classes.take(),
            object_rows: lowering.object_rows.take(),
            array_tests: lowering.array_tests.take(),
            volatile_elements: lowering.volatile_elements.take(),
            proxy_classes: Default::default(),
            receivers: Default::default(),
        };
        Ok((class, lowering.real.clone(), failed))
    })
}

/// A node of javac's tree.
#[derive(Clone, Copy)]
struct Node<'t>(&'t Value);

/// The declaration a name, call or declaration refers to.
#[derive(Clone, Copy)]
struct Symbol<'t> {
    kind: &'t str,
    name: &'t str,
    owner: &'t str,
    is_static: bool,
    /// Whether a method or constructor takes a variable number of arguments.
    varargs: bool,
    /// Whether a method is `native`: the JVM implements it (see `natives.rs`).
    native: bool,
    /// The declared type, such as `int` or, for a method, `(int,long)double`.
    ty: &'t str,
    /// The value of a compile-time constant variable, as text (`Character`s as their code unit).
    constant: Option<&'t str>,
}

impl<'t> Node<'t> {
    fn kind(self) -> &'t str {
        self.0["kind"].as_str().unwrap_or("")
    }

    fn pos(self) -> Pos {
        let at = |f: &str| self.0[f].as_u64().unwrap_or(0) as usize;
        Pos::new(at("line"), at("column"))
    }

    fn get(self, field: &str) -> Option<Node<'t>> {
        let v = &self.0[field];
        (!v.is_null()).then_some(Node(v))
    }

    fn child(self, field: &str) -> Result<Node<'t>, Error> {
        self.get(field).ok_or_else(|| Error {
            pos: self.pos(),
            message: format!("malformed javac tree: {} without {field}", self.kind()),
        })
    }

    fn list(self, field: &str) -> impl Iterator<Item = Node<'t>> {
        self.0[field].as_array().into_iter().flatten().map(Node)
    }

    fn str(self, field: &str) -> &'t str {
        self.0[field].as_str().unwrap_or("")
    }

    fn java_type(self) -> &'t str {
        self.str("javaType")
    }

    fn symbol(self) -> Option<Symbol<'t>> {
        let s = self.get("symbol")?;
        let mut symbol = Symbol {
            kind: "",
            name: "",
            owner: "",
            is_static: false,
            varargs: false,
            native: false,
            ty: "",
            constant: None,
        };
        // One pass over the fields, as lowering asks for the symbols of most nodes.
        for (field, value) in s.0.as_object().into_iter().flat_map(|fields| fields.iter()) {
            let text = || value.as_str().unwrap_or("");
            let flag = || value.as_bool().unwrap_or(false);
            match field.as_str() {
                "kind" => symbol.kind = text(),
                "name" => symbol.name = text(),
                "owner" => symbol.owner = text(),
                "static" => symbol.is_static = flag(),
                "varargs" => symbol.varargs = flag(),
                "native" => symbol.native = flag(),
                "type" => symbol.ty = text(),
                "constant" => symbol.constant = value.as_str(),
                _ => {}
            }
        }
        Some(symbol)
    }

    /// Whether the declaration has a modifier. `STATIC` comes from the declaration's symbol when it
    /// has one, which also knows the implicit modifier of an interface's fields and nested types.
    fn has_flag(self, flag: &str) -> bool {
        if flag == "STATIC"
            && let Some(s) = self.get("symbol").and_then(|s| s.0["static"].as_bool())
        {
            return s;
        }
        self.get("modifiers")
            .is_some_and(|m| m.0["flags"].as_array().is_some_and(|f| f.iter().any(|x| x == flag)))
    }
}

/// What the rejection of an `Arrays.sort` or `Arrays.fill` of a `String[]` that is an `Object[]`
/// says, where the runtime would do it (see `Reach::string_array_writes`).
pub const STRING_ARRAY_WRITE: &str = "a change in place of a `String[]` that is also an `Object[]`";

impl<'t> Lowering<'t> {
    /// `a == b` or `a != b` of two strings, which compares references. The program's own code
    /// gets `Builtin::StringSame`, which `analysis::string_identity` decides from where the two
    /// strings can come from. The JDK compares by reference only strings it interned (the parts of
    /// a `Locale`, `Charset`'s canonical names) or as a shortcut before comparing the characters
    /// (`LocaleUtils.caseIgnoreMatch`), and no code of it that the targets reach makes a string
    /// with an identity of its own to tell it apart (a `new String` kept as a marker): there, the
    /// reference comparison gives what value equality does, which is how the library's code is
    /// lowered (`Objects.equals`).
    fn string_comparison(&self, node: Node<'t>, op: BinOp, left: Expr, right: Expr) -> ExprKind {
        let library = self.library.get(self.current.get()).copied().unwrap_or(false);
        let builtin = if library { Builtin::ObjectsEquals } else { Builtin::StringSame };
        let equal = ExprKind::Builtin(builtin, vec![left, right]);
        match op {
            BinOp::Eq => equal,
            _ => ExprKind::Unary(UnOp::Not, Box::new(self.synth(equal, node.pos(), Type::Boolean))),
        }
    }
}

/// The kind of a node that stands for a node of the program in a tree that lowering writes
/// (`Lowering::stand_in`).
const STAND_IN: &str = "STAND_IN";

fn unsupported<T>(node: Node, what: &str) -> Result<T, Error> {
    error(node.pos(), format!("{what} not supported yet"))
}

/// `ENHANCED_FOR_LOOP` as `enhanced for loop`.
/// Whether javac's type `name` is an instantiation of the generic class `class`, such as
/// `java.lang.Enum<?>` (and not an array of one, `java.lang.Enum<?>[]`).
fn generic_of(name: &str, class: &str) -> bool {
    name.strip_prefix(class).is_some_and(|rest| rest.starts_with('<') && rest.ends_with('>'))
}

fn kind_words(kind: &str) -> String {
    kind.to_lowercase().replace('_', " ")
}

/// The calls noted while lowering (`Lowering::calls`): by the class whose code calls, the
/// call's position and the method's name, the position of the first argument and the method.
type NotedCalls = std::collections::HashMap<(String, Pos, String), (Option<Pos>, Option<CallName>)>;

struct Lowering<'t> {
    next_id: Cell<ExprId>,
    /// Index of the class whose members are being lowered: `this` and unqualified names refer to it.
    current: Cell<usize>,
    /// The method being lowered, whose type variables are in scope.
    method_ctx: Cell<Option<usize>>,
    /// Whether the code being lowered is a field initializer or an initializer block that runs in
    /// a constructor, which reads the enclosing instance and the captured locals from the fields,
    /// as javac's code for it does, not from the constructor's parameters.
    initializing: Cell<bool>,
    /// The class and method whose type variables a declared type is read in, when that is not
    /// the code being lowered (see `generics::declared_type`).
    scope: Cell<Option<(usize, Option<usize>)>>,
    /// Whether the method being lowered is a constructor, which cannot `return` early.
    in_ctor: Cell<bool>,
    /// The return type of the method being lowered, which a returned value is converted to.
    ret: Cell<Type>,
    /// Every class of the file, in the order of `Class::types`.
    types: Vec<TypeInfo<'t>>,
    /// Every method and constructor of every class, in the order of `Class::methods`.
    methods: Vec<MethodInfo<'t>>,
    /// `types` and `methods` by name (`type_index`, `method_index`).
    indices: RefCell<objects::Indices<'t>>,
    /// In a whole program, what is reachable from `main`: only that is lowered.
    reach: Option<&'t crate::program::Reach>,
    /// In a whole program, the class whose `main` is the entry point; in a file, every `main` is.
    entry: Option<usize>,
    /// The standard exception classes that the program uses (see `exceptions`).
    standard: std::rc::Rc<exceptions::Standard>,
    /// The array types whose arrays become `Object`s or are cast back from them: each is a
    /// runtime class beside `Object` (see `object::object_types`).
    object_arrays: std::cell::RefCell<Vec<Type>>,
    /// The array types that an `Object` is tested for or cast to (`object_array`), which an array
    /// that becomes an `Object` meets where it may be one (`covariant::meets_through_object`).
    object_tests: std::cell::RefCell<Vec<Type>>,
    /// The expressions that erasure's checked cast of a generic member's value wraps (see
    /// `generics::erased_use`), by id.
    erased_casts: RefCell<std::collections::HashSet<ExprId>>,
    /// The class that names each static field read, and the methods of `String` that calls name
    /// (see `Class::class_file`).
    static_qualifiers: RefCell<std::collections::HashMap<(String, Pos, String), Option<String>>>,
    /// With the position of the first argument of the call first noted at each key, until a
    /// second differs (`chained_calls`).
    calls: RefCell<NotedCalls>,
    chained_calls: RefCell<crate::ast::ChainedCalls>,
    /// See `ClassFileNames::iterators`.
    iterators: RefCell<std::collections::HashMap<(String, Pos), Pos>>,
    /// The binary name of the anonymous class of the enum constant's body whose method is being
    /// lowered (`constant_bodies`), which javac names that body's own methods by (`note_call`).
    body_class: RefCell<Option<String>>,
    /// The nodes of the program that a tree lowering writes stands for (`stand_in`), by number.
    stand_ins: RefCell<Vec<Node<'t>>>,
    /// The Java name of each local that `local_names` renamed (see `Class::class_file`).
    renamed_locals:
        RefCell<std::collections::HashMap<(String, Pos), std::collections::HashMap<String, String>>>,
    /// The atomic updates of fields through `Unsafe` that the code makes, each a method that
    /// lowering adds to the field's class (see `handles`).
    field_updates: RefCell<Vec<handles::FieldUpdateMethod>>,
    /// The calls of the program's methods, found the first time the value of a handle's
    /// parameter is asked for (see `handle_values`).
    handle_calls: RefCell<Option<handle_values::Calls<'t>>>,
    /// The places of each class's code where a handle may be a value, found the first time one
    /// is lowered (see `handle_values`).
    handle_slots: RefCell<std::collections::HashMap<usize, std::collections::HashSet<*const Value>>>,
    /// The writes of each class's static fields in its static blocks, found the first time one
    /// of its fields' values is asked for (see `handles::static_value`).
    static_writes: RefCell<std::collections::HashMap<usize, std::rc::Rc<handles::StaticWrites<'t>>>>,
    /// The arrays of references that are `Object[]`s (see `covariant`).
    covariant: covariant::Covariant,
    /// Where `Object` goes, in a program that uses it (see `object`).
    object: Option<object::ObjectIndexes>,
    /// The class of the objects that `new Object()` creates, when the code has one (see `object`).
    plain_object: Option<usize>,
    /// The generic classes and methods that are Rust generics (see `real_generics`).
    real: real_generics::Real,
    /// Those of `real` that lowering found cannot be: the program is lowered again without them.
    failed: RefCell<real_generics::Real>,
    /// The pattern variables in scope that name a value without testing it (see `patterns`).
    aliases: RefCell<Vec<patterns::Alias>>,
    /// The JDK's functional interfaces that the program uses and lowering provides (see `functional`).
    functional: std::rc::Rc<functional::Functional>,
    /// The lambdas and method references of the code being lowered (see `lambdas`).
    lambdas: Vec<lambdas::LambdaInfo<'t>>,
    /// Where each lambda is in `lambdas`, by its class, its constructor and method, and its node.
    lambda_index: lambdas::LambdaIndex,
    /// The identifiers that read a local captured by the class whose code they are in, by
    /// address (see `captures`).
    capture_refs: std::collections::HashSet<*const Value>,
    /// The `new` expressions, by address, with the captures of the created class that are
    /// captures of the creating class there (see `captures`).
    capture_sites: std::collections::HashSet<(*const Value, String)>,
    /// The member classes of local and anonymous classes, each with its outer class, by index:
    /// they are named after that class (see `inner::name_nested`).
    member_outers: Vec<(usize, usize)>,
    /// In a whole program, whether each class, by index, is the library's (the JDK's) rather than
    /// the program's own; a class without an entry is the program's.
    library: Vec<bool>,
    /// In a census (see `census`), the code that lowering rejected, each error with the index of
    /// the type whose code it is in and the position of the method being lowered, and lowering
    /// goes on past it; `None` otherwise, when the first error ends lowering.
    census: Option<RefCell<Vec<CensusError>>>,
    /// The code that holds each `Arrays.sort` or `Arrays.fill` of a `String[]` that is an
    /// `Object[]` lowered as a call of the JDK's method (`jdk_string_array_write`), as a type and
    /// a method or none, as `Rejected` names the code that a rejection is in.
    string_array_writes: RefCell<Vec<(usize, Option<usize>)>>,
    /// The answers of the scans of the trees that each attempt of lowering asks the same, shared
    /// by the attempts (`scans`).
    scans: std::rc::Rc<scans::Scans<'t>>,
    /// Whether a census takes a panic while lowering a method for a rejection of the method. A
    /// whole program's lowering, which goes on past rejections too (`tolerant_lower_program`),
    /// lets it end the translation, as it would without them.
    catch_panics: bool,
    /// Whether the method being lowered is a census's stub of one that lowering rejected.
    stubbing: Cell<bool>,
    /// The rejection that the stub being lowered stands for, which its body names (`never`).
    stub_message: RefCell<Option<String>>,
    /// In a census whose translation is written (`census_lower_program`), a rejected field
    /// initializer or initializer block becomes code that ends the program with the rejection's
    /// message, where a census otherwise leaves it out.
    ends_rejected: bool,
    /// In a census, the classes whose declaration lowering rejected (one that extends or
    /// implements a class that is not translated), with the error: such a class extends and
    /// implements none of those, and all of its code is rejected with it (see `census`).
    rejected_classes: RefCell<std::collections::BTreeMap<usize, Error>>,
    /// The methods that are a census's stubs.
    stubbed: RefCell<std::collections::BTreeSet<usize>>,
    /// In a census, the statements of methods that lowering rejected, each with the class and the
    /// message of the stub that stands for it and the rest of its block (`census::cut_stub`).
    cut_stmts: RefCell<std::collections::HashMap<*const Value, (usize, String)>>,
    /// In a census, the innermost statement of a block whose lowering failed last, with the
    /// position and message of the error (`census::note_failed_stmt`).
    failed_stmt: RefCell<Option<(*const Value, Pos, String)>>,
    /// What lowering assumed (see `ast::Note`).
    notes: RefCell<Vec<crate::ast::Note>>,
    /// The classes of the covariant arrays the code makes (`Class::element_classes`).
    element_classes: RefCell<std::collections::HashMap<ExprId, Type>>,
    /// The levels of the new arrays whose rows at some level are `Object`s (`Class::object_rows`).
    object_rows: RefCell<std::collections::HashMap<ExprId, Vec<(Type, Type)>>>,
    /// The covariant array types that `instanceof`s and casts test (`Class::array_tests`).
    array_tests: RefCell<std::collections::HashMap<ExprId, Type>>,
    /// The volatile element accesses (`Class::volatile_elements`).
    volatile_elements: RefCell<std::collections::HashSet<ExprId>>,
    /// The annotation types whose class literals the code has (`Class::annotation_classes`).
    annotation_classes: RefCell<std::collections::BTreeMap<String, String>>,
    /// The annotation types declared in other classes, with their binary names
    /// (`Outer$Annotation`): `None` for one in a local or anonymous class, whose binary name
    /// lowering does not know, and whose class literal is rejected.
    nested_annotation_types: std::collections::HashMap<String, Option<String>>,
    /// The methods of enums that only the bodies of their constants declare, each implementing a
    /// method of an interface that the enum leaves to it (see `constant_bodies::body_methods`).
    body_methods: std::collections::HashSet<usize>,
    /// The result type of those of `body_methods` that the bodies declare with different result
    /// types, narrowing the interface's: the interface's (see `constant_bodies::body_methods`).
    body_returns: std::collections::HashMap<usize, &'t str>,
    /// The methods of enums that a constant's body declares and that override nothing, which only
    /// that body's code calls (see `constant_bodies::body_methods`).
    body_helpers: std::collections::HashSet<usize>,
    /// The annotation types (`@interface`) that were left out, by javac's name, with their
    /// declarations: an annotation does nothing when the program runs, but a value of one is not
    /// translated, and `isAnnotationPresent` reads them (`annotation_presence`).
    annotation_types: std::collections::HashMap<String, Node<'t>>,
    /// In a whole program, the annotation types that are the JDK's, which may annotate the
    /// classes that the runtime has in place of the JDK's: another library's annotate only
    /// classes that are translated, as the program's do.
    library_annotations: std::collections::HashSet<String>,
    /// What each method asks about its caller (see `callers`).
    callers: callers::Callers,
    /// The reflection tables made so far (`reflection::table`).
    reflection_tables: RefCell<std::collections::HashMap<reflection::TableKey, Vec<reflection::Entry<'t>>>>,
}

/// An error that lowering for a census records: the index of the type whose code it is in, the
/// position of the method being lowered, the index of that method when the code rejected is part
/// of its body alone, and the error.
type CensusError = (usize, Option<Pos>, Option<usize>, Error);

/// What `finish_types` keeps from one enum to the next.
struct EnumsKept {
    writes: enums::EnumFieldWrites,
    uses: enum_tables::EnumFieldUses,
    clinit_ctors: clinit_errors::ClinitCtors,
}

impl<'t> Lowering<'t> {
    /// What lowering adds once every declared method is lowered, for a file and a whole program
    /// alike: the `<clinit>` of each class with static initialization, and each enum's constant
    /// bodies and constants. An error comes with the index of the type it is in.
    fn finish_types(&self, types: &mut [TypeDecl], methods: &mut Vec<Method>) -> Result<(), (usize, Error)> {
        let ctors = self.clinit_ctors(methods);
        for index in (0..types.len()).filter(|&t| self.has_static_init(t)) {
            let mut clinit = self.clinit(index).map_err(|e| (index, e))?;
            self.guard_clinit(index, &mut clinit, &mut types[index], &ctors);
            methods.push(clinit);
        }
        let mut kept =
            EnumsKept { writes: Default::default(), uses: Default::default(), clinit_ctors: ctors };
        let enum_indices: Vec<usize> =
            (0..types.len()).filter(|&t| self.types[t].node.kind() == "ENUM").collect();
        // Every enum's constant bodies come first: a body's method may read another enum's fields,
        // and that enum's tables must rewrite the read, so the methods must all exist before any
        // enum takes its form.
        let made: Vec<Result<Vec<Vec<Field>>, Error>> = enum_indices
            .iter()
            .map(|&index| {
                self.constant_bodies(index, methods).and_then(|()| {
                    kept.writes.forget(index);
                    self.constant_body_fields(index)
                })
            })
            .collect();
        for (index, bodies) in enum_indices.into_iter().zip(made) {
            let bodies_rejected =
                bodies.as_ref().err().map(|e| Error { pos: e.pos, message: e.message.clone() });
            let finished = self.finish_enum(index, types, methods, bodies, &mut kept);
            // The enum's own methods may have changed.
            kept.writes.forget(index);
            kept.uses.forget(methods, index);
            if let Err(e) = finished {
                // In a census, an enum that lowering rejects keeps its constants without values.
                self.reject_at(index, Some(self.types[index].node.pos()), e).map_err(|e| (index, e))?;
                let names = self.enum_constant_names(index);
                let constants = names.map(|name| EnumConstant { name: name.to_string(), values: Vec::new() });
                types[index].enum_constants = Some(constants.collect());
                // The methods that the constants' bodies override have no `switch` over the
                // constants, and those that implement an interface's abstract method no body at
                // all (Guava's `Predicates.ObjectPredicate.apply`): they are rejected with it.
                if let Some(error) = bodies_rejected {
                    for m in self.overridden_by_bodies(index) {
                        methods[m].body = self.stub_body(index, m, &error);
                        methods[m].is_abstract = false;
                    }
                }
            }
        }
        self.field_update_methods(methods);
        Ok(())
    }

    /// The constants of enum `index`, with its constant bodies' fields `bodies` (see `finish_types`).
    fn finish_enum(
        &self,
        index: usize,
        types: &mut [TypeDecl],
        methods: &mut Vec<Method>,
        bodies: Result<Vec<Vec<Field>>, Error>,
        kept: &mut EnumsKept,
    ) -> Result<(), Error> {
        let bodies = bodies?;
        let own_fields = types[index].fields.len();
        let pure = self.check_enum(index, methods, &mut kept.writes).and_then(|()| {
            self.add_body_fields(&bodies, &mut types[index].fields)?;
            self.enum_constants(index, &types[index].fields, methods, &bodies)
        });
        // An enum whose constructors do more than the pure form allows has tables instead, the
        // fields of its constants' bodies too.
        let constants = match pure {
            Ok(constants) => constants,
            Err(_) => {
                types[index].fields.truncate(own_fields);
                self.add_body_fields(&bodies, &mut types[index].fields)?;
                self.enum_tables(index, types, methods, bodies, &mut kept.uses, &kept.clinit_ctors)?;
                let names = self.enum_constant_names(index);
                names.map(|name| EnumConstant { name: name.to_string(), values: Vec::new() }).collect()
            }
        };
        types[index].enum_constants = Some(constants);
        Ok(())
    }

    fn method(&self, index: usize) -> Result<Method, Error> {
        let outer = self.method_ctx.replace(Some(index));
        let method = self.method_in_scope(index);
        self.method_ctx.set(outer);
        let mut method = method?;
        let renamed = local_names::distinct_locals(&mut method).into_iter().collect();
        self.renamed_locals.borrow_mut().insert((self.types[method.class].name.clone(), method.pos), renamed);
        self.share_element_reads(&mut method);
        self.arguments_before_receivers(&mut method);
        Ok(method)
    }

    fn method_in_scope(&self, index: usize) -> Result<Method, Error> {
        if let Some(method) = self.lambda_member(index) {
            return method;
        }
        let info = &self.methods[index];
        let node = info.node;
        self.current.set(info.owner);
        self.in_ctor.set(node.str("name") == "<init>");
        // A stub runs none of the method's code, so the override it stands in for is no matter.
        if !self.stubbing.get() {
            self.check_throwable_override(node)?;
        }
        let is_ctor = node.str("name") == "<init>";
        let is_static = node.has_flag("STATIC");
        let name = node.str("name").to_string();
        let ret = if is_ctor {
            Type::Void
        } else {
            let ret_node = node.child("returnType")?;
            self.signature_type(ret_node, self.return_java_type(index, ret_node))?
        };
        self.ret.set(ret);
        let params: Vec<Node> = node.list("parameters").collect();
        let param_type = |p: Node<'t>| p.symbol().map_or("", |s| s.ty);
        let is_main = name == "main"
            && is_static
            && ret == Type::Void
            && params.len() == 1
            && param_type(params[0]) == "java.lang.String[]"
            && self.entry.is_none_or(|entry| entry == info.owner);
        let args_param = is_main.then(|| params[0]);
        let mut params: Vec<Param> = if is_main {
            Vec::new()
        } else {
            params
                .into_iter()
                .map(|p| {
                    Ok(Param { name: p.str("name").to_string(), ty: self.signature_type(p, param_type(p))? })
                })
                .collect::<Result<_, Error>>()?
        };
        params.extend(self.caller_param(index, node, is_main)?);
        // An abstract method of an enum is implemented by the constants' bodies, which
        // `constant_bodies` makes its body.
        let abstract_in_enum =
            node.has_flag("ABSTRACT") && self.enum_index(&self.types[info.owner].name).is_some();
        let in_interface = self.types[info.owner].node.kind() == "INTERFACE";
        let is_abstract = node.get("body").is_none()
            && (node.has_flag("ABSTRACT") || in_interface && !is_static)
            && !abstract_in_enum;
        let dormant = self.reach.is_some_and(|r| r.dormant(&self.types[info.owner].javac, node.0));
        let mut body = match node.get("body") {
            Some(_) if dormant || self.stubbing.get() => self.never(info.owner, node.pos()),
            // The constants without a body of their own run the interface's (`body_methods`).
            Some(_) if self.body_methods.contains(&index) => self.interface_default(index, &params, ret)?,
            Some(body) => self.block(body)?,
            None if abstract_in_enum || is_abstract => Vec::new(),
            None if node.has_flag("NATIVE") => self.native_body(index, &params, ret),
            None => return unsupported(node, "methods without a body"),
        };
        let params = if is_ctor && self.stubbing.get() {
            let (first, last) = self.synthetic_params(info.owner)?;
            first.into_iter().chain(params).chain(last).collect()
        } else if is_ctor {
            // A declared parameter named as the enclosing instance's (an anonymous
            // `new ClassType(outer, ..) { .. }` takes its superclass's parameter names) is renamed.
            let mut params = params;
            if self.nesting(info.owner).is_some_and(|n| n.outer)
                && let Some(p) = params.iter_mut().find(|p| p.name == inner::OUTER)
            {
                let renamed = format!("{}_arg", inner::OUTER);
                p.name = renamed.clone();
                for stmt in &mut body {
                    enum_tables::rewrite_stmt(stmt, &mut |e| {
                        if matches!(&e.kind, ExprKind::Var(v) if v == inner::OUTER) {
                            e.kind = ExprKind::Var(renamed.clone());
                        }
                    });
                }
            }
            self.constructor_prologue(node, info.owner, &mut body)?;
            self.run_initializer_blocks(info.owner, &mut body)?;
            self.start_constructor(info.owner, &mut body)?;
            let (first, last) = self.synthetic_params(info.owner)?;
            first.into_iter().chain(params).chain(last).collect()
        } else {
            params
        };
        if let Some(param) = args_param.filter(|p| reads_name(&body, p.str("name"))) {
            body.insert(0, self.args_decl(param));
        }
        if is_main && let Some(hook) = self.trace_hook(node.pos()) {
            body.insert(0, hook);
        }
        Ok(Method {
            name,
            params,
            ret,
            body,
            is_main,
            is_static,
            is_ctor,
            class: info.owner,
            pos: node.pos(),
            is_abstract,
            is_synchronized: node.has_flag("SYNCHRONIZED"),
            overrides: self.overrides(node),
            type_params: self.method_type_params(index),
            jdk_code: self.is_proxy_class(info.owner),
            field_update: None,
        })
    }

    /// The supported type named by javac's `name` for it.
    fn ty(&self, node: Node, name: &str) -> Result<Type, Error> {
        if let Some(boxed) = boxing::boxed_type(name) {
            return Ok(boxed);
        }
        match name {
            "int" => Ok(Type::Int),
            "long" => Ok(Type::Long),
            "double" => Ok(Type::Double),
            "boolean" => Ok(Type::Boolean),
            "void" => Ok(Type::Void),
            "java.lang.String" => Ok(Type::String),
            "java.lang.StringBuilder" | "java.lang.StringBuffer" => Ok(Type::StringBuilder),
            // An array of these (`Class<?>[]`, `Enum<?>[]`) is an array type, below.
            _ if name == "java.lang.Class" || generic_of(name, "java.lang.Class") => Ok(Type::JavaClass),
            // A handle made at run time is a token that only says whether it is null; lowering
            // knows which handles it can be (`handle_values`).
            _ if handle_values::is_handle_type(name) => Ok(Type::JavaClass),
            // A value of one of the program's enums (see `enums::enum_call`).
            _ if (name == "java.lang.Enum" || generic_of(name, "java.lang.Enum"))
                && self.object.is_some_and(|at| at.enumeration.is_some()) =>
            {
                Ok(Type::class(ENUM))
            }
            // The JDK's classes of the standard charsets too (`charsets::STANDARD_CHARSET_CLASSES`).
            _ if charsets::is_charset_type(name) => Ok(Type::Charset),
            "byte" => Ok(Type::Byte),
            "short" => Ok(Type::Short),
            "char" => Ok(Type::Char),
            "float" => Ok(Type::Float),
            // A type variable hides a class of the same name.
            _ if let Some(erased) = self.type_variable(node, name) => erased,
            // The declared name, which can differ from javac's (a hoisted enum, see `local_types`).
            _ if let Some(e) = self.enum_index(name) => Ok(Type::enum_type(&self.types[e].name)),
            _ if let Some(t) = self.type_index(name) => {
                // A Rust generic used without type arguments.
                if self.is_real(real_generics::Owner::Class(t)) && !self.class_type_params(t).is_empty() {
                    self.fail(real_generics::Owner::Class(t));
                }
                Ok(Type::class(&self.types[t].name))
            }
            "<nulltype>" => Ok(Type::Null),
            // The intersection of a `?:`'s or a `switch`'s branch types, such as `Serializable &
            // Comparable<?>` for an enum and a string, whose erasure may be a JDK interface that
            // a single file lacks: the value is an `Object`, which holds any of them.
            _ if generics::intersection_bounds(name).is_some()
                && matches!(
                    node.kind(),
                    "CONDITIONAL_EXPRESSION" | "SWITCH_EXPRESSION" | "PARENTHESIZED"
                ) =>
            {
                self.ty(node, "java.lang.Object")
            }
            // `this` in the body of an enum constant, which is the constant.
            _ if name.starts_with("<anonymous ") && self.enum_index(&name[11..name.len() - 1]).is_some() => {
                Ok(Type::enum_type(&name[11..name.len() - 1]))
            }
            _ if name.ends_with("[]") => {
                Ok(self.covariant_array(Type::array(self.ty(node, &name[..name.len() - 2])?)))
            }
            "java.lang.Object" if self.object.is_some() => Ok(Type::class(OBJECT)),
            "java.lang.Comparable" if self.object.is_some_and(|at| at.comparable.is_some()) => {
                Ok(Type::class(COMPARABLE))
            }
            "java.lang.Number" if self.object.is_some_and(|at| at.number.is_some()) => {
                Ok(Type::class(NUMBER))
            }
            "java.lang.CharSequence" if self.object.is_some_and(|at| at.char_sequence.is_some()) => {
                Ok(Type::class(CHAR_SEQUENCE))
            }
            "java.lang.Record" if self.object.is_some_and(|at| at.record.is_some()) => {
                Ok(Type::class(RECORD))
            }
            _ if let Some(erased) = self.erased(node, name) => erased,
            // A `...` parameter has the array type here, so `Object...` is rejected as `Object[]`.
            "java.lang.Object" => unsupported(node, "Object"),
            // A value of an annotation type (what `getAnnotation(Documented.class)` returns) is
            // an `Annotation`, which is all the code may call on it.
            _ if self.annotation_types.contains_key(name)
                && let Some(t) = self.type_index("java.lang.annotation.Annotation") =>
            {
                Ok(self.self_type(t))
            }
            _ if self.annotation_types.contains_key(name) => {
                unsupported(node, &format!("using the annotation type `{name}` other than as an annotation"))
            }
            _ => unsupported(node, &format!("type `{name}`")),
        }
    }

    fn block(&self, node: Node<'t>) -> Result<Vec<Stmt>, Error> {
        self.javac_error(node)?;
        let mut out = Vec::new();
        // The `if`s that test a property that is never set run only their `else` branch.
        let unset = crate::program::unset_property_ifs(node.0);
        for stmt in node.list("statements") {
            // A census's stub of a rejected statement stands for the rest of the block too.
            if let Some(stub) = self.cut_stub(stmt) {
                out.push(stub);
                break;
            }
            self.block_stmt(node, stmt, &unset, &mut out).inspect_err(|e| self.note_failed_stmt(stmt, e))?;
        }
        Ok(self.exits_into_rest(out))
    }

    /// Statement `stmt` of block `node` into `out`; `unset` are the block's `if`s that run only
    /// their `else` branch.
    fn block_stmt(
        &self,
        node: Node<'t>,
        stmt: Node<'t>,
        unset: &[&Value],
        out: &mut Vec<Stmt>,
    ) -> Result<(), Error> {
        if is_super_call(stmt) {
            out.extend(self.super_init(stmt)?);
            return Ok(());
        }
        if unset.iter().any(|s| std::ptr::eq(*s, stmt.0)) {
            if let Some(otherwise) = stmt.get("elseStatement") {
                self.stmt_into(otherwise, out)?;
            }
            return Ok(());
        }
        // A field found by reflection only to make a handle of it, which is no value.
        if self.handle_field_local(node, stmt) {
            return Ok(());
        }
        self.stmt_into(stmt, out)
    }

    /// A statement in a position that takes one statement, such as the body of an `if`.
    fn single_stmt(&self, node: Node<'t>) -> Result<Stmt, Error> {
        let mut out = Vec::new();
        self.stmt_into(node, &mut out)?;
        Ok(if out.len() == 1 { out.pop().unwrap() } else { Stmt::Block(out) })
    }

    fn stmt_into(&self, node: Node<'t>, out: &mut Vec<Stmt>) -> Result<(), Error> {
        let stmt = match node.kind() {
            "BLOCK" => Stmt::Block(self.block(node)?),
            "EMPTY_STATEMENT" => Stmt::Block(Vec::new()),
            // Assertions are disabled by default in Java, so an `assert` does nothing at all.
            "ASSERT" => return Ok(()),
            "SYNCHRONIZED" => return self.synchronized(node, out),
            // A `Runtime` has no state of its own: a local that holds `Runtime.getRuntime()` is a
            // name for it, and calls on that name are calls on it (`lower/natives.rs`).
            "VARIABLE"
                if node.symbol().is_some_and(|s| s.ty == "java.lang.Runtime")
                    && node
                        .get("initializer")
                        .and_then(|v| v.symbol())
                        .is_some_and(|s| (s.owner, s.name) == ("java.lang.Runtime", "getRuntime")) =>
            {
                return Ok(());
            }
            "VARIABLE" => {
                let ty = self.ty(node, node.symbol().map_or("", |s| s.ty))?;
                let mut init = match node.get("initializer") {
                    Some(v) => Some(self.convert(self.expr(v)?, ty)),
                    None => None,
                };
                let (name, pos) = (node.str("name").to_string(), node.pos());
                // A local that its own value assigns (`T x = c ? x = a : b`, which Java allows) is
                // declared first and then assigned: the value names it.
                if let Some(value) = init.take_if(|value| assigns(value, &name)) {
                    out.push(Stmt::Local { ty, name: name.clone(), init: None, pos });
                    let assign =
                        ExprKind::Assign { op: None, target: Place::Var(name), value: Box::new(value) };
                    Stmt::Expr(self.synth(assign, pos, ty))
                } else {
                    Stmt::Local { ty, name, init, pos }
                }
            }
            "EXPRESSION_STATEMENT" if let Some(set_all) = self.set_all(node)? => set_all,
            // The value of a field handle is no field's (see `handles`).
            "EXPRESSION_STATEMENT" if let Some((owner, member)) = self.handle_definition(node) => {
                out.extend(self.handle_warning(owner, member)?.map(Stmt::Expr));
                return Ok(());
            }
            "EXPRESSION_STATEMENT" => match self.expr(node.child("expression")?)? {
                // A native that does nothing here, such as `registerNatives()`, leaves no trace.
                Expr { kind: ExprKind::Builtin(Builtin::Native(Native::Nothing), _), .. } => return Ok(()),
                // A store in a covariant array, whose value is not used.
                Expr { kind: ExprKind::Cast(_, assign), .. }
                    if matches!(assign.kind, ExprKind::Assign { .. }) =>
                {
                    Stmt::Expr(*assign)
                }
                // The checked cast that erasure gives a generic method's result (`erased_use`):
                // Java casts only a value that it uses, and this one is dropped. javac's
                // `translate(tree.def);` in `Lower.visitNewClass` gets back the block that
                // replaces an anonymous class's declaration, not the declaration.
                Expr { kind: ExprKind::Cast(_, call), .. }
                    if matches!(call.kind, ExprKind::MethodCall { .. } | ExprKind::Call { .. }) =>
                {
                    Stmt::Expr(*call)
                }
                e => Stmt::Expr(e),
            },
            // Reachability skipped the other branch (`program::live_branch`).
            "IF" if let Some(branch) = self.live_branch(node) => match branch {
                Some(n) => self.single_stmt(n)?,
                None => Stmt::Block(Vec::new()),
            },
            // Reachability did not take the `then` branch, as the class that the condition tests
            // for has no objects (`Reach::objectless_branch`): the condition is false, so it runs
            // for what it does and the `else` branch follows. An empty `then` branch would leave
            // a variable that only the branches assign unassigned on its path, for Rust.
            "IF" if self.reach.is_some_and(|r| r.objectless_branch(node.0)) => {
                let condition = Stmt::Expr(self.expr(node.child("condition")?)?);
                match node.get("elseStatement") {
                    Some(n) => Stmt::Block(vec![condition, self.single_stmt(n)?]),
                    None => condition,
                }
            }
            // Java takes a constant condition as known where it decides which locals are assigned
            // (JLS 16): only the branch that runs is kept, so that Rust knows it too.
            "IF" => match constant_condition(node.child("condition")?) {
                Some(true) => Stmt::Block(vec![self.single_stmt(node.child("thenStatement")?)?]),
                Some(false) => match node.get("elseStatement") {
                    Some(n) => Stmt::Block(vec![self.single_stmt(n)?]),
                    None => Stmt::Block(Vec::new()),
                },
                None => self.if_stmt(
                    self.expr(node.child("condition")?)?,
                    self.single_stmt(node.child("thenStatement")?)?,
                    match node.get("elseStatement") {
                        Some(n) => Some(self.single_stmt(n)?),
                        None => None,
                    },
                ),
            },
            "WHILE_LOOP" => Stmt::While {
                cond: self.expr(node.child("condition")?)?,
                body: Box::new(self.single_stmt(node.child("statement")?)?),
            },
            "DO_WHILE_LOOP" => Stmt::DoWhile {
                body: Box::new(self.single_stmt(node.child("statement")?)?),
                cond: self.expr(node.child("condition")?)?,
            },
            "FOR_LOOP" => {
                let mut init = Vec::new();
                for n in node.list("initializer") {
                    self.stmt_into(n, &mut init)?;
                }
                let cond = match node.get("condition") {
                    Some(n) => Some(self.expr(n)?),
                    None => None,
                };
                let update = node
                    .list("update")
                    .map(|n| self.expr(n.child("expression")?))
                    .collect::<Result<_, _>>()?;
                let body = Box::new(self.single_stmt(node.child("statement")?)?);
                Stmt::For { init, cond, update, body }
            }
            "RETURN" => match node.get("expression") {
                Some(value) => {
                    Stmt::Return(Some(self.convert(self.expr(value)?, self.ret.get())), node.pos())
                }
                None => Stmt::Return(None, node.pos()),
            },
            "BREAK" | "CONTINUE" => switch::jump(node),
            "YIELD" => Stmt::Yield(self.expr(node.child("value")?)?),
            "ENHANCED_FOR_LOOP" => self.for_each(node, None)?,
            "SWITCH" => self.switch_stmt(node)?,
            "TRY" => self.try_stmt(node)?,
            "THROW" => self.throw(node)?,
            "LABELED_STATEMENT" => self.labeled(node)?,
            // A local class is declared with the classes (see `inner`).
            "CLASS" | "INTERFACE" | "ENUM" | "RECORD" => return Ok(()),
            kind => return unsupported(node, &kind_words(kind)),
        };
        out.push(stmt);
        Ok(())
    }

    fn expr(&self, node: Node<'t>) -> Result<Expr, Error> {
        self.javac_error(node)?;
        if let Some(thrown) = self.inaccessible(node) {
            return Ok(thrown);
        }
        self.check_handle_value(node)?;
        if let Some(code) = self.reference_native(node)? {
            return Ok(code);
        }
        let kind = match node.kind() {
            _ if charsets::is_charset_op(node) => self.charset_op(node)?,
            "PARENTHESIZED" => return self.expr(node.child("expression")?),
            STAND_IN => {
                return self
                    .expr(self.stand_ins.borrow()[node.0["standsFor"].as_u64().unwrap_or(0) as usize]);
            }
            "INT_LITERAL" => ExprKind::Int(self.number(node)?),
            "LONG_LITERAL" => ExprKind::Long(self.number(node)?),
            "DOUBLE_LITERAL" => ExprKind::Double(self.number(node)?),
            "BOOLEAN_LITERAL" => ExprKind::Bool(node.0["value"] == true),
            "STRING_LITERAL" => match string_units(&node.0["value"]) {
                Some(units) => string_literal(units),
                None => return unsupported(node, "this string literal"),
            },
            "CHAR_LITERAL" => self.char_literal(node)?,
            "FLOAT_LITERAL" => self.float_literal(node)?,
            "NULL_LITERAL" => ExprKind::Null,
            // `TRUE` and `FALSE` that a static import of `Boolean`'s names are its constants too.
            "IDENTIFIER" => match self.alias(node).or_else(|| self.boolean_constant(node)) {
                Some(value) => value,
                None => self.name(node)?,
            },
            // javac folds the minus sign into integer literals only; fold it into doubles too.
            "UNARY_MINUS" if node.child("expression")?.kind() == "DOUBLE_LITERAL" => {
                ExprKind::Double(-self.number::<f64>(node.child("expression")?)?)
            }
            "UNARY_MINUS" if node.child("expression")?.kind() == "FLOAT_LITERAL" => {
                ExprKind::Float(-self.number::<f32>(node.child("expression")?)?)
            }
            kind @ ("UNARY_MINUS" | "UNARY_PLUS" | "LOGICAL_COMPLEMENT" | "BITWISE_COMPLEMENT") => {
                let op = match kind {
                    "UNARY_MINUS" => UnOp::Neg,
                    "UNARY_PLUS" => UnOp::Plus,
                    "LOGICAL_COMPLEMENT" => UnOp::Not,
                    _ => UnOp::BitNot,
                };
                ExprKind::Unary(op, Box::new(self.expr(node.child("expression")?)?))
            }
            kind @ ("PREFIX_INCREMENT" | "PREFIX_DECREMENT" | "POSTFIX_INCREMENT" | "POSTFIX_DECREMENT") => {
                ExprKind::IncDec {
                    target: self.target(node.child("expression")?)?,
                    increment: kind.ends_with("INCREMENT"),
                    prefix: kind.starts_with("PREFIX"),
                }
            }
            "ASSIGNMENT" => {
                // A field of a type variable holds its erasure.
                let variable = node.child("variable")?;
                let to = match self.field_type(variable)? {
                    Some(declared) => declared,
                    None => self.ty(node, node.java_type())?,
                };
                let target = self.target(variable)?;
                // An element of a covariant array is an `Object` (see `covariant`), and so is the
                // assignment's value, cast back to javac's type where it is used (an expression
                // statement drops the cast).
                let declared = to;
                let to = match &target {
                    Place::Index(array, _) => match array.ty {
                        Type::Array(element) if Self::is_object(*element) => *element,
                        _ => to,
                    },
                    _ => to,
                };
                let value = Box::new(self.convert(self.expr(node.child("expression")?)?, to));
                let assign = ExprKind::Assign { op: None, target, value };
                if to != declared && declared.is_reference() {
                    let inner = self.synth(assign, node.pos(), to);
                    ExprKind::Cast(declared, Box::new(inner))
                } else {
                    assign
                }
            }
            kind if kind.ends_with("_ASSIGNMENT") => {
                let op = binary_op(node, kind.trim_end_matches("_ASSIGNMENT"))?;
                let mut value = self.expr(node.child("expression")?)?;
                if op == BinOp::Add && node.java_type() == "java.lang.String" {
                    value = self.array_text(node, value)?;
                }
                ExprKind::Assign {
                    op: Some(op),
                    target: self.target(node.child("variable")?)?,
                    value: Box::new(value),
                }
            }
            "TYPE_CAST" if let Some(name) = crate::program::created_by_name(node.0) => {
                self.created_by_name(node, Node(name))?
            }
            "TYPE_CAST" if arrays::casts_component_array(node) => self.component_array(node)?,
            "TYPE_CAST" if crate::program::literal_array(node.0).is_some() => self.literal_array(node)?,
            "TYPE_CAST" if let Some(cast) = self.intersection_cast(node).transpose() => return cast,
            "TYPE_CAST" => {
                let ty = self.ty(node, node.java_type())?;
                if let Some(created) = self.constructor_handle_call(node, ty)? {
                    return Ok(self.synth(created, node.pos(), ty));
                }
                let value = self.expr(node.child("expression")?)?;
                if Self::is_object(value.ty) {
                    self.object_array(node, ty)?;
                }
                let value = self.array_test(node, value, ty, node.java_type())?;
                // `(T[]) objects` or `(Object[]) versions`.
                if value.ty != ty {
                    self.meets_object_array(value.ty, ty);
                }
                // A cast between the program's types, up, down or across: `Cast` converts as the
                // implicit upcasts do, and checks a downcast. `(int) o` on an `Object` casts to
                // `Integer` and unboxes. `(C) super.clone()` in `C` is the copy itself, which has
                // `C`'s type: it is not converted to `Object` and back. So does `(long) n` on a
                // `Number` or a `Comparable`, which may hold a box.
                let holds_boxes = matches!(value.ty, Type::Class(name, _)
                    if name == OBJECT || name == NUMBER || name == COMPARABLE);
                if Self::is_clone(&value) {
                    self.cast_clone(value, ty)
                } else if holds_boxes && (ty.is_numeric() || ty == Type::Boolean) {
                    let boxed = self.synth(
                        ExprKind::Cast(Type::boxed(ty), Box::new(value)),
                        node.pos(),
                        Type::boxed(ty),
                    );
                    ExprKind::Cast(ty, Box::new(boxed))
                } else {
                    ExprKind::Cast(ty, Box::new(value))
                }
            }
            // As with `if`, reachability skipped the branch that the condition guards.
            "CONDITIONAL_EXPRESSION" if let Some(Some(branch)) = self.live_branch(node) => {
                let ty = self.ty(node, node.java_type())?;
                let value = self.convert(self.expr(branch)?, ty);
                // The branch converts to the type of the whole, as `cond ? 1 : 2L` is a `long`.
                if value.ty == ty {
                    return Ok(value);
                }
                ExprKind::Cast(ty, Box::new(value))
            }
            "CONDITIONAL_EXPRESSION" => {
                let condition = self.expr(node.child("condition")?)?;
                let (yes, no) =
                    (self.expr(node.child("trueExpression")?)?, self.expr(node.child("falseExpression")?)?);
                // An argument `c ? s : t` of strings for a `CharSequence` parameter has javac's
                // type of the parameter (a poly expression): it stays a string, which converts to
                // the parameter where it is passed, as the string calls need (`String.replace`).
                let strings = |e: &Expr| matches!(e.ty, Type::String | Type::Null);
                let ty = if node.java_type() == "java.lang.CharSequence" && strings(&yes) && strings(&no) {
                    Type::String
                } else {
                    self.ty(node, node.java_type())?
                };
                self.conditional(condition, self.convert(yes, ty), self.convert(no, ty))
            }
            "METHOD_INVOCATION" => self.call(node)?,
            "NEW_CLASS" => match self.new_text(node)? {
                Some(kind) => kind,
                None => self.new_object(node)?,
            },
            "NEW_ARRAY" => self.new_array(node)?,
            "ARRAY_ACCESS" => {
                let (array, index) = self.index(node)?;
                ExprKind::Index(Box::new(array), Box::new(index))
            }
            "MEMBER_SELECT" if classes::is_literal(node) => self.class_literal(node)?,
            "MEMBER_SELECT" if is_array_length(node) => {
                ExprKind::Length(Box::new(self.expr(node.child("expression")?)?))
            }
            "MEMBER_SELECT" => match self.constant_field(node).or_else(|| self.boolean_constant(node)) {
                Some(constant) => constant,
                None => self.select(node)?,
            },
            "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE" => self.lambda(node)?,
            "INSTANCE_OF" => self.instance_of(node)?,
            "SWITCH_EXPRESSION" => self.switch_expr(node)?,
            "EQUAL_TO" | "NOT_EQUAL_TO" if self.compares_classes(node) => self.class_comparison(node)?,
            "EQUAL_TO" | "NOT_EQUAL_TO" if charsets::compares_charsets(node) => {
                self.charset_comparison(node)?
            }
            kind => {
                let left = node.child("leftOperand");
                let op = binary_op(node, kind);
                let (Ok(left), Ok(op)) = (left, op) else { return unsupported(node, &kind_words(kind)) };
                let (left, right) = (self.expr(left)?, self.expr(node.child("rightOperand")?)?);
                if left.ty == Type::String && right.ty == Type::String && matches!(op, BinOp::Eq | BinOp::Ne)
                {
                    self.string_comparison(node, op, left, right)
                } else {
                    let (left, right) = if op == BinOp::Add && node.java_type() == "java.lang.String" {
                        (self.array_text(node, left)?, self.array_text(node, right)?)
                    } else {
                        (left, right)
                    };
                    let (left, right) = if matches!(op, BinOp::Eq | BinOp::Ne) {
                        self.convert_operands(left, right)
                    } else {
                        (left, right)
                    };
                    self.binary_before_assignment(node, op, left, right)
                }
            }
        };
        // `printf` returns its stream, which the translation has no type for; it is a statement.
        let ty = match kind {
            ExprKind::Builtin(Builtin::Print, _) if node.kind() == "METHOD_INVOCATION" => Type::Void,
            // A `CharSequence` conditional of strings, or of a string and null, stays a string (see
            // `CONDITIONAL_EXPRESSION`).
            ExprKind::Conditional(_, ref yes, ref no)
                if node.java_type() == "java.lang.CharSequence"
                    && matches!(yes.ty, Type::String | Type::Null)
                    && matches!(no.ty, Type::String | Type::Null) =>
            {
                Type::String
            }
            _ => self.ty(node, node.java_type())?,
        };
        let kind = match node.kind() {
            "METHOD_INVOCATION" | "IDENTIFIER" | "MEMBER_SELECT" | "ARRAY_ACCESS" => {
                self.erased_use(node, kind, ty)?
            }
            _ => kind,
        };
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let expr = Expr { kind, pos: node.pos(), ty, id };
        self.note_element_class(node, &expr)?;
        Ok(expr)
    }

    /// An operand `e` of a string concatenation: an array as an `Object`, whose text is its type
    /// and identity hash, such as `[I@1b6d3586` (`object::concatenates_array` plans `Object`).
    fn array_text(&self, node: Node<'t>, e: Expr) -> Result<Expr, Error> {
        if !matches!(e.ty, Type::Array(_)) {
            return Ok(e);
        }
        self.object_array(node, e.ty)?;
        Ok(self.convert(e, Type::class(OBJECT)))
    }

    /// `left op right`, where `right` may assign the variable that `left` reads, as in
    /// `a == (a = next(a))`: Java reads `left` first, so its value is kept in a local before
    /// `right` runs.
    fn binary_before_assignment(&self, node: Node<'t>, op: BinOp, left: Expr, right: Expr) -> ExprKind {
        let read = match &left.kind {
            ExprKind::Cast(_, inner) => inner,
            _ => &left,
        };
        let ExprKind::Var(name) = &read.kind else {
            return ExprKind::Binary(op, Box::new(left), Box::new(right));
        };
        if !assigns(&right, name) {
            return ExprKind::Binary(op, Box::new(left), Box::new(right));
        }
        let (pos, ty) = (node.pos(), left.ty);
        let copy = self.fresh_name("left");
        let var = self.synth(ExprKind::Var(copy.clone()), pos, ty);
        let binary = ExprKind::Binary(op, Box::new(var), Box::new(right));
        let result = self.synth(binary, pos, self.ty(node, node.java_type()).unwrap_or(Type::Boolean));
        let block = vec![Stmt::Local { ty, name: copy, init: Some(left), pos }, Stmt::Yield(result)];
        ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        }
    }

    fn number<T: std::str::FromStr>(&self, node: Node) -> Result<T, Error> {
        let text = node.str("value");
        text.parse().or_else(|_| error(node.pos(), format!("malformed literal {text}")))
    }

    /// The name of the local variable or parameter that an identifier refers to.
    fn local(&self, node: Node) -> Result<String, Error> {
        match node.symbol() {
            Some(s)
                if matches!(
                    s.kind,
                    "LOCAL_VARIABLE"
                        | "PARAMETER"
                        | "EXCEPTION_PARAMETER"
                        | "BINDING_VARIABLE"
                        | "RESOURCE_VARIABLE"
                ) =>
            {
                Ok(node.str("name").to_string())
            }
            _ => unsupported(node, &format!("the name `{}` here", node.str("name"))),
        }
    }

    /// What `=`, `+=`, `++` and the like assign.
    fn target(&self, node: Node<'t>) -> Result<Place, Error> {
        if let Some(place) = self.field_place(node)? {
            return Ok(place);
        }
        match node.kind() {
            // A pattern variable of a record or switch pattern names a value it does not hold (an
            // alias, see `patterns`); that of `x instanceof T v` is a variable.
            "IDENTIFIER"
                if node.symbol().is_some_and(|s| s.kind == "BINDING_VARIABLE")
                    && self.alias(node).is_some() =>
            {
                unsupported(node, "assigning a pattern variable of a record or switch pattern")
            }
            "IDENTIFIER" => Ok(Place::Var(self.local(node)?)),
            "ARRAY_ACCESS" => {
                let (array, index) = self.index(node)?;
                Ok(Place::Index(Box::new(array), Box::new(index)))
            }
            _ => unsupported(node, "assignment to anything but a variable, field or array element"),
        }
    }

    fn call(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let select = node.child("methodSelect")?;
        let Some(symbol) = node.symbol() else { return unsupported(node, "this call") };
        self.note_call(node, &symbol, select);
        if let Some(kind) = self.handle_access(node, &symbol)? {
            return Ok(kind);
        }
        if let Some(kind) = self.sun_memory_call(node, &symbol)? {
            return Ok(kind);
        }
        if let Some(kind) = self.handle_value_call(node, &symbol)? {
            return Ok(kind);
        }
        if let Some(kind) = self.var_handle_access(node, &symbol)? {
            return Ok(kind);
        }
        if let Some(kind) = self.updater_access(node, &symbol)? {
            return Ok(kind);
        }
        self.check_method_handle_call(node, &symbol)?;
        if let Some(kind) = self.class_literal_call(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.constant_desc_call(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.system_log(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.module_call(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.package_call(node, &symbol)? {
            return Ok(kind);
        }
        if let Some(kind) = self.class_call(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.caller_class(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.copy_of_with_class(node, &symbol)? {
            return Ok(kind);
        }
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        if let Some(kind) = self.annotation_member(node, &symbol, receiver)? {
            return Ok(kind);
        }
        if let Some(kind) = self.member_annotations(node, &symbol, receiver)? {
            return Ok(kind);
        }
        let args: Vec<Expr> = node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let mut args = self.packed_args(node, &symbol, args)?;
        if select.kind() == "IDENTIFIER" && select.str("name") == "this" {
            let resolved = self.method_index(&symbol);
            let args = self.convert_args(resolved, args)?;
            let args = self.with_synthetic(self.passed_on(self.current.get(), node.pos())?, args);
            return Ok(ExprKind::ThisCall { args, resolved });
        }
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        if let Some(kind) = self.monitor_call(node, &symbol, receiver, &mut args)? {
            return Ok(kind);
        }
        if symbol.owner == "java.lang.Object" && symbol.name == "clone" {
            return self.object_clone(node, receiver);
        }
        // `Record` declares `Object`'s `equals`, `hashCode` and `toString` again, abstract.
        if matches!(symbol.owner, "java.lang.Object" | "java.lang.Record") && self.object.is_some() {
            return self.object_call(node, &symbol, receiver, args);
        }
        if symbol.owner == "java.lang.Comparable" && symbol.name == "compareTo" {
            return self.comparable_call(node, receiver, args);
        }
        if symbol.owner == "java.lang.Number" {
            return self.number_call(node, &symbol, receiver);
        }
        if symbol.owner == "java.lang.CharSequence"
            && receiver.is_none_or(|r| {
                !matches!(
                    r.java_type(),
                    "java.lang.String" | "java.lang.StringBuilder" | "java.lang.StringBuffer"
                )
            })
        {
            return self.char_sequence_call(node, &symbol, receiver);
        }
        if symbol.name == "equals"
            && !symbol.is_static
            && matches!(&args[..], [arg] if Self::is_object(arg.ty))
            && runtime_type(symbol.owner).is_some()
        {
            return self.value_equals(node, receiver, args);
        }
        if let Some(kind) = self.system_property_call(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        // The translator's method in place of the JDK's (`program::REDIRECTED_CALLS`), but for a
        // number in a range of a string, which the runtime parses (`BitsOp::ParseRange`,
        // `UnsignedOp::ParseRange`): a string's `length` and `charAt` do nothing else.
        if symbol.is_static
            && !(matches!(symbol.owner, "java.lang.Integer" | "java.lang.Long")
                && args.first().is_some_and(|a| a.ty == Type::String))
            && let Some((class, to)) = crate::program::redirected_call(symbol.owner, symbol.name, symbol.ty)
        {
            let to = self.restricted(&symbol, to);
            let resolved = self.template_method(class, to, symbol.ty);
            if resolved.is_none() {
                return unsupported(
                    node,
                    &format!("`{}.{}` outside a whole program", symbol.owner, symbol.name),
                );
            }
            // The arguments of a variable arity are packed for the JDK's method where it is lowered,
            // and for the translator's otherwise.
            let args = match self.method_index(&symbol) {
                Some(_) => args,
                None => self.packed_args_of(node, resolved.expect("checked above"), args)?,
            };
            let mut args = self.convert_generic_args(node, resolved, args)?;
            args.extend(self.caller_arg(node, resolved)?);
            return Ok(ExprKind::Call { name: to.to_string(), args, resolved });
        }
        if let Some(kind) = self.descriptor_call(node, &symbol, receiver, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.file_system_call(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.vm_code_call(&symbol) {
            return Ok(kind);
        }
        if let Some(kind) = self.channel_native_call(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.file_channel_call(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.library_call(node, &symbol, receiver, &args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.blocker_call(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.reflection_native(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.proxy_native(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.module_native(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.annotation_members_native(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.processor_annotations_native(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(builtin) = self.native_call(node, &symbol, receiver, &mut args)? {
            return Ok(ExprKind::Builtin(builtin, args));
        }
        let system_out = receiver.and_then(|r| r.symbol()).is_some_and(|s| {
            s.kind == "FIELD"
                && s.owner == "java.lang.System"
                && s.name == "out"
                && !self.replaced_stream("out")
        });
        if let Some(kind) = self.enum_call(node, &symbol, receiver, &mut args)? {
            return Ok(kind);
        }
        if let Some(builtin) = self.bits_call(&symbol, &args) {
            return Ok(ExprKind::Builtin(builtin, args));
        }
        if let Some(kind) = self.constant_body_call(node, &symbol, receiver, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.class_name_call(&symbol, receiver)? {
            return Ok(kind);
        }
        if let Some(kind) = self.inherit_call(&symbol) {
            return Ok(kind);
        }
        self.check_standard_member(node, &symbol)?;
        let builtin = match (symbol.owner, symbol.name) {
            ("java.io.PrintStream", "println") if system_out => Builtin::Println,
            ("java.io.PrintStream", "print") if system_out => Builtin::Print,
            ("java.io.PrintStream", "printf" | "format") if system_out => {
                return self.printf_call(node, args);
            }
            ("java.io.PrintStream", "printf" | "format") if self.type_index(symbol.owner).is_some() => {
                return self.print_stream_format(node, receiver, args);
            }
            ("java.lang.System", "exit") => Builtin::Exit,
            ("java.lang.Math", "abs") => Builtin::MathAbs,
            ("java.lang.Math", "max") => Builtin::MathMax,
            ("java.lang.Math", "min") => Builtin::MathMin,
            ("java.lang.Math", "sqrt") => Builtin::MathSqrt,
            ("java.lang.Math", "pow") => Builtin::MathPow,
            // Where the JDK's `Arrays` is translated, a call that the runtime does not do is one of
            // it, such as a sort with a `Comparator`, and so is a sort or a fill of a `String[]`
            // that is an `Object[]` where reachability kept the JDK's for it.
            ("java.util.Arrays", name)
                if self.type_index(symbol.owner).is_none() || {
                    let types: Vec<&str> = node.list("arguments").map(|a| a.java_type()).collect();
                    arrays::runtime_arrays_call(name, &types) && !self.jdk_string_array_write(node)
                } =>
            {
                let (writes, copies) =
                    (matches!(name, "sort" | "fill"), matches!(name, "copyOf" | "copyOfRange"));
                let args = self.string_array_args(node, args, writes, copies)?;
                return self.arrays_call(node, name, args);
            }
            ("java.lang.String", "length" | "equals")
                if symbol.name == "length" || matches!(args[0].ty, Type::String | Type::Null) =>
            {
                let receiver = receiver.expect("an instance method has a receiver");
                args.insert(0, self.expr(receiver)?);
                if symbol.name == "length" { Builtin::StringLength } else { Builtin::StringEquals }
            }
            ("java.lang.String", "charAt") => {
                let receiver = receiver.expect("an instance method has a receiver");
                args.insert(0, self.expr(receiver)?);
                Builtin::StringCharAt
            }
            ("java.lang.String", "getBytes") if args.len() <= 1 => {
                let receiver = receiver.expect("an instance method has a receiver");
                if let Some(name) = args.first() {
                    charsets::check_name(node, name)?;
                }
                args.insert(0, self.expr(receiver)?);
                Builtin::String(StringOp::GetBytes)
            }
            ("java.lang.String", _) => {
                let args = self.string_array_args(node, args, false, false)?;
                let kind = self.string_call(node, &symbol, receiver, args)?;
                return Ok(self.object_array_result(node, kind));
            }
            // `String`'s own `join` of the first `size` elements between a prefix and a suffix,
            // which `StringJoiner` reaches through `SharedSecrets` (`String` is the runtime's).
            ("jdk.internal.access.JavaLangAccess", "join") if args.len() == 5 => {
                // Its elements, a `String[]` that may be an `Object[]` (`covariant`).
                args = self.string_array_args(node, std::mem::take(&mut args), false, false)?;
                Builtin::String(StringOp::JoinRange)
            }
            // What the JVM's `JavaLangAccess` does, which a translated program does not have
            // (`program::TRANSLATED_LANG_ACCESS`). Its object, a static field, reads nothing else.
            ("jdk.internal.access.JavaLangAccess", name)
                if crate::program::TRANSLATED_LANG_ACCESS.contains(&name) =>
            {
                let class = crate::program::TRANSLATED_LANG_ACCESS_CLASS;
                let resolved = self.template_method(class, name, symbol.ty);
                if resolved.is_none() {
                    return unsupported(node, &format!("`JavaLangAccess.{name}` outside a whole program"));
                }
                let args = self.convert_args(resolved, args)?;
                return Ok(ExprKind::Call { name: name.to_string(), args, resolved });
            }
            // The shutdown hooks are the translator's `Thread`'s (`java/Threads.java`), which the
            // runtime runs as the program exits, and `exec` is the translator's
            // (`program::REDIRECTED_CALLS`). A `Runtime` has no state of its own: the receiver is
            // `Runtime.getRuntime()` or a name that holds one, and neither needs evaluating.
            (
                "java.lang.Runtime",
                name @ ("addShutdownHook" | "removeShutdownHook" | "exec" | "load" | "loadLibrary"),
            ) if receiver.is_some_and(|r| {
                r.kind() == "IDENTIFIER" || r.symbol().is_some_and(|s| s.name == "getRuntime")
            }) =>
            {
                let resolved = match crate::program::redirected_call(symbol.owner, name, symbol.ty) {
                    Some((class, to)) => self.template_method(class, self.restricted(&symbol, to), symbol.ty),
                    None => self.template_method("java.lang.Thread", name, symbol.ty),
                };
                if resolved.is_none() {
                    return unsupported(node, &format!("`Runtime.{name}` outside a whole program"));
                }
                let mut args = self.convert_args(resolved, args)?;
                args.extend(self.caller_arg(node, resolved)?);
                return Ok(ExprKind::Call { name: name.to_string(), args, resolved });
            }
            // There are no virtual threads: a thread's carrier is the thread itself
            // (`ThreadLocalRandom` keeps its seeds in the carrier).
            ("jdk.internal.access.JavaLangAccess", "currentCarrierThread") => {
                let resolved =
                    self.template_method("java.lang.Thread", "currentThread", "()java.lang.Thread");
                return Ok(ExprKind::Call { name: "currentThread".to_string(), args, resolved });
            }
            // The JDK's own shutdown hooks (`DeleteOnExitHook`'s) are kept by the translator's
            // `Thread` too, which runs them in the order of their slots around the program's.
            ("jdk.internal.access.JavaLangAccess", "registerShutdownHook") => {
                let resolved = self.template_method("java.lang.Thread", "registerShutdownHook", symbol.ty);
                if resolved.is_none() {
                    return unsupported(
                        node,
                        "`JavaLangAccess.registerShutdownHook` outside a whole program",
                    );
                }
                let args = self.convert_args(resolved, args)?;
                return Ok(ExprKind::Call { name: "registerShutdownHook".to_string(), args, resolved });
            }
            // Any other method of the JVM's `JavaLangAccess`, whose object is null in a
            // translated program: rejected where it can run, rather than a `NullPointerException`
            // there. The branches for virtual threads never run (`program::FALSE_INSTANCE_CALLS`).
            ("jdk.internal.access.JavaLangAccess", name) => {
                return unsupported(node, &format!("`JavaLangAccess.{name}`, which the JVM gives,"));
            }
            (
                "java.lang.StringBuilder"
                | "java.lang.StringBuffer"
                | "java.lang.AbstractStringBuilder"
                | "java.lang.CharSequence",
                _,
            ) => {
                return self.builder_call(node, &symbol, receiver, args);
            }
            ("java.lang.Byte" | "java.lang.Short", "compareUnsigned") if args.len() == 2 => {
                return Ok(self.small_compare_unsigned(node, &symbol, args));
            }
            (owner, name) if unsigned::is_unsigned_method(owner, name) => {
                self.unsigned_call(node, &symbol, &args)?
            }
            (owner, name) if code_point::is_code_point_method(owner, name, &args) => {
                self.code_point_call(node, &symbol, &args)?
            }
            (owner, name) if boxing::boxed_type(owner).is_some() && boxing::is_boxed_method(name) => {
                self.boxed_call(node, &symbol, &mut args, receiver)?
            }
            // The rest of `Character` is the JDK's, where a whole program has it.
            ("java.lang.Character", name)
                if runtime_character_method(name, symbol.ty) || self.type_index(symbol.owner).is_none() =>
            {
                let char_arg = args.first().is_some_and(|a| a.ty == Type::Char);
                self.character_method(node, name, symbol.ty, char_arg)?
            }
            ("java.util.Objects", "requireNonNull")
                if args.first().is_some_and(|a| a.ty == Type::Charset) =>
            {
                return self.required_charset(node, args);
            }
            // Where the JDK's `Objects` is translated, its calls are calls of it, but for the calls
            // that the members written for records make on values that are not `Object`s.
            ("java.util.Objects", name)
                if self.type_index(symbol.owner).is_none() || node.0["implicit"] == Value::Bool(true) =>
            {
                return self.objects_call(node, name, args);
            }
            _ if self.type_index(symbol.owner).is_some() => {
                let name = symbol.name.to_string();
                let resolved = self.method_index(&symbol);
                // The default and static methods of a JDK interface that lowering declares (see
                // `functional`) are the JDK's code, which only a translation with its sources has.
                if resolved.is_none() && self.functional.has(symbol.owner) {
                    let kind = if symbol.is_static { "static" } else { "default" };
                    let message = format!(
                        "the {kind} method `{}.{}` is the JDK's code: translate the program with the \
                         JDK's sources (`--main` and `--library`) for it",
                        symbol.owner, symbol.name
                    );
                    return error(node.pos(), message);
                }
                let mut args = self.convert_generic_args(node, resolved, args)?;
                if symbol.is_static {
                    args.extend(self.caller_arg(node, resolved)?);
                    return Ok(ExprKind::Call { name, args, resolved });
                }
                // `super.m()`, or `Iface.super.m()` for a default method of an interface, in a
                // class or in another interface's default method.
                let is_super = receiver.is_some_and(|r| {
                    r.kind() == "IDENTIFIER" && r.str("name") == "super"
                        || r.kind() == "MEMBER_SELECT" && r.str("identifier") == "super"
                });
                let receiver = match receiver {
                    Some(r) if is_super => self.super_receiver(r)?,
                    Some(r) => self.receiver(r, symbol.owner)?,
                    None => self.this(node),
                };
                // An interface's method on an enum that leaves it to its constants' bodies is the
                // enum's own (see `constant_bodies::body_methods`).
                let resolved = match receiver.ty {
                    Type::Enum(e) if !is_super => self.body_method(e, resolved).or(resolved),
                    _ => resolved,
                };
                let receiver = Box::new(receiver);
                return Ok(ExprKind::MethodCall { receiver, name, args, resolved, is_super });
            }
            (owner, name) => return unsupported(node, &format!("method `{owner}.{name}`")),
        };
        Ok(ExprKind::Builtin(builtin, args))
    }
}

/// `super(...)` or `outer.super(...)`, which javac writes into every constructor that does not
/// call another one, or a constructor that calls its superclass's with an enclosing instance.
fn is_super_call(stmt: Node) -> bool {
    stmt.get("expression").is_some_and(|call| {
        call.kind() == "METHOD_INVOCATION"
            && call.get("methodSelect").is_some_and(|s| {
                s.kind() == "IDENTIFIER" && s.str("name") == "super"
                    || s.kind() == "MEMBER_SELECT" && s.str("identifier") == "super"
            })
    })
}

fn binary_op(node: Node, kind: &str) -> Result<BinOp, Error> {
    Ok(match kind {
        "PLUS" => BinOp::Add,
        "MINUS" => BinOp::Sub,
        "MULTIPLY" => BinOp::Mul,
        "DIVIDE" => BinOp::Div,
        "REMAINDER" => BinOp::Rem,
        "LEFT_SHIFT" => BinOp::Shl,
        "RIGHT_SHIFT" => BinOp::Shr,
        "UNSIGNED_RIGHT_SHIFT" => BinOp::UShr,
        "AND" => BinOp::BitAnd,
        "OR" => BinOp::BitOr,
        "XOR" => BinOp::BitXor,
        "CONDITIONAL_AND" => BinOp::And,
        "CONDITIONAL_OR" => BinOp::Or,
        "EQUAL_TO" => BinOp::Eq,
        "NOT_EQUAL_TO" => BinOp::Ne,
        "LESS_THAN" => BinOp::Lt,
        "LESS_THAN_EQUAL" => BinOp::Le,
        "GREATER_THAN" => BinOp::Gt,
        "GREATER_THAN_EQUAL" => BinOp::Ge,
        _ => return unsupported(node, &kind_words(kind)),
    })
}

/// The value of the condition `node` when it is a constant expression (JLS 15.29) of `true` and
/// `false`, constant variables named simply or by their class, parentheses, `!`, `&&` and `||`.
fn constant_condition(node: Node) -> Option<bool> {
    match node.kind() {
        "BOOLEAN_LITERAL" => node.0["value"].as_bool(),
        "PARENTHESIZED" => constant_condition(node.get("expression")?),
        "LOGICAL_COMPLEMENT" => constant_condition(node.get("expression")?).map(|b| !b),
        "CONDITIONAL_AND" => Some(
            constant_condition(node.get("leftOperand")?)? && constant_condition(node.get("rightOperand")?)?,
        ),
        "CONDITIONAL_OR" => Some(
            constant_condition(node.get("leftOperand")?)? || constant_condition(node.get("rightOperand")?)?,
        ),
        "IDENTIFIER" | "MEMBER_SELECT" => {
            if node.kind() == "MEMBER_SELECT" {
                let class = node.get("expression")?.symbol()?;
                if !matches!(class.kind, "CLASS" | "INTERFACE" | "ENUM" | "RECORD" | "ANNOTATION_TYPE") {
                    return None;
                }
            }
            let symbol = node.symbol()?;
            match (symbol.ty, symbol.constant?) {
                ("boolean", "true" | "1") => Some(true),
                ("boolean", "false" | "0") => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}
