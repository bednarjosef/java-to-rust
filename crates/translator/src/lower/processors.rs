//! The annotations of the program's annotation processors, which the JDK's `AbstractProcessor`
//! reads by reflection (`getClass().getAnnotation(SupportedOptions.class).value()`). The
//! translation has no annotation objects, so the JDK's `AbstractProcessor`, patched
//! (`java/patches/javax/annotation/processing`), reads them through the native
//! `TranslatedProcessorAnnotations.values0(c, name)`, which is a chain of tests of `c` against
//! the classes of the program that have the annotation `name` themselves (none of the three is
//! `@Inherited`), each giving its `value` as strings: `{ Class c = ..; yield c.equals(A.class) ?
//! new String[] {"*"} : c.equals(B.class) ? .. : null; }`. A value is a string literal, an array
//! of them, a constant, or an enum constant, which gives its name; any other is rejected.

use super::*;

const ANNOTATIONS: &str = "javax.annotation.processing.TranslatedProcessorAnnotations";

impl<'t> Lowering<'t> {
    /// A call of the native of `TranslatedProcessorAnnotations`, with its arguments (see the
    /// module's comment); `None` for any other call.
    pub(super) fn processor_annotations_native(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner != ANNOTATIONS || !symbol.native {
            return Ok(None);
        }
        let annotation = node.list("arguments").nth(1).filter(|n| n.kind() == "STRING_LITERAL");
        let Some(annotation) = annotation.and_then(|n| n.0["value"].as_str()) else {
            return unsupported(node, "`TranslatedProcessorAnnotations.values0` of a computed annotation");
        };
        let pos = node.pos();
        let strings = Type::array(Type::String);
        let local = self.fresh_name("processor");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        let mut chain = self.synth(ExprKind::Null, pos, Type::Null);
        for t in (0..self.types.len()).rev() {
            let Some(values) = self.annotation_values(self.types[t].node, annotation)? else { continue };
            let values =
                values.into_iter().map(|v| self.synth(ExprKind::Str(v), pos, Type::String)).collect();
            let array = self.synth(ExprKind::ArrayLit(values), pos, strings);
            let test = self.class_equals(node, var(), t)?;
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

    /// The `value` of the annotation `annotation` (a qualified name) that the declaration `decl`
    /// has, as strings, or `None` when it has none.
    pub(super) fn annotation_values(
        &self,
        decl: Node<'t>,
        annotation: &str,
    ) -> Result<Option<Vec<String>>, Error> {
        let Ok(modifiers) = decl.child("modifiers") else { return Ok(None) };
        for at in modifiers.list("annotations") {
            let Ok(ty) = at.child("annotationType") else { continue };
            if ty.java_type() != annotation {
                continue;
            }
            let mut values = Vec::new();
            for argument in at.list("arguments") {
                // `@A(x)` is `@A(value = x)`.
                let value = match argument.kind() {
                    "ASSIGNMENT" if argument.child("variable")?.str("name") == "value" => {
                        argument.child("expression")?
                    }
                    "ASSIGNMENT" => continue,
                    _ => argument,
                };
                match value.kind() {
                    "NEW_ARRAY" => {
                        for element in value.list("initializers") {
                            values.push(self.annotation_string(element)?);
                        }
                    }
                    _ => values.push(self.annotation_string(value)?),
                }
            }
            return Ok(Some(values));
        }
        Ok(None)
    }

    /// The value `node` of an annotation's element as a string: a string literal, a constant, or
    /// an enum constant by its name.
    fn annotation_string(&self, node: Node<'t>) -> Result<String, Error> {
        if node.kind() == "STRING_LITERAL"
            && let Some(text) = node.0["value"].as_str()
        {
            return Ok(text.to_string());
        }
        match node.symbol() {
            Some(symbol) if symbol.kind == "ENUM_CONSTANT" => Ok(symbol.name.to_string()),
            Some(symbol) if symbol.kind == "FIELD" && symbol.ty == "java.lang.String" => {
                match symbol.constant {
                    Some(text) => Ok(text.to_string()),
                    None => unsupported(node, "this value of an annotation"),
                }
            }
            _ => unsupported(node, "this value of an annotation"),
        }
    }
}
