//! Lowering of a whole program: the reachable classes of every compilation unit, each named by
//! its qualified name (`com.example.Point`, as javac writes it in symbols and types) and tagged
//! with its package.

use super::*;
use crate::program::Reach;
use std::rc::Rc;

impl<'t> Lowering<'t> {
    /// Whether a member of the class `class` is lowered: in a whole program only reachable
    /// members are, in a single file all of them. A native method has no body, and its calls
    /// become the runtime's code or are rejected (`native_call`), so it is not lowered, but for
    /// one that a call of a method it implements or overrides may run (`native_dispatch`).
    pub(super) fn keeps(&self, member: Node<'t>, class: &str) -> bool {
        if member.kind() == "METHOD" && member.has_flag("NATIVE") {
            return self.native_dispatch(member, class);
        }
        if self.standard.has(class) {
            return self.standard.keeps(member, class);
        }
        if self.functional.has(class) || self.plain_object.is_some_and(|t| self.types[t].javac == class) {
            return true;
        }
        self.reach.is_none_or(|reach| reach.member(class, member.0))
    }

    /// Whether the native method `member` of class `class` is lowered as a method whose body calls
    /// it: in a whole program, a reachable instance method that is not private, as
    /// `sun.management.VMManagementImpl`'s natives implement `VMManagement`'s methods. A call of
    /// the method it implements may run it, which needs a method to dispatch to (`native_body`).
    /// The natives whose calls lowering builds on its own (`native_call`) are left to their calls.
    pub(super) fn native_dispatch(&self, member: Node<'t>, class: &str) -> bool {
        let Some(reach) = self.reach else { return false };
        let signature = member.symbol().map_or("", |s| s.ty);
        let lowered_alone = matches!(
            crate::ast::declared_native(class, member.str("name"), signature),
            Some(
                Native::ThreadStart
                    | Native::InstallUncaught
                    | Native::InstallShutdown
                    | Native::ReflectArrayLength
                    | Native::ReflectArraySet
            )
        );
        !member.has_flag("STATIC")
            && !member.has_flag("PRIVATE")
            && !lowered_alone
            && reach.member(class, member.0)
    }

    /// The only branch of an `if` or a `?:` that can run, when reachability skipped the other
    /// (`program::live_branch`): that one is not lowered either. `None` for an `if` without the
    /// branch.
    pub(super) fn live_branch(&self, node: Node<'t>) -> Option<Option<Node<'t>>> {
        let sequential = self.reach.is_some_and(|r| r.sequential);
        crate::program::live_branch(node.0, sequential).map(|v| (!v.is_null()).then_some(Node(v)))
    }

    /// Whether the program replaces the standard stream `stream` (`out` or `err`), whose printing
    /// calls then go to the object of the field, not to the runtime (`Reach::replaced_streams`).
    pub(super) fn replaced_stream(&self, stream: &str) -> bool {
        self.reach.is_some_and(|r| r.replaced_streams.contains(stream))
    }
}

/// Lowers the reachable part of a program made of these compilation units, whose entry point is
/// the `main` of the class with qualified name `main`. An error comes with the index of the unit
/// it is in.
pub fn lower_program<'t>(
    units: &[&'t Value],
    program_units: usize,
    reach: &'t Reach,
    main: &str,
) -> Result<Class, (Option<usize>, Error)> {
    let scans = Rc::default();
    super::real_generics::retry(|real, covariant| {
        lower_with(units, program_units, reach, main, real, covariant, &scans, None)
    })
}

/// A program lowered for a census: the lowered program, or the error that ended lowering all the
/// same; and every piece of code that lowering rejected, each error with the unit it is in and
/// the position of the method, initializer or lambda that it is in.
pub struct Lowered {
    pub class: Result<Class, (Option<usize>, Error)>,
    pub rejected: Vec<Rejected>,
    /// The unit of each type of the lowered program, `None` for those that lowering provides.
    pub units: Vec<Option<usize>>,
    /// The methods of the lowered program that can run, when lowering found them (to know
    /// whether its Rust generics stay, see `lower_with`).
    pub live: Option<crate::analysis::live_methods::LiveMethods>,
    /// The code that holds each `Arrays.sort` or `Arrays.fill` of a `String[]` that is an
    /// `Object[]` and that calls the JDK's method for it (with `Reach::string_array_writes`), as
    /// `Rejected::ty` and `Rejected::method` name code.
    pub string_array_writes: Vec<(usize, Option<usize>)>,
    /// Whether any attempt of lowering (`real_generics::retry`) lowered one: when none did,
    /// `Reach::string_array_writes` changed nothing.
    pub wrote_string_arrays: bool,
}

/// An error in a piece of code that lowering rejected and went on past: the unit of the code,
/// the position of the method or lambda that it is in (none in an initializer), and the error.
pub struct Rejected {
    pub unit: Option<usize>,
    pub anchor: Option<Pos>,
    pub error: Error,
    /// The type whose code it is in, by index into `Class::types`.
    pub ty: usize,
    /// The method, by index into `Class::methods`, when the code is part of its body alone: a
    /// method that lowering stubbed, or a block or an initializer left out of its body.
    pub method: Option<usize>,
}

/// `lower_program` for a census (see `census`): a piece of code that lowering rejects is recorded
/// and replaced by a stub, and lowering goes on. A panic while lowering a method is a rejection
/// of the method.
pub fn census_lower_program<'t>(
    units: &[&'t Value],
    program_units: usize,
    reach: &'t Reach,
    main: &str,
    ends_rejected: bool,
) -> Lowered {
    lower_going_on(units, program_units, reach, main, Some(ends_rejected))
}

/// `census_lower_program` for the translation of a whole program, which keeps the methods that
/// can run (`analysis::live_methods`): what lowering rejects in the others is no reason to
/// reject the program. A panic ends lowering, as in `lower_program`, and the check for null
/// enums is left to the caller, on the methods that can run.
pub fn tolerant_lower_program<'t>(
    units: &[&'t Value],
    program_units: usize,
    reach: &'t Reach,
    main: &str,
) -> Lowered {
    lower_going_on(units, program_units, reach, main, None)
}

/// `census_lower_program`, with `Some(ends_rejected)` for a census, which takes panics for
/// rejections; `None` for a translation, which does not.
fn lower_going_on<'t>(
    units: &[&'t Value],
    program_units: usize,
    reach: &'t Reach,
    main: &str,
    census_of: Option<bool>,
) -> Lowered {
    let census = RefCell::new(CensusSink::default());
    let scans = Rc::default();
    let class = super::real_generics::retry(|real, covariant| {
        lower_with(units, program_units, reach, main, real, covariant, &scans, Some((&census, census_of)))
    });
    let CensusSink { rejected, units, live, string_array_writes, wrote_string_arrays } = census.into_inner();
    Lowered { class, rejected, units, live, string_array_writes, wrote_string_arrays }
}

/// What a census's lowering hands back on each attempt: the rejections, the unit of each type,
/// the methods that can run, when it found them, and the code with the JDK's writes of
/// `String[]`s; and whether any attempt lowered one of those.
#[derive(Default)]
struct CensusSink {
    rejected: Vec<Rejected>,
    units: Vec<Option<usize>>,
    live: Option<crate::analysis::live_methods::LiveMethods>,
    string_array_writes: Vec<(usize, Option<usize>)>,
    wrote_string_arrays: bool,
}

/// `lower_program` with the Rust generics `real`, or with the candidates when it is `None`, and
/// the arrays `covariant`; `scans` holds what the attempts before found of the trees.
#[allow(clippy::too_many_arguments)]
fn lower_with<'t>(
    units: &[&'t Value],
    program_units: usize,
    reach: &'t Reach,
    main: &str,
    real: Option<&super::real_generics::Real>,
    covariant: &super::covariant::Covariant,
    scans: &Rc<super::scans::Scans<'t>>,
    census: Option<(&RefCell<CensusSink>, Option<bool>)>,
) -> Result<(Class, super::real_generics::Real, super::real_generics::Real), (Option<usize>, Error)> {
    let mut lowering = Lowering::empty();
    lowering.scans = scans.clone();
    lowering.covariant = covariant.clone();
    lowering.reach = Some(reach);
    lowering.census = census.map(|_| RefCell::default());
    lowering.catch_panics = census.is_some_and(|(_, census_of)| census_of.is_some());
    lowering.ends_rejected = census.is_some_and(|(_, census_of)| census_of == Some(true));
    let mut unit_of = Vec::new();
    let lowered = lower_into(&mut lowering, &mut unit_of, units, program_units, reach, main, real);
    let Some((census, _)) = census else { return lowered };
    // Each attempt's rejections replace the last's.
    let unit = |t: usize| unit_of.get(t).copied().filter(|&u| u != usize::MAX);
    let rejected = lowering.census.take().unwrap_or_default().into_inner();
    // Whether each rejection may go away with the Rust generics erased: it is in code that uses
    // them (`Lowering::uses_real`), and not of a missing class (`names_missing_class`,
    // `names_untranslated_class`), which stays missing, nor one that no type changes
    // (`no_type_changes`). Code that uses none of them is lowered the same without them.
    let generic: Vec<bool> = rejected
        .iter()
        .map(|(.., error, uses_real)| {
            !uses_real.is_empty()
                && !names_missing_class(error)
                && !names_untranslated_class(error)
                && !no_type_changes(error)
        })
        .collect();
    let (rejected, uses): (Vec<Rejected>, Vec<super::real_generics::Real>) = rejected
        .into_iter()
        .map(|(t, anchor, method, error, uses)| {
            (Rejected { unit: unit(t), anchor, error, ty: t, method }, uses)
        })
        .unzip();
    // An error with Rust generics is retried without the Rust generics that the rejected code
    // uses (where a lowering without a census retries with all of them erased), but only an error
    // in code that can run (`analysis::live_methods`) and that may go away without them
    // (`generic`): the others leave the program as it is, and the Rust generics may stay. A
    // lowering that noted covariant arrays is lowered again whatever it found
    // (`real_generics::retry`), so its methods that can run are not looked for; nor are they for
    // one with Rust generics that failed (`real_generics::verify`) and no such error, which is
    // lowered again without those. Nor are they once a rejection can run, as that lowering is
    // retried: the walk stops there (`live_methods_unless`). Those found are handed on with the
    // program, which a census narrows to them (`program::narrow`).
    let live = match &lowered {
        Ok((class, _, failed))
            if !lowering.real.is_empty()
                && lowering.covariant.noted.borrow().is_empty()
                && !rejected.is_empty()
                && (failed.is_empty() || generic.contains(&true)) =>
        {
            let code: Vec<(usize, Option<usize>)> =
                rejected.iter().zip(&generic).filter(|(_, g)| **g).map(|(r, _)| (r.ty, r.method)).collect();
            Some(crate::timings::phase("methods that can run", || {
                crate::analysis::live_methods::live_methods_unless(class, &code)
            }))
        }
        _ => None,
    };
    let retried = live.as_ref().is_some_and(|live| match live {
        Ok(Some(live)) => rejected.iter().zip(&generic).any(|(r, g)| *g && live.runs(r.ty, r.method)),
        // The walk stopped at one, or the facts do not tell which can run.
        Ok(None) | Err(_) => generic.contains(&true),
    });
    let live = live.and_then(Result::ok).flatten();
    // The error retried for: one that runs, or, when the walk stopped at one (and says which),
    // the first that may go away without the Rust generics.
    let first = rejected
        .iter()
        .zip(&generic)
        .find(|(r, g)| **g && live.as_ref().is_none_or(|l| l.runs(r.ty, r.method)))
        .map(|(r, _)| r)
        .or(rejected.first())
        .map(|r| Error { pos: r.error.pos, message: r.error.message.clone() });
    // The Rust generics that the code of the errors retried for uses, which are erased.
    let mut erased = super::real_generics::Real::default();
    for ((r, uses), _) in rejected.iter().zip(&uses).zip(&generic).filter(|(_, g)| **g) {
        if live.as_ref().is_none_or(|l| l.runs(r.ty, r.method)) {
            erased.extend(uses);
        }
    }
    let mut sink = census.borrow_mut();
    let string_array_writes = lowering.string_array_writes.take();
    sink.wrote_string_arrays |= !string_array_writes.is_empty();
    (sink.rejected, sink.units, sink.live, sink.string_array_writes) =
        (rejected, (0..lowering.types.len()).map(unit).collect(), live, string_array_writes);
    drop(sink);
    match first {
        Some(error) if retried => {
            if crate::timings::enabled() {
                eprintln!("timing: lowering with Rust generics rejects code that can run: {}", error.message);
            }
            lowered.map(|(class, used, mut failed)| {
                failed.extend(&erased);
                (class, used, failed)
            })
        }
        _ => lowered,
    }
}

/// Words of the errors that the Rust generics do not cause, each with why no type changes it.
const NO_TYPE_CHANGES: &[&str] = &[
    // Asking for the caller (`callers`): where the method is and what it is.
    "getCallerClass",
    // An access through a `VarHandle` that is neither a constant nor one that lowering follows:
    // which handle a value holds, the trees tell (`handle_values`).
    super::handles::UNFOLLOWED_HANDLE,
    // A method of `JavaLangAccess` that the JVM implements (`lower.rs`).
    ", which the JVM gives,",
    // An annotation of the JDK's that the translation leaves out (`annotation_values`,
    // `annotation_presence`).
    " of the JDK's `",
    // javac's own error in the source (`census`).
    "javac reports a compile error",
    // A handle or a field offset where lowering takes none (`handle_values::check_handle_value`,
    // `handles`), and a handle's call that it could not resolve: both read from the trees.
    " used as a value",
    "that lowering left unresolved",
    // A method of `Class` that the translation does not give (`classes`).
    "`Class.",
    // `printStackTrace` of a stream or writer overridden (`exceptions`).
    "of a stream or writer",
];

/// Whether `error` is one that the Rust generics do not cause (`NO_TYPE_CHANGES`): the JDK's
/// `AbstractMemorySegmentImpl` and `MetadataRepository` cost Minecraft a lowering each without
/// it, erasing the Rust generics their methods use and leaving the rejections as they were.
fn no_type_changes(error: &Error) -> bool {
    NO_TYPE_CHANGES.iter().any(|words| error.message.contains(words))
}

/// Whether `error` is that of a class that the program does not translate, which it names with
/// its feature (``new java.io.ObjectOutputStream` (serialization) not supported yet`, as of a
/// static field of one or `super(..)` of one): erased, the class stays untranslated.
fn names_untranslated_class(error: &Error) -> bool {
    let Some((named, feature)) =
        error.message.strip_suffix(") not supported yet").and_then(|rest| rest.rsplit_once(" ("))
    else {
        return false;
    };
    let class =
        named.strip_suffix('`').and_then(|n| n.rsplit_once('`')).map(|(_, c)| c.trim_start_matches("new "));
    class.and_then(crate::program::untranslated) == Some(feature)
}

/// Whether `error` is that of a class the program does not have (`type `a.b.C` not supported
/// yet`, `new a.b.C`, as of a field of an untranslated class's type and its initializer, and
/// `extending `a.b.C`` or `implementing `a.b.C`` of a class whose superclass or interface is
/// not translated, named by its erasure): erasing the Rust generics leaves the class missing, so
/// it is no reason to lower the program again without them. A type variable's error (`type `B``)
/// may be one.
fn names_missing_class(error: &Error) -> bool {
    let message = &error.message;
    ["type `", "`new ", "extending `", "implementing `"]
        .iter()
        .find_map(|prefix| message.strip_prefix(prefix))
        .and_then(|rest| rest.strip_suffix("` not supported yet"))
        .is_some_and(|name| name.contains('.') && !name.contains(['<', '`', ' ']))
}

/// `lower_with` on `lowering`, which fills `unit_of` with the unit of each type (`usize::MAX` for
/// those that belong to none).
fn lower_into<'t>(
    lowering: &mut Lowering<'t>,
    unit_of: &mut Vec<usize>,
    units: &[&'t Value],
    program_units: usize,
    reach: &'t Reach,
    main: &str,
    real: Option<&super::real_generics::Real>,
) -> Result<(Class, super::real_generics::Real, super::real_generics::Real), (Option<usize>, Error)> {
    // The unit and the package of each class, in `Class::types` order.
    let mut packages = Vec::new();
    for (index, unit) in units.iter().enumerate() {
        let unit = Node(unit);
        let package = unit.get("packageName").map(dotted).unwrap_or_default();
        for decl in unit.list("typeDecls") {
            // An annotation type does nothing when the program runs: annotations are left out,
            // and a value of one is rejected where the code has it (`Lowering::annotation_types`).
            if decl.kind() == "ANNOTATION_TYPE" {
                let name = crate::program::class_name(decl.0);
                if index >= program_units && crate::program::in_jdk(&name) {
                    lowering.library_annotations.insert(name.clone());
                }
                lowering.annotation_types.insert(name, decl);
            } else if reach.classes.contains(&crate::program::class_name(decl.0)) {
                let outer = (!package.is_empty()).then_some(package.as_str());
                lowering.declare_type(decl, outer).map_err(|e| (Some(index), e))?;
            } else {
                // The classes nested in a runtime class that are the JDK's code all the same.
                let outer = crate::program::class_name(decl.0);
                for nested in translated_nested(decl, reach) {
                    lowering.declare_type(nested, Some(&outer)).map_err(|e| (Some(index), e))?;
                }
            }
        }
        unit_of.resize(lowering.types.len(), index);
        packages.resize(lowering.types.len(), package);
    }
    // The classes that are not translated but are types of variables, which are only ever null.
    for (class, methods) in &reach.stubs {
        lowering.declare_stub(class, methods).map_err(|e| (None, e))?;
        unit_of.resize(lowering.types.len(), usize::MAX);
        packages.resize(lowering.types.len(), super::stubs::stub_package(class));
    }
    // The standard exception classes that reachable code uses; they belong to no unit.
    let mut reachable = Vec::new();
    for unit in units {
        for decl in Node(unit).list("typeDecls") {
            reachable_members(decl, reach, &mut reachable);
        }
    }
    lowering.standard = lowering.scans.standard(&reachable);
    lowering.declare_standard().map_err(|e| (None, e))?;
    let declared = lowering.types.iter().map(|t| t.name.clone()).collect();
    // The standard classes may name interfaces too (`Thread` holds a `Runnable`).
    let trees: Vec<&Value> =
        reachable.iter().copied().chain(lowering.standard.classes.iter().map(|(_, d)| *d)).collect();
    lowering.functional = lowering.scans.functional(&trees, &declared);
    lowering.declare_functional().map_err(|e| (None, e))?;
    lowering.declare_plain_object(reachable.iter().copied()).map_err(|e| (None, e))?;
    unit_of.resize(lowering.types.len(), usize::MAX);
    lowering.name_nested();
    lowering.check_class_headers();
    lowering.plan_captures().map_err(|(t, e)| ((unit_of[t] != usize::MAX).then_some(unit_of[t]), e))?;
    packages.resize(lowering.types.len(), String::new());
    lowering.entry = lowering.type_index(main);
    // The units after the program's are the library's; a stub's class is the library's too.
    lowering.library = unit_of.iter().map(|&unit| unit >= program_units).collect();
    let unit = |t: usize| (unit_of[t] != usize::MAX).then_some(unit_of[t]);
    let decls: Vec<Node> = units
        .iter()
        .flat_map(|u| Node(u).list("typeDecls"))
        .flat_map(|d| match reach.classes.contains(&crate::program::class_name(d.0)) {
            true if d.kind() != "ANNOTATION_TYPE" => vec![d],
            true => Vec::new(),
            false => translated_nested(d, reach),
        })
        .collect();
    let planned: Vec<Node> = decls.iter().copied().chain(lowering.functional_decls()).collect();
    lowering.start_real(real);
    lowering.plan_object(&planned);
    lowering.predict_arrays();
    let mut types = Vec::new();
    for (t, package) in packages.into_iter().enumerate() {
        let mut decl = lowering.type_decl_or_stub(t).map_err(|e| (unit(t), e))?;
        decl.package = package;
        types.push(decl);
    }
    let mut methods: Vec<Method> = (0..lowering.methods.len())
        .map(|m| lowering.method_or_stub(m).map_err(|e| (unit(lowering.methods[m].owner), e)))
        .collect::<Result<_, _>>()?;
    lowering.suppress_checks(&mut methods);
    lowering.init_cause_checks(&mut methods);
    lowering.object_methods(&mut methods).map_err(|e| (None, e))?;
    lowering.finish_types(&mut types, &mut methods).map_err(|(t, e)| (unit(t), e))?;
    lowering.object_types(&mut types);
    lowering.param_decls(&mut types);
    let mut failed = lowering.verify(&types, &methods);
    lowering.noted_later(&mut failed);
    let hierarchy = Hierarchy::new(&types, &methods);
    let raised = lowering.raised_objects(&types, &methods);
    let class_file = lowering.class_file_names(&types);
    let notes = lowering.notes.take();
    let annotation_classes = lowering.annotation_classes.take();
    let class = Class {
        name: main.to_string(),
        types,
        methods,
        hierarchy,
        raised,
        class_file,
        notes,
        annotation_classes,
        shared_strings: false,
        module_labels: module_labels(reach),
        thread_statics: None,
        element_classes: lowering.element_classes.take(),
        array_likes: lowering.array_likes.take(),
        covariant_arrays: lowering.covariant_arrays(),
        object_rows: lowering.object_rows.take(),
        array_tests: lowering.array_tests.take(),
        volatile_elements: lowering.volatile_elements.take(),
        proxy_classes: (0..lowering.types.len()).filter(|&t| lowering.is_proxy_class(t)).collect(),
        receivers: Default::default(),
    };
    Ok((class, lowering.real.clone(), failed))
}

/// `Class::module_labels` of the named modules among the roots; a package in two roots is the
/// first's, as the class path would have it.
fn module_labels(reach: &Reach) -> std::collections::HashMap<String, String> {
    let mut labels = std::collections::HashMap::new();
    for m in reach.modules.iter().filter(|m| !m.name.is_empty()) {
        let label = format!("module {} of loader '{}'", m.name, crate::program::loader_name(&m.name));
        for package in &m.packages {
            labels.entry(package.clone()).or_insert_with(|| label.clone());
        }
    }
    labels
}

/// The classes nested in `decl`, a class that is not translated, that are translated all the
/// same and reachable (`program::TRANSLATED_NESTED`).
fn translated_nested<'t>(decl: Node<'t>, reach: &Reach) -> Vec<Node<'t>> {
    if !crate::program::provided(&crate::program::class_name(decl.0)) {
        return Vec::new();
    }
    decl.list("members")
        .filter(|m| {
            let class = crate::program::class_name(m.0);
            crate::program::CLASS_KINDS.contains(&m.kind())
                && crate::program::TRANSLATED_NESTED.contains(&class.as_str())
                && reach.classes.contains(&class)
        })
        .collect()
}

/// The members of a class declaration (and of its nested classes) that are reachable.
fn reachable_members<'t>(decl: Node<'t>, reach: &Reach, out: &mut Vec<&'t Value>) {
    let class = crate::program::class_name(decl.0);
    if decl.kind() != "ANNOTATION_TYPE" && !reach.classes.contains(&class) {
        for nested in translated_nested(decl, reach) {
            reachable_members(nested, reach, out);
        }
        return;
    }
    if decl.kind() == "ANNOTATION_TYPE" {
        return;
    }
    for member in decl.list("members") {
        if crate::program::CLASS_KINDS.contains(&member.kind()) {
            reachable_members(member, reach, out);
        } else if reach.member(&class, member.0) {
            out.push(member.0);
        }
    }
}

/// A package name such as `com.example` from its tree, a chain of selects.
fn dotted(node: Node) -> String {
    match node.get("expression") {
        Some(outer) => format!("{}.{}", dotted(outer), node.str("identifier")),
        None => node.str("name").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(message: &str) -> Error {
        Error { pos: Pos::default(), message: message.to_string() }
    }

    #[test]
    fn an_error_no_type_changes_is_no_reason_to_erase_the_rust_generics() {
        let message =
            format!("`VarHandle.set` {} not supported yet", super::super::handles::UNFOLLOWED_HANDLE);
        assert!(no_type_changes(&error(&message)));
        assert!(no_type_changes(&error("`getAnnotation` of the JDK's `jdk.jfr.Name` not supported yet")));
        assert!(no_type_changes(&error(
            "`JavaLangAccess.addExports`, which the JVM gives, not supported yet"
        )));
        assert!(no_type_changes(&error(
            "asking for the caller (`getCallerClass`) in a method that is not static not supported yet"
        )));
        assert!(no_type_changes(&error("a `MethodHandle` used as a value not supported yet")));
        assert!(no_type_changes(&error("`Class.getAnnotations` (reflection) not supported yet")));
        assert!(!no_type_changes(&error("`VarHandle.set` not supported yet")));
        assert!(!no_type_changes(&error(
            "a `java.lang.Class` as a `java.lang.reflect.AnnotatedElement` not supported yet"
        )));
        assert!(!no_type_changes(&error("an atomic update of an element of a `T[]` not supported yet")));
    }

    #[test]
    fn an_untranslated_class_is_no_reason_to_erase_the_rust_generics() {
        assert!(names_untranslated_class(&error(
            "`new java.io.ObjectOutputStream` (serialization) not supported yet"
        )));
        assert!(names_untranslated_class(&error(
            "`super(..)` of `java.io.ObjectInputStream` (serialization) not supported yet"
        )));
        assert!(!names_untranslated_class(&error(
            "`new java.util.ArrayList` (serialization) not supported yet"
        )));
        assert!(!names_untranslated_class(&error("`Class.getAnnotations` (reflection) not supported yet")));
    }

    #[test]
    fn a_missing_class_is_no_reason_to_erase_the_rust_generics() {
        assert!(names_missing_class(&error("type `jdk.internal.loader.ClassLoaderValue` not supported yet")));
        assert!(names_missing_class(&error("`new jdk.internal.loader.ClassLoaderValue` not supported yet")));
        assert!(names_missing_class(&error("extending `java.security.SecureClassLoader` not supported yet")));
        assert!(names_missing_class(&error("implementing `java.nio.file.WatchService` not supported yet")));
        assert!(!names_missing_class(&error("type `B` not supported yet")));
        assert!(!names_missing_class(&error("type `java.util.List<B>` not supported yet")));
        assert!(!names_missing_class(&error(
            "`Arrays.copyOf` into an array of another type not supported yet"
        )));
    }
}
