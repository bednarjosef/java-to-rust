//! `c.isAnnotationPresent(A.class)`, which javac asks of an annotation type
//! (`annoType.isAnnotationPresent(Inherited.class)`). The translation has no annotations, so the
//! call is a chain of tests of `c` against the classes and annotation types whose declarations
//! have `A`: `{ Class c = ..; yield c.equals(X.class) || c.equals(Y.class); }`, which is `false`
//! for every class unless `A` is kept when the program runs (`@Retention(RUNTIME)`). A class has
//! `A` when its declaration has it once (twice or more puts the container of a `@Repeatable` `A`
//! there instead), or, for an `@Inherited` `A`, when its superclass has it.
//!
//! The classes are the program's and its annotation types, the only ones a class value can be of
//! besides the object model's own (`String`, the boxes, ...), arrays and primitive types, which
//! have no annotation of the program. A JDK annotation that the object model's classes could
//! have is rejected unless it can only be on annotation types (`@Target(ANNOTATION_TYPE)`), as
//! `Inherited` and `Documented` are. A nested annotation type has no class value (its class
//! literal is rejected), so it is not tested.

use super::*;

const RETENTION: &str = "java.lang.annotation.Retention";
const TARGET: &str = "java.lang.annotation.Target";
const INHERITED: &str = "java.lang.annotation.Inherited";
const REPEATABLE: &str = "java.lang.annotation.Repeatable";

impl<'t> Lowering<'t> {
    /// `receiver.isAnnotationPresent(annotation)` (see the module's comment).
    pub(super) fn annotation_present(
        &self,
        node: Node<'t>,
        receiver: Node<'t>,
        annotation: Node<'t>,
    ) -> Result<ExprKind, Error> {
        let named = annotation.get("expression").filter(|_| super::classes::is_literal(annotation));
        let Some(name) = named.map(|n| n.java_type()) else {
            return unsupported(node, "`isAnnotationPresent` of a class other than a class literal's");
        };
        let Some(&decl) = self.annotation_types.get(name) else {
            return unsupported(
                node,
                &format!("`isAnnotationPresent` of `{name}`, which is not an annotation type"),
            );
        };
        let runtime = self.annotation_values(decl, RETENTION)?.is_some_and(|v| v == ["RUNTIME"]);
        let only_annotation_types =
            self.annotation_values(decl, TARGET)?.is_some_and(|v| v == ["ANNOTATION_TYPE"]);
        if runtime && !only_annotation_types && self.library_annotations.contains(name) {
            return unsupported(node, &format!("`isAnnotationPresent` of the JDK's `{name}`"));
        }
        let inherited = self.has_annotation(decl, INHERITED);
        let pos = node.pos();
        let local = self.fresh_name("class");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        let equals = |literal: Expr| {
            let kind = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![var(), literal]);
            self.synth(kind, pos, Type::Boolean)
        };
        let mut tests = Vec::new();
        if runtime {
            for t in 0..self.types.len() {
                if self.class_has(t, name, inherited)? {
                    tests.push(equals(self.class_literal_of(node, t)?));
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
                tests.push(equals(self.synth(literal, pos, Type::JavaClass)));
            }
        }
        // `c.equals(null)`, which is false once `c` is dereferenced as Java's call does.
        let chain = tests
            .into_iter()
            .reduce(|a, b| {
                self.synth(ExprKind::Binary(BinOp::Or, Box::new(a), Box::new(b)), pos, Type::Boolean)
            })
            .unwrap_or_else(|| equals(self.synth(ExprKind::Null, pos, Type::Null)));
        let class = self.convert(self.expr(receiver)?, Type::JavaClass);
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(class), pos },
            Stmt::Yield(chain),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// Whether class `t` of the program has the annotation `name`: its declaration has it, or,
    /// for an `inherited` one, its superclass has it (an interface inherits none).
    fn class_has(&self, t: usize, name: &str, inherited: bool) -> Result<bool, Error> {
        if self.directly_present(self.types[t].node, name) {
            return Ok(true);
        }
        if !inherited || self.types[t].node.kind() != "CLASS" {
            return Ok(false);
        }
        match self.superclass(t)? {
            Some(s) => self.class_has(s, name, inherited),
            None => Ok(false),
        }
    }

    /// Whether the declaration `decl` has the annotation `name` itself: once, or as the
    /// container of an annotation that it has twice or more.
    fn directly_present(&self, decl: Node<'t>, name: &str) -> bool {
        let Ok(modifiers) = decl.child("modifiers") else { return false };
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for at in modifiers.list("annotations") {
            let Ok(ty) = at.child("annotationType") else { continue };
            match counts.iter_mut().find(|(n, _)| *n == ty.java_type()) {
                Some((_, count)) => *count += 1,
                None => counts.push((ty.java_type(), 1)),
            }
        }
        counts.iter().any(|&(annotation, count)| match count {
            1 => annotation == name,
            _ => self.container(annotation).is_some_and(|c| c == name),
        })
    }

    /// The container of the `@Repeatable` annotation type `name`, by name.
    fn container(&self, name: &str) -> Option<&'t str> {
        let decl = self.annotation_types.get(name)?;
        let at = decl
            .child("modifiers")
            .ok()?
            .list("annotations")
            .find(|at| at.child("annotationType").is_ok_and(|ty| ty.java_type() == REPEATABLE))?;
        let value = at.list("arguments").next()?;
        let value = if value.kind() == "ASSIGNMENT" { value.child("expression").ok()? } else { value };
        super::classes::is_literal(value).then(|| value.get("expression").map(|e| e.java_type()))?
    }

    /// Whether the declaration `decl` has the annotation `name`, whatever its values.
    fn has_annotation(&self, decl: Node<'t>, name: &str) -> bool {
        let Ok(modifiers) = decl.child("modifiers") else { return false };
        modifiers
            .list("annotations")
            .any(|at| at.child("annotationType").is_ok_and(|ty| ty.java_type() == name))
    }
}
