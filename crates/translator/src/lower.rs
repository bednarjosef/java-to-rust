//! Lowers the syntax tree that javac printed (see `java/JavaTree.java`) into the typed syntax
//! tree of `ast`. javac has already checked the program, given every expression its type and
//! resolved every name and call, so lowering only maps them onto the supported subset. Anything
//! outside it is reported as an error naming the construct.

mod code_point;
mod enum_nulls;
mod enums;
mod objects;
mod statics;
mod strings;
mod unsigned;

use crate::ast::*;
use objects::{MethodInfo, TypeInfo};
use serde_json::Value;
use std::cell::Cell;

mod arrays;
mod boxing;
mod charsets;
mod constant_bodies;
mod initializers;
mod java_arrays;
mod natives;
mod primitives;
mod program;
mod switch;
mod sync;
mod system;
mod varargs;
use arrays::{is_array_length, reads_name};
pub use program::lower_program;

pub fn lower(unit: &Value) -> Result<Class, Error> {
    let unit = Node(unit);
    // A `;` at top level and an annotation type (`@interface`) do nothing when the program runs.
    let decls: Vec<Node> = unit
        .list("typeDecls")
        .filter(|n| !matches!(n.kind(), "EMPTY_STATEMENT" | "ANNOTATION_TYPE"))
        .collect();
    let Some(first) = decls.first().copied() else { return error(unit.pos(), "no class found") };
    let lowering = Lowering::declare(&decls)?;
    let mut types: Vec<TypeDecl> =
        (0..lowering.types.len()).map(|t| lowering.type_decl(t)).collect::<Result<_, _>>()?;
    let mut methods: Vec<Method> =
        (0..lowering.methods.len()).map(|m| lowering.method(m)).collect::<Result<_, _>>()?;
    for index in (0..types.len()).filter(|&t| lowering.has_static_init(t)) {
        methods.push(lowering.clinit(index)?);
    }
    for index in (0..types.len()).filter(|&t| lowering.types[t].node.kind() == "ENUM") {
        lowering.constant_bodies(index, &mut methods)?;
        lowering.check_enum(index, &methods)?;
        let constants = lowering.enum_constants(index, &types[index].fields, &methods)?;
        types[index].enum_constants = Some(constants);
    }
    let class = Class { name: first.str("simpleName").to_string(), types, methods };
    enums::check_nulls(&class)?;
    Ok(class)
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

    fn has_flag(self, flag: &str) -> bool {
        self.get("modifiers")
            .is_some_and(|m| m.0["flags"].as_array().is_some_and(|f| f.iter().any(|x| x == flag)))
    }
}

fn unsupported<T>(node: Node, what: &str) -> Result<T, Error> {
    error(node.pos(), format!("{what} not supported yet"))
}

/// `ENHANCED_FOR_LOOP` as `enhanced for loop`.
fn kind_words(kind: &str) -> String {
    kind.to_lowercase().replace('_', " ")
}

struct Lowering<'t> {
    next_id: Cell<ExprId>,
    /// Index of the class whose members are being lowered: `this` and unqualified names refer to it.
    current: Cell<usize>,
    /// Whether the method being lowered is a constructor, which cannot `return` early.
    in_ctor: Cell<bool>,
    /// Every class of the file, in the order of `Class::types`.
    types: Vec<TypeInfo<'t>>,
    /// Every method and constructor of every class, in the order of `Class::methods`.
    methods: Vec<MethodInfo<'t>>,
    /// In a whole program, what is reachable from `main`: only that is lowered.
    reach: Option<&'t crate::program::Reach>,
    /// In a whole program, the class whose `main` is the entry point; in a file, every `main` is.
    entry: Option<usize>,
}

impl<'t> Lowering<'t> {
    fn method(&self, index: usize) -> Result<Method, Error> {
        let info = &self.methods[index];
        let node = info.node;
        self.current.set(info.owner);
        self.in_ctor.set(node.str("name") == "<init>");
        if node.list("typeParameters").next().is_some() {
            return unsupported(node, "generic methods");
        }
        let is_ctor = node.str("name") == "<init>";
        let is_static = node.has_flag("STATIC");
        let name = node.str("name").to_string();
        let ret = if is_ctor {
            Type::Void
        } else {
            let ret_node = node.child("returnType")?;
            self.ty(ret_node, ret_node.java_type())?
        };
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
                .map(|p| Ok(Param { name: p.str("name").to_string(), ty: self.ty(p, param_type(p))? }))
                .collect::<Result<_, Error>>()?
        };
        // An abstract method of an enum is implemented by the constants' bodies, which
        // `constant_bodies` makes its body.
        let abstract_in_enum =
            node.has_flag("ABSTRACT") && self.enum_index(&self.types[info.owner].name).is_some();
        let mut body = match node.get("body") {
            Some(body) => self.block(body)?,
            None if abstract_in_enum => Vec::new(),
            None => return unsupported(node, "methods without a body"),
        };
        if is_ctor {
            self.run_initializer_blocks(info.owner, &mut body)?;
        }
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
            "java.nio.charset.Charset" => {
                error(node.pos(), "a `Charset` outside `getBytes` or `new String` not supported yet")
            }
            "byte" => Ok(Type::Byte),
            "short" => Ok(Type::Short),
            "char" => Ok(Type::Char),
            "float" => Ok(Type::Float),
            _ if self.enum_index(name).is_some() => Ok(Type::enum_type(name)),
            _ if self.type_index(name).is_some() => Ok(Type::class(name)),
            "<nulltype>" => Ok(Type::Null),
            // `this` in the body of an enum constant, which is the constant.
            _ if name.starts_with("<anonymous ") && self.enum_index(&name[11..name.len() - 1]).is_some() => {
                Ok(Type::enum_type(&name[11..name.len() - 1]))
            }
            _ if name.ends_with("[]") => Ok(Type::array(self.ty(node, &name[..name.len() - 2])?)),
            // A `...` parameter has the array type here, so `Object...` is rejected as `Object[]`.
            "java.lang.Object" => unsupported(node, "Object"),
            _ => unsupported(node, &format!("type `{name}`")),
        }
    }

    fn block(&self, node: Node<'t>) -> Result<Vec<Stmt>, Error> {
        let mut out = Vec::new();
        for stmt in node.list("statements") {
            // Every class extends `Object`, whose constructor does nothing.
            if is_super_call(stmt) {
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
                    Some(v) => Some(self.expr(v)?),
                    None => None,
                };
                Stmt::Local { ty, name: node.str("name").to_string(), init, pos: node.pos() }
            }
            "EXPRESSION_STATEMENT" => match self.expr(node.child("expression")?)? {
                // A native that does nothing here, such as `registerNatives()`, leaves no trace.
                Expr { kind: ExprKind::Builtin(Builtin::Native(Native::Nothing), _), .. } => return Ok(()),
                e => Stmt::Expr(e),
            },
            // Reachability skipped the branch that the condition guards (`program::never_true`).
            "IF" if crate::program::never_true(&node.0["condition"]) => match node.get("elseStatement") {
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
                Some(value) => Stmt::Return(Some(self.expr(value)?), node.pos()),
                None => Stmt::Return(None, node.pos()),
            },
            "BREAK" | "CONTINUE" => switch::jump(node),
            "YIELD" => Stmt::Yield(self.expr(node.child("value")?)?),
            "ENHANCED_FOR_LOOP" => self.for_each(node)?,
            "SWITCH" => self.switch_stmt(node)?,
            "TRY" | "THROW" => return unsupported(node, "exceptions"),
            "LABELED_STATEMENT" => self.labeled(node)?,
            "CLASS" | "INTERFACE" | "ENUM" | "RECORD" => return unsupported(node, "local classes"),
            kind => return unsupported(node, &kind_words(kind)),
        };
        out.push(stmt);
        Ok(())
    }

    fn expr(&self, node: Node<'t>) -> Result<Expr, Error> {
        let kind = match node.kind() {
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
            "IDENTIFIER" => self.name(node)?,
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
            "ASSIGNMENT" => ExprKind::Assign {
                op: None,
                target: self.target(node.child("variable")?)?,
                value: Box::new(self.expr(node.child("expression")?)?),
            },
            kind if kind.ends_with("_ASSIGNMENT") => {
                let op = binary_op(node, kind.trim_end_matches("_ASSIGNMENT"))?;
                let value = self.expr(node.child("expression")?)?;
                if op == BinOp::Add {
                    self.check_text(node, value.ty)?;
                }
                ExprKind::Assign {
                    op: Some(op),
                    target: self.target(node.child("variable")?)?,
                    value: Box::new(value),
                }
            }
            "TYPE_CAST" => ExprKind::Cast(
                self.ty(node, node.java_type())?,
                Box::new(self.expr(node.child("expression")?)?),
            ),
            "CONDITIONAL_EXPRESSION" => ExprKind::Conditional(
                Box::new(self.expr(node.child("condition")?)?),
                Box::new(self.expr(node.child("trueExpression")?)?),
                Box::new(self.expr(node.child("falseExpression")?)?),
            ),
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
            "MEMBER_SELECT" if is_array_length(node) => {
                ExprKind::Length(Box::new(self.expr(node.child("expression")?)?))
            }
            "MEMBER_SELECT" => match self.constant_field(node).or_else(|| self.boolean_constant(node)) {
                Some(constant) => constant,
                None => self.select(node)?,
            },
            "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE" => return unsupported(node, "lambdas"),
            "INSTANCE_OF" => return unsupported(node, "instanceof"),
            "SWITCH_EXPRESSION" => self.switch_expr(node)?,
            kind => {
                let left = node.child("leftOperand");
                let op = binary_op(node, kind);
                let (Ok(left), Ok(op)) = (left, op) else { return unsupported(node, &kind_words(kind)) };
                let (left, right) = (self.expr(left)?, self.expr(node.child("rightOperand")?)?);
                if left.ty == Type::String && right.ty == Type::String && matches!(op, BinOp::Eq | BinOp::Ne)
                {
                    return error(node.pos(), "== on strings compares references; not supported yet");
                }
                if op == BinOp::Add && (left.ty == Type::String || right.ty == Type::String) {
                    self.check_text(node, left.ty)?;
                    self.check_text(node, right.ty)?;
                }
                ExprKind::Binary(op, Box::new(left), Box::new(right))
            }
        };
        // `printf` returns its stream, which the translation has no type for; it is a statement.
        let ty = match kind {
            ExprKind::Builtin(Builtin::Print, _) if node.kind() == "METHOD_INVOCATION" => Type::Void,
            _ => self.ty(node, node.java_type())?,
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
            Some(s) if matches!(s.kind, "LOCAL_VARIABLE" | "PARAMETER" | "EXCEPTION_PARAMETER") => {
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
        if let Some(kind) = self.class_literal_call(node, &symbol, select)? {
            return Ok(kind);
        }
        let args: Vec<Expr> = node.list("arguments").map(|a| self.call_arg(a)).collect::<Result<_, _>>()?;
        let mut args = self.packed_args(node, &symbol, args)?;
        if select.kind() == "IDENTIFIER" && select.str("name") == "this" {
            return Ok(ExprKind::ThisCall { args, resolved: self.method_index(&symbol) });
        }
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
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
            ("java.util.Arrays", name) => return self.arrays_call(node, name, args),
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
            ("java.lang.String", _) => return self.string_call(node, &symbol, receiver, args),
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
            ("java.lang.Character", name) => {
                let char_arg = args.first().is_some_and(|a| a.ty == Type::Char);
                self.character_method(node, name, symbol.ty, char_arg)?
            }
            _ if self.type_index(symbol.owner).is_some() => {
                let name = symbol.name.to_string();
                let resolved = self.method_index(&symbol);
                if symbol.is_static {
                    return Ok(ExprKind::Call { name, args, resolved });
                }
                let receiver = match receiver {
                    Some(r) => self.expr(r)?,
                    None => self.this(node),
                };
                return Ok(ExprKind::MethodCall { receiver: Box::new(receiver), name, args, resolved });
            }
            (owner, name) => return unsupported(node, &format!("method `{owner}.{name}`")),
        };
        if let (Builtin::Println | Builtin::Print, Some(arg)) = (&builtin, args.first()) {
            self.check_text(node, arg.ty)?;
        }
        Ok(ExprKind::Builtin(builtin, args))
    }
}

/// `super();`, which javac writes into every constructor that does not call another one.
fn is_super_call(stmt: Node) -> bool {
    stmt.get("expression").is_some_and(|call| {
        call.kind() == "METHOD_INVOCATION"
            && call.list("arguments").next().is_none()
            && call.get("methodSelect").is_some_and(|s| s.str("name") == "super")
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
