//! Lowering of a whole program: the reachable classes of every compilation unit, each named by
//! its qualified name (`com.example.Point`, as javac writes it in symbols and types) and tagged
//! with its package.

use super::*;
use crate::program::Reach;

impl<'t> Lowering<'t> {
    /// Whether a member of the class `class` is lowered: in a whole program only reachable
    /// members are, in a single file all of them. A native method never is: it has no body, and
    /// its calls become the runtime's code or are rejected (`native_call`).
    pub(super) fn keeps(&self, member: Node<'t>, class: &str) -> bool {
        if member.kind() == "METHOD" && member.has_flag("NATIVE") {
            return false;
        }
        if self.standard.has(class) {
            return self.standard.keeps(member, class);
        }
        if self.functional.has(class) || self.plain_object.is_some_and(|t| self.types[t].javac == class) {
            return true;
        }
        self.reach.is_none_or(|reach| reach.member(class, member.0))
    }

    /// The only branch of an `if` or a `?:` that can run, when reachability skipped the other
    /// (`program::live_branch`): that one is not lowered either. `None` for an `if` without the
    /// branch.
    pub(super) fn live_branch(&self, node: Node<'t>) -> Option<Option<Node<'t>>> {
        let sequential = self.reach.is_some_and(|r| r.sequential);
        crate::program::live_branch(node.0, sequential).map(|v| (!v.is_null()).then_some(Node(v)))
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
    super::real_generics::retry(|real, covariant| {
        lower_with(units, program_units, reach, main, real, covariant, None)
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
    let census = RefCell::new((Vec::new(), Vec::new()));
    let class = super::real_generics::retry(|real, covariant| {
        lower_with(units, program_units, reach, main, real, covariant, Some((&census, census_of)))
    });
    let (rejected, units) = census.into_inner();
    Lowered { class, rejected, units }
}

/// What a census's lowering hands back on each attempt: the rejections, and the unit of each type.
type CensusSink = RefCell<(Vec<Rejected>, Vec<Option<usize>>)>;

/// `lower_program` with the Rust generics `real`, or with the candidates when it is `None`, and
/// the arrays `covariant`.
fn lower_with<'t>(
    units: &[&'t Value],
    program_units: usize,
    reach: &'t Reach,
    main: &str,
    real: Option<&super::real_generics::Real>,
    covariant: &super::covariant::Covariant,
    census: Option<(&CensusSink, Option<bool>)>,
) -> Result<(Class, super::real_generics::Real, super::real_generics::Real), (Option<usize>, Error)> {
    let mut lowering = Lowering::empty();
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
    let rejected: Vec<Rejected> = rejected
        .into_iter()
        .map(|(t, anchor, method, error)| Rejected { unit: unit(t), anchor, error, ty: t, method })
        .collect();
    // As without a census, an error with Rust generics is retried with all of them erased, but
    // only an error in code that can run (`analysis::live_methods`): the others leave the
    // program as it is, and the Rust generics may stay.
    let retried = !lowering.real.is_empty()
        && !rejected.is_empty()
        && lowered.as_ref().is_ok_and(|(class, ..)| {
            match crate::analysis::live_methods::live_methods(class) {
                Ok(live) => rejected.iter().any(|r| live.runs(r.ty, r.method)),
                Err(_) => true,
            }
        });
    let first = rejected.first().map(|r| Error { pos: r.error.pos, message: r.error.message.clone() });
    *census.borrow_mut() = (rejected, (0..lowering.types.len()).map(unit).collect());
    match first {
        Some(error) if retried => Err((None, error)),
        _ => lowered,
    }
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
                lowering.annotation_types.insert(crate::program::class_name(decl.0), decl);
                if index >= program_units {
                    lowering.library_annotations.insert(crate::program::class_name(decl.0));
                }
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
    lowering.standard = super::exceptions::Standard::select(reachable.iter().copied());
    lowering.declare_standard().map_err(|e| (None, e))?;
    let declared = lowering.types.iter().map(|t| t.name.clone()).collect();
    // The standard classes may name interfaces too (`Thread` holds a `Runnable`).
    let trees = reachable.iter().copied().chain(lowering.standard.classes.iter().map(|(_, d)| *d));
    lowering.functional = super::functional::Functional::select(trees, &declared);
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
    lowering.real = real.cloned().unwrap_or_else(|| lowering.candidates());
    let planned: Vec<Node> = decls.iter().copied().chain(lowering.functional_decls()).collect();
    lowering.plan_object(&planned);
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
    let failed = lowering.verify(&types, &methods);
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
        volatile_elements: lowering.volatile_elements.take(),
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
