//! Lowers the syntax tree that javac printed (see `java/JavaTree.java`) into the typed syntax
//! tree of `ast`. javac has already checked the program, given every expression its type and
//! resolved every name and call, so lowering only maps them onto the supported subset. Anything
//! outside it is reported as an error naming the construct.

use crate::ast::*;
use serde_json::Value;
use std::cell::Cell;

pub fn lower(unit: &Value) -> Result<Class, Error> {
    let unit = Node(unit);
    let mut classes = unit.list("typeDecls").filter(|n| n.kind() != "EMPTY_STATEMENT");
    let Some(class) = classes.next() else { return error(unit.pos(), "no class found") };
    if let Some(extra) = classes.next() {
        return unsupported(extra, "more than one class");
    }
    if class.kind() != "CLASS" {
        return unsupported(class, &kind_words(class.kind()));
    }
    Lowering { next_id: Cell::new(0), class: class.str("simpleName"), methods: Vec::new() }.class(class)
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
    /// The declared type, such as `int` or, for a method, `(int,long)double`.
    ty: &'t str,
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
            ty: s.str("type"),
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
    class: &'t str,
    /// Name and javac signature of each method of the class, in the order of `Class::methods`.
    methods: Vec<(&'t str, &'t str)>,
}

impl<'t> Lowering<'t> {
    fn class(mut self, node: Node<'t>) -> Result<Class, Error> {
        if node.get("extendsClause").is_some()
            || node.list("implementsClause").next().is_some()
            || node.list("typeParameters").next().is_some()
        {
            return unsupported(node, "generic classes, inheritance and interfaces");
        }
        let mut members = Vec::new();
        for member in node.list("members") {
            match member.kind() {
                "METHOD" if member.str("name") == "<init>" => {
                    if !is_default_constructor(member) {
                        return unsupported(member, "constructors");
                    }
                }
                "METHOD" => members.push(member),
                "VARIABLE" => return unsupported(member, "fields"),
                "BLOCK" => return unsupported(member, "initializer blocks"),
                "CLASS" | "INTERFACE" | "ENUM" | "RECORD" => return unsupported(member, "nested classes"),
                kind => return unsupported(member, &kind_words(kind)),
            }
        }
        self.methods = members.iter().map(|m| (m.str("name"), m.symbol().map_or("", |s| s.ty))).collect();
        let methods = members.into_iter().map(|m| self.method(m)).collect::<Result<_, _>>()?;
        Ok(Class { name: self.class.to_string(), methods })
    }

    fn method(&self, node: Node<'t>) -> Result<Method, Error> {
        if !node.has_flag("STATIC") {
            return unsupported(node, "instance methods");
        }
        if node.list("typeParameters").next().is_some() {
            return unsupported(node, "generic methods");
        }
        let name = node.str("name").to_string();
        let ret_node = node.child("returnType")?;
        let ret = self.ty(ret_node, ret_node.java_type())?;
        let params: Vec<Node> = node.list("parameters").collect();
        let param_type = |p: Node<'t>| p.symbol().map_or("", |s| s.ty);
        let is_main = name == "main"
            && ret == Type::Void
            && params.len() == 1
            && param_type(params[0]) == "java.lang.String[]";
        let params = if is_main {
            Vec::new()
        } else {
            params
                .into_iter()
                .map(|p| Ok(Param { name: p.str("name").to_string(), ty: self.ty(p, param_type(p))? }))
                .collect::<Result<_, Error>>()?
        };
        let Some(body) = node.get("body") else { return unsupported(node, "methods without a body") };
        Ok(Method { name, params, ret, body: self.block(body)?, is_main, pos: node.pos() })
    }

    /// The supported type named by javac's `name` for it.
    fn ty(&self, node: Node, name: &str) -> Result<Type, Error> {
        match name {
            "int" => Ok(Type::Int),
            "long" => Ok(Type::Long),
            "double" => Ok(Type::Double),
            "boolean" => Ok(Type::Boolean),
            "void" => Ok(Type::Void),
            "java.lang.String" => Ok(Type::String),
            "char" | "float" | "byte" | "short" => unsupported(node, name),
            "<nulltype>" => unsupported(node, "null"),
            _ if name.ends_with("[]") || name.ends_with("...") => unsupported(node, "arrays"),
            _ => unsupported(node, &format!("type `{name}`")),
        }
    }

    fn block(&self, node: Node<'t>) -> Result<Vec<Stmt>, Error> {
        let mut out = Vec::new();
        for stmt in node.list("statements") {
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
            "VARIABLE" => {
                let ty = self.ty(node, node.symbol().map_or("", |s| s.ty))?;
                let init = match node.get("initializer") {
                    Some(v) => Some(self.expr(v)?),
                    None => None,
                };
                Stmt::Local { ty, name: node.str("name").to_string(), init, pos: node.pos() }
            }
            "EXPRESSION_STATEMENT" => Stmt::Expr(self.expr(node.child("expression")?)?),
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
            "BREAK" | "CONTINUE" => {
                if node.get("label").is_some() {
                    return unsupported(node, "labeled break and continue");
                }
                if node.kind() == "BREAK" { Stmt::Break } else { Stmt::Continue }
            }
            "ENHANCED_FOR_LOOP" => return unsupported(node, "for-each loops"),
            "SWITCH" => return unsupported(node, "switch"),
            "TRY" | "THROW" => return unsupported(node, "exceptions"),
            "LABELED_STATEMENT" => return unsupported(node, "labeled statements"),
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
            "STRING_LITERAL" => ExprKind::Str(node.str("value").to_string()),
            "CHAR_LITERAL" => return unsupported(node, "char"),
            "FLOAT_LITERAL" => return unsupported(node, "float"),
            "NULL_LITERAL" => return unsupported(node, "null"),
            "IDENTIFIER" => ExprKind::Var(self.local(node)?),
            // javac folds the minus sign into integer literals only; fold it into doubles too.
            "UNARY_MINUS" if node.child("expression")?.kind() == "DOUBLE_LITERAL" => {
                ExprKind::Double(-self.number::<f64>(node.child("expression")?)?)
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
            kind if kind.ends_with("_ASSIGNMENT") => ExprKind::Assign {
                op: Some(binary_op(node, kind.trim_end_matches("_ASSIGNMENT"))?),
                target: self.target(node.child("variable")?)?,
                value: Box::new(self.expr(node.child("expression")?)?),
            },
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
            "NEW_CLASS" => return unsupported(node, "objects"),
            "NEW_ARRAY" | "ARRAY_ACCESS" => return unsupported(node, "arrays"),
            "MEMBER_SELECT" => return unsupported(node, "fields"),
            "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE" => return unsupported(node, "lambdas"),
            "INSTANCE_OF" => return unsupported(node, "instanceof"),
            "SWITCH_EXPRESSION" => return unsupported(node, "switch"),
            kind => {
                let left = node.child("leftOperand");
                let op = binary_op(node, kind);
                let (Ok(left), Ok(op)) = (left, op) else { return unsupported(node, &kind_words(kind)) };
                let (left, right) = (self.expr(left)?, self.expr(node.child("rightOperand")?)?);
                if left.ty == Type::String && right.ty == Type::String && matches!(op, BinOp::Eq | BinOp::Ne)
                {
                    return error(node.pos(), "== on strings compares references; not supported yet");
                }
                ExprKind::Binary(op, Box::new(left), Box::new(right))
            }
        };
        let ty = self.ty(node, node.java_type())?;
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
            Some(s) if s.kind == "FIELD" => unsupported(node, "fields"),
            _ => unsupported(node, &format!("the name `{}` here", node.str("name"))),
        }
    }

    /// What `=`, `+=`, `++` and the like assign.
    fn target(&self, node: Node) -> Result<Place, Error> {
        match node.kind() {
            "IDENTIFIER" => Ok(Place::Var(self.local(node)?)),
            "ARRAY_ACCESS" => unsupported(node, "arrays"),
            _ => unsupported(node, "assignment to anything but a local variable"),
        }
    }

    fn call(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let select = node.child("methodSelect")?;
        let Some(symbol) = node.symbol() else { return unsupported(node, "this call") };
        let mut args: Vec<Expr> = node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let receiver = select.get("expression").filter(|_| select.kind() == "MEMBER_SELECT");
        let system_out = receiver
            .and_then(|r| r.symbol())
            .is_some_and(|s| s.kind == "FIELD" && s.owner == "java.lang.System" && s.name == "out");
        let builtin = match (symbol.owner, symbol.name) {
            ("java.io.PrintStream", "println") if system_out => Builtin::Println,
            ("java.io.PrintStream", "print") if system_out => Builtin::Print,
            ("java.lang.System", "exit") => Builtin::Exit,
            ("java.lang.Math", "abs") => Builtin::MathAbs,
            ("java.lang.Math", "max") => Builtin::MathMax,
            ("java.lang.Math", "min") => Builtin::MathMin,
            ("java.lang.Math", "sqrt") => Builtin::MathSqrt,
            ("java.lang.Math", "pow") => Builtin::MathPow,
            ("java.lang.String", "length" | "equals") => {
                if symbol.name == "equals" && args[0].ty != Type::String {
                    return unsupported(node, "equals with an argument that is not a String");
                }
                let receiver = receiver.expect("an instance method has a receiver");
                args.insert(0, self.expr(receiver)?);
                if symbol.name == "length" { Builtin::StringLength } else { Builtin::StringEquals }
            }
            (owner, name) if owner == self.class && symbol.is_static => {
                let resolved = self.methods.iter().position(|&(n, sig)| n == name && sig == symbol.ty);
                return Ok(ExprKind::Call { name: name.to_string(), args, resolved });
            }
            (owner, name) => return unsupported(node, &format!("method `{owner}.{name}`")),
        };
        Ok(ExprKind::Builtin(builtin, args))
    }
}

/// The constructor javac adds to a class that declares none: `{ super(); }`.
fn is_default_constructor(node: Node) -> bool {
    let body: Vec<Node> = node.get("body").into_iter().flat_map(|b| b.list("statements")).collect();
    node.list("parameters").next().is_none()
        && matches!(&body[..], [stmt] if stmt.get("expression").is_some_and(|call| {
            call.kind() == "METHOD_INVOCATION"
                && call.list("arguments").next().is_none()
                && call.get("methodSelect").is_some_and(|s| s.str("name") == "super")
        }))
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
