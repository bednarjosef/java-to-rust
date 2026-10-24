//! `c.getGenericSuperclass()`. A translation has no class files and no `Signature` attributes,
//! so the call is a chain of tests of `c` against the classes whose superclass javac gives with
//! type arguments, each making that type as the JDK's parser makes it from the signature: the
//! JDK's own `ParameterizedTypeImpl`, `WildcardTypeImpl` and `GenericArrayTypeImpl`, through the
//! translator's `TranslatedTypes` (`java/overlay/sun/reflect/generics/reflectiveObjects/`), so
//! they print, compare and hash as in Java. Any other class answers its superclass, which is
//! `null` for `Object`, an interface and a primitive type, as in Java.
//!
//! The classes tested are those `c` can be: for `o.getClass()`, the classes that can be made
//! (not abstract) and are of `o`'s type; otherwise every class of the program. A type that is a
//! class (`String`, `int[]`, `Map.Entry`) is the class; a member class's owner is its declaring
//! class, or the parameterized type javac writes before it (`Outer<String>.Inner`); an array of
//! a parameterized type is a `GenericArrayType`; a wildcard without an upper bound has `Object`.
//!
//! A type variable is not supported yet (its `TypeVariable` would need the class or method that
//! declares it as a `GenericDeclaration`): making a type that mentions one throws
//! `UnsupportedOperationException`, which says so, when it runs, as most classes a call can test
//! (`ArrayList`, which extends `AbstractList<E>`) are never the ones it gets. Rejected, with the
//! call: a superclass that mentions a captured type or an intersection, among the classes tested.
//! An enum's superclass, `Enum<E>`, is made like any other. As with `getAnnotation`, each call makes new objects, where Java's class keeps the
//! ones it made, so two calls give types that are `equals` but not `==`.

use super::classes::{is_get_class, is_literal};
use super::*;

const TYPE: &str = "java.lang.reflect.Type";

/// A type as javac writes it (`java.util.Map<java.lang.String,java.util.List<? extends T>>`).
enum Generic {
    /// A class with its type arguments, and the parameterized type it is a member of where javac
    /// writes one (`Outer<T>.Inner`).
    Class {
        name: String,
        args: Vec<Generic>,
        owner: Option<Box<Generic>>,
    },
    Array(Box<Generic>),
    /// `?`, `? extends upper` or `? super lower`.
    Wildcard {
        upper: Option<Box<Generic>>,
        lower: Option<Box<Generic>>,
    },
}

/// `text` as a `Generic`; `None` for what this does not make (a captured type, an intersection,
/// an annotated type).
fn parse(text: &str) -> Option<Generic> {
    let text = text.trim();
    if text.contains('@') || text.contains('&') || text.starts_with("capture#") || text.is_empty() {
        return None;
    }
    if text == "?" {
        return Some(Generic::Wildcard { upper: None, lower: None });
    }
    if let Some(bound) = text.strip_prefix("? extends ") {
        return Some(Generic::Wildcard { upper: Some(Box::new(parse(bound)?)), lower: None });
    }
    if let Some(bound) = text.strip_prefix("? super ") {
        return Some(Generic::Wildcard { upper: None, lower: Some(Box::new(parse(bound)?)) });
    }
    if let Some(component) = text.strip_suffix("[]") {
        return Some(Generic::Array(Box::new(parse(component)?)));
    }
    let (mut owner, mut rest, mut prefix) = (None, text, String::new());
    loop {
        let Some(open) = rest.find('<') else {
            return Some(Generic::Class { name: prefix + rest, args: Vec::new(), owner });
        };
        let name = prefix + &rest[..open];
        let mut depth = 0;
        let close = rest[open..].char_indices().find_map(|(i, c)| {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(open + i)
        })?;
        let args =
            generics::split_args(&rest[open + 1..close]).into_iter().map(parse).collect::<Option<_>>()?;
        let current = Generic::Class { name: name.clone(), args, owner };
        let after = &rest[close + 1..];
        if after.is_empty() {
            return Some(current);
        }
        rest = after.strip_prefix('.')?;
        prefix = name + ".";
        owner = Some(Box::new(current));
    }
}

impl<'t> Lowering<'t> {
    /// `receiver.getGenericSuperclass()` (see the module's comment).
    pub(super) fn generic_superclass(&self, node: Node<'t>, receiver: Node<'t>) -> Result<ExprKind, Error> {
        let methods = crate::program::GENERIC_TYPES.map(|(c, m, s)| self.template_method(c, m, s));
        let [Some(_), Some(_), Some(_), Some(_)] = methods else {
            return unsupported(node, "`getGenericSuperclass` outside a whole program");
        };
        let Some(type_ty) = self.type_index(TYPE).map(|t| self.self_type(t)) else {
            return unsupported(node, "`getGenericSuperclass` without `java.lang.reflect.Type`");
        };
        let pos = node.pos();
        let local = self.fresh_name("class");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        // The classes `c` can be, each with the superclass javac gives it.
        let of = self.classes_of(receiver)?;
        let mut found = Vec::new();
        for t in 0..self.types.len() {
            let decl = self.types[t].node;
            if !of
                .as_ref()
                .is_none_or(|(of, made)| self.is_subclass(t, *of) && (!made || !decl.has_flag("ABSTRACT")))
            {
                continue;
            }
            let superclass = match decl.kind() {
                "CLASS" => match decl.get("extendsClause") {
                    Some(clause) if clause.java_type().contains('<') => clause.java_type().to_string(),
                    _ => continue,
                },
                "ENUM" => format!("java.lang.Enum<{}>", self.types[t].javac),
                _ => continue,
            };
            let Some(generic) = parse(&superclass) else {
                return unsupported(
                    node,
                    &format!("`getGenericSuperclass` of a class that extends `{superclass}`"),
                );
            };
            let made = self.generic_type(node, &generic, &superclass, type_ty)?;
            found.push((self.class_literal_of(node, t)?, made));
        }
        let superclass = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::GetSuperclass), vec![var()]);
        let mut chain = self.convert(self.synth(superclass, pos, Type::JavaClass), type_ty);
        for (literal, made) in found.into_iter().rev() {
            let test = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![var(), literal]);
            let test = self.synth(test, pos, Type::Boolean);
            chain = self.synth(
                ExprKind::Conditional(Box::new(test), Box::new(made), Box::new(chain)),
                pos,
                type_ty,
            );
        }
        let class = self.convert(self.expr(receiver)?, Type::JavaClass);
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: local, init: Some(class), pos },
            Stmt::Yield(chain),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// The class of the program that the class `receiver` is of, with whether it is an object's
    /// (`o.getClass()`, which only a class that can be made is); `None` where it can be any.
    fn classes_of(&self, receiver: Node<'t>) -> Result<Option<(usize, bool)>, Error> {
        let named = |n: Node| {
            self.ty(n, n.java_type()).ok().and_then(|ty| match ty {
                Type::Class(name, _) | Type::Enum(name) => self.type_index(name),
                _ => None,
            })
        };
        if is_literal(receiver) {
            return Ok(receiver.get("expression").and_then(named).map(|t| (t, false)));
        }
        if !is_get_class(receiver) {
            return Ok(None);
        }
        let select = receiver.child("methodSelect")?;
        let class = match select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT") {
            Some(object) => named(object),
            None => match self.this(receiver).ty {
                Type::Class(name, _) | Type::Enum(name) => self.type_index(name),
                _ => None,
            },
        };
        Ok(class.map(|t| (t, true)))
    }

    /// The `java.lang.reflect.Type` that the JDK's parser makes of `generic`, a part of the type
    /// `whole` that javac gives.
    fn generic_type(
        &self,
        node: Node<'t>,
        generic: &Generic,
        whole: &str,
        type_ty: Type,
    ) -> Result<Expr, Error> {
        let pos = node.pos();
        let [parameterized, wildcard, array, variable] = crate::program::GENERIC_TYPES
            .map(|(c, m, s)| self.template_method(c, m, s).expect("the generic types' methods are reached"));
        let types = self.param_types(wildcard)?[0];
        let call = |name: &str, method: usize, args: Vec<Expr>| {
            let call = ExprKind::Call { name: name.to_string(), args, resolved: Some(method) };
            self.synth(call, pos, type_ty)
        };
        let list = |items: Vec<Expr>| {
            let items = items.into_iter().map(|i| self.convert(i, types.element())).collect();
            self.synth(ExprKind::ArrayLit(items), pos, types)
        };
        match generic {
            Generic::Class { name, args, owner }
                if args.is_empty() && owner.is_none() && self.is_variable(name) =>
            {
                let name = self.synth(ExprKind::Str(name.clone()), pos, Type::String);
                Ok(call("typeVariable", variable, vec![name]))
            }
            Generic::Class { name, args, owner } => {
                let raw = self.class_of_name(node, name, whole)?;
                if args.is_empty() && owner.is_none() {
                    return Ok(self.convert(raw, type_ty));
                }
                let owner = match owner {
                    Some(owner) => self.generic_type(node, owner, whole, type_ty)?,
                    None => self.declaring_class(node, name, whole, type_ty)?,
                };
                let args = args
                    .iter()
                    .map(|a| self.generic_type(node, a, whole, type_ty))
                    .collect::<Result<_, _>>()?;
                Ok(call("parameterized", parameterized, vec![raw, list(args), owner]))
            }
            // An array of a class is a class, as `Reifier` makes it.
            Generic::Array(component) if self.is_class(component) => {
                let text = array_text(component).expect("a class's array has a name");
                Ok(self.convert(self.class_of_name(node, &text, whole)?, type_ty))
            }
            Generic::Array(component) => {
                let component = self.generic_type(node, component, whole, type_ty)?;
                Ok(call("array", array, vec![component]))
            }
            Generic::Wildcard { upper, lower } => {
                let upper = match upper {
                    Some(upper) => self.generic_type(node, upper, whole, type_ty)?,
                    None => self.convert(self.class_of_name(node, "java.lang.Object", whole)?, type_ty),
                };
                let lower = match lower {
                    Some(lower) => vec![self.generic_type(node, lower, whole, type_ty)?],
                    None => Vec::new(),
                };
                Ok(call("wildcard", wildcard, vec![list(vec![upper]), list(lower)]))
            }
        }
    }

    /// Whether javac's `name` is a type variable's: not a primitive type's, and with neither a
    /// package nor a class of the program of that name.
    fn is_variable(&self, name: &str) -> bool {
        let primitive =
            matches!(name, "boolean" | "byte" | "char" | "short" | "int" | "long" | "float" | "double");
        !primitive && !name.contains('.') && self.type_index(name).is_none()
    }

    /// Whether `generic` is a class or an array of classes, which the JDK's parser makes a `Class`.
    fn is_class(&self, generic: &Generic) -> bool {
        match generic {
            Generic::Class { name, args, owner } => {
                args.is_empty() && owner.is_none() && !self.is_variable(name)
            }
            Generic::Array(component) => self.is_class(component),
            Generic::Wildcard { .. } => false,
        }
    }

    /// The class literal of the class or array of classes that javac names `name`, in the type
    /// `whole`.
    fn class_of_name(&self, node: Node<'t>, name: &str, whole: &str) -> Result<Expr, Error> {
        if self.is_variable(name.trim_end_matches("[]")) {
            return unsupported(node, &format!("`getGenericSuperclass` of a class that extends `{whole}`"));
        }
        let ty = self.ty(node, name)?;
        Ok(self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), node.pos(), Type::JavaClass))
    }

    /// The owner of the class javac names `name` where javac writes none: the class it is a
    /// member of, as `Class.getDeclaringClass()`, or `null` for a top-level or local class.
    fn declaring_class(&self, node: Node<'t>, name: &str, whole: &str, type_ty: Type) -> Result<Expr, Error> {
        let outer = name.rsplit_once('.').map(|(outer, _)| outer);
        match outer {
            Some(outer) if self.type_index(outer).is_some() => {
                Ok(self.convert(self.class_of_name(node, outer, whole)?, type_ty))
            }
            // A package's name, which by convention does not start with a capital letter.
            Some(outer)
                if outer.rsplit('.').next().is_some_and(|last| last.starts_with(char::is_uppercase)) =>
            {
                unsupported(node, &format!("`getGenericSuperclass` of a class that extends `{whole}`"))
            }
            _ => Ok(self.synth(ExprKind::Null, node.pos(), type_ty)),
        }
    }
}

/// javac's name of the array of `component`, a class or an array of classes.
fn array_text(component: &Generic) -> Option<String> {
    match component {
        Generic::Class { name, .. } => Some(format!("{name}[]")),
        Generic::Array(inner) => Some(format!("{}[]", array_text(inner)?)),
        Generic::Wildcard { .. } => None,
    }
}
