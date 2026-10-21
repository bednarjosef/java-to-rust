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
//!
//! The reflection that creates an object from a `Class` value (`program::reach::by_name`):
//! `c.getConstructor()` and `c.getDeclaredConstructor()` are `c` itself, as a `Constructor`
//! without parameters is a `Type::JavaClass` too, and `newInstance()` of either is a chain
//! `c_.equals(A.class) ? new A() : ..` over the classes that it can create, with the end of the
//! program for any other class (`Builtin::UnknownClass`). So is `getModifiers()` of such a
//! constructor, with its modifiers' bits, and `setAccessible` does nothing. Two deviations: a
//! class without a constructor without parameters ends the program where Java throws
//! `NoSuchMethodException`, and an exception that the constructor throws is not wrapped in an
//! `InvocationTargetException`. `Class.forName(module, name)` with a name that code computes
//! is a `switch` over the locale data that reachability found (`Reach::found_by_name`), with
//! `null` for any other name, and `Class.forName(name)` over the classes named
//! (`Reach::named`), each initialized, with the end of the program for any other name.
//! `X.class.isAssignableFrom(c)` is a chain of `c.equals(Y.class)` over the program's classes
//! that are or extend `X`.

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
        for t in (0..self.types.len()).filter(|&t| self.is_created_by_name(t, Some(target))) {
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

    /// Whether a creation of a `target` (`None`: of an `Object`) from a class name can create an
    /// object of the class `t`: a top-level class, neither abstract nor generic, that is or
    /// extends or implements `target` and has a constructor without parameters. In a whole
    /// program, reachability kept that constructor if code names the class
    /// (`program::reach::by_name`).
    fn is_created_by_name(&self, t: usize, target: Option<usize>) -> bool {
        let info = &self.types[t];
        let node = info.node;
        let top_level = info.nested.is_none()
            && !info.javac.contains('$')
            && node.symbol().is_some_and(|s| self.type_index(s.owner).is_none());
        top_level
            && node.kind() == "CLASS"
            && !node.has_flag("ABSTRACT")
            && node.list("typeParameters").next().is_none()
            && target.is_none_or(|target| self.is_subclass(t, target))
            && self.methods.iter().any(|m| m.owner == t && m.name == "<init>" && m.sig == "()void")
    }

    /// The classes that an object created from a `Class` value of a type bounded by `bound` can
    /// be of: those that `is_created_by_name` accepts and, in a whole program, that a name in
    /// reachable code finds, which are the ones whose constructor reachability kept for it. A
    /// bound that is no class of the program, `Object` or a type variable, bounds nothing. With
    /// them comes the type of the object that `node` creates.
    fn created_from_class(&self, node: Node<'t>, bound: &str) -> Result<(Type, Vec<usize>), Error> {
        let target = self.type_index(bound);
        let ty = self.ty(node, node.java_type())?;
        Ok((ty, self.created_from_bound(target)))
    }

    /// The classes that `is_created_by_name` accepts for `target` and, in a whole program, that a
    /// name in reachable code finds.
    fn created_from_bound(&self, target: Option<usize>) -> Vec<usize> {
        let named = |t: usize| {
            self.reach.is_none_or(|reach| {
                reach.named.contains(&self.types[t].javac)
                    || reach.found_by_name.contains(&self.types[t].javac)
            })
        };
        (0..self.types.len()).filter(|&t| self.is_created_by_name(t, target) && named(t)).collect()
    }

    /// `{ Class c_ = class; c_.equals(A.class) ? a : c_.equals(B.class) ? b : .. : end }`, with
    /// `case` giving `a` of `A`, over `classes`, and `end` the end of the program with the name
    /// of `c_`'s class (`Builtin::UnknownClass`).
    fn class_chain(
        &self,
        node: Node<'t>,
        class: Expr,
        classes: &[usize],
        ty: Type,
        case: impl Fn(usize) -> Result<Expr, Error>,
    ) -> Result<ExprKind, Error> {
        let pos = node.pos();
        let local = self.fresh_name("class");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        let name = self.synth(
            ExprKind::Builtin(Builtin::ClassMethod(ClassOp::GetName), vec![var()]),
            pos,
            Type::String,
        );
        let mut chain = self.synth(ExprKind::Builtin(Builtin::UnknownClass, vec![name]), pos, ty);
        for &t in classes.iter().rev() {
            let test = self.class_equals(node, var(), t)?;
            chain = self.synth(
                ExprKind::Conditional(Box::new(test), Box::new(case(t)?), Box::new(chain)),
                pos,
                ty,
            );
        }
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(class), pos },
            Stmt::Yield(chain),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// `class.equals(T.class)` of the class `t` of the program.
    fn class_equals(&self, node: Node<'t>, class: Expr, t: usize) -> Result<Expr, Error> {
        let literal = self.class_literal_of(node, t)?;
        let equals = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![class, literal]);
        Ok(self.synth(equals, node.pos(), Type::Boolean))
    }

    /// `T.class` of the class `t` of the program.
    fn class_literal_of(&self, node: Node<'t>, t: usize) -> Result<Expr, Error> {
        let ty = self.ty(node, &self.types[t].javac)?;
        Ok(self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), node.pos(), Type::JavaClass))
    }

    /// `newInstance()` of the `Class` or `Constructor` value `class`, whose type `class_type`
    /// bounds the classes it can be of (see the module's comment).
    pub(super) fn new_instance(
        &self,
        node: Node<'t>,
        class: Expr,
        class_type: &str,
    ) -> Result<ExprKind, Error> {
        let (ty, classes) = self.created_from_class(node, &crate::program::class_bound(class_type))?;
        self.class_chain(node, class, &classes, ty, |t| {
            let ctor =
                self.methods.iter().position(|m| m.owner == t && m.name == "<init>" && m.sig == "()void");
            let class = Type::class(&self.types[t].name);
            let created = self.synth(ExprKind::New { class: t, args: Vec::new(), ctor }, node.pos(), class);
            Ok(self.convert(created, ty))
        })
    }

    /// `getModifiers()` of the `Constructor` value `class`: the bits of the modifiers of the
    /// constructor without parameters of its class (`java.lang.reflect.Modifier`).
    pub(super) fn constructor_modifiers(
        &self,
        node: Node<'t>,
        class: Expr,
        class_type: &str,
    ) -> Result<ExprKind, Error> {
        let classes = self.created_from_bound(self.type_index(&crate::program::class_bound(class_type)));
        self.class_chain(node, class, &classes, Type::Int, |t| {
            let ctor = self.methods.iter().find(|m| m.owner == t && m.name == "<init>" && m.sig == "()void");
            let bits = [("PUBLIC", 1), ("PRIVATE", 2), ("PROTECTED", 4)]
                .into_iter()
                .filter(|(flag, _)| ctor.is_some_and(|m| m.node.has_flag(flag)))
                .map(|(_, bit)| bit)
                .sum();
            Ok(self.synth(ExprKind::Int(bits), node.pos(), Type::Int))
        })
    }

    /// `Class.forName(module, name)`: the class of the program with that name, which the module
    /// does not change, as the program's classes are all one module's, or `null` (see the
    /// module's comment). The module must be `X.class.getModule()`, which has no effects.
    pub(super) fn module_for_name(&self, node: Node<'t>, name: Option<String>) -> Result<ExprKind, Error> {
        let mut arguments = node.list("arguments");
        let module = arguments.next().expect("forName has a module");
        let name_node = arguments.next().expect("forName has a name");
        let of_literal = module.kind() == "METHOD_INVOCATION"
            && module.symbol().is_some_and(|s| (s.owner, s.name) == ("java.lang.Class", "getModule"))
            && module.child("methodSelect")?.get("expression").is_some_and(super::classes::is_literal);
        if !of_literal {
            return unsupported(node, "`Class.forName` in a module other than a class literal's");
        }
        let pos = node.pos();
        if let Some(name) = name {
            return Ok(match self.type_index(&name) {
                Some(t) => self.class_literal_of(node, t)?.kind,
                None => ExprKind::Null,
            });
        }
        let Some(reach) = self.reach else {
            return unsupported(node, "`Class.forName` of a computed name outside a whole program");
        };
        let class_name = self.fresh_name("class_name");
        let var = || self.synth(ExprKind::Var(class_name.clone()), pos, Type::String);
        let mut cases = Vec::new();
        for t in (0..self.types.len()).filter(|&t| reach.found_by_name.contains(&self.types[t].javac)) {
            let label = self.synth(ExprKind::Str(self.types[t].javac.clone()), pos, Type::String);
            let body = vec![Stmt::Yield(self.class_literal_of(node, t)?)];
            cases.push(Case { labels: vec![CaseLabel::Value(label)], body });
        }
        let null = self.synth(ExprKind::Null, pos, Type::Null);
        cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Yield(null)] });
        let switch = self.synth(ExprKind::Switch { selector: Box::new(var()), cases }, pos, Type::JavaClass);
        let name = self.convert(self.expr(name_node)?, Type::String);
        let block = vec![
            Stmt::Local { ty: Type::String, name: class_name.clone(), init: Some(name), pos },
            Stmt::Yield(switch),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// `Class.forName(name)`, or `Class.forName(name, initialize, loader)` with a literal
    /// `initialize`, of a name that code computes: a `switch` over the classes that names in
    /// reachable code find (`Reach::named`), each initialized if the call says so.
    pub(super) fn computed_for_name(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let Some(reach) = self.reach else {
            return unsupported(node, "`Class.forName` of a computed name outside a whole program");
        };
        let arguments: Vec<Node<'t>> = node.list("arguments").collect();
        let initialize = match arguments.get(1) {
            None => true,
            Some(flag) if flag.kind() == "BOOLEAN_LITERAL" => flag.0["value"].as_bool() == Some(true),
            Some(_) => {
                return unsupported(
                    node,
                    "`Class.forName` that initializes the class or not as code computes",
                );
            }
        };
        let pos = node.pos();
        let mut block = Vec::new();
        let class_name = self.fresh_name("class_name");
        let name = self.convert(self.expr(arguments[0])?, Type::String);
        block.push(Stmt::Local { ty: Type::String, name: class_name.clone(), init: Some(name), pos });
        if let Some(loader) = arguments.get(2) {
            let loader = self.expr(*loader)?;
            if has_effects(&loader) {
                block.push(Stmt::Expr(loader));
            }
        }
        let var = || self.synth(ExprKind::Var(class_name.clone()), pos, Type::String);
        let mut cases = Vec::new();
        for t in (0..self.types.len()).filter(|&t| reach.named.contains(&self.types[t].javac)) {
            let label = self.synth(ExprKind::Str(self.types[t].javac.clone()), pos, Type::String);
            let mut body = Vec::new();
            if initialize {
                let init = ExprKind::Builtin(Builtin::InitializeClass { class: t }, Vec::new());
                body.push(Stmt::Expr(self.synth(init, pos, Type::Void)));
            }
            body.push(Stmt::Yield(self.class_literal_of(node, t)?));
            cases.push(Case { labels: vec![CaseLabel::Value(label)], body });
        }
        let unknown = self.synth(ExprKind::Builtin(Builtin::UnknownClass, vec![var()]), pos, Type::Void);
        cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Expr(unknown)] });
        let switch = ExprKind::Switch { selector: Box::new(var()), cases };
        block.push(Stmt::Yield(self.synth(switch, pos, Type::JavaClass)));
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// `X.class.isAssignableFrom(c)` of a class `X` of the program that no class of the runtime
    /// extends: whether `c` is one of the program's classes that are or extend `X`. An interface,
    /// or a class such as `Number` that the runtime's boxes extend, is not supported.
    pub(super) fn is_assignable_from(
        &self,
        node: Node<'t>,
        literal: Node<'t>,
        class: Node<'t>,
    ) -> Result<ExprKind, Error> {
        let named = literal.child("expression")?;
        let target = match self.ty(named, named.java_type())? {
            Type::Class(name, _) => self.type_index(name),
            _ => None,
        };
        let supported =
            |t: &usize| self.types[*t].node.kind() == "CLASS" && self.types[*t].javac != "java.lang.Number";
        let Some(target) = target.filter(supported) else {
            return unsupported(node, "`isAssignableFrom` of this class");
        };
        let pos = node.pos();
        let local = self.fresh_name("class");
        let var = || self.synth(ExprKind::Var(local.clone()), pos, Type::JavaClass);
        let mut chain = self.synth(ExprKind::Bool(false), pos, Type::Boolean);
        for t in (0..self.types.len()).rev().filter(|&t| self.is_subclass(t, target)) {
            let test = self.class_equals(node, var(), t)?;
            let yes = self.synth(ExprKind::Bool(true), pos, Type::Boolean);
            let conditional = ExprKind::Conditional(Box::new(test), Box::new(yes), Box::new(chain));
            chain = self.synth(conditional, pos, Type::Boolean);
        }
        let class = self.expr(class)?;
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(class), pos },
            Stmt::Yield(chain),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }
}
