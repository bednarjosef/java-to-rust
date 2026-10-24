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
//! The manifest attributes of a package of the unnamed module (titles, versions, vendors,
//! sealing, `isCompatibleWith` and `toString`) are those of the jar it is run from, which the
//! translation does not know, and they throw where they are asked for. Where the package is
//! known to be one of the unnamed module, because it is that of a class literal or of the
//! `getClass()` of a class of it, or a constant name of `Package.getPackage`, directly or through
//! a local variable that holds nothing else, they are rejected here instead; so are a package's
//! annotations, which always throw.

use super::*;

const PACKAGE: &str = "java.lang.Package";

/// `TranslatedModules.packageOf`, which `c.getPackage()` calls (`program::MODULE_CALLS`).
const PACKAGE_OF: (&str, &str) = ("packageOf", "(java.lang.Class<?>)java.lang.Package");

/// The methods of `Package` that give the attributes of the manifest of its jar.
const MANIFEST_METHODS: &[&str] = &[
    "getSpecificationTitle",
    "getSpecificationVersion",
    "getSpecificationVendor",
    "getImplementationTitle",
    "getImplementationVersion",
    "getImplementationVendor",
    "isSealed",
    "isCompatibleWith",
    "toString",
];

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
    pub(super) fn package_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        select: Node<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
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
            (PACKAGE, name)
                if MANIFEST_METHODS.contains(&name)
                    && receiver
                        .and_then(|r| self.known_package(r))
                        .is_some_and(|p| !self.in_named_module(&p)) =>
            {
                unsupported(
                    node,
                    &format!("the manifest attributes of a package of the class path (`Package.{name}`)"),
                )
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

    /// The name of the package that `receiver`, a `Package` value, is known to be: that of
    /// `c.getPackage()` where `c` is a class literal, or a `Class<X>` or the `getClass()` of a
    /// value of a class `X`, a class of the translation; or the constant name of
    /// `Package.getPackage`; or a local variable that only ever holds one of those.
    fn known_package(&self, receiver: Node<'t>) -> Option<String> {
        match receiver.kind() {
            "PARENTHESIZED" => return self.known_package(receiver.get("expression")?),
            "IDENTIFIER" if receiver.symbol()?.kind == "LOCAL_VARIABLE" => {
                return self.known_package(self.only_value(receiver.str("name"))?);
            }
            _ => {}
        }
        if receiver.kind() != "METHOD_INVOCATION" {
            return None;
        }
        let symbol = receiver.symbol()?;
        if (symbol.owner, symbol.name, symbol.ty)
            == (PACKAGE, "getPackage", "(java.lang.String)java.lang.Package")
        {
            return self.constant_text(receiver.list("arguments").next());
        }
        if (symbol.owner, symbol.name, symbol.ty) != ("java.lang.Class", "getPackage", "()java.lang.Package")
        {
            return None;
        }
        let select = receiver.get("methodSelect")?;
        let class = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT")?;
        let argument = class.java_type().strip_prefix("java.lang.Class<")?.strip_suffix('>')?;
        // `Class<? extends X>`, as `getClass()` gives it, is of `X` or a subclass: of the same
        // module where `X` is a class of the unnamed one, which no named module reads. (An
        // interface's could be a proxy's, of a module of its own.)
        let argument = argument.split_once(" of ").map_or(argument, |(_, a)| a);
        let (bound, exact) = match argument.strip_prefix("? extends ") {
            Some(bound) => (bound, false),
            None => (argument, true),
        };
        let t = self.type_index(&super::captures::raw(bound))?;
        if !exact && !matches!(self.types[t].node.kind(), "CLASS" | "ENUM" | "RECORD") {
            return None;
        }
        let package = self.package_of(t);
        Some(package.strip_suffix('.').unwrap_or(&package).to_string())
    }

    /// The initializer of the local variable `name` of the method being lowered, when the method
    /// declares one variable of that name and never assigns it.
    fn only_value(&self, name: &str) -> Option<Node<'t>> {
        fn visit<'t>(v: &'t Value, name: &str, declared: &mut Vec<&'t Value>, assigned: &mut bool) {
            match v {
                Value::Array(items) => items.iter().for_each(|i| visit(i, name, declared, assigned)),
                Value::Object(map) => {
                    let kind = map.get("kind").and_then(Value::as_str).unwrap_or_default();
                    if kind == "VARIABLE" && v["name"] == name {
                        declared.push(v);
                    }
                    if kind.ends_with("ASSIGNMENT") && v["variable"]["name"] == name {
                        *assigned = true;
                    }
                    map.values().for_each(|child| visit(child, name, declared, assigned));
                }
                _ => {}
            }
        }
        let body = self.methods[self.method_ctx.get()?].node.get("body")?;
        let (mut declared, mut assigned) = (Vec::new(), false);
        visit(body.0, name, &mut declared, &mut assigned);
        match declared[..] {
            [variable] if !assigned => Node(variable).get("initializer"),
            _ => None,
        }
    }

    /// Whether package `name` is in a named module of the translation, as `Module.of` finds it.
    fn in_named_module(&self, name: &str) -> bool {
        self.reach.is_none_or(|reach| {
            reach.modules.iter().any(|m| !m.name.is_empty() && m.packages.iter().any(|p| p == name))
        })
    }
}
