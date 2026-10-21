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
        self.reach.is_none_or(|reach| reach.member(class, member.0))
    }

    /// Whether the condition of an `if` or a `?:` is never true, so that reachability skipped the
    /// branch it guards (`program::never_true`): that branch is not lowered either.
    pub(super) fn never_true(&self, node: Node<'t>) -> bool {
        crate::program::never_true(&node.0["condition"], self.reach.is_some_and(|r| r.sequential))
    }
}

/// Lowers the reachable part of a program made of these compilation units, whose entry point is
/// the `main` of the class with qualified name `main`. An error comes with the index of the unit
/// it is in.
pub fn lower_program<'t>(
    units: &[&'t Value],
    reach: &'t Reach,
    main: &str,
) -> Result<Class, (Option<usize>, Error)> {
    super::real_generics::retry(|real| lower_with(units, reach, main, real))
}

/// `lower_program` with the Rust generics `real`, or with the candidates when it is `None`.
fn lower_with<'t>(
    units: &[&'t Value],
    reach: &'t Reach,
    main: &str,
    real: Option<&super::real_generics::Real>,
) -> Result<(Class, super::real_generics::Real, super::real_generics::Real), (Option<usize>, Error)> {
    let mut lowering = Lowering::empty();
    lowering.reach = Some(reach);
    // The unit and the package of each class, in `Class::types` order.
    let mut unit_of = Vec::new();
    let mut packages = Vec::new();
    for (index, unit) in units.iter().enumerate() {
        let unit = Node(unit);
        let package = unit.get("packageName").map(dotted).unwrap_or_default();
        for decl in unit.list("typeDecls") {
            if reach.classes.contains(&crate::program::class_name(decl.0)) {
                let outer = (!package.is_empty()).then_some(package.as_str());
                lowering.declare_type(decl, outer).map_err(|e| (Some(index), e))?;
            }
        }
        unit_of.resize(lowering.types.len(), index);
        packages.resize(lowering.types.len(), package);
    }
    // The standard exception classes that reachable code uses; they belong to no unit.
    let mut reachable = Vec::new();
    for unit in units {
        for decl in Node(unit).list("typeDecls") {
            reachable_members(decl, reach, &mut reachable);
        }
    }
    lowering.standard = super::exceptions::Standard::select(reachable);
    lowering.declare_standard().map_err(|e| (None, e))?;
    unit_of.resize(lowering.types.len(), usize::MAX);
    packages.resize(lowering.types.len(), String::new());
    lowering.entry = lowering.type_index(main);
    let unit = |t: usize| (unit_of[t] != usize::MAX).then_some(unit_of[t]);
    let decls: Vec<Node> = units
        .iter()
        .flat_map(|u| Node(u).list("typeDecls"))
        .filter(|d| reach.classes.contains(&crate::program::class_name(d.0)))
        .collect();
    lowering.real = real.cloned().unwrap_or_else(|| lowering.candidates());
    lowering.plan_object(&decls);
    let mut types = Vec::new();
    for (t, package) in packages.into_iter().enumerate() {
        let mut decl = lowering.type_decl(t).map_err(|e| (unit(t), e))?;
        decl.package = package;
        types.push(decl);
    }
    let mut methods: Vec<Method> = (0..lowering.methods.len())
        .map(|m| lowering.method(m).map_err(|e| (unit(lowering.methods[m].owner), e)))
        .collect::<Result<_, _>>()?;
    lowering.object_methods(&mut methods).map_err(|e| (None, e))?;
    lowering.finish_types(&mut types, &mut methods).map_err(|(t, e)| (unit(t), e))?;
    lowering.object_types(&mut types);
    lowering.param_decls(&mut types);
    let failed = lowering.verify(&types, &methods);
    let hierarchy = Hierarchy::new(&types, &methods);
    let raised = lowering.raised_objects(&types, &methods);
    let closes = lowering.closes.take();
    let class = Class { name: main.to_string(), types, methods, hierarchy, raised, closes };
    enums::check_nulls(&class).map_err(|e| (None, e))?;
    Ok((class, lowering.real.clone(), failed))
}

/// The members of a class declaration (and of its nested classes) that are reachable.
fn reachable_members<'t>(decl: Node<'t>, reach: &Reach, out: &mut Vec<&'t Value>) {
    let class = crate::program::class_name(decl.0);
    if !reach.classes.contains(&class) {
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
