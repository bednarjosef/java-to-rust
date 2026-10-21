//! Objects created from a class name, `(T) Class.forName(name).getDeclaredConstructor()
//! .newInstance()` (see `program::reach::by_name`): a `switch` over the name with a case for each
//! class of the program that the cast can create, which creates an object of it with its
//! constructor without parameters. Any other name is of no class of the translated program, and
//! the default case ends the program with a `ClassNotFoundException` (`Builtin::UnknownClass`).

use super::*;

impl<'t> Lowering<'t> {
    /// The creation that the cast `node` makes from the class name `name`:
    /// `{ String n = name; switch (n) { case "C" -> (T) new C(); .. default -> unknown(n) } }`.
    pub(super) fn created_by_name(&self, node: Node<'t>, name: Node<'t>) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let ty = self.ty(node, node.java_type())?;
        let target = match ty {
            Type::Class(class, _) => self.type_index(class),
            _ => None,
        };
        let Some(target) = target else {
            return unsupported(node, "creating an object of this type from a class name");
        };
        let class_name = self.fresh_name("class_name");
        let var = || self.synth(ExprKind::Var(class_name.clone()), pos, Type::String);
        let mut cases = Vec::new();
        for t in (0..self.types.len()).filter(|&t| self.is_created_by_name(t, target)) {
            let ctor =
                self.methods.iter().position(|m| m.owner == t && m.name == "<init>" && m.sig == "()void");
            let class = Type::class(&self.types[t].name);
            let mut created = self.synth(ExprKind::New { class: t, args: Vec::new(), ctor }, pos, class);
            if t != target {
                created = self.synth(ExprKind::Cast(ty, Box::new(created)), pos, ty);
            }
            let label = self.synth(ExprKind::Str(self.types[t].javac.clone()), pos, Type::String);
            cases.push(Case { labels: vec![CaseLabel::Value(label)], body: vec![Stmt::Yield(created)] });
        }
        let unknown = self.synth(ExprKind::Builtin(Builtin::UnknownClass, vec![var()]), pos, Type::Void);
        cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Expr(unknown)] });
        let switch = self.synth(ExprKind::Switch { selector: Box::new(var()), cases }, pos, ty);
        let name = self.convert(self.expr(name)?, Type::String);
        let block = vec![
            Stmt::Local { ty: Type::String, name: class_name.clone(), init: Some(name), pos },
            Stmt::Yield(switch),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// Whether a creation of a `target` from a class name can create an object of the class `t`:
    /// a top-level class, neither abstract nor generic, that is or extends or implements
    /// `target` and has a constructor without parameters. In a whole program, reachability kept
    /// that constructor if code names the class (`program::reach::by_name`).
    fn is_created_by_name(&self, t: usize, target: usize) -> bool {
        let info = &self.types[t];
        let node = info.node;
        let top_level = info.nested.is_none()
            && !info.javac.contains('$')
            && node.symbol().is_some_and(|s| self.type_index(s.owner).is_none());
        top_level
            && node.kind() == "CLASS"
            && !node.has_flag("ABSTRACT")
            && node.list("typeParameters").next().is_none()
            && self.is_subclass(t, target)
            && self.methods.iter().any(|m| m.owner == t && m.name == "<init>" && m.sig == "()void")
    }
}
