//! Lowering of a whole program: the reachable classes of every compilation unit, each named by
//! its qualified name (`com.example.Point`, as javac writes it in symbols and types) and tagged
//! with its package.

use super::*;
use crate::program::Reach;

impl<'t> Lowering<'t> {
    /// Whether a member of the class `class` is lowered: in a whole program only reachable
    /// members are, in a single file all of them.
    pub(super) fn keeps(&self, member: Node<'t>, class: &str) -> bool {
        self.reach.is_none_or(|reach| reach.member(class, member.0))
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
    lowering.entry = lowering.type_index(main);
    let decls: Vec<Node> = units
        .iter()
        .flat_map(|u| Node(u).list("typeDecls"))
        .filter(|d| reach.classes.contains(&crate::program::class_name(d.0)))
        .collect();
    lowering.real = real.cloned().unwrap_or_else(|| lowering.candidates());
    lowering.plan_object(&decls);
    let mut types = Vec::new();
    for (t, package) in packages.into_iter().enumerate() {
        let mut decl = lowering.type_decl(t).map_err(|e| (Some(unit_of[t]), e))?;
        decl.package = package;
        types.push(decl);
    }
    let mut methods: Vec<Method> = (0..lowering.methods.len())
        .map(|m| lowering.method(m).map_err(|e| (Some(unit_of[lowering.methods[m].owner]), e)))
        .collect::<Result<_, _>>()?;
    lowering.object_methods(&mut methods).map_err(|e| (None, e))?;
    lowering.finish_types(&mut types, &mut methods).map_err(|(t, e)| (Some(unit_of[t]), e))?;
    lowering.object_types(&mut types);
    lowering.param_decls(&mut types);
    let failed = lowering.verify(&types, &methods);
    let hierarchy = Hierarchy::new(&types, &methods);
    let class = Class { name: main.to_string(), types, methods, hierarchy };
    enums::check_nulls(&class).map_err(|e| (None, e))?;
    Ok((class, lowering.real.clone(), failed))
}

/// A package name such as `com.example` from its tree, a chain of selects.
fn dotted(node: Node) -> String {
    match node.get("expression") {
        Some(outer) => format!("{}.{}", dotted(outer), node.str("identifier")),
        None => node.str("name").to_string(),
    }
}
