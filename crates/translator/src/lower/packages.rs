//! Packages by name and what a package knows of itself, in a translation with the JDK's sources
//! (`c.getPackage()` is `TranslatedModules.packageOf`, see `lower/modules.rs` and
//! `java/patches/java/lang/Package.java`).
//!
//! Java knows a package by name only once a class of it has been loaded, and the JVM loads
//! classes when it first needs them, its own at startup among them: `Package.getPackage
//! ("javax.swing")` is null until a Swing class is loaded. A deprecated `Package.getPackage(name)`
//! is translated only where the answer is certain: the name is a constant, and it is the package
//! of the class whose code calls it, of one of that class's supertypes (loaded before it), or
//! `java.lang`. The call is then `TranslatedModules.packageOf` of that class. Any other name, and
//! `Package.getPackages()` and the package lookups of `ClassLoader`, which list or find the
//! packages loaded so far, are rejected.
//!
//! The manifest attributes of a package of the unnamed module (titles, versions, vendors) are
//! those of the manifest of its root of the class path, which the translator reads
//! (`program::modules::package_attributes`, `Module.manifest0`). A package's annotations, which
//! the translation does not have, are rejected here.

use super::*;

const PACKAGE: &str = "java.lang.Package";

/// `TranslatedModules.packageOf`, which `c.getPackage()` calls (`program::MODULE_CALLS`).
const PACKAGE_OF: (&str, &str) = ("packageOf", "(java.lang.Class<?>)java.lang.Package");

/// The methods of `Package` that give its annotations, those of its `package-info` class.
const ANNOTATION_METHODS: &[&str] = &[
    "getAnnotation",
    "isAnnotationPresent",
    "getAnnotationsByType",
    "getAnnotations",
    "getDeclaredAnnotation",
    "getDeclaredAnnotationsByType",
    "getDeclaredAnnotations",
];

/// What the rejected lookups depend on.
const LOADED_SO_FAR: &str = "which depends on the classes the JVM has loaded so far";

impl<'t> Lowering<'t> {
    /// A call of `Package.getPackage(name)`, as described in the module's comment, or a call that
    /// the module rejects; `None` for any other call. It runs before the arguments are lowered.
    pub(super) fn package_call(&self, node: Node<'t>, symbol: &Symbol) -> Result<Option<ExprKind>, Error> {
        match (symbol.owner, symbol.name) {
            (PACKAGE, "getPackage") if symbol.is_static => self.package_by_name(node),
            (PACKAGE, "getPackages") if symbol.is_static => {
                unsupported(node, &format!("`Package.getPackages`, {LOADED_SO_FAR}"))
            }
            (
                "java.lang.ClassLoader",
                name @ ("getDefinedPackage" | "getDefinedPackages" | "getPackage" | "getPackages"),
            ) => unsupported(node, &format!("`ClassLoader.{name}`, {LOADED_SO_FAR}")),
            (PACKAGE, name) if ANNOTATION_METHODS.contains(&name) => {
                unsupported(node, &format!("the annotations of a package (`Package.{name}`)"))
            }
            _ => Ok(None),
        }
    }

    /// `Package.getPackage(name)` of a constant name of a package that the caller has loaded:
    /// `TranslatedModules.packageOf` of a class of it; any other name is rejected. `None` in a
    /// translation without the JDK's sources.
    fn package_by_name(&self, node: Node<'t>) -> Result<Option<ExprKind>, Error> {
        let Some(resolved) = self.translated_modules(PACKAGE_OF.0, PACKAGE_OF.1) else { return Ok(None) };
        let name = self.constant_text(node.list("arguments").next());
        let Some(class) = name.map(|n| self.loaded_class_of(node, &n)).transpose()?.flatten() else {
            return unsupported(
                node,
                &format!("`Package.getPackage` of a package that the caller has not loaded, {LOADED_SO_FAR}"),
            );
        };
        let literal = self.synth(
            ExprKind::Builtin(Builtin::ClassLiteral(class), Vec::new()),
            node.pos(),
            Type::JavaClass,
        );
        let args = self.convert_generic_args(node, Some(resolved), vec![literal])?;
        Ok(Some(ExprKind::Call { name: PACKAGE_OF.0.to_string(), args, resolved: Some(resolved) }))
    }

    /// A class of package `name` that is loaded wherever the code being lowered runs: the class
    /// that calls (for a lambda's code, the class that holds it) or one of its supertypes, which
    /// are loaded before it, or `String` for `java.lang`.
    fn loaded_class_of(&self, node: Node<'t>, name: &str) -> Result<Option<Type>, Error> {
        if name == "java.lang" {
            return self.ty(node, "java.lang.String").map(Some);
        }
        let caller = self.lambda_holder(self.current.get());
        let found = self.supertypes_of(caller)?.into_iter().find(|&t| {
            let package = self.package_of(t);
            package.strip_suffix('.').unwrap_or(&package) == name
        });
        Ok(found.map(|t| self.self_type(t)))
    }
}
