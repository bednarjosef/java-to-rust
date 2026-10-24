//! `c.getAnnotation(A.class)`, the annotations of reflection's members, and the reads of an
//! annotation's members. The translation has no annotations, so the call is a chain of tests of
//! `c` against the classes and annotation types whose declarations have `A` (as
//! `annotation_presence` finds them), each giving the annotation object (`annotation_objects`)
//! made from that declaration's values: `TranslatedAnnotations.cached(key, A.class, defaults,
//! values)`, which builds the map of members as `AnnotationParser` does, so `toString` lists them
//! in Java's order. A class without `A`, and every class when `A` is not kept at run time, gives
//! `null`. The `key` names the declaration and `A`, and `cached` keeps the object it made under
//! it: Java's class or member makes its annotations once, so every call gives the same object,
//! also through a subclass that inherits the annotation or the member.
//!
//! `m.getAnnotation(A.class)` of a `Method`, `Field` or `Constructor` `m` (and
//! `getDeclaredAnnotation`, `isAnnotationPresent`, `getAnnotations` and `getDeclaredAnnotations`)
//! is a `switch` over the slot that `m` holds in the reflection table (`reflection.rs`), with a
//! case for each member of the table that has the annotations asked for: those kept at run time
//! whose `@Target` allows such a member (not a type annotation such as `@Nullable`), in the order
//! of the declaration.
//!
//! A value may be a constant, a class literal, an enum constant, an annotation (kept as the one
//! that has it is, and a default one once for its type, as Java parses a type's defaults once),
//! or an array of any of them but annotations, written as one element where the member is an
//! array; an array that the translation holds as an `Object[]` carries its own class
//! (`covariant::made_as`), which the JDK's `memberValueToString` tells apart. Rejected: a member
//! whose type is an array of annotations or of arrays, a nested annotation type, and the
//! container that a repeated annotation leaves.
//! The call of a member, `a.value()`, is `TranslatedAnnotations.member(a, "value")`, unboxed or
//! cast to the member's type; it copies an array that is not empty, as the JDK's proxy does.

use super::*;
use crate::program::LookupKind;

const RETENTION: &str = "java.lang.annotation.Retention";
const INHERITED: &str = "java.lang.annotation.Inherited";
const ANNOTATION: &str = "java.lang.annotation.Annotation";
const TARGET: &str = "java.lang.annotation.Target";

/// Whether `owner.name` is a method of reflection's members that asks for their annotations
/// (`Lowering::member_annotations`).
pub fn is_member_annotation_call(owner: &str, name: &str) -> bool {
    matches!(
        owner,
        "java.lang.reflect.Method"
            | "java.lang.reflect.Field"
            | "java.lang.reflect.Constructor"
            | "java.lang.reflect.Executable"
            | "java.lang.reflect.AccessibleObject"
    ) && matches!(
        name,
        "getAnnotation"
            | "getDeclaredAnnotation"
            | "isAnnotationPresent"
            | "getAnnotations"
            | "getDeclaredAnnotations"
    )
}

impl<'t> Lowering<'t> {
    /// `receiver.getAnnotation(annotation)` (see the module's comment).
    pub(super) fn annotation_of(
        &self,
        node: Node<'t>,
        receiver: Node<'t>,
        annotation: Node<'t>,
    ) -> Result<ExprKind, Error> {
        let named = annotation.get("expression").filter(|_| super::classes::is_literal(annotation));
        let Some(name) = named.map(|n| n.java_type()) else {
            return unsupported(node, "`getAnnotation` of a class other than a class literal's");
        };
        let Some(&decl) = self.annotation_types.get(name) else {
            return unsupported(
                node,
                &format!("`getAnnotation` of `{name}`, which is not an annotation type"),
            );
        };
        if self.nested_annotation_types.contains(name) {
            return unsupported(node, &format!("`getAnnotation` of the nested annotation type `{name}`"));
        }
        if self.library_annotations.contains(name) {
            return unsupported(node, &format!("`getAnnotation` of the JDK's `{name}`"));
        }
        let (make, member) = (&crate::program::ANNOTATION_VALUES[0], &crate::program::ANNOTATION_VALUES[1]);
        let (Some(make), Some(_)) = (
            self.template_method(make.0, make.1, make.2),
            self.template_method(member.0, member.1, member.2),
        ) else {
            return unsupported(node, "`getAnnotation` outside a whole program");
        };
        let Some(annotation_type) = self.type_index(ANNOTATION).map(|t| self.self_type(t)) else {
            return unsupported(node, "`getAnnotation` without `Annotation`");
        };
        let runtime = self.annotation_values(decl, RETENTION)?.is_some_and(|v| v == ["RUNTIME"]);
        let inherited = self.has_annotation(decl, INHERITED);
        let pos = node.pos();
        let local = self.fresh_name("class");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        let equals = |literal: Expr| {
            let kind = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![var(), literal]);
            self.synth(kind, pos, Type::Boolean)
        };
        let null = || self.synth(ExprKind::Null, pos, annotation_type);
        // The classes with `A`, each with the declaration that has it and that declaration's name.
        let mut found: Vec<(Expr, Node<'t>, String)> = Vec::new();
        if runtime {
            for t in 0..self.types.len() {
                if let Some(holder) = self.holder_of(t, name, inherited)? {
                    let info = &self.types[holder];
                    found.push((self.class_literal_of(node, t)?, info.node, info.javac.clone()));
                }
            }
            let mut annotation_types: Vec<(&String, &Node<'t>)> = self.annotation_types.iter().collect();
            annotation_types.sort_by_key(|(n, _)| n.as_str());
            for (other, &other_decl) in annotation_types {
                if self.nested_annotation_types.contains(other) || !self.directly_present(other_decl, name) {
                    continue;
                }
                self.annotation_classes.borrow_mut().insert(other.clone());
                let literal = ExprKind::Builtin(Builtin::ClassLiteral(Type::class(other)), Vec::new());
                found.push((self.synth(literal, pos, Type::JavaClass), other_decl, other.clone()));
            }
        }
        // `c.equals(null)`, which dereferences `c` as Java's call does, where no class has `A`.
        // The `null` is a local's, which gives it its type.
        let none = self.fresh_name("none");
        let mut tests =
            vec![Stmt::Local { ty: annotation_type, name: none.clone(), init: Some(null()), pos }];
        if found.is_empty() {
            tests.push(Stmt::Expr(equals(self.synth(ExprKind::Null, pos, Type::Null))));
        }
        let mut chain = self.synth(ExprKind::Var(none), pos, annotation_type);
        for (literal, holder, key) in found.into_iter().rev() {
            let at = self.annotation_on(node, holder, name)?;
            let made = self.annotation_object(node, make, &key, name, decl, at, annotation_type)?;
            let test = equals(literal);
            chain = self.synth(
                ExprKind::Conditional(Box::new(test), Box::new(made), Box::new(chain)),
                pos,
                annotation_type,
            );
        }
        let class = self.convert(self.expr(receiver)?, Type::JavaClass);
        let mut body = vec![Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(class), pos }];
        body.extend(tests);
        body.push(Stmt::Yield(chain));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// The class whose declaration gives class `t` of the program the annotation `name`: `t`, or,
    /// for an `inherited` one, its superclass's (an interface inherits none).
    fn holder_of(&self, t: usize, name: &str, inherited: bool) -> Result<Option<usize>, Error> {
        let decl = self.types[t].node;
        if self.directly_present(decl, name) {
            return Ok(Some(t));
        }
        if !inherited || decl.kind() != "CLASS" {
            return Ok(None);
        }
        match self.superclass(t)? {
            Some(s) => self.holder_of(s, name, inherited),
            None => Ok(None),
        }
    }

    /// The annotation `name` that the declaration `holder` has once.
    fn annotation_on(&self, node: Node<'t>, holder: Node<'t>, name: &str) -> Result<Node<'t>, Error> {
        let modifiers = holder.child("modifiers")?;
        let mut found = modifiers
            .list("annotations")
            .filter(|at| at.child("annotationType").is_ok_and(|ty| ty.java_type() == name));
        match (found.next(), found.next()) {
            (Some(at), None) => Ok(at),
            _ => unsupported(
                node,
                &format!("`getAnnotation` of `{name}`, the container of a repeated annotation"),
            ),
        }
    }

    /// `TranslatedAnnotations.cached(key, A.class, defaults, values)` of the annotation `at` of
    /// the type `name`, declared by `decl`, that the declaration named `holder` has.
    #[allow(clippy::too_many_arguments)]
    fn annotation_object(
        &self,
        node: Node<'t>,
        make: usize,
        holder: &str,
        name: &str,
        decl: Node<'t>,
        at: Node<'t>,
        annotation_type: Type,
    ) -> Result<Expr, Error> {
        let pos = at.pos();
        let params = self.param_types(make)?;
        let objects = params[2];
        let text = |s: &str| {
            self.convert(self.synth(ExprKind::Str(s.to_string()), pos, Type::String), objects.element())
        };
        let mut defaults = Vec::new();
        for member in decl.list("members").filter(|m| m.kind() == "METHOD") {
            if let Some(value) = member.get("defaultValue") {
                defaults.push(text(member.str("name")));
                // Java parses a type's defaults once, so its default annotations are shared.
                let key = format!("{name}.{} default", member.str("name"));
                let value =
                    self.member_value(node, make, &key, decl, member.str("name"), value, objects.element())?;
                defaults.push(value);
            }
        }
        let mut values = Vec::new();
        for argument in at.list("arguments") {
            let (member, value) = match argument.kind() {
                "ASSIGNMENT" => (argument.child("variable")?.str("name"), argument.child("expression")?),
                _ => ("value", argument),
            };
            values.push(text(member));
            let key = format!("{holder}@{name}.{member}");
            values.push(self.member_value(node, make, &key, decl, member, value, objects.element())?);
        }
        self.annotation_classes.borrow_mut().insert(name.to_string());
        let literal = ExprKind::Builtin(Builtin::ClassLiteral(Type::class(name)), Vec::new());
        let array = |items: Vec<Expr>| self.synth(ExprKind::ArrayLit(items), pos, objects);
        let key = self.synth(ExprKind::Str(format!("{holder}@{name}")), pos, Type::String);
        let args = vec![key, self.synth(literal, pos, Type::JavaClass), array(defaults), array(values)];
        let call = ExprKind::Call { name: "cached".to_string(), args, resolved: Some(make) };
        Ok(self.synth(call, pos, annotation_type))
    }

    /// The value `value` of the member `member` of the annotation type `decl`, as an `object`;
    /// an annotation is made with `make` and kept under `key` and its type.
    #[allow(clippy::too_many_arguments)]
    fn member_value(
        &self,
        node: Node<'t>,
        make: usize,
        key: &str,
        decl: Node<'t>,
        member: &str,
        value: Node<'t>,
        object: Type,
    ) -> Result<Expr, Error> {
        let Some(method) = decl.list("members").find(|m| m.kind() == "METHOD" && m.str("name") == member)
        else {
            return unsupported(node, &format!("the annotation member `{member}`"));
        };
        let java = method.child("returnType")?.java_type();
        let element = java.strip_suffix("[]").unwrap_or(java);
        // An annotation, made as the one that has it is.
        if let Some(&nested) = self.annotation_types.get(java)
            && value.kind() == "ANNOTATION"
            && !self.nested_annotation_types.contains(java)
            && let Some(annotation) = self.type_index(ANNOTATION).map(|t| self.self_type(t))
        {
            let made = self.annotation_object(node, make, key, java, nested, value, annotation)?;
            return Ok(self.convert(made, object));
        }
        if self.annotation_types.contains_key(element) || element.ends_with("[]") {
            return unsupported(node, &format!("`getAnnotation` of a member of type `{java}`"));
        }
        let ty = self.ty(method, java)?;
        // The JDK's `toString` takes an array of enum constants as an `Enum<?>[]`, so it is an
        // `Object[]` (`covariant`).
        if let Type::Array(element) = ty
            && matches!(*element, Type::Enum(_))
        {
            self.note_covariant(ty);
        }
        let value = match ty {
            Type::Array(_) => {
                let items: Vec<Node<'t>> = match value.kind() {
                    "NEW_ARRAY" => value.list("initializers").collect(),
                    _ => vec![value],
                };
                let element_ty = ty.element();
                let items = items
                    .into_iter()
                    .map(|i| self.element_value(i, element_ty))
                    .collect::<Result<_, Error>>()?;
                let array = self.synth(ExprKind::ArrayLit(items), value.pos(), ty);
                self.made_as(&array, self.exact_ty(method, java)?);
                array
            }
            _ => self.element_value(value, ty)?,
        };
        Ok(self.convert(value, object))
    }

    /// The value `value` as a `ty`, the type of a member or of its array's elements: a constant
    /// widened to it as javac does (`7` for a `long`).
    fn element_value(&self, value: Node<'t>, ty: Type) -> Result<Expr, Error> {
        let lowered = self.expr(value)?;
        if (ty.is_numeric() || ty == Type::Boolean) && lowered.ty != ty {
            let pos = lowered.pos;
            return Ok(self.synth(ExprKind::Cast(ty, Box::new(lowered)), pos, ty));
        }
        Ok(self.convert(lowered, ty))
    }

    /// `m.getAnnotation(A.class)`, `getDeclaredAnnotation`, `isAnnotationPresent`,
    /// `getAnnotations` or `getDeclaredAnnotations` of a `Method`, `Field` or `Constructor` `m`
    /// (see the module's comment); `None` for any other call.
    pub(super) fn member_annotations(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.is_static || !is_member_annotation_call(symbol.owner, symbol.name) {
            return Ok(None);
        }
        let Some(receiver) = receiver else { return Ok(None) };
        let class = receiver.java_type().split('<').next().unwrap_or_default();
        let (kind, target) = match class {
            "java.lang.reflect.Method" => (LookupKind::Method, "METHOD"),
            "java.lang.reflect.Field" => (LookupKind::Field, "FIELD"),
            "java.lang.reflect.Constructor" => (LookupKind::Constructor, "CONSTRUCTOR"),
            _ => return Ok(None),
        };
        let what = format!("`{}.{}`", class.rsplit('.').next().unwrap_or_default(), symbol.name);
        let make = &crate::program::ANNOTATION_VALUES[0];
        let (Some(make), Some(annotation_type)) = (
            self.template_method(make.0, make.1, make.2),
            self.type_index(ANNOTATION).map(|t| self.self_type(t)),
        ) else {
            return unsupported(node, &format!("{what} outside a whole program"));
        };
        // The annotation type asked for, or `None` for all of them.
        let wanted = match node.list("arguments").next() {
            None => None,
            Some(annotation) => {
                let named = annotation.get("expression").filter(|_| super::classes::is_literal(annotation));
                let Some(name) = named.map(|n| n.java_type()) else {
                    return unsupported(node, &format!("{what} of a class other than a class literal's"));
                };
                if !self.annotation_types.contains_key(name) {
                    return unsupported(
                        node,
                        &format!("{what} of `{name}`, which is not an annotation type"),
                    );
                }
                Some(name)
            }
        };
        let pos = node.pos();
        let ty = self.ty(node, node.java_type())?;
        // The members with annotations that Java finds, by the name of their declaration, each
        // with the slots of the table that hold it and what the call gives for it.
        let mut cases: Vec<(Vec<i32>, Expr)> = Vec::new();
        let mut seen: std::collections::HashMap<String, Option<usize>> = Default::default();
        for (slot, decl, key) in self.table_declarations(node, kind)? {
            if let Some(&case) = seen.get(&key) {
                if let Some(case) = case {
                    cases[case].0.push(slot);
                }
                continue;
            }
            seen.insert(key.clone(), None);
            let mut made = Vec::new();
            for (name, at) in self.declared_annotations(node, decl, target, wanted, &what)? {
                let decl = self.annotation_types[name];
                made.push(self.annotation_object(node, make, &key, name, decl, at, annotation_type)?);
            }
            if made.is_empty() {
                continue;
            }
            let value = match symbol.name {
                "isAnnotationPresent" => self.synth(ExprKind::Bool(true), pos, Type::Boolean),
                "getAnnotation" | "getDeclaredAnnotation" => made.remove(0),
                _ => {
                    let made = made.into_iter().map(|a| self.convert(a, ty.element())).collect();
                    self.synth(ExprKind::ArrayLit(made), pos, ty)
                }
            };
            seen.insert(key, Some(cases.len()));
            cases.push((vec![slot], value));
        }
        let none = match symbol.name {
            "isAnnotationPresent" => ExprKind::Bool(false),
            "getAnnotation" | "getDeclaredAnnotation" => ExprKind::Null,
            _ => ExprKind::NewArray(vec![self.synth(ExprKind::Int(0), pos, Type::Int)]),
        };
        let none = self.synth(none, pos, ty);
        let member = self.expr(receiver)?;
        let local = self.fresh_name("member");
        let var = self.synth(ExprKind::Var(local.clone()), pos, member.ty);
        let field = ExprKind::Field { object: Box::new(var), name: "slot".to_string() };
        let mut switch: Vec<Case> = cases
            .into_iter()
            .map(|(slots, value)| Case {
                labels: slots
                    .into_iter()
                    .map(|s| CaseLabel::Value(self.synth(ExprKind::Int(s), pos, Type::Int)))
                    .collect(),
                body: vec![Stmt::Yield(value)],
            })
            .collect();
        switch.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Yield(none)] });
        let selector = Box::new(self.synth(field, pos, Type::Int));
        let body = vec![
            Stmt::Local { ty: member.ty, name: local, init: Some(member), pos },
            Stmt::Yield(self.synth(ExprKind::Switch { selector, cases: switch }, pos, ty)),
        ];
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        }))
    }

    /// The annotations that Java finds on the declaration `decl` of a member, of the kind that
    /// `target` names (`METHOD`, `FIELD` or `CONSTRUCTOR`), in its order, by type: those kept at
    /// run time whose type may be on such a declaration (not one that may only be on a type, as
    /// `@Target(TYPE_USE)` makes `@Nullable`), of the type `wanted` if one is.
    fn declared_annotations(
        &self,
        node: Node<'t>,
        decl: Node<'t>,
        target: &str,
        wanted: Option<&str>,
        what: &str,
    ) -> Result<Vec<(&'t str, Node<'t>)>, Error> {
        let Ok(modifiers) = decl.child("modifiers") else { return Ok(Vec::new()) };
        let mut out: Vec<(&'t str, Node<'t>)> = Vec::new();
        for at in modifiers.list("annotations") {
            let name = at.child("annotationType")?.java_type();
            if wanted.is_some_and(|w| w != name) {
                continue;
            }
            let Some(&annotation) = self.annotation_types.get(name) else {
                return unsupported(
                    node,
                    &format!("{what} where a member has `{name}`, which is not in the program"),
                );
            };
            let runtime = self.annotation_values(annotation, RETENTION)?.is_some_and(|v| v == ["RUNTIME"]);
            let applies =
                self.annotation_values(annotation, TARGET)?.is_none_or(|v| v.iter().any(|t| t == target));
            if !runtime || !applies {
                continue;
            }
            if self.nested_annotation_types.contains(name) {
                return unsupported(
                    node,
                    &format!("{what} where a member has the nested annotation type `{name}`"),
                );
            }
            if out.iter().any(|(n, _)| *n == name) {
                return unsupported(
                    node,
                    &format!("{what} where a member has `{name}` twice (a repeated annotation)"),
                );
            }
            out.push((name, at));
        }
        Ok(out)
    }

    /// `a.m()` of a member `m` of an annotation type (see the module's comment); `None` for any
    /// other call.
    pub(super) fn annotation_member(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
    ) -> Result<Option<ExprKind>, Error> {
        let Some(&decl) = self.annotation_types.get(symbol.owner) else { return Ok(None) };
        let Some(receiver) =
            receiver.filter(|_| !symbol.is_static && node.list("arguments").next().is_none())
        else {
            return Ok(None);
        };
        if !decl.list("members").any(|m| m.kind() == "METHOD" && m.str("name") == symbol.name) {
            return Ok(None);
        }
        let member = &crate::program::ANNOTATION_VALUES[1];
        let (Some(resolved), Some(annotation_type)) = (
            self.template_method(member.0, member.1, member.2),
            self.type_index(ANNOTATION).map(|t| self.self_type(t)),
        ) else {
            return unsupported(
                node,
                &format!("reading the member `{}` of an annotation outside a whole program", symbol.name),
            );
        };
        let pos = node.pos();
        let annotation = self.convert(self.expr(receiver)?, annotation_type);
        let name = self.synth(ExprKind::Str(symbol.name.to_string()), pos, Type::String);
        let call = ExprKind::Call {
            name: "member".to_string(),
            args: vec![annotation, name],
            resolved: Some(resolved),
        };
        let call = self.synth(call, pos, Type::class(OBJECT));
        let ty = self.ty(node, node.java_type())?;
        Ok(Some(self.unboxed(node, call, ty)?.kind))
    }
}
