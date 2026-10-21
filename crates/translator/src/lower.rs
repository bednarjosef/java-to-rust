//! Lowers the syntax tree that javac printed (see `java/JavaTree.java`) into the typed syntax
//! tree of `ast`. javac has already checked the program, given every expression its type and
//! resolved every name and call, so lowering only maps them onto the supported subset. Anything
//! outside it is reported as an error naming the construct.

mod code_point;
mod covariant;
mod enum_nulls;
mod enum_tables;
mod enums;
mod exceptions;
mod foreach;
mod functional;
mod generic_args;
mod generics;
mod handles;
mod hierarchy;
mod lambdas;
mod local_names;
mod object;
mod object_clone;
mod objects;
mod real_generics;
mod references;
mod statics;
mod strings;
mod stubs;
mod suppress;
mod unsigned;

use crate::ast::*;
use objects::{MethodInfo, TypeInfo};
use serde_json::Value;
use std::cell::{Cell, RefCell};

mod arrays;
mod boxing;
mod by_name;
mod captures;
mod census;
mod charsets;
mod classes;
mod clinit_errors;
mod constant_bodies;
mod initializers;
mod inner;
mod java_arrays;
mod java_objects;
mod local_types;
mod natives;
mod patterns;
mod primitives;
mod program;
mod prologues;
mod switch;
mod sync;
mod system;
mod varargs;
pub use arrays::runtime_arrays_call;
use arrays::{is_array_length, reads_name};
pub use captures::raw as raw_type;
pub use census::panic_message;
pub(crate) use clinit_errors::rewritten_clinit;
pub use exceptions::{is_standard, standard_local_name, standard_names, standard_subclass};
pub use functional::is_functional;
pub use lambdas::computed_receiver;
pub use primitives::runtime_character_method;
pub use program::{Lowered, census_lower_program, lower_program};

/// Checks that no enum of the program can be null: the translation has no null enums.
pub fn check_enum_nulls(class: &Class) -> Result<(), Error> {
    enums::check_nulls(class)
}

pub fn lower(unit: &Value) -> Result<Class, Error> {
    let unit = Node(unit);
    charsets::check(unit)?;
    // A `;` at top level and an annotation type (`@interface`) do nothing when the program runs.
    let decls: Vec<Node> = unit
        .list("typeDecls")
        .filter(|n| !matches!(n.kind(), "EMPTY_STATEMENT" | "ANNOTATION_TYPE"))
        .collect();
    let Some(first) = decls.first().copied() else { return error(unit.pos(), "no class found") };
    real_generics::retry(|real, covariant| {
        let mut lowering = Lowering::declare(&decls)?;
        lowering.covariant = covariant.clone();
        lowering.standard = exceptions::Standard::select(decls.iter().map(|d| d.0));
        lowering.declare_standard()?;
        let declared = lowering.types.iter().map(|t| t.name.clone()).collect();
        // The standard classes may name interfaces too (`Thread` holds a `Runnable`).
        let trees = decls.iter().map(|d| d.0).chain(lowering.standard.classes.iter().map(|(_, d)| *d));
        lowering.functional = functional::Functional::select(trees, &declared);
        lowering.declare_functional()?;
        lowering.declare_plain_object(decls.iter().map(|d| d.0))?;
        lowering.name_nested();
        lowering.plan_captures().map_err(|(_, e)| e)?;
        lowering.real = real.cloned().unwrap_or_else(|| lowering.candidates());
        let planned: Vec<Node> = decls.iter().copied().chain(lowering.functional_decls()).collect();
        lowering.plan_object(&planned);
        let mut types: Vec<TypeDecl> =
            (0..lowering.types.len()).map(|t| lowering.type_decl(t)).collect::<Result<_, _>>()?;
        let mut methods: Vec<Method> =
            (0..lowering.methods.len()).map(|m| lowering.method(m)).collect::<Result<_, _>>()?;
        lowering.suppress_checks(&mut methods);
        lowering.object_methods(&mut methods)?;
        lowering.finish_types(&mut types, &mut methods).map_err(|(_, e)| e)?;
        lowering.object_types(&mut types);
        lowering.param_decls(&mut types);
        let failed = lowering.verify(&types, &methods);
        let hierarchy = Hierarchy::new(&types, &methods);
        let raised = lowering.raised_objects(&types, &methods);
        let name = first.str("simpleName").to_string();
        let class = Class { name, types, methods, hierarchy, raised };
        enums::check_nulls(&class)?;
        Ok((class, lowering.real.clone(), failed))
    })
}

/// A node of javac's tree.
#[derive(Clone, Copy)]
struct Node<'t>(&'t Value);

/// The declaration a name, call or declaration refers to.
struct Symbol<'t> {
    kind: &'t str,
    name: &'t str,
    owner: &'t str,
    is_static: bool,
    /// Whether a method or constructor takes a variable number of arguments.
    varargs: bool,
    /// Whether a method is `native`: the JVM implements it (see `natives.rs`).
    native: bool,
    /// The declared type, such as `int` or, for a method, `(int,long)double`.
    ty: &'t str,
    /// The value of a compile-time constant variable, as text (`Character`s as their code unit).
    constant: Option<&'t str>,
}

impl<'t> Node<'t> {
    fn kind(self) -> &'t str {
        self.0["kind"].as_str().unwrap_or("")
    }

    fn pos(self) -> Pos {
        let at = |f: &str| self.0[f].as_u64().unwrap_or(0) as usize;
        Pos { line: at("line"), column: at("column") }
    }

    fn get(self, field: &str) -> Option<Node<'t>> {
        let v = &self.0[field];
        (!v.is_null()).then_some(Node(v))
    }

    fn child(self, field: &str) -> Result<Node<'t>, Error> {
        self.get(field).ok_or_else(|| Error {
            pos: self.pos(),
            message: format!("malformed javac tree: {} without {field}", self.kind()),
        })
    }

    fn list(self, field: &str) -> impl Iterator<Item = Node<'t>> {
        self.0[field].as_array().into_iter().flatten().map(Node)
    }

    fn str(self, field: &str) -> &'t str {
        self.0[field].as_str().unwrap_or("")
    }

    fn java_type(self) -> &'t str {
        self.str("javaType")
    }

    fn symbol(self) -> Option<Symbol<'t>> {
        let s = self.get("symbol")?;
        Some(Symbol {
            kind: s.str("kind"),
            name: s.str("name"),
            owner: s.str("owner"),
            is_static: s.0["static"].as_bool().unwrap_or(false),
            varargs: s.0["varargs"].as_bool().unwrap_or(false),
            native: s.0["native"].as_bool().unwrap_or(false),
            ty: s.str("type"),
            constant: s.0.get("constant").and_then(Value::as_str),
        })
    }

    /// Whether the declaration has a modifier. `STATIC` comes from the declaration's symbol when it
    /// has one, which also knows the implicit modifier of an interface's fields and nested types.
    fn has_flag(self, flag: &str) -> bool {
        if flag == "STATIC"
            && let Some(s) = self.get("symbol").and_then(|s| s.0["static"].as_bool())
        {
            return s;
        }
        self.get("modifiers")
            .is_some_and(|m| m.0["flags"].as_array().is_some_and(|f| f.iter().any(|x| x == flag)))
    }
}

/// What the rejection of an `Arrays.sort` or `Arrays.fill` of a `String[]` that is an `Object[]`
/// says, where the runtime would do it (see `Reach::string_array_writes`).
pub const STRING_ARRAY_WRITE: &str = "a change in place of a `String[]` that is also an `Object[]`";

impl<'t> Lowering<'t> {
    /// `a == b` or `a != b` of two strings, which compares references. A translated string is a
    /// value without an identity, so the program's own code is rejected. The JDK compares by
    /// reference only strings it interned (the parts of a `Locale`, `Charset`'s canonical names)
    /// or as a shortcut before comparing the characters (`LocaleUtils.caseIgnoreMatch`), and no
    /// code of it that the targets reach makes a string with an identity of its own to tell it
    /// apart (a `new String` kept as a marker): there, the reference comparison gives what value
    /// equality does, which is how the library's code is lowered (`Objects.equals`).
    fn library_string_comparison(
        &self,
        node: Node<'t>,
        op: BinOp,
        left: Expr,
        right: Expr,
    ) -> Result<ExprKind, Error> {
        if !self.library.get(self.current.get()).copied().unwrap_or(false) {
            return error(node.pos(), "== on strings compares references; not supported yet");
        }
        let equal = ExprKind::Builtin(Builtin::ObjectsEquals, vec![left, right]);
        Ok(match op {
            BinOp::Eq => equal,
            _ => ExprKind::Unary(UnOp::Not, Box::new(self.synth(equal, node.pos(), Type::Boolean))),
        })
    }
}

fn unsupported<T>(node: Node, what: &str) -> Result<T, Error> {
    error(node.pos(), format!("{what} not supported yet"))
}

/// `ENHANCED_FOR_LOOP` as `enhanced for loop`.
/// Whether javac's type `name` is an instantiation of the generic class `class`, such as
/// `java.lang.Enum<?>` (and not an array of one, `java.lang.Enum<?>[]`).
fn generic_of(name: &str, class: &str) -> bool {
    name.strip_prefix(class).is_some_and(|rest| rest.starts_with('<') && rest.ends_with('>'))
}

fn kind_words(kind: &str) -> String {
    kind.to_lowercase().replace('_', " ")
}

struct Lowering<'t> {
    next_id: Cell<ExprId>,
    /// Index of the class whose members are being lowered: `this` and unqualified names refer to it.
    current: Cell<usize>,
    /// The method being lowered, whose type variables are in scope.
    method_ctx: Cell<Option<usize>>,
    /// The class and method whose type variables a declared type is read in, when that is not
    /// the code being lowered (see `generics::declared_type`).
    scope: Cell<Option<(usize, Option<usize>)>>,
    /// Whether the method being lowered is a constructor, which cannot `return` early.
    in_ctor: Cell<bool>,
    /// The return type of the method being lowered, which a returned value is converted to.
    ret: Cell<Type>,
    /// Every class of the file, in the order of `Class::types`.
    types: Vec<TypeInfo<'t>>,
    /// Every method and constructor of every class, in the order of `Class::methods`.
    methods: Vec<MethodInfo<'t>>,
    /// In a whole program, what is reachable from `main`: only that is lowered.
    reach: Option<&'t crate::program::Reach>,
    /// In a whole program, the class whose `main` is the entry point; in a file, every `main` is.
    entry: Option<usize>,
    /// The standard exception classes that the program uses (see `exceptions`).
    standard: exceptions::Standard,
    /// The array types whose arrays become `Object`s or are cast back from them: each is a
    /// runtime class beside `Object` (see `object::object_types`).
    object_arrays: std::cell::RefCell<Vec<Type>>,
    /// The atomic updates of fields through `Unsafe` that the code makes, each a method that
    /// lowering adds to the field's class (see `handles`).
    field_updates: RefCell<Vec<handles::FieldUpdateMethod>>,
    /// The arrays of references that are `Object[]`s (see `covariant`).
    covariant: covariant::Covariant,
    /// Where `Object` goes, in a program that uses it (see `object`).
    object: Option<object::ObjectIndexes>,
    /// The class of the objects that `new Object()` creates, when the code has one (see `object`).
    plain_object: Option<usize>,
    /// The generic classes and methods that are Rust generics (see `real_generics`).
    real: real_generics::Real,
    /// Those of `real` that lowering found cannot be: the program is lowered again without them.
    failed: RefCell<real_generics::Real>,
    /// The pattern variables in scope that name a value without testing it (see `patterns`).
    aliases: RefCell<Vec<patterns::Alias>>,
    /// The JDK's functional interfaces that the program uses and lowering provides (see `functional`).
    functional: functional::Functional,
    /// The lambdas and method references of the code being lowered (see `lambdas`).
    lambdas: Vec<lambdas::LambdaInfo<'t>>,
    /// The identifiers that read a local captured by the class whose code they are in, by
    /// address (see `captures`).
    capture_refs: std::collections::HashSet<*const Value>,
    /// The `new` expressions, by address, with the captures of the created class that are
    /// captures of the creating class there (see `captures`).
    capture_sites: std::collections::HashSet<(*const Value, String)>,
    /// The member classes of local and anonymous classes, each with its outer class, by index:
    /// they are named after that class (see `inner::name_nested`).
    member_outers: Vec<(usize, usize)>,
    /// In a whole program, whether each class, by index, is the library's (the JDK's) rather than
    /// the program's own; a class without an entry is the program's.
    library: Vec<bool>,
    /// In a census (see `census`), the code that lowering rejected, each error with the index of
    /// the type whose code it is in and the position of the method being lowered, and lowering
    /// goes on past it; `None` otherwise, when the first error ends lowering.
    census: Option<RefCell<Vec<CensusError>>>,
    /// Whether the method being lowered is a census's stub of one that lowering rejected.
    stubbing: Cell<bool>,
    /// The methods that are a census's stubs.
    stubbed: RefCell<std::collections::BTreeSet<usize>>,
}

/// An error that lowering for a census records: the index of the type whose code it is in, the
/// position of the method being lowered, and the error.
type CensusError = (usize, Option<Pos>, Error);

impl<'t> Lowering<'t> {
    /// What lowering adds once every declared method is lowered, for a file and a whole program
    /// alike: the `<clinit>` of each class with static initialization, and each enum's constant
    /// bodies and constants. An error comes with the index of the type it is in.
    fn finish_types(&self, types: &mut [TypeDecl], methods: &mut Vec<Method>) -> Result<(), (usize, Error)> {
        for index in (0..types.len()).filter(|&t| self.has_static_init(t)) {
            let mut clinit = self.clinit(index).map_err(|e| (index, e))?;
            self.guard_clinit(index, &mut clinit, &mut types[index], methods.as_slice());
            methods.push(clinit);
        }
        for index in (0..types.len()).filter(|&t| self.types[t].node.kind() == "ENUM") {
            if let Err(e) = self.finish_enum(index, types, methods) {
                // In a census, an enum that lowering rejects keeps its constants without values.
                self.reject_at(index, Some(self.types[index].node.pos()), e).map_err(|e| (index, e))?;
                let names = self.enum_constant_names(index);
                let constants = names.map(|name| EnumConstant { name: name.to_string(), values: Vec::new() });
                types[index].enum_constants = Some(constants.collect());
            }
        }
        self.field_update_methods(methods);
        Ok(())
    }

    /// The constant bodies and the constants of enum `index`.
    fn finish_enum(
        &self,
        index: usize,
        types: &mut [TypeDecl],
        methods: &mut Vec<Method>,
    ) -> Result<(), Error> {
        self.constant_bodies(index, methods)?;
        let bodies = self.constant_body_fields(index)?;
        let own_fields = types[index].fields.len();
        let pure = self.check_enum(index, methods).and_then(|()| {
            self.add_body_fields(&bodies, &mut types[index].fields)?;
            self.enum_constants(index, &types[index].fields, methods, &bodies)
        });
        // An enum whose constructors do more than the pure form allows has tables instead,
        // when its constants' bodies declare no fields.
        let constants = match pure {
            Ok(constants) => constants,
            Err(e) if bodies.iter().any(|b| !b.is_empty()) => return Err(e),
            Err(_) => {
                types[index].fields.truncate(own_fields);
                self.enum_tables(index, types, methods)?;
                let names = self.enum_constant_names(index);
                names.map(|name| EnumConstant { name: name.to_string(), values: Vec::new() }).collect()
            }
        };
        types[index].enum_constants = Some(constants);
        Ok(())
    }

    fn method(&self, index: usize) -> Result<Method, Error> {
        let outer = self.method_ctx.replace(Some(index));
        let method = self.method_in_scope(index);
        self.method_ctx.set(outer);
        let mut method = method?;
        local_names::distinct_locals(&mut method);
        Ok(method)
    }

    fn method_in_scope(&self, index: usize) -> Result<Method, Error> {
        if let Some(method) = self.lambda_member(index) {
            return method;
        }
        let info = &self.methods[index];
        let node = info.node;
        self.current.set(info.owner);
        self.in_ctor.set(node.str("name") == "<init>");
        let is_ctor = node.str("name") == "<init>";
        let is_static = node.has_flag("STATIC");
        let name = node.str("name").to_string();
        let ret = if is_ctor {
            Type::Void
        } else {
            let ret_node = node.child("returnType")?;
            self.signature_type(ret_node, ret_node.java_type())?
        };
        self.ret.set(ret);
        let params: Vec<Node> = node.list("parameters").collect();
        let param_type = |p: Node<'t>| p.symbol().map_or("", |s| s.ty);
        let is_main = name == "main"
            && is_static
            && ret == Type::Void
            && params.len() == 1
            && param_type(params[0]) == "java.lang.String[]"
            && self.entry.is_none_or(|entry| entry == info.owner);
        let args_param = is_main.then(|| params[0]);
        let params = if is_main {
            Vec::new()
        } else {
            params
                .into_iter()
                .map(|p| {
                    Ok(Param { name: p.str("name").to_string(), ty: self.signature_type(p, param_type(p))? })
                })
                .collect::<Result<_, Error>>()?
        };
        // An abstract method of an enum is implemented by the constants' bodies, which
        // `constant_bodies` makes its body.
        let abstract_in_enum =
            node.has_flag("ABSTRACT") && self.enum_index(&self.types[info.owner].name).is_some();
        let in_interface = self.types[info.owner].node.kind() == "INTERFACE";
        let is_abstract = node.get("body").is_none()
            && (node.has_flag("ABSTRACT") || in_interface && !is_static)
            && !abstract_in_enum;
        let dormant = self.reach.is_some_and(|r| r.dormant(&self.types[info.owner].javac, node.0));
        let mut body = match node.get("body") {
            Some(_) if dormant || self.stubbing.get() => self.never(info.owner, node.pos()),
            Some(body) => self.block(body)?,
            None if abstract_in_enum || is_abstract => Vec::new(),
            None => return unsupported(node, "methods without a body"),
        };
        let params = if is_ctor && self.stubbing.get() {
            let (first, last) = self.synthetic_params(info.owner)?;
            first.into_iter().chain(params).chain(last).collect()
        } else if is_ctor {
            self.constructor_prologue(node, info.owner, &mut body)?;
            self.run_initializer_blocks(info.owner, &mut body)?;
            self.start_constructor(info.owner, &mut body)?;
            let (first, last) = self.synthetic_params(info.owner)?;
            first.into_iter().chain(params).chain(last).collect()
        } else {
            params
        };
        if let Some(param) = args_param.filter(|p| reads_name(&body, p.str("name"))) {
            body.insert(0, self.args_decl(param));
        }
        Ok(Method {
            name,
            params,
            ret,
            body,
            is_main,
            is_static,
            is_ctor,
            class: info.owner,
            pos: node.pos(),
            is_abstract,
            is_synchronized: node.has_flag("SYNCHRONIZED"),
            overrides: self.overrides(node),
            type_params: self.method_type_params(index),
        })
    }

    /// The supported type named by javac's `name` for it.
    fn ty(&self, node: Node, name: &str) -> Result<Type, Error> {
        if let Some(boxed) = boxing::boxed_type(name) {
            return Ok(boxed);
        }
        match name {
            "int" => Ok(Type::Int),
            "long" => Ok(Type::Long),
            "double" => Ok(Type::Double),
            "boolean" => Ok(Type::Boolean),
            "void" => Ok(Type::Void),
            "java.lang.String" => Ok(Type::String),
            "java.lang.StringBuilder" | "java.lang.StringBuffer" => Ok(Type::StringBuilder),
            // An array of these (`Class<?>[]`, `Enum<?>[]`) is an array type, below.
            _ if name == "java.lang.Class" || generic_of(name, "java.lang.Class") => Ok(Type::JavaClass),
            // A value of one of the program's enums (see `enums::enum_call`).
            _ if (name == "java.lang.Enum" || generic_of(name, "java.lang.Enum"))
                && self.object.is_some_and(|at| at.enumeration.is_some()) =>
            {
                Ok(Type::class(ENUM))
            }
            "java.nio.charset.Charset" => Ok(Type::Charset),
            "byte" => Ok(Type::Byte),
            "short" => Ok(Type::Short),
            "char" => Ok(Type::Char),
            "float" => Ok(Type::Float),
            // A type variable hides a class of the same name.
            _ if let Some(erased) = self.type_variable(node, name) => erased,
            // The declared name, which can differ from javac's (a hoisted enum, see `local_types`).
            _ if let Some(e) = self.enum_index(name) => Ok(Type::enum_type(&self.types[e].name)),
            _ if let Some(t) = self.type_index(name) => {
                // A Rust generic used without type arguments.
                if self.is_real(real_generics::Owner::Class(t)) && !self.class_type_params(t).is_empty() {
                    self.fail(real_generics::Owner::Class(t));
                }
                Ok(Type::class(&self.types[t].name))
            }
            "<nulltype>" => Ok(Type::Null),
            // `this` in the body of an enum constant, which is the constant.
            _ if name.starts_with("<anonymous ") && self.enum_index(&name[11..name.len() - 1]).is_some() => {
                Ok(Type::enum_type(&name[11..name.len() - 1]))
            }
            _ if name.ends_with("[]") => {
                Ok(self.covariant_array(Type::array(self.ty(node, &name[..name.len() - 2])?)))
            }
            "java.lang.Object" if self.object.is_some() => Ok(Type::class(OBJECT)),
            "java.lang.Comparable" if self.object.is_some_and(|at| at.comparable.is_some()) => {
                Ok(Type::class(COMPARABLE))
            }
            "java.lang.Number" if self.object.is_some_and(|at| at.number.is_some()) => {
                Ok(Type::class(NUMBER))
            }
            "java.lang.CharSequence" if self.object.is_some_and(|at| at.char_sequence.is_some()) => {
                Ok(Type::class(CHAR_SEQUENCE))
            }
            _ if let Some(erased) = self.erased(node, name) => erased,
            // A `...` parameter has the array type here, so `Object...` is rejected as `Object[]`.
            "java.lang.Object" => unsupported(node, "Object"),
            _ => unsupported(node, &format!("type `{name}`")),
        }
    }

    fn block(&self, node: Node<'t>) -> Result<Vec<Stmt>, Error> {
        let mut out = Vec::new();
        for stmt in node.list("statements") {
            if is_super_call(stmt) {
                out.extend(self.super_init(stmt)?);
                continue;
            }
            self.stmt_into(stmt, &mut out)?;
        }
        Ok(out)
    }

    /// A statement in a position that takes one statement, such as the body of an `if`.
    fn single_stmt(&self, node: Node<'t>) -> Result<Stmt, Error> {
        let mut out = Vec::new();
        self.stmt_into(node, &mut out)?;
        Ok(if out.len() == 1 { out.pop().unwrap() } else { Stmt::Block(out) })
    }

    fn stmt_into(&self, node: Node<'t>, out: &mut Vec<Stmt>) -> Result<(), Error> {
        let stmt = match node.kind() {
            "BLOCK" => Stmt::Block(self.block(node)?),
            "EMPTY_STATEMENT" => Stmt::Block(Vec::new()),
            // Assertions are disabled by default in Java, so an `assert` does nothing at all.
            "ASSERT" => return Ok(()),
            "SYNCHRONIZED" => return self.synchronized(node, out),
            "VARIABLE" => {
                let ty = self.ty(node, node.symbol().map_or("", |s| s.ty))?;
                let init = match node.get("initializer") {
                    Some(v) => Some(self.convert(self.expr(v)?, ty)),
                    None => None,
                };
                Stmt::Local { ty, name: node.str("name").to_string(), init, pos: node.pos() }
            }
            "EXPRESSION_STATEMENT" if let Some(set_all) = self.set_all(node)? => set_all,
            "EXPRESSION_STATEMENT" => match self.expr(node.child("expression")?)? {
                // A native that does nothing here, such as `registerNatives()`, leaves no trace.
                Expr { kind: ExprKind::Builtin(Builtin::Native(Native::Nothing), _), .. } => return Ok(()),
                e => Stmt::Expr(e),
            },
            // Reachability skipped the other branch (`program::live_branch`).
            "IF" if let Some(branch) = self.live_branch(node) => match branch {
                Some(n) => self.single_stmt(n)?,
                None => Stmt::Block(Vec::new()),
            },
            "IF" => Stmt::If {
                cond: self.expr(node.child("condition")?)?,
                then: Box::new(self.single_stmt(node.child("thenStatement")?)?),
                otherwise: match node.get("elseStatement") {
                    Some(n) => Some(Box::new(self.single_stmt(n)?)),
                    None => None,
                },
            },
            "WHILE_LOOP" => Stmt::While {
                cond: self.expr(node.child("condition")?)?,
                body: Box::new(self.single_stmt(node.child("statement")?)?),
            },
            "DO_WHILE_LOOP" => Stmt::DoWhile {
                body: Box::new(self.single_stmt(node.child("statement")?)?),
                cond: self.expr(node.child("condition")?)?,
            },
            "FOR_LOOP" => {
                let mut init = Vec::new();
                for n in node.list("initializer") {
                    self.stmt_into(n, &mut init)?;
                }
                let cond = match node.get("condition") {
                    Some(n) => Some(self.expr(n)?),
                    None => None,
                };
                let update = node
                    .list("update")
                    .map(|n| self.expr(n.child("expression")?))
                    .collect::<Result<_, _>>()?;
                let body = Box::new(self.single_stmt(node.child("statement")?)?);
                Stmt::For { init, cond, update, body }
            }
            "RETURN" => match node.get("expression") {
                Some(value) => {
                    Stmt::Return(Some(self.convert(self.expr(value)?, self.ret.get())), node.pos())
                }
                None => Stmt::Return(None, node.pos()),
            },
            "BREAK" | "CONTINUE" => switch::jump(node),
            "YIELD" => Stmt::Yield(self.expr(node.child("value")?)?),
            "ENHANCED_FOR_LOOP" => self.for_each(node, None)?,
            "SWITCH" => self.switch_stmt(node)?,
            "TRY" => self.try_stmt(node)?,
            "THROW" => self.throw(node)?,
            "LABELED_STATEMENT" => self.labeled(node)?,
            // A local class is declared with the classes (see `inner`).
            "CLASS" | "INTERFACE" | "ENUM" | "RECORD" => return Ok(()),
            kind => return unsupported(node, &kind_words(kind)),
        };
        out.push(stmt);
        Ok(())
    }

    fn expr(&self, node: Node<'t>) -> Result<Expr, Error> {
        if let Some(code) = self.reference_native(node)? {
            return Ok(code);
        }
        let kind = match node.kind() {
            _ if charsets::is_charset_op(node) => self.charset_op(node)?,
            "PARENTHESIZED" => return self.expr(node.child("expression")?),
            "INT_LITERAL" => ExprKind::Int(self.number(node)?),
            "LONG_LITERAL" => ExprKind::Long(self.number(node)?),
            "DOUBLE_LITERAL" => ExprKind::Double(self.number(node)?),
            "BOOLEAN_LITERAL" => ExprKind::Bool(node.0["value"] == true),
            "STRING_LITERAL" => match node.0["value"].as_str() {
                Some(text) => ExprKind::Str(text.to_string()),
                None => return unsupported(node, "a string with an unpaired surrogate"),
            },
            "CHAR_LITERAL" => self.char_literal(node)?,
            "FLOAT_LITERAL" => self.float_literal(node)?,
            "NULL_LITERAL" => ExprKind::Null,
            "IDENTIFIER" => match self.alias(node) {
                Some(value) => value,
                None => self.name(node)?,
            },
            // javac folds the minus sign into integer literals only; fold it into doubles too.
            "UNARY_MINUS" if node.child("expression")?.kind() == "DOUBLE_LITERAL" => {
                ExprKind::Double(-self.number::<f64>(node.child("expression")?)?)
            }
            "UNARY_MINUS" if node.child("expression")?.kind() == "FLOAT_LITERAL" => {
                ExprKind::Float(-self.number::<f32>(node.child("expression")?)?)
            }
            kind @ ("UNARY_MINUS" | "UNARY_PLUS" | "LOGICAL_COMPLEMENT" | "BITWISE_COMPLEMENT") => {
                let op = match kind {
                    "UNARY_MINUS" => UnOp::Neg,
                    "UNARY_PLUS" => UnOp::Plus,
                    "LOGICAL_COMPLEMENT" => UnOp::Not,
                    _ => UnOp::BitNot,
                };
                ExprKind::Unary(op, Box::new(self.expr(node.child("expression")?)?))
            }
            kind @ ("PREFIX_INCREMENT" | "PREFIX_DECREMENT" | "POSTFIX_INCREMENT" | "POSTFIX_DECREMENT") => {
                ExprKind::IncDec {
                    target: self.target(node.child("expression")?)?,
                    increment: kind.ends_with("INCREMENT"),
                    prefix: kind.starts_with("PREFIX"),
                }
            }
            "ASSIGNMENT" => {
                // A field of a type variable holds its erasure.
                let variable = node.child("variable")?;
                let to = match self.field_type(variable)? {
                    Some(declared) => declared,
                    None => self.ty(node, node.java_type())?,
                };
                let target = self.target(variable)?;
                // An element of a covariant array is an `Object` (see `covariant`).
                let to = match &target {
                    Place::Index(array, _) => match array.ty {
                        Type::Array(element) if Self::is_object(*element) => *element,
                        _ => to,
                    },
                    _ => to,
                };
                ExprKind::Assign {
                    op: None,
                    target,
                    value: Box::new(self.convert(self.expr(node.child("expression")?)?, to)),
                }
            }
            kind if kind.ends_with("_ASSIGNMENT") => {
                let op = binary_op(node, kind.trim_end_matches("_ASSIGNMENT"))?;
                let value = self.expr(node.child("expression")?)?;
                ExprKind::Assign {
                    op: Some(op),
                    target: self.target(node.child("variable")?)?,
                    value: Box::new(value),
                }
            }
            "TYPE_CAST" if let Some(name) = crate::program::created_by_name(node.0) => {
                self.created_by_name(node, Node(name))?
            }
            "TYPE_CAST" if arrays::casts_component_array(node) => self.component_array(node)?,
            "TYPE_CAST" => {
                let ty = self.ty(node, node.java_type())?;
                let value = self.expr(node.child("expression")?)?;
                if Self::is_object(value.ty) {
                    self.object_array(node, ty)?;
                }
                // `(T[]) objects` or `(Object[]) versions`.
                if value.ty != ty {
                    self.meets_object_array(value.ty, ty);
                }
                // A cast between the program's types, up, down or across: `Cast` converts as the
                // implicit upcasts do, and checks a downcast. `(int) o` on an `Object` casts to
                // `Integer` and unboxes. `(C) super.clone()` in `C` is the copy itself, which has
                // `C`'s type: it is not converted to `Object` and back.
                if Self::is_clone(&value) {
                    self.cast_clone(value, ty)
                } else if Self::is_object(value.ty) && (ty.is_numeric() || ty == Type::Boolean) {
                    let boxed = self.synth(
                        ExprKind::Cast(Type::boxed(ty), Box::new(value)),
                        node.pos(),
                        Type::boxed(ty),
                    );
                    ExprKind::Cast(ty, Box::new(boxed))
                } else {
                    ExprKind::Cast(ty, Box::new(value))
                }
            }
            // As with `if`, reachability skipped the branch that the condition guards.
            "CONDITIONAL_EXPRESSION" if let Some(Some(branch)) = self.live_branch(node) => {
                let ty = self.ty(node, node.java_type())?;
                let value = self.convert(self.expr(branch)?, ty);
                // The branch converts to the type of the whole, as `cond ? 1 : 2L` is a `long`.
                if value.ty == ty {
                    return Ok(value);
                }
                ExprKind::Cast(ty, Box::new(value))
            }
            "CONDITIONAL_EXPRESSION" => {
                let ty = self.ty(node, node.java_type())?;
                ExprKind::Conditional(
                    Box::new(self.expr(node.child("condition")?)?),
                    Box::new(self.convert(self.expr(node.child("trueExpression")?)?, ty)),
                    Box::new(self.convert(self.expr(node.child("falseExpression")?)?, ty)),
                )
            }
            "METHOD_INVOCATION" => self.call(node)?,
            "NEW_CLASS" => match self.new_text(node)? {
                Some(kind) => kind,
                None => self.new_object(node)?,
            },
            "NEW_ARRAY" => self.new_array(node)?,
            "ARRAY_ACCESS" => {
                let (array, index) = self.index(node)?;
                ExprKind::Index(Box::new(array), Box::new(index))
            }
            "MEMBER_SELECT" if classes::is_literal(node) => self.class_literal(node)?,
            "MEMBER_SELECT" if is_array_length(node) => {
                ExprKind::Length(Box::new(self.expr(node.child("expression")?)?))
            }
            "MEMBER_SELECT" => match self.constant_field(node).or_else(|| self.boolean_constant(node)) {
                Some(constant) => constant,
                None => self.select(node)?,
            },
            "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE" => self.lambda(node)?,
            "INSTANCE_OF" => self.instance_of(node)?,
            "SWITCH_EXPRESSION" => self.switch_expr(node)?,
            "EQUAL_TO" | "NOT_EQUAL_TO" if self.compares_classes(node) => self.class_comparison(node)?,
            "EQUAL_TO" | "NOT_EQUAL_TO" if charsets::compares_charsets(node) => {
                self.charset_comparison(node)?
            }
            kind => {
                let left = node.child("leftOperand");
                let op = binary_op(node, kind);
                let (Ok(left), Ok(op)) = (left, op) else { return unsupported(node, &kind_words(kind)) };
                let (left, right) = (self.expr(left)?, self.expr(node.child("rightOperand")?)?);
                if left.ty == Type::String && right.ty == Type::String && matches!(op, BinOp::Eq | BinOp::Ne)
                {
                    self.library_string_comparison(node, op, left, right)?
                } else {
                    // An array's text is its type and identity hash, such as `[I@1b6d3586`.
                    if op == BinOp::Add
                        && (left.ty == Type::String || right.ty == Type::String)
                        && (matches!(left.ty, Type::Array(_)) || matches!(right.ty, Type::Array(_)))
                    {
                        return unsupported(node, "the text of an array");
                    }
                    let (left, right) = if matches!(op, BinOp::Eq | BinOp::Ne) {
                        self.convert_operands(left, right)
                    } else {
                        (left, right)
                    };
                    ExprKind::Binary(op, Box::new(left), Box::new(right))
                }
            }
        };
        // `printf` returns its stream, which the translation has no type for; it is a statement.
        let ty = match kind {
            ExprKind::Builtin(Builtin::Print, _) if node.kind() == "METHOD_INVOCATION" => Type::Void,
            _ => self.ty(node, node.java_type())?,
        };
        let kind = match node.kind() {
            "METHOD_INVOCATION" | "IDENTIFIER" | "MEMBER_SELECT" | "ARRAY_ACCESS" => {
                self.erased_use(node, kind, ty)?
            }
            _ => kind,
        };
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        Ok(Expr { kind, pos: node.pos(), ty, id })
    }

    fn number<T: std::str::FromStr>(&self, node: Node) -> Result<T, Error> {
        let text = node.str("value");
        text.parse().or_else(|_| error(node.pos(), format!("malformed literal {text}")))
    }

    /// The name of the local variable or parameter that an identifier refers to.
    fn local(&self, node: Node) -> Result<String, Error> {
        match node.symbol() {
            Some(s)
                if matches!(
                    s.kind,
                    "LOCAL_VARIABLE"
                        | "PARAMETER"
                        | "EXCEPTION_PARAMETER"
                        | "BINDING_VARIABLE"
                        | "RESOURCE_VARIABLE"
                ) =>
            {
                Ok(node.str("name").to_string())
            }
            _ => unsupported(node, &format!("the name `{}` here", node.str("name"))),
        }
    }

    /// What `=`, `+=`, `++` and the like assign.
    fn target(&self, node: Node<'t>) -> Result<Place, Error> {
        if let Some(place) = self.field_place(node)? {
            return Ok(place);
        }
        match node.kind() {
            "IDENTIFIER" if node.symbol().is_some_and(|s| s.kind == "BINDING_VARIABLE") => {
                unsupported(node, "assigning a pattern variable")
            }
            "IDENTIFIER" => Ok(Place::Var(self.local(node)?)),
            "ARRAY_ACCESS" => {
                let (array, index) = self.index(node)?;
                Ok(Place::Index(Box::new(array), Box::new(index)))
            }
            _ => unsupported(node, "assignment to anything but a variable, field or array element"),
        }
    }

    fn call(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let select = node.child("methodSelect")?;
        let Some(symbol) = node.symbol() else { return unsupported(node, "this call") };
        if let Some(kind) = self.handle_access(node, &symbol)? {
            return Ok(kind);
        }
        if let Some(kind) = self.class_literal_call(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.class_call(node, &symbol, select)? {
            return Ok(kind);
        }
        if let Some(kind) = self.copy_of_with_class(node, &symbol)? {
            return Ok(kind);
        }
        let args: Vec<Expr> = node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let mut args = self.packed_args(node, &symbol, args)?;
        if select.kind() == "IDENTIFIER" && select.str("name") == "this" {
            let resolved = self.method_index(&symbol);
            let args = self.convert_args(resolved, args)?;
            let args = self.with_synthetic(self.passed_on(self.current.get(), node.pos())?, args);
            return Ok(ExprKind::ThisCall { args, resolved });
        }
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        if let Some(kind) = self.monitor_call(node, &symbol, receiver, &mut args)? {
            return Ok(kind);
        }
        if symbol.owner == "java.lang.Object" && symbol.name == "clone" {
            return self.object_clone(node, receiver);
        }
        if symbol.owner == "java.lang.Object" && self.object.is_some() {
            return self.object_call(node, &symbol, receiver, args);
        }
        if symbol.owner == "java.lang.Comparable" && symbol.name == "compareTo" {
            return self.comparable_call(node, receiver, args);
        }
        if symbol.owner == "java.lang.Number" {
            return self.number_call(node, &symbol, receiver);
        }
        if symbol.owner == "java.lang.CharSequence"
            && receiver.is_none_or(|r| {
                !matches!(
                    r.java_type(),
                    "java.lang.String" | "java.lang.StringBuilder" | "java.lang.StringBuffer"
                )
            })
        {
            return self.char_sequence_call(node, &symbol, receiver);
        }
        if symbol.name == "equals"
            && !symbol.is_static
            && matches!(&args[..], [arg] if Self::is_object(arg.ty))
            && runtime_type(symbol.owner).is_some()
        {
            return self.value_equals(node, receiver, args);
        }
        if let Some(kind) = self.system_property_call(node, &symbol, &mut args)? {
            return Ok(kind);
        }
        if let Some(builtin) = self.native_call(node, &symbol, receiver, &mut args)? {
            return Ok(ExprKind::Builtin(builtin, args));
        }
        let system_out = receiver
            .and_then(|r| r.symbol())
            .is_some_and(|s| s.kind == "FIELD" && s.owner == "java.lang.System" && s.name == "out");
        if let Some(kind) = self.enum_call(node, &symbol, receiver, &mut args)? {
            return Ok(kind);
        }
        if let Some(builtin) = self.bits_call(&symbol, &args) {
            return Ok(ExprKind::Builtin(builtin, args));
        }
        if let Some(kind) = self.constant_body_call(node, &symbol, receiver, &mut args)? {
            return Ok(kind);
        }
        if let Some(kind) = self.class_name_call(&symbol, receiver)? {
            return Ok(kind);
        }
        self.check_standard_member(node, &symbol)?;
        let builtin = match (symbol.owner, symbol.name) {
            ("java.io.PrintStream", "println") if system_out => Builtin::Println,
            ("java.io.PrintStream", "print") if system_out => Builtin::Print,
            ("java.io.PrintStream", "printf" | "format") if system_out => {
                return self.printf_call(node, args);
            }
            ("java.lang.System", "exit") => Builtin::Exit,
            ("java.lang.Math", "abs") => Builtin::MathAbs,
            ("java.lang.Math", "max") => Builtin::MathMax,
            ("java.lang.Math", "min") => Builtin::MathMin,
            ("java.lang.Math", "sqrt") => Builtin::MathSqrt,
            ("java.lang.Math", "pow") => Builtin::MathPow,
            // Where the JDK's `Arrays` is translated, a call that the runtime does not do is one of
            // it, such as a sort with a `Comparator`, and so is a sort or a fill of a `String[]`
            // that is an `Object[]` where reachability kept the JDK's for it.
            ("java.util.Arrays", name)
                if self.type_index(symbol.owner).is_none() || {
                    let types: Vec<&str> = node.list("arguments").map(|a| a.java_type()).collect();
                    arrays::runtime_arrays_call(name, &types) && !self.jdk_string_array_write(node)
                } =>
            {
                let (writes, copies) =
                    (matches!(name, "sort" | "fill"), matches!(name, "copyOf" | "copyOfRange"));
                let args = self.string_array_args(node, args, writes, copies)?;
                return self.arrays_call(node, name, args);
            }
            ("java.lang.String", "length" | "equals")
                if symbol.name == "length" || matches!(args[0].ty, Type::String | Type::Null) =>
            {
                let receiver = receiver.expect("an instance method has a receiver");
                args.insert(0, self.expr(receiver)?);
                if symbol.name == "length" { Builtin::StringLength } else { Builtin::StringEquals }
            }
            ("java.lang.String", "charAt") => {
                let receiver = receiver.expect("an instance method has a receiver");
                args.insert(0, self.expr(receiver)?);
                Builtin::StringCharAt
            }
            ("java.lang.String", "getBytes") if args.len() <= 1 => {
                let receiver = receiver.expect("an instance method has a receiver");
                args.insert(0, self.expr(receiver)?);
                Builtin::String(StringOp::GetBytes)
            }
            ("java.lang.String", _) => {
                let args = self.string_array_args(node, args, false, false)?;
                let kind = self.string_call(node, &symbol, receiver, args)?;
                return Ok(self.object_array_result(node, kind));
            }
            // `String`'s own `join` of the first `size` elements between a prefix and a suffix,
            // which `StringJoiner` reaches through `SharedSecrets` (`String` is the runtime's).
            ("jdk.internal.access.JavaLangAccess", "join") if args.len() == 5 => {
                Builtin::String(StringOp::JoinRange)
            }
            (
                "java.lang.StringBuilder"
                | "java.lang.StringBuffer"
                | "java.lang.AbstractStringBuilder"
                | "java.lang.CharSequence",
                _,
            ) => {
                return self.builder_call(node, &symbol, receiver, args);
            }
            (owner, name) if unsigned::is_unsigned_method(owner, name) => {
                self.unsigned_call(node, &symbol, &args)?
            }
            (owner, name) if code_point::is_code_point_method(owner, name, &args) => {
                self.code_point_call(node, &symbol, &args)?
            }
            (owner, name) if boxing::boxed_type(owner).is_some() && boxing::is_boxed_method(name) => {
                self.boxed_call(node, &symbol, &mut args, receiver)?
            }
            // The rest of `Character` is the JDK's, where a whole program has it.
            ("java.lang.Character", name)
                if runtime_character_method(name, symbol.ty) || self.type_index(symbol.owner).is_none() =>
            {
                let char_arg = args.first().is_some_and(|a| a.ty == Type::Char);
                self.character_method(node, name, symbol.ty, char_arg)?
            }
            // Where the JDK's `Objects` is translated, its calls are calls of it, but for the calls
            // that the members written for records make on values that are not `Object`s.
            ("java.util.Objects", name)
                if self.type_index(symbol.owner).is_none() || node.0["implicit"] == Value::Bool(true) =>
            {
                return self.objects_call(node, name, args);
            }
            _ if self.type_index(symbol.owner).is_some() => {
                let name = symbol.name.to_string();
                let resolved = self.method_index(&symbol);
                // The default and static methods of a JDK interface that lowering declares (see
                // `functional`) are the JDK's code, which only a translation with its sources has.
                if resolved.is_none() && self.functional.has(symbol.owner) {
                    let kind = if symbol.is_static { "static" } else { "default" };
                    let message = format!(
                        "the {kind} method `{}.{}` is the JDK's code: translate the program with the \
                         JDK's sources (`--main` and `--library`) for it",
                        symbol.owner, symbol.name
                    );
                    return error(node.pos(), message);
                }
                let args = self.convert_generic_args(node, resolved, args)?;
                if symbol.is_static {
                    return Ok(ExprKind::Call { name, args, resolved });
                }
                // `super.m()`, or `Iface.super.m()` for a default method of an interface.
                let is_super = receiver.is_some_and(|r| {
                    r.kind() == "IDENTIFIER" && r.str("name") == "super"
                        || r.kind() == "MEMBER_SELECT"
                            && r.str("identifier") == "super"
                            && self.types[self.current.get()].node.kind() != "INTERFACE"
                });
                let receiver = match receiver {
                    Some(r) if is_super => self.super_receiver(r)?,
                    Some(r) => self.receiver(r, symbol.owner)?,
                    None => self.this(node),
                };
                let receiver = Box::new(receiver);
                return Ok(ExprKind::MethodCall { receiver, name, args, resolved, is_super });
            }
            (owner, name) => return unsupported(node, &format!("method `{owner}.{name}`")),
        };
        Ok(ExprKind::Builtin(builtin, args))
    }
}

/// `super(...)` or `outer.super(...)`, which javac writes into every constructor that does not
/// call another one, or a constructor that calls its superclass's with an enclosing instance.
fn is_super_call(stmt: Node) -> bool {
    stmt.get("expression").is_some_and(|call| {
        call.kind() == "METHOD_INVOCATION"
            && call.get("methodSelect").is_some_and(|s| {
                s.kind() == "IDENTIFIER" && s.str("name") == "super"
                    || s.kind() == "MEMBER_SELECT" && s.str("identifier") == "super"
            })
    })
}

fn binary_op(node: Node, kind: &str) -> Result<BinOp, Error> {
    Ok(match kind {
        "PLUS" => BinOp::Add,
        "MINUS" => BinOp::Sub,
        "MULTIPLY" => BinOp::Mul,
        "DIVIDE" => BinOp::Div,
        "REMAINDER" => BinOp::Rem,
        "LEFT_SHIFT" => BinOp::Shl,
        "RIGHT_SHIFT" => BinOp::Shr,
        "UNSIGNED_RIGHT_SHIFT" => BinOp::UShr,
        "AND" => BinOp::BitAnd,
        "OR" => BinOp::BitOr,
        "XOR" => BinOp::BitXor,
        "CONDITIONAL_AND" => BinOp::And,
        "CONDITIONAL_OR" => BinOp::Or,
        "EQUAL_TO" => BinOp::Eq,
        "NOT_EQUAL_TO" => BinOp::Ne,
        "LESS_THAN" => BinOp::Lt,
        "LESS_THAN_EQUAL" => BinOp::Le,
        "GREATER_THAN" => BinOp::Gt,
        "GREATER_THAN_EQUAL" => BinOp::Ge,
        _ => return unsupported(node, &kind_words(kind)),
    })
}
