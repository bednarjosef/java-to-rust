//! Parses Java with tree-sitter and lowers the parse tree into the syntax tree of `ast`.
//! Anything outside the supported subset is reported as an error naming the construct.

use crate::ast::*;
use tree_sitter::Node;

pub fn parse(source: &str) -> Result<Class, Error> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("the tree-sitter Java grammar matches the tree-sitter version");
    let tree = parser.parse(source, None).expect("parsing without a timeout always finishes");
    let root = tree.root_node();
    if root.has_error() {
        return syntax_error(root);
    }
    Lowering { source, next_id: std::cell::Cell::new(0) }.program(root)
}

fn syntax_error<T>(node: Node) -> Result<T, Error> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() && !child.is_error() && !child.is_missing() {
            return syntax_error(child);
        }
        if child.is_missing() {
            return error(pos(child), format!("syntax error: missing {}", child.kind()));
        }
        if child.is_error() {
            return error(pos(child), "syntax error");
        }
    }
    error(pos(node), "syntax error")
}

fn pos(node: Node) -> Pos {
    let p = node.start_position();
    Pos { line: p.row + 1, column: p.column + 1 }
}

/// Named children without comments.
fn children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).filter(|n| !n.is_extra()).collect()
}

fn field<'t>(node: Node<'t>, name: &str) -> Result<Node<'t>, Error> {
    match node.child_by_field_name(name) {
        Some(n) => Ok(n),
        None => error(pos(node), format!("syntax error: {} without {}", node.kind(), name)),
    }
}

fn unsupported<T>(node: Node, what: &str) -> Result<T, Error> {
    error(pos(node), format!("{what} not supported yet"))
}

struct Lowering<'s> {
    source: &'s str,
    next_id: std::cell::Cell<ExprId>,
}

impl Lowering<'_> {
    fn text(&self, node: Node) -> &str {
        &self.source[node.byte_range()]
    }

    fn program(&self, root: Node) -> Result<Class, Error> {
        let mut class = None;
        for node in children(root) {
            match node.kind() {
                "class_declaration" if class.is_none() => class = Some(self.class(node)?),
                "class_declaration" => return unsupported(node, "more than one class"),
                "package_declaration" => {}
                "import_declaration" => return unsupported(node, "imports"),
                kind => return unsupported(node, &format!("top-level {kind}")),
            }
        }
        match class {
            Some(class) => Ok(class),
            None => error(pos(root), "no class found"),
        }
    }

    fn class(&self, node: Node) -> Result<Class, Error> {
        for name in ["type_parameters", "superclass", "interfaces", "permits"] {
            if let Some(n) = node.child_by_field_name(name) {
                return unsupported(n, "generic classes, inheritance and interfaces");
            }
        }
        let name = self.text(field(node, "name")?).to_string();
        let mut methods = Vec::new();
        for member in children(field(node, "body")?) {
            match member.kind() {
                "method_declaration" => methods.push(self.method(member)?),
                "field_declaration" => return unsupported(member, "fields"),
                "constructor_declaration" => return unsupported(member, "constructors"),
                kind => return unsupported(member, &kind.replace('_', " ")),
            }
        }
        Ok(Class { name, methods })
    }

    fn modifiers(&self, node: Node) -> Vec<String> {
        let mut out = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut c = child.walk();
                out.extend(child.children(&mut c).map(|m| self.text(m).to_string()));
            }
        }
        out
    }

    fn method(&self, node: Node) -> Result<Method, Error> {
        let modifiers = self.modifiers(node);
        if !modifiers.iter().any(|m| m == "static") {
            return unsupported(node, "instance methods");
        }
        if let Some(n) = node.child_by_field_name("type_parameters") {
            return unsupported(n, "generic methods");
        }
        let name = self.text(field(node, "name")?).to_string();
        let ret = self.ty(field(node, "type")?)?;
        let params_node = field(node, "parameters")?;
        let param_nodes = children(params_node);
        let is_main = name == "main"
            && ret == Type::Void
            && param_nodes.len() == 1
            && self.is_string_array(param_nodes[0]);
        let mut params = Vec::new();
        if !is_main {
            for p in param_nodes {
                if p.kind() != "formal_parameter" || p.child_by_field_name("dimensions").is_some() {
                    return unsupported(p, "array and variable-length parameters");
                }
                let ty = self.ty(field(p, "type")?)?;
                params.push(Param { name: self.text(field(p, "name")?).to_string(), ty });
            }
        }
        let Some(body) = node.child_by_field_name("body") else {
            return unsupported(node, "methods without a body");
        };
        Ok(Method { name, params, ret, body: self.block(body)?, is_main, pos: pos(node) })
    }

    /// `String[] args` or `String... args`, the parameter of `main`.
    fn is_string_array(&self, param: Node) -> bool {
        let Some(ty) = param.child_by_field_name("type") else { return false };
        match param.kind() {
            "formal_parameter" => {
                ty.kind() == "array_type"
                    && self.text(ty).split_whitespace().collect::<String>() == "String[]"
            }
            "spread_parameter" => self.text(ty) == "String",
            _ => false,
        }
    }

    fn ty(&self, node: Node) -> Result<Type, Error> {
        match (node.kind(), self.text(node)) {
            ("integral_type", "int") => Ok(Type::Int),
            ("integral_type", "long") => Ok(Type::Long),
            ("floating_point_type", "double") => Ok(Type::Double),
            ("boolean_type", _) => Ok(Type::Boolean),
            ("void_type", _) => Ok(Type::Void),
            ("type_identifier", "String") => Ok(Type::String),
            ("type_identifier", "var") => unsupported(node, "`var`"),
            ("array_type", _) => unsupported(node, "arrays"),
            (_, text) => unsupported(node, &format!("type `{text}`")),
        }
    }

    fn block(&self, node: Node) -> Result<Vec<Stmt>, Error> {
        let mut out = Vec::new();
        for child in children(node) {
            self.stmt_into(child, &mut out)?;
        }
        Ok(out)
    }

    /// A statement in a position that takes one statement, such as the body of an `if`.
    fn single_stmt(&self, node: Node) -> Result<Stmt, Error> {
        if !node.is_named() {
            // The empty statement `;`.
            return Ok(Stmt::Block(Vec::new()));
        }
        let mut out = Vec::new();
        self.stmt_into(node, &mut out)?;
        Ok(if out.len() == 1 { out.pop().unwrap() } else { Stmt::Block(out) })
    }

    fn stmt_into(&self, node: Node, out: &mut Vec<Stmt>) -> Result<(), Error> {
        let stmt = match node.kind() {
            "block" => Stmt::Block(self.block(node)?),
            "local_variable_declaration" => return self.locals_into(node, out),
            "expression_statement" => {
                let [expr] = children(node)[..] else { return error(pos(node), "syntax error") };
                Stmt::Expr(self.expr(expr)?)
            }
            "if_statement" => Stmt::If {
                cond: self.condition(node)?,
                then: Box::new(self.single_stmt(field(node, "consequence")?)?),
                otherwise: match node.child_by_field_name("alternative") {
                    Some(n) => Some(Box::new(self.single_stmt(n)?)),
                    None => None,
                },
            },
            "while_statement" => Stmt::While {
                cond: self.condition(node)?,
                body: Box::new(self.single_stmt(field(node, "body")?)?),
            },
            "do_statement" => Stmt::DoWhile {
                body: Box::new(self.single_stmt(field(node, "body")?)?),
                cond: self.condition(node)?,
            },
            "for_statement" => {
                let mut init = Vec::new();
                let mut cursor = node.walk();
                for n in node.children_by_field_name("init", &mut cursor) {
                    if n.kind() == "local_variable_declaration" {
                        self.locals_into(n, &mut init)?;
                    } else {
                        init.push(Stmt::Expr(self.expr(n)?));
                    }
                }
                let mut cursor = node.walk();
                let update = node
                    .children_by_field_name("update", &mut cursor)
                    .map(|n| self.expr(n))
                    .collect::<Result<_, _>>()?;
                let cond = match node.child_by_field_name("condition") {
                    Some(n) => Some(self.expr(n)?),
                    None => None,
                };
                let body = Box::new(self.single_stmt(field(node, "body")?)?);
                Stmt::For { init, cond, update, body }
            }
            "return_statement" => match children(node)[..] {
                [] => Stmt::Return(None, pos(node)),
                [value] => Stmt::Return(Some(self.expr(value)?), pos(node)),
                _ => return error(pos(node), "syntax error"),
            },
            "break_statement" | "continue_statement" => {
                if !children(node).is_empty() {
                    return unsupported(node, "labeled break and continue");
                }
                if node.kind() == "break_statement" { Stmt::Break } else { Stmt::Continue }
            }
            "enhanced_for_statement" => return unsupported(node, "for-each loops"),
            "switch_expression" => return unsupported(node, "switch"),
            "try_statement" | "throw_statement" => return unsupported(node, "exceptions"),
            kind => return unsupported(node, &kind.replace('_', " ")),
        };
        out.push(stmt);
        Ok(())
    }

    fn condition(&self, node: Node) -> Result<Expr, Error> {
        self.expr(field(node, "condition")?)
    }

    fn locals_into(&self, node: Node, out: &mut Vec<Stmt>) -> Result<(), Error> {
        let ty = self.ty(field(node, "type")?)?;
        let mut cursor = node.walk();
        for declarator in node.children_by_field_name("declarator", &mut cursor) {
            if declarator.child_by_field_name("dimensions").is_some() {
                return unsupported(declarator, "arrays");
            }
            let name = self.text(field(declarator, "name")?).to_string();
            let init = match declarator.child_by_field_name("value") {
                Some(v) => Some(self.expr(v)?),
                None => None,
            };
            out.push(Stmt::Local { ty, name, init, pos: pos(declarator) });
        }
        Ok(())
    }

    fn expr(&self, node: Node) -> Result<Expr, Error> {
        let kind = match node.kind() {
            "parenthesized_expression" => match children(node)[..] {
                [inner] => return self.expr(inner),
                _ => return error(pos(node), "syntax error"),
            },
            "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal" => self.integer(node, false)?,
            "decimal_floating_point_literal" => ExprKind::Double(self.double(node, false)?),
            "true" => ExprKind::Bool(true),
            "false" => ExprKind::Bool(false),
            "string_literal" => ExprKind::Str(self.string(node)?),
            "identifier" => ExprKind::Var(self.text(node).to_string()),
            "unary_expression" => {
                let operand = field(node, "operand")?;
                let op = match self.text(field(node, "operator")?) {
                    "-" => UnOp::Neg,
                    "+" => UnOp::Plus,
                    "!" => UnOp::Not,
                    "~" => UnOp::BitNot,
                    op => return unsupported(node, &format!("operator {op}")),
                };
                // A minus sign belongs to the literal, so that `-2147483648` is a valid int.
                match (op, operand.kind()) {
                    (UnOp::Neg, k) if k.ends_with("integer_literal") => self.integer(operand, true)?,
                    (UnOp::Neg, "decimal_floating_point_literal") => {
                        ExprKind::Double(self.double(operand, true)?)
                    }
                    _ => ExprKind::Unary(op, Box::new(self.expr(operand)?)),
                }
            }
            "binary_expression" => {
                let op = self.binary_op(field(node, "operator")?)?;
                ExprKind::Binary(
                    op,
                    Box::new(self.expr(field(node, "left")?)?),
                    Box::new(self.expr(field(node, "right")?)?),
                )
            }
            "assignment_expression" => {
                let left = field(node, "left")?;
                if left.kind() != "identifier" {
                    return unsupported(left, "assignment to anything but a local variable");
                }
                let op_node = field(node, "operator")?;
                let op = match self.text(op_node) {
                    "=" => None,
                    text => Some(self.binary_op_text(op_node, text.trim_end_matches('='))?),
                };
                ExprKind::Assign {
                    op,
                    target: self.text(left).to_string(),
                    value: Box::new(self.expr(field(node, "right")?)?),
                }
            }
            "update_expression" => {
                let mut cursor = node.walk();
                let parts: Vec<Node> = node.children(&mut cursor).filter(|n| !n.is_extra()).collect();
                let [first, second] = parts[..] else { return error(pos(node), "syntax error") };
                let (op, target, prefix) =
                    if first.is_named() { (second, first, false) } else { (first, second, true) };
                if target.kind() != "identifier" {
                    return unsupported(target, "++ and -- on anything but a local variable");
                }
                ExprKind::IncDec {
                    target: self.text(target).to_string(),
                    increment: self.text(op) == "++",
                    prefix,
                }
            }
            "cast_expression" => {
                ExprKind::Cast(self.ty(field(node, "type")?)?, Box::new(self.expr(field(node, "value")?)?))
            }
            "ternary_expression" => ExprKind::Conditional(
                Box::new(self.expr(field(node, "condition")?)?),
                Box::new(self.expr(field(node, "consequence")?)?),
                Box::new(self.expr(field(node, "alternative")?)?),
            ),
            "method_invocation" => self.call(node)?,
            "character_literal" => return unsupported(node, "char"),
            "null_literal" => return unsupported(node, "null"),
            "object_creation_expression" => return unsupported(node, "objects"),
            "array_access" | "array_creation_expression" | "array_initializer" => {
                return unsupported(node, "arrays");
            }
            "field_access" => return unsupported(node, "fields"),
            kind => return unsupported(node, &kind.replace('_', " ")),
        };
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        Ok(Expr { kind, pos: pos(node), ty: Type::Void, id })
    }

    fn call(&self, node: Node) -> Result<ExprKind, Error> {
        let name = self.text(field(node, "name")?);
        let mut args: Vec<Expr> = children(field(node, "arguments")?)
            .into_iter()
            .map(|a| self.expr(a))
            .collect::<Result<_, _>>()?;
        let Some(object) = node.child_by_field_name("object") else {
            return Ok(ExprKind::Call { name: name.to_string(), args, resolved: None });
        };
        let object_text: String = self.text(object).split_whitespace().collect();
        let builtin = match (object_text.as_str(), name) {
            ("System.out", "println") => Builtin::Println,
            ("System.out", "print") => Builtin::Print,
            ("System", "exit") => Builtin::Exit,
            ("Math", "abs") => Builtin::MathAbs,
            ("Math", "max") => Builtin::MathMax,
            ("Math", "min") => Builtin::MathMin,
            ("Math", "sqrt") => Builtin::MathSqrt,
            ("Math", "pow") => Builtin::MathPow,
            (_, "length" | "equals") => {
                args.insert(0, self.expr(object)?);
                if name == "length" { Builtin::StringLength } else { Builtin::StringEquals }
            }
            _ if object.kind() == "identifier" && self.is_own_class(node, &object_text) => {
                return Ok(ExprKind::Call { name: name.to_string(), args, resolved: None });
            }
            _ => return unsupported(node, &format!("method `{object_text}.{name}`")),
        };
        Ok(ExprKind::Builtin(builtin, args))
    }

    /// Whether `name` is the name of the class that contains `node`.
    fn is_own_class(&self, node: Node, name: &str) -> bool {
        let mut n = node;
        while let Some(parent) = n.parent() {
            if parent.kind() == "class_declaration" {
                return parent.child_by_field_name("name").is_some_and(|c| self.text(c) == name);
            }
            n = parent;
        }
        false
    }

    fn binary_op(&self, node: Node) -> Result<BinOp, Error> {
        self.binary_op_text(node, self.text(node))
    }

    fn binary_op_text(&self, node: Node, text: &str) -> Result<BinOp, Error> {
        Ok(match text {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            "%" => BinOp::Rem,
            "<<" => BinOp::Shl,
            ">>" => BinOp::Shr,
            ">>>" => BinOp::UShr,
            "&" => BinOp::BitAnd,
            "|" => BinOp::BitOr,
            "^" => BinOp::BitXor,
            "&&" => BinOp::And,
            "||" => BinOp::Or,
            "==" => BinOp::Eq,
            "!=" => BinOp::Ne,
            "<" => BinOp::Lt,
            "<=" => BinOp::Le,
            ">" => BinOp::Gt,
            ">=" => BinOp::Ge,
            op => return unsupported(node, &format!("operator {op}")),
        })
    }

    fn integer(&self, node: Node, negate: bool) -> Result<ExprKind, Error> {
        let text: String = self.text(node).chars().filter(|&c| c != '_').collect();
        let (digits, long) = match text.strip_suffix(['l', 'L']) {
            Some(d) => (d, true),
            None => (text.as_str(), false),
        };
        let (digits, radix) = if let Some(d) = digits.strip_prefix("0x").or(digits.strip_prefix("0X")) {
            (d, 16)
        } else if let Some(d) = digits.strip_prefix("0b").or(digits.strip_prefix("0B")) {
            (d, 2)
        } else if digits.len() > 1 && digits.starts_with('0') {
            (&digits[1..], 8)
        } else {
            (digits, 10)
        };
        let too_large = || error(pos(node), format!("integer number too large: {text}"));
        let Ok(value) = u64::from_str_radix(digits, radix) else { return too_large() };
        // Decimal literals carry their sign; the others are two's complement bit patterns.
        let (int_limit, long_limit) = match (radix, negate) {
            (10, false) => (i32::MAX as u64, i64::MAX as u64),
            (10, true) => (1 << 31, 1 << 63),
            _ => (u32::MAX as u64, u64::MAX),
        };
        if long {
            if value > long_limit {
                return too_large();
            }
            let v = value as i64;
            Ok(ExprKind::Long(if negate { v.wrapping_neg() } else { v }))
        } else {
            if value > int_limit {
                return too_large();
            }
            let v = value as u32 as i32;
            Ok(ExprKind::Int(if negate { v.wrapping_neg() } else { v }))
        }
    }

    fn double(&self, node: Node, negate: bool) -> Result<f64, Error> {
        let text: String = self.text(node).chars().filter(|&c| c != '_').collect();
        if text.ends_with(['f', 'F']) {
            return unsupported(node, "float");
        }
        let Ok(value) = text.trim_end_matches(['d', 'D']).parse::<f64>() else {
            return error(pos(node), format!("malformed floating-point literal: {text}"));
        };
        Ok(if negate { -value } else { value })
    }

    fn string(&self, node: Node) -> Result<String, Error> {
        if self.text(node).starts_with("\"\"\"") {
            return unsupported(node, "text blocks");
        }
        let mut out = String::new();
        for part in children(node) {
            let text = self.text(part);
            match part.kind() {
                "string_fragment" => out.push_str(text),
                "escape_sequence" => out.push_str(&unescape(text).ok_or_else(|| Error {
                    pos: pos(part),
                    message: format!("unknown escape sequence {text}"),
                })?),
                kind => return unsupported(part, kind),
            }
        }
        Ok(out)
    }
}

/// Decodes one Java escape sequence such as `\n`, `é` or `\101`.
fn unescape(escape: &str) -> Option<String> {
    let body = escape.strip_prefix('\\')?;
    let c = match body {
        "n" => '\n',
        "t" => '\t',
        "b" => '\u{8}',
        "r" => '\r',
        "f" => '\u{c}',
        "s" => ' ',
        "'" => '\'',
        "\"" => '"',
        "\\" => '\\',
        _ if body.starts_with('u') => {
            let hex = body.trim_start_matches('u');
            char::from_u32(u32::from_str_radix(hex, 16).ok()?)?
        }
        _ => char::from_u32(u32::from_str_radix(body, 8).ok()?)?,
    };
    Some(c.to_string())
}
