//! Annotation objects. The JDK makes one (`AnnotationParser.annotationForMap`, which javac's
//! `AnnotationProxyMaker` calls for `getAnnotation`) as a `Proxy` of the annotation type; a
//! translated program has no proxies, and the patched `AnnotationInvocationHandler` is the
//! annotation itself (`java/patches/sun/reflect/annotation`). Its class is not the annotation
//! type's, which is not translated, so a class test of an object that may be one also asks
//! whether it is one holding that type: `c.isInstance(x)` is `x instanceof
//! AnnotationInvocationHandler h && h.type == c || c.isInstance(x)`, and `c.cast(x)` checks the
//! same. Only such an object can be of an annotation type: a class implementing one is rejected.
//! Its `equals` compares the values of the type's members, which the native
//! `TranslatedAnnotations.members0(type)` names: a chain of tests of the type against the
//! program's annotation types, each giving the names of its elements in declaration order.

use super::*;

const HANDLER: &str = "sun.reflect.annotation.AnnotationInvocationHandler";
const MEMBERS: &str = "sun.reflect.annotation.TranslatedAnnotations";

impl<'t> Lowering<'t> {
    /// `value instanceof AnnotationInvocationHandler h && h.type == class`, for a value that may
    /// be an annotation object (see the module's comment); `None` when the program has none, or
    /// when `value`'s type cannot hold one. `class` and `value` are reads of locals.
    pub(super) fn annotation_object_test(&self, class: &Expr, value: &Expr) -> Option<Expr> {
        let handler = self.type_index(HANDLER)?;
        let may_hold = match value.ty {
            Type::Class(name, _) if name == OBJECT => true,
            Type::Class(name, _) => self.type_index(name).is_some_and(|t| self.is_subclass(handler, t)),
            _ => false,
        };
        if !may_hold {
            return None;
        }
        let pos = value.pos;
        let ty = self.self_type(handler);
        let binding = self.fresh_name("annotation");
        let test =
            ExprKind::InstanceOf { value: Box::new(self.copy(value)), ty, binding: Some(binding.clone()) };
        let test = self.synth(test, pos, Type::Boolean);
        let object = self.synth(ExprKind::Var(binding), pos, ty);
        let field = ExprKind::Field { object: Box::new(object), name: "type".to_string() };
        let held = self.synth(field, pos, Type::JavaClass);
        let same = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![held, self.copy(class)]);
        let same = self.synth(same, pos, Type::Boolean);
        Some(self.synth(ExprKind::Binary(BinOp::And, Box::new(test), Box::new(same)), pos, Type::Boolean))
    }

    /// A call of the native of `TranslatedAnnotations`, with its arguments (see the module's
    /// comment); `None` for any other call.
    pub(super) fn annotation_members_native(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        // `made0` is the runtime's (`Native::NoteAnnotation`).
        if symbol.owner != MEMBERS || !symbol.native || symbol.name != "members0" {
            return Ok(None);
        }
        let pos = node.pos();
        // An `Object[]` where `String[]` is covariant (`covariant`), its strings stored as objects.
        let strings = self.covariant_array(Type::array(Type::String));
        let local = self.fresh_name("annotation");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        let mut chain = self.synth(ExprKind::Null, pos, Type::Null);
        let mut types: Vec<(&String, &Node<'t>)> = self.annotation_types.iter().collect();
        types.sort_by_key(|(name, _)| std::cmp::Reverse(name.as_str()));
        for (name, decl) in types {
            if self.nested_annotation_types.contains(name) {
                continue;
            }
            let members = decl
                .list("members")
                .filter(|m| m.kind() == "METHOD")
                .map(|m| {
                    let name = self.synth(ExprKind::Str(m.str("name").to_string()), pos, Type::String);
                    self.convert(name, strings.element())
                })
                .collect();
            let array = self.synth(ExprKind::ArrayLit(members), pos, strings);
            self.annotation_classes.borrow_mut().insert(name.clone());
            let literal = ExprKind::Builtin(Builtin::ClassLiteral(Type::class(name)), Vec::new());
            let literal = self.synth(literal, pos, Type::JavaClass);
            let test = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![var(), literal]);
            let test = self.synth(test, pos, Type::Boolean);
            chain = self.synth(
                ExprKind::Conditional(Box::new(test), Box::new(array), Box::new(chain)),
                pos,
                strings,
            );
        }
        let class = self.convert(args.remove(0), Type::JavaClass);
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(class), pos },
            Stmt::Yield(chain),
        ];
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        }))
    }
}
