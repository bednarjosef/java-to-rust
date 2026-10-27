//! Reflection's generic types: `c.getGenericSuperclass()`, `c.getGenericInterfaces()`,
//! `c.getTypeParameters()`, a type variable's bounds and `f.getGenericType()`. A translation has
//! no class files and no `Signature` attributes, so each is made from the types javac gives the
//! declarations, as the JDK's parser makes them from the signature: the JDK's own
//! `ParameterizedTypeImpl`, `WildcardTypeImpl`, `GenericArrayTypeImpl` and `TypeVariableImpl`,
//! through the translator's `TranslatedTypes` (`java/overlay/sun/reflect/generics/reflectiveObjects/`),
//! so they print, compare and hash as in Java.
//!
//! `getGenericSuperclass` is a chain, at the call, of tests of `c` against the classes whose
//! superclass javac gives with type arguments, each making that type. Any other class answers
//! its superclass, which is `null` for `Object`, an interface and a primitive type, as in Java.
//! The classes tested are those `c` can be: for `o.getClass()`, the classes that can be made
//! (not abstract) and are of `o`'s type; otherwise every class of the program. An enum's
//! superclass, `Enum<E>`, is made like any other.
//!
//! `getGenericInterfaces` and `getTypeParameters` call `TranslatedTypes`, whose natives are each
//! one chain over the classes of the program: `genericInterfaces0` over those that implement an
//! interface with type arguments (an interface's are those it extends), `typeParameters0` over
//! those that declare type variables, and `bounds0` over those whose type variables have bounds.
//! Any other class answers its interfaces, no type variables, and `Object` as a variable's
//! bound, as in Java. A variable's bounds are made when they are first asked for, as the JDK's
//! are: a bound may name the variable itself (`E extends Enum<E>`). A field's generic type is a
//! case of the field's slot in `genericTypePart` (`lower::reflection`), for each field of the
//! reflection table whose type has type arguments; the others answer their class.
//!
//! In a type, a class is the class (`String`, `int[]`, `Map.Entry`); a member class's owner is
//! its declaring class, or the parameterized type javac writes before it
//! (`Outer<String>.Inner`); an array of a parameterized type or of a type variable is a
//! `GenericArrayType`; a wildcard without an upper bound has `Object`. A type variable is that of
//! the class that declares it, the class whose declaration the type is in or one around it. A
//! method's type variable is not made (its `GenericDeclaration` would be a `Method`): a type
//! that mentions one throws `UnsupportedOperationException`, which says so, when it is asked
//! for, as most classes a chain tests are never the ones a call gets. So does a type that this
//! does not make (a captured type, an intersection, a class the program leaves out) where it is
//! a field's or in a native; at a `getGenericSuperclass` call, such a superclass among the
//! classes tested is rejected with the call. As with `getAnnotation`, each call makes new
//! objects, where Java's class keeps the ones it made, so two calls give types that are `equals`
//! but not `==`.

use super::classes::{is_get_class, is_literal};
use super::*;

const TYPE: &str = "java.lang.reflect.Type";

/// The translator's class that makes the generic types (`program::GENERIC_TYPES`).
pub(super) const TRANSLATED_TYPES: &str = "sun.reflect.generics.reflectiveObjects.TranslatedTypes";

/// The indexes of the methods of `GENERIC_TYPES`.
const PARAMETERIZED: usize = 0;
const WILDCARD: usize = 1;
const ARRAY: usize = 2;
const VARIABLE: usize = 3;
const UNSUPPORTED: usize = 4;
const TYPE_PARAMETERS: usize = 5;
const GENERIC_INTERFACES: usize = 6;

/// The type variables of the runtime's generic classes (`program::RUNTIME_CLASSES`), which the
/// program does not declare, with their bounds as javac writes them.
const RUNTIME_TYPE_PARAMETERS: [(&str, &str, &[&str]); 3] = [
    ("java.lang.Class", "T", &[]),
    ("java.lang.Comparable", "T", &[]),
    ("java.lang.Enum", "E", &["java.lang.Enum<E>"]),
];

/// The class whose declaration a type is in, which declares the type variables it can name.
#[derive(Clone, Copy)]
enum Context {
    /// A class of the program.
    Class(usize),
    /// A class of the runtime's, of `RUNTIME_TYPE_PARAMETERS`.
    Runtime(&'static str),
}

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
    /// The methods of `GENERIC_TYPES`, which a whole program has when it uses one, and the type
    /// `java.lang.reflect.Type`.
    fn generic_methods(&self, node: Node<'t>, what: &str) -> Result<([usize; 7], Type), Error> {
        let found = crate::program::GENERIC_TYPES.map(|(c, m, s)| self.template_method(c, m, s));
        if found.iter().any(Option::is_none) {
            return unsupported(node, &format!("{what} outside a whole program"));
        }
        let Some(type_ty) = self.type_index(TYPE).map(|t| self.self_type(t)) else {
            return unsupported(node, &format!("{what} without `java.lang.reflect.Type`"));
        };
        Ok((found.map(|m| m.expect("found")), type_ty))
    }

    /// `receiver.getGenericSuperclass()` (see the module's comment).
    pub(super) fn generic_superclass(&self, node: Node<'t>, receiver: Node<'t>) -> Result<ExprKind, Error> {
        let (_, type_ty) = self.generic_methods(node, "`getGenericSuperclass`")?;
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
            let made = self.generic_type(node, &generic, &superclass, Context::Class(t), type_ty)?;
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

    /// `receiver.getGenericInterfaces()` or `receiver.getTypeParameters()` (`name`): a call of
    /// `TranslatedTypes`'s method (see the module's comment).
    pub(super) fn generic_declarations(
        &self,
        node: Node<'t>,
        name: &str,
        receiver: Node<'t>,
    ) -> Result<ExprKind, Error> {
        let (methods, _) = self.generic_methods(node, &format!("`Class.{name}`"))?;
        let (method, made) = match name {
            "getGenericInterfaces" => (methods[GENERIC_INTERFACES], "genericInterfaces"),
            _ => (methods[TYPE_PARAMETERS], "typeParameters"),
        };
        let class = self.convert(self.expr(receiver)?, Type::JavaClass);
        let ret = self.return_type(node, method)?;
        let call = ExprKind::Call { name: made.to_string(), args: vec![class], resolved: Some(method) };
        Ok(self.synth(call, node.pos(), ret).kind)
    }

    /// A native of `TranslatedTypes`, with its arguments: `typeParameters0(c)`,
    /// `genericInterfaces0(c)` or `bounds0(c, name)`, each a chain over the classes of the
    /// program that declare what it asks for (see the module's comment), `null` for any other.
    pub(super) fn generic_types_native(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let what = format!("`{TRANSLATED_TYPES}.{}`", symbol.name);
        let (_, type_ty) = self.generic_methods(node, &what)?;
        let ret = self.ty(node, symbol.ty.rsplit(')').next().unwrap_or_default())?;
        let names: Vec<String> = (0..args.len()).map(|i| self.fresh_name(&format!("generic_{i}"))).collect();
        let types: Vec<Type> = args.iter().map(|a| a.ty).collect();
        let var = |i: usize| self.synth(ExprKind::Var(names[i].clone()), pos, types[i]);
        let array = |items: Vec<Expr>| {
            let items = items.into_iter().map(|i| self.convert(i, ret.element())).collect();
            self.synth(ExprKind::ArrayLit(items), pos, ret)
        };
        // `switch (name)` over the variables `cases` gives the bounds of.
        let by_name = |mut cases: Vec<Case>| {
            let null = self.synth(ExprKind::Null, pos, ret);
            cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Yield(null)] });
            self.synth(ExprKind::Switch { selector: Box::new(var(1)), cases }, pos, ret)
        };
        let bounds_case = |name: &str, bounds: Vec<Expr>| {
            let label = self.synth(ExprKind::Str(name.to_string()), pos, Type::String);
            Case { labels: vec![CaseLabel::Value(label)], body: vec![Stmt::Yield(array(bounds))] }
        };
        let why = "the bound of a type variable,";
        let mut found = Vec::new();
        for t in 0..self.types.len() {
            let decl = self.types[t].node;
            if !matches!(decl.kind(), "CLASS" | "INTERFACE" | "ENUM" | "RECORD") {
                continue;
            }
            let made = match symbol.name {
                "typeParameters0" => {
                    let variables: Vec<Expr> = decl
                        .list("typeParameters")
                        .map(|p| self.declared_variable(node, self.class_literal_of(node, t)?, p.str("name")))
                        .collect::<Result<_, _>>()?;
                    if variables.is_empty() {
                        continue;
                    }
                    array(variables)
                }
                "genericInterfaces0" => {
                    let clauses: Vec<Node<'t>> = decl.list("implementsClause").collect();
                    if !clauses.iter().any(|c| c.java_type().contains('<')) {
                        continue;
                    }
                    let why = "`Class.getGenericInterfaces` of a class that implements";
                    array(
                        clauses
                            .iter()
                            .map(|c| {
                                self.made_or_thrown(node, c.java_type(), Context::Class(t), type_ty, why)
                            })
                            .collect(),
                    )
                }
                "bounds0" => {
                    let mut cases = Vec::new();
                    for variable in decl.list("typeParameters") {
                        let bounds: Vec<Node<'t>> = variable.list("bounds").collect();
                        if bounds.is_empty() {
                            continue;
                        }
                        let made = bounds
                            .iter()
                            .map(|b| {
                                self.made_or_thrown(node, b.java_type(), Context::Class(t), type_ty, why)
                            })
                            .collect();
                        cases.push(bounds_case(variable.str("name"), made));
                    }
                    if cases.is_empty() {
                        continue;
                    }
                    by_name(cases)
                }
                _ => return unsupported(node, &what),
            };
            found.push((self.class_literal_of(node, t)?, made));
        }
        // The runtime's generic classes, which the program does not declare.
        for (class, variable, bounds) in RUNTIME_TYPE_PARAMETERS {
            let literal = || self.class_of_name(node, class, class);
            let made = match symbol.name {
                "typeParameters0" => array(vec![self.declared_variable(node, literal()?, variable)?]),
                "bounds0" if !bounds.is_empty() => {
                    let made = bounds
                        .iter()
                        .map(|b| self.made_or_thrown(node, b, Context::Runtime(class), type_ty, why))
                        .collect();
                    by_name(vec![bounds_case(variable, made)])
                }
                _ => continue,
            };
            found.push((literal()?, made));
        }
        let mut chain = self.synth(ExprKind::Null, pos, ret);
        for (literal, made) in found.into_iter().rev() {
            let test = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![var(0), literal]);
            let test = self.synth(test, pos, Type::Boolean);
            chain =
                self.synth(ExprKind::Conditional(Box::new(test), Box::new(made), Box::new(chain)), pos, ret);
        }
        let mut body: Vec<Stmt> = args
            .into_iter()
            .zip(&names)
            .map(|(a, name)| Stmt::Local { ty: a.ty, name: name.clone(), init: Some(a), pos })
            .collect();
        body.push(Stmt::Yield(chain));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// The generic type of a field of class `class` whose type javac gives as `text`, for
    /// `Field.getGenericType()` (`genericTypePart` in `java/Reflection.java`): `None` for a type
    /// without type arguments, which is the field's class.
    pub(super) fn field_generic_type(
        &self,
        node: Node<'t>,
        text: &str,
        class: usize,
    ) -> Result<Option<Expr>, Error> {
        if !text.contains('<') {
            return Ok(None);
        }
        let (_, type_ty) = self.generic_methods(node, "`Field.getGenericType`")?;
        Ok(Some(self.made_or_thrown(
            node,
            text,
            Context::Class(class),
            type_ty,
            "`Field.getGenericType` of a field of type",
        )))
    }

    /// The type that javac gives as `text` in the declaration of class `context`, or, where this
    /// does not make it, a call that throws `UnsupportedOperationException` with `why` and the
    /// type.
    fn made_or_thrown(&self, node: Node<'t>, text: &str, context: Context, type_ty: Type, why: &str) -> Expr {
        parse(text)
            .and_then(|generic| self.generic_type(node, &generic, text, context, type_ty).ok())
            .unwrap_or_else(|| self.thrown_type(node, &format!("{why} `{text}`"), type_ty))
    }

    /// `TranslatedTypes.unsupported(why)`, which throws.
    fn thrown_type(&self, node: Node<'t>, why: &str, type_ty: Type) -> Expr {
        let pos = node.pos();
        let unsupported_type = crate::program::GENERIC_TYPES[UNSUPPORTED];
        let method = self
            .template_method(unsupported_type.0, unsupported_type.1, unsupported_type.2)
            .expect("the generic types' methods are reached");
        let why = self.synth(ExprKind::Str(why.to_string()), pos, Type::String);
        let call =
            ExprKind::Call { name: "unsupported".to_string(), args: vec![why], resolved: Some(method) };
        self.synth(call, pos, type_ty)
    }

    /// The type variable `name` that the class of `class`, a class literal, declares.
    fn declared_variable(&self, node: Node<'t>, class: Expr, name: &str) -> Result<Expr, Error> {
        let pos = node.pos();
        let (methods, _) = self.generic_methods(node, "a type variable")?;
        let ret = self.return_type(node, methods[VARIABLE])?;
        let name = self.synth(ExprKind::Str(name.to_string()), pos, Type::String);
        let args = vec![class, name];
        let call =
            ExprKind::Call { name: "typeVariable".to_string(), args, resolved: Some(methods[VARIABLE]) };
        Ok(self.synth(call, pos, ret))
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
    /// `whole` that javac gives in the declaration of class `context`.
    fn generic_type(
        &self,
        node: Node<'t>,
        generic: &Generic,
        whole: &str,
        context: Context,
        type_ty: Type,
    ) -> Result<Expr, Error> {
        let pos = node.pos();
        let (methods, _) = self.generic_methods(node, "a generic type")?;
        let types = self.param_types(methods[WILDCARD])?[0];
        let call = |name: &str, method: usize, args: Vec<Expr>| {
            let call = ExprKind::Call { name: name.to_string(), args, resolved: Some(methods[method]) };
            self.synth(call, pos, type_ty)
        };
        let list = |items: Vec<Expr>| {
            let items = items.into_iter().map(|i| self.convert(i, types.element())).collect();
            self.synth(ExprKind::ArrayLit(items), pos, types)
        };
        let part = |generic: &Generic| self.generic_type(node, generic, whole, context, type_ty);
        match generic {
            Generic::Class { name, args, owner }
                if args.is_empty() && owner.is_none() && self.is_variable(name) =>
            {
                let declaring = match context {
                    Context::Class(c) => match self.type_var_around(name, c, None) {
                        Some((_, super::real_generics::Owner::Class(t))) => {
                            Some(self.class_literal_of(node, t)?)
                        }
                        _ => None,
                    },
                    Context::Runtime(class) => RUNTIME_TYPE_PARAMETERS
                        .iter()
                        .any(|(c, variable, _)| *c == class && variable == name)
                        .then(|| self.class_of_name(node, class, whole))
                        .transpose()?,
                };
                match declaring {
                    Some(class) => Ok(self.convert(self.declared_variable(node, class, name)?, type_ty)),
                    _ => Ok(self.thrown_type(
                        node,
                        &format!("the type variable `{name}` of a method, in `{whole}`,"),
                        type_ty,
                    )),
                }
            }
            Generic::Class { name, args, owner } => {
                let raw = self.class_of_name(node, name, whole)?;
                if args.is_empty() && owner.is_none() {
                    return Ok(self.convert(raw, type_ty));
                }
                let owner = match owner {
                    Some(owner) => part(owner)?,
                    None => self.declaring_class(node, name, whole, type_ty)?,
                };
                let args = args.iter().map(part).collect::<Result<_, _>>()?;
                Ok(call("parameterized", PARAMETERIZED, vec![raw, list(args), owner]))
            }
            // An array of a class is a class, as `Reifier` makes it.
            Generic::Array(component) if self.is_class(component) => {
                let text = array_text(component).expect("a class's array has a name");
                Ok(self.convert(self.class_of_name(node, &text, whole)?, type_ty))
            }
            Generic::Array(component) => Ok(call("array", ARRAY, vec![part(component)?])),
            Generic::Wildcard { upper, lower } => {
                let upper = match upper {
                    Some(upper) => part(upper)?,
                    None => self.convert(self.class_of_name(node, "java.lang.Object", whole)?, type_ty),
                };
                let lower = match lower {
                    Some(lower) => vec![part(lower)?],
                    None => Vec::new(),
                };
                Ok(call("wildcard", WILDCARD, vec![list(vec![upper]), list(lower)]))
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
