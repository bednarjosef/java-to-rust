//! Objects created from a class name, `(T) Class.forName(name).getDeclaredConstructor()
//! .newInstance()` (see `program::reach::by_name`): a `switch` over the name with a case for each
//! class of the program that the cast can create, which creates an object of it with its
//! constructor without parameters. Any other name is of no class of the translated program, and
//! the default case throws a `ClassNotFoundException` that says so (`Builtin::UnknownClass`).
//!
//! `Class.forName` with a name that the code spells out (`program::class_for_name`) finds that
//! class of the program: the call is its class literal, after its static initialization
//! (`Builtin::InitializeClass`) when the call initializes it.
//!
//! `MethodHandles.lookup().ensureInitialized(c)` initializes the class of the `Class` value `c`
//! and gives it back: a class literal's class directly, and any other value's by its name, among
//! the classes that reachability found code gives it (`program::reach::initialize`).
//!
//! An object created from a `Class` value, `c.getConstructor().newInstance()`, goes through the
//! reflection table (`lower/reflection.rs`), whose constructors are those of the classes that
//! names in reachable code find. `Class.forName(module, name)` with a name that code computes
//! is a `switch` over the locale data that reachability found (`Reach::found_by_name`), with
//! `null` for any other name, and `Class.forName(name)` over the classes named
//! (`Reach::named`), each initialized, with a `ClassNotFoundException` for any other name.
//! `X.class.isAssignableFrom(c)` is a chain of `c.equals(Y.class)` over the program's classes
//! that are or extend `X`.

use super::*;

/// The message of the `NullPointerException` that `Class.forName(name)` and
/// `Class.forName(name, initialize, loader)` throw for a null name.
const FOR_NAME_NULL: &str = "Cannot invoke \"String.length()\" because \"str\" is null";

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
            let name = self.binary_name(t).unwrap_or_else(|| self.types[t].javac.clone());
            let label = self.synth(ExprKind::Str(name), pos, Type::String);
            cases.push(Case { labels: vec![CaseLabel::Value(label)], body: vec![Stmt::Yield(created)] });
        }
        let unknown = self.synth(ExprKind::Builtin(Builtin::UnknownClass, vec![var()]), pos, Type::Void);
        cases.push(Case { labels: vec![CaseLabel::Default], body: vec![Stmt::Expr(unknown)] });
        let switch = self.synth(ExprKind::Switch { selector: Box::new(var()), cases }, pos, ty);
        let name = self.convert(self.expr(name)?, Type::String);
        let given = self.fresh_name("given_name");
        let block = vec![
            Stmt::Local { ty: Type::String, name: given.clone(), init: Some(name), pos },
            self.checked_name(&class_name, &given, Some(FOR_NAME_NULL), pos),
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
    /// as `{ loader; init a.B; a.B.class }`, and a `ClassNotFoundException` for a name of no class
    /// of the program; `None` when the name is not a string literal. The
    /// loader is evaluated for its effects only: the program's classes are all one loader's.
    pub(super) fn class_for_name(&self, node: Node<'t>) -> Result<Option<ExprKind>, Error> {
        let Some((name, initialize)) = crate::program::class_for_name(node.0) else { return Ok(None) };
        let Some(class) = self.type_index(&name) else {
            // A class that the program does not have, such as `java.lang.invoke`'s, which is not
            // translated: the call throws when it runs (`Builtin::UnknownClass`).
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
    /// object of the class `t`: a top-level class (in a file, a static member class too), neither
    /// abstract nor generic, that is or extends or implements `target` and has a constructor
    /// without parameters. In a whole
    /// program, reachability kept that constructor if code names the class
    /// (`program::reach::by_name`).
    fn is_created_by_name(&self, t: usize, target: Option<usize>) -> bool {
        let info = &self.types[t];
        let node = info.node;
        let top_level = info.nested.is_none()
            && !info.javac.contains('$')
            && node.symbol().is_some_and(|s| self.type_index(s.owner).is_none());
        // In a file, a static member class is found by its binary name too.
        let member = self.reach.is_none() && node.has_flag("STATIC") && self.binary_name(t).is_some();
        (top_level || member)
            && node.kind() == "CLASS"
            && !node.has_flag("ABSTRACT")
            && node.list("typeParameters").next().is_none()
            && target.is_none_or(|target| self.is_subclass(t, target))
            && self.methods.iter().any(|m| m.owner == t && m.name == "<init>" && m.sig == "()void")
    }

    /// `class.equals(T.class)` of the class `t` of the program.
    pub(super) fn class_equals(&self, node: Node<'t>, class: Expr, t: usize) -> Result<Expr, Error> {
        let literal = self.class_literal_of(node, t)?;
        let equals = ExprKind::Builtin(Builtin::ClassMethod(ClassOp::Equals), vec![class, literal]);
        Ok(self.synth(equals, node.pos(), Type::Boolean))
    }

    /// `T.class` of the class `t` of the program.
    pub(super) fn class_literal_of(&self, node: Node<'t>, t: usize) -> Result<Expr, Error> {
        let ty = self.ty(node, &self.types[t].javac)?;
        Ok(self.synth(ExprKind::Builtin(Builtin::ClassLiteral(ty), Vec::new()), node.pos(), Type::JavaClass))
    }

    /// `Class.forName(module, name)`: the class of the program with that name, or `null` (see
    /// the module's comment), if it is in the module. With the JDK's sources, the translator's
    /// `TranslatedModules.inModule(module, class)` tells (`lower/modules.rs`); without them, the
    /// program's classes are all one module's, and the module must be `X.class.getModule()`,
    /// which has no effects.
    pub(super) fn module_for_name(&self, node: Node<'t>, name: Option<String>) -> Result<ExprKind, Error> {
        let mut arguments = node.list("arguments");
        let module = arguments.next().expect("forName has a module");
        let name_node = arguments.next().expect("forName has a name");
        let (in_module, signature) = crate::program::IN_MODULE;
        if let Some(resolved) = self.translated_modules(in_module, signature) {
            let module = self.expr(module)?;
            let class = self.class_in_module(node, name, name_node)?;
            let class = self.synth(class, node.pos(), Type::JavaClass);
            let args = self.convert_generic_args(node, Some(resolved), vec![module, class])?;
            return Ok(ExprKind::Call { name: in_module.to_string(), args, resolved: Some(resolved) });
        }
        let of_literal = module.kind() == "METHOD_INVOCATION"
            && module.symbol().is_some_and(|s| (s.owner, s.name) == ("java.lang.Class", "getModule"))
            && module.child("methodSelect")?.get("expression").is_some_and(super::classes::is_literal);
        if !of_literal {
            return unsupported(node, "`Class.forName` in a module other than a class literal's");
        }
        self.class_in_module(node, name, name_node)
    }

    /// The class of the program that `Class.forName(module, name)` finds by the name, if the
    /// module has it: that of a name spelled out, or one of the locale data found by name for a
    /// name that code computes (`Reach::found_by_name`), or `null`.
    fn class_in_module(
        &self,
        node: Node<'t>,
        name: Option<String>,
        name_node: Node<'t>,
    ) -> Result<ExprKind, Error> {
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
        let given = self.fresh_name("given_name");
        let block = vec![
            Stmt::Local { ty: Type::String, name: given.clone(), init: Some(name), pos },
            // `Objects.requireNonNull(name)`, whose exception has no message.
            self.checked_name(&class_name, &given, None, pos),
            Stmt::Yield(switch),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body: block }],
        })
    }

    /// `Class.forName(name)`, or `Class.forName(name, initialize, loader)` with a literal
    /// `initialize`, of a name that code computes: a `switch` over the classes that names in
    /// reachable code find (`Reach::named`), or in a file over its classes that are not local
    /// nor anonymous, by binary name, each initialized if the call says so.
    pub(super) fn computed_for_name(&self, node: Node<'t>) -> Result<ExprKind, Error> {
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
        let given = self.fresh_name("given_name");
        block.push(Stmt::Local { ty: Type::String, name: given.clone(), init: Some(name), pos });
        if let Some(loader) = arguments.get(2) {
            let loader = self.expr(*loader)?;
            if has_effects(&loader) {
                block.push(Stmt::Expr(loader));
            }
        }
        let var = || self.synth(ExprKind::Var(class_name.clone()), pos, Type::String);
        block.push(self.checked_name(&class_name, &given, Some(FOR_NAME_NULL), pos));
        let mut cases = Vec::new();
        let found = |t: usize| match self.reach {
            Some(reach) => reach.named.contains(&self.types[t].javac).then(|| self.types[t].javac.clone()),
            None => self.binary_name(t),
        };
        for (t, name) in (0..self.types.len()).filter_map(|t| found(t).map(|name| (t, name))) {
            let label = self.synth(ExprKind::Str(name), pos, Type::String);
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

    /// The binary name of the class `t` of a file (`Outer$Inner`), which `Class.forName` finds it
    /// by: `None` for a local or anonymous class, a standard class, or one the lowering adds.
    pub(super) fn binary_name(&self, t: usize) -> Option<String> {
        let info = &self.types[t];
        let declared = matches!(info.node.kind(), "CLASS" | "INTERFACE" | "ENUM" | "RECORD");
        if !declared || is_standard(&info.javac) || info.javac.contains('$') || self.plain_object == Some(t) {
            return None;
        }
        let symbol = info.node.symbol()?;
        match self.type_index(symbol.owner) {
            Some(outer) => Some(format!("{}${}", self.binary_name(outer)?, info.node.str("simpleName"))),
            None => Some(info.javac.clone()),
        }
    }

    /// `X.class.isAssignableFrom(c)` of a class or interface `X` of the program that no class of
    /// the runtime extends or implements: whether `c` is one of the program's classes that are or
    /// extend or implement `X`. A class such as `Number` that the runtime's boxes extend is not
    /// supported.
    pub(super) fn is_assignable_from(
        &self,
        node: Node<'t>,
        literal: Node<'t>,
        class: Expr,
    ) -> Result<ExprKind, Error> {
        let named = literal.child("expression")?;
        let target = match self.ty(named, named.java_type())? {
            Type::Class(name, _) => self.type_index(name),
            _ => None,
        };
        let supported = |t: &usize| {
            matches!(self.types[*t].node.kind(), "CLASS" | "INTERFACE")
                && self.types[*t].javac != "java.lang.Number"
        };
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
        let body = vec![
            Stmt::Local { ty: Type::JavaClass, name: local.clone(), init: Some(class), pos },
            Stmt::Yield(chain),
        ];
        Ok(ExprKind::Switch {
            selector: Box::new(self.synth(ExprKind::Int(0), pos, Type::Int)),
            cases: vec![Case { labels: vec![CaseLabel::Default], body }],
        })
    }

    /// `String class_name = given != null ? given : throw new NullPointerException(message);`,
    /// as the JDK's `Class.forName` throws for a null name, with its message or none, when the
    /// program has that class; `class_name` is then never null, so the `switch` over it
    /// does not check it again. Without the class, `String class_name = given;`: the
    /// `switch` throws, with a message that the analysis gives only where it is certain
    /// (`analysis/npe_messages.rs`).
    fn checked_name(&self, class_name: &str, given: &str, message: Option<&str>, pos: Pos) -> Stmt {
        let given = || self.synth(ExprKind::Var(given.to_string()), pos, Type::String);
        let init = match self.null_thrown(message, Type::String, pos) {
            Some(throw) => {
                let null = self.synth(ExprKind::Null, pos, Type::Null);
                let cond = ExprKind::Binary(BinOp::Ne, Box::new(given()), Box::new(null));
                let cond = self.synth(cond, pos, Type::Boolean);
                let kind = ExprKind::Conditional(Box::new(cond), Box::new(given()), Box::new(throw));
                self.synth(kind, pos, Type::String)
            }
            None => given(),
        };
        Stmt::Local { ty: Type::String, name: class_name.to_string(), init: Some(init), pos }
    }

    /// `switch (0) { default -> throw new NullPointerException(message); }`, a value of type `ty`
    /// that is never given, when the program has that class.
    pub(super) fn null_thrown(&self, message: Option<&str>, ty: Type, pos: Pos) -> Option<Expr> {
        const NULL: &str = "java.lang.NullPointerException";
        let sig = if message.is_some() { "(java.lang.String)void" } else { "()void" };
        // A standard class of the translator (`lower::exceptions`), with its constructors, where
        // the program has it.
        let class = self.type_index(NULL)?;
        let ctor =
            self.methods.iter().position(|m| m.owner == class && m.name == "<init>" && m.sig == sig)?;
        let args = message
            .map(|m| self.synth(ExprKind::Str(m.to_string()), pos, Type::String))
            .into_iter()
            .collect();
        let new = self.synth(ExprKind::New { class, args, ctor: Some(ctor) }, pos, Type::class(NULL));
        let throw = Stmt::Throw(self.convert(new, Type::class("java.lang.Throwable")));
        let selector = Box::new(self.synth(ExprKind::Int(0), pos, Type::Int));
        let cases = vec![Case { labels: vec![CaseLabel::Default], body: vec![throw] }];
        Some(self.synth(ExprKind::Switch { selector, cases }, pos, ty))
    }
}
