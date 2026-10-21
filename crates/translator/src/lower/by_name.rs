//! Objects created from a class name, `(T) Class.forName(name).getDeclaredConstructor()
//! .newInstance()` (see `program::reach::by_name`): a `switch` over the name with a case for each
//! class of the program that the cast can create, which creates an object of it with its
//! constructor without parameters. Any other name is of no class of the translated program, and
//! the default case ends the program with a `ClassNotFoundException` (`Builtin::UnknownClass`).
//!
//! `Class.forName` with a name that the code spells out (`program::class_for_name`) finds that
//! class of the program: the call is its class literal, after its static initialization
//! (`Builtin::InitializeClass`) when the call initializes it.
//!
//! `MethodHandles.lookup().ensureInitialized(c)` initializes the class of the `Class` value `c`
//! and gives it back: a class literal's class directly, and any other value's by its name, among
//! the classes that reachability found code gives it (`program::reach::initialize`).

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

    /// `Class.getPrimitiveClass("char")`, the native behind the boxes' `TYPE` fields, is the class
    /// literal `char.class`.
    pub(super) fn primitive_class(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let name = node.list("arguments").next().filter(|n| n.kind() == "STRING_LITERAL");
        let ty = match name.and_then(|n| n.0["value"].as_str()) {
            Some("boolean") => Type::Boolean,
            Some("byte") => Type::Byte,
            Some("char") => Type::Char,
            Some("short") => Type::Short,
            Some("int") => Type::Int,
            Some("long") => Type::Long,
            Some("float") => Type::Float,
            Some("double") => Type::Double,
            Some("void") => Type::Void,
            _ => return unsupported(node, "`Class.getPrimitiveClass` of this name"),
        };
        Ok(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()))
    }

    /// The call `node`, `Class.forName("a.B", initialize, loader)` or `Class.forName("a.B")`,
    /// as `{ loader; init a.B; a.B.class }`, and an end of the program for a name of no class of the
    /// program; `None` when the name is not a string literal. The
    /// loader is evaluated for its effects only: the program's classes are all one loader's.
    pub(super) fn class_for_name(&self, node: Node<'t>) -> Result<Option<ExprKind>, Error> {
        let Some((name, initialize)) = crate::program::class_for_name(node.0) else { return Ok(None) };
        let Some(class) = self.type_index(&name) else {
            // A class that the program does not have, such as `java.lang.invoke`'s, which is not
            // translated: the call ends the program when it runs (`Builtin::UnknownClass`).
            let name = self.expr(node.list("arguments").next().expect("forName has a name"))?;
            return Ok(Some(ExprKind::Builtin(Builtin::UnknownClass, vec![name])));
        };
        let pos = node.pos();
        let mut block = Vec::new();
        if let Some(loader) = node.list("arguments").nth(2) {
            let loader = self.expr(loader)?;
            if has_effects(&loader) {
                block.push(Stmt::Expr(loader));
            }
        }
        if initialize {
            let init = ExprKind::Builtin(Builtin::InitializeClass { class }, Vec::new());
            block.push(Stmt::Expr(self.synth(init, pos, Type::Void)));
        }
        let ty = self.ty(node, &self.types[class].javac)?;
        let literal = ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new());
        if block.is_empty() {
            return Ok(Some(literal));
        }
        block.push(Stmt::Yield(self.synth(literal, pos, Type::JavaClass)));
        Ok(Some(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        }))
    }

    /// `MethodHandles.lookup().ensureInitialized(c)` (see the module's comment): `{ init X;
    /// X.class }` for a literal `X.class`, and for any other `c`, `{ Class c_ = c; switch
    /// (c_.getName()) { case "X" -> init X; .. default -> {} } c_ }`.
    pub(super) fn ensure_initialized(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let select = node.child("methodSelect")?;
        let lookup = select.child("expression")?;
        let is_lookup = lookup.kind() == "METHOD_INVOCATION"
            && lookup
                .symbol()
                .is_some_and(|s| (s.owner, s.name) == ("java.lang.invoke.MethodHandles", "lookup"));
        if !is_lookup {
            return unsupported(node, "`ensureInitialized` of a lookup other than `MethodHandles.lookup()`");
        }
        let pos = node.pos();
        let argument = node.list("arguments").next().expect("ensureInitialized has an argument");
        let init = |class: usize| {
            let init = ExprKind::Builtin(Builtin::InitializeClass { class }, Vec::new());
            Stmt::Expr(self.synth(init, pos, Type::Void))
        };
        let block = |body: Vec<Stmt>| ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        };
        if let Some(name) = crate::program::literal_class(argument.0) {
            let Some(class) = self.type_index(&name) else {
                return unsupported(
                    node,
                    &format!("`ensureInitialized` of `{name}`, which is not a class of the program"),
                );
            };
            let literal = self.expr(argument)?;
            return Ok(block(vec![init(class), Stmt::Yield(literal)]));
        }
        let value = self.expr(argument)?;
        if value.ty != Type::JavaClass {
            return unsupported(node, "`ensureInitialized` of this value");
        }
        let local = self.fresh_name("class");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        // `if (c_.equals(X.class)) init X; else if ..`, over the classes it can be.
        let mut chain: Option<Box<Stmt>> = None;
        for t in (0..self.types.len()).rev() {
            let ensured = match self.reach {
                Some(reach) => reach.ensured.contains(&self.types[t].javac),
                None => self.types[t].nested.is_none(),
            };
            if !ensured {
                continue;
            }
            let ty = self.ty(node, &self.types[t].javac)?;
            let literal =
                self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), pos, Type::JavaClass);
            let equals = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![var(), literal]);
            let cond = self.synth(equals, pos, Type::Boolean);
            chain = Some(Box::new(Stmt::If { cond, then: Box::new(init(t)), otherwise: chain }));
        }
        let mut body = vec![Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(value), pos }];
        body.extend(chain.map(|c| *c));
        body.push(Stmt::Yield(var()));
        Ok(block(body))
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
