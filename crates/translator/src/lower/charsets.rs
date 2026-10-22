//! Charsets, which are values of `Type::Charset`: a `StandardCharsets` constant, `Charset.forName`
//! and the methods of a charset are `Builtin::Charset` expressions, and `==` on two charsets is
//! `Builtin::Charset(Equals)`. A charset is never null, so a null sink is rejected here, before
//! lowering: converting a value cannot report an error.
//!
//! The JDK's classes of the standard charsets (`sun.nio.cs.UTF_8` and the others of
//! `STANDARD_CHARSET_CLASSES`), which extend `Charset`, are the runtime's charsets too: their
//! `INSTANCE`, and a new object of one, is the charset, and their types are `Type::Charset`.
//! Reachability does not enter them (`program::RUNTIME_CLASSES`). Any other subclass of
//! `Charset` is still rejected, as extending a class of the runtime.

use super::*;

const CHARSET: &str = "java.nio.charset.Charset";

/// The JDK's classes of the charsets that the runtime has, with the charset of each.
const STANDARD_CHARSET_CLASSES: &[(&str, CharsetOp)] = &[
    ("sun.nio.cs.UTF_8", CharsetOp::Utf8),
    ("sun.nio.cs.US_ASCII", CharsetOp::UsAscii),
    ("sun.nio.cs.ISO_8859_1", CharsetOp::Latin1),
    ("sun.nio.cs.UTF_16", CharsetOp::Utf16),
    ("sun.nio.cs.UTF_16BE", CharsetOp::Utf16Be),
    ("sun.nio.cs.UTF_16LE", CharsetOp::Utf16Le),
];

/// The constant of the runtime's `charset`.
fn constant(charset: java_runtime::Charset) -> CharsetOp {
    match charset {
        java_runtime::Charset::Utf8 => CharsetOp::Utf8,
        java_runtime::Charset::UsAscii => CharsetOp::UsAscii,
        java_runtime::Charset::Latin1 => CharsetOp::Latin1,
        java_runtime::Charset::Utf16 => CharsetOp::Utf16,
        java_runtime::Charset::Utf16Be => CharsetOp::Utf16Be,
        java_runtime::Charset::Utf16Le => CharsetOp::Utf16Le,
    }
}

/// The charset of one of `STANDARD_CHARSET_CLASSES`.
fn standard_class(name: &str) -> Option<CharsetOp> {
    STANDARD_CHARSET_CLASSES.iter().find(|(class, _)| *class == name).map(|(_, op)| *op)
}

/// Whether `name` is the type of a charset: `Charset` or the class of a standard charset.
pub(super) fn is_charset_type(name: &str) -> bool {
    name == CHARSET || standard_class(name).is_some()
}

/// Whether a comparison has a charset operand, which makes it a charset comparison.
pub(super) fn compares_charsets(node: Node) -> bool {
    ["leftOperand", "rightOperand"].into_iter().any(|side| node.get(side).is_some_and(is_charset))
}

/// Whether `node` is a `StandardCharsets` constant, a call of a `Charset` method, or the
/// `INSTANCE` or a new object of a standard charset's class.
pub(super) fn is_charset_op(node: Node) -> bool {
    let Some(symbol) = node.symbol() else { return false };
    match node.kind() {
        "METHOD_INVOCATION" => symbol.kind == "METHOD" && is_charset_type(symbol.owner),
        "IDENTIFIER" | "MEMBER_SELECT" => {
            symbol.kind == "FIELD"
                && (symbol.owner == "java.nio.charset.StandardCharsets"
                    || (symbol.name == "INSTANCE" && standard_class(symbol.owner).is_some()))
        }
        "NEW_CLASS" => standard_class(symbol.owner).is_some(),
        _ => false,
    }
}

fn is_charset(node: Node) -> bool {
    is_charset_type(node.java_type())
}

impl<'t> Lowering<'t> {
    /// A `StandardCharsets` constant or a `Charset` method call, as its `Builtin::Charset`.
    pub(super) fn charset_op(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let Some(symbol) = node.symbol() else { return unsupported(node, "this charset") };
        if node.kind() == "NEW_CLASS"
            && node.list("arguments").next().is_none()
            && node.get("classBody").is_none()
        {
            let op = standard_class(symbol.owner).expect("a standard charset's class");
            return Ok(ExprKind::Builtin(Builtin::Charset(op), Vec::new()));
        }
        if node.kind() == "NEW_CLASS" {
            return unsupported(node, "this charset");
        }
        if let Some(op) = standard_class(symbol.owner).filter(|_| node.kind() != "METHOD_INVOCATION") {
            return Ok(ExprKind::Builtin(Builtin::Charset(op), Vec::new()));
        }
        if node.kind() != "METHOD_INVOCATION" {
            let op = match symbol.name {
                "UTF_8" => CharsetOp::Utf8,
                "US_ASCII" => CharsetOp::UsAscii,
                "ISO_8859_1" => CharsetOp::Latin1,
                "UTF_16" => CharsetOp::Utf16,
                "UTF_16BE" => CharsetOp::Utf16Be,
                "UTF_16LE" => CharsetOp::Utf16Le,
                _ => return unsupported(node, "this `StandardCharsets` constant"),
            };
            return Ok(ExprKind::Builtin(Builtin::Charset(op), Vec::new()));
        }
        let receiver =
            if symbol.is_static { None } else { Some(node.child("methodSelect")?.child("expression")?) };
        let args: Vec<Node> = node.list("arguments").collect();
        let op = match (symbol.name, args.len(), receiver.is_some()) {
            ("forName", 1, false) => CharsetOp::ForName,
            ("isSupported", 1, false) => CharsetOp::IsSupported,
            ("defaultCharset", 0, false) => CharsetOp::DefaultCharset,
            ("name", 0, true) => CharsetOp::Name,
            ("displayName", 0, true) => CharsetOp::DisplayName,
            ("toString", 0, true) => CharsetOp::ToString,
            ("hashCode", 0, true) => CharsetOp::HashCode,
            ("equals", 1, true) => CharsetOp::Equals,
            _ => return unsupported(node, "this method of `Charset`"),
        };
        let operands: Vec<Node> = receiver.into_iter().chain(args).collect();
        if op == CharsetOp::Equals && !is_charset(operands[1]) {
            return unsupported(node, "`equals` with other than a charset");
        }
        if operands.iter().any(|n| is_charset(*n) && may_be_null(*n)) {
            return unsupported(node, "a `null` charset");
        }
        let operands: Vec<Expr> = operands.into_iter().map(|n| self.expr(n)).collect::<Result<_, _>>()?;
        // `forName` of a constant name of a charset the runtime has, such as the value of a
        // property that the translation fixes (`lower::system`), is that charset, which cannot fail.
        if op == CharsetOp::ForName
            && let [Expr { kind: ExprKind::Str(name), .. }] = &operands[..]
            && let Some(charset) = java_runtime::Charset::known(name)
        {
            return Ok(ExprKind::Builtin(Builtin::Charset(constant(charset)), Vec::new()));
        }
        Ok(ExprKind::Builtin(Builtin::Charset(op), operands))
    }

    /// `==` or `!=` on charsets: the `Equals` operation, negated for `!=`.
    pub(super) fn charset_comparison(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let (left, right) = (node.child("leftOperand")?, node.child("rightOperand")?);
        if may_be_null(left) || may_be_null(right) {
            return unsupported(node, "a comparison with `null`");
        }
        if !is_charset(left) || !is_charset(right) {
            return unsupported(node, "a comparison of a `Charset` with other than a charset");
        }
        let (left, right) = (self.expr(left)?, self.expr(right)?);
        let equal = ExprKind::Builtin(Builtin::Charset(CharsetOp::Equals), vec![left, right]);
        if node.kind() == "EQUAL_TO" {
            return Ok(equal);
        }
        let equal = self.synth(equal, node.pos(), Type::Boolean);
        Ok(ExprKind::Unary(UnOp::Not, Box::new(equal)))
    }
}

/// Rejects the null that a charset cannot hold: a `null` stored as a charset, and a field or an
/// array of charsets that is created without values, which would start out null.
pub(super) fn check(unit: Node) -> Result<(), Error> {
    check_tree(unit, "")
}

/// Checks `node` and everything under it; `returns` is the return type of the method around it.
fn check_tree<'t>(node: Node<'t>, returns: &'t str) -> Result<(), Error> {
    let returns = match node.kind() {
        "METHOD" => node.symbol().map_or("", |s| s.ty.rsplit_once(')').map_or("", |(_, r)| r)),
        // A lambda's `return` is not the return of the method around it.
        "LAMBDA_EXPRESSION" => "",
        _ => returns,
    };
    check_sink(node, returns)?;
    for child in children(node) {
        check_tree(child, returns)?;
    }
    Ok(())
}

fn check_sink(node: Node, returns: &str) -> Result<(), Error> {
    match node.kind() {
        "VARIABLE" if node.symbol().is_some_and(|s| is_charset_type(s.ty)) => {
            if node.get("initializer").is_some_and(may_be_null) {
                return null_charset(node);
            }
            if node.get("initializer").is_none() && node.symbol().is_some_and(|s| s.kind == "FIELD") {
                return default_null(node);
            }
        }
        "ASSIGNMENT"
            if node.get("variable").is_some_and(is_charset)
                && node.get("expression").is_some_and(may_be_null) =>
        {
            return null_charset(node);
        }
        "RETURN" if is_charset_type(returns) && node.get("expression").is_some_and(may_be_null) => {
            return null_charset(node);
        }
        "METHOD_INVOCATION" | "NEW_CLASS" => {
            if let Some(symbol) = node.symbol() {
                let params = parameter_types(symbol.ty);
                for (i, arg) in node.list("arguments").enumerate() {
                    let last = params.len().saturating_sub(1);
                    let param = if symbol.varargs && i >= last {
                        params.last().and_then(|p| p.strip_suffix("[]"))
                    } else {
                        params.get(i).copied()
                    };
                    if param.is_some_and(is_charset_type) && may_be_null(arg) {
                        return null_charset(arg);
                    }
                }
            }
        }
        "NEW_ARRAY" if node.java_type().contains(CHARSET) => {
            let values: Vec<Node> = node.list("initializers").collect();
            let creates = node.list("dimensions").next().is_some();
            if creates && values.is_empty() {
                return default_null(node);
            }
            if node.java_type() == "java.nio.charset.Charset[]" && values.iter().any(|v| may_be_null(*v)) {
                return null_charset(node);
            }
        }
        _ => {}
    }
    Ok(())
}

/// Whether `node` may be a null charset: a `null`, or a cast or conditional that may be one. A
/// cast of anything but a charset may be null, as an `Object` can be.
fn may_be_null(node: Node) -> bool {
    match node.kind() {
        "NULL_LITERAL" => true,
        "PARENTHESIZED" => node.get("expression").is_some_and(may_be_null),
        "TYPE_CAST" => node.get("expression").is_some_and(|e| !is_charset(e) || may_be_null(e)),
        "CONDITIONAL_EXPRESSION" => {
            ["trueExpression", "falseExpression"].into_iter().any(|f| node.get(f).is_some_and(may_be_null))
        }
        _ => false,
    }
}

/// The parameter types of a method type's text, such as `(java.nio.charset.Charset,int)void`.
fn parameter_types(ty: &str) -> Vec<&str> {
    let Some(inner) = ty.strip_prefix('(').and_then(|t| t.find(')').map(|end| &t[..end])) else {
        return Vec::new();
    };
    if inner.is_empty() {
        return Vec::new();
    }
    let (mut out, mut depth, mut start) = (Vec::new(), 0, 0);
    for (i, c) in inner.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&inner[start..]);
    out
}

/// The children of `node` in the tree: its object fields and the objects in its arrays, but not
/// its symbol, which names declarations rather than holding code.
fn children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let Some(fields) = node.0.as_object() else { return Vec::new() };
    let mut out = Vec::new();
    for (key, value) in fields {
        match value {
            _ if key == "symbol" => {}
            Value::Array(items) => out.extend(items.iter().filter(|v| v.is_object()).map(Node)),
            Value::Object(_) => out.push(Node(value)),
            _ => {}
        }
    }
    out
}

fn null_charset<T>(node: Node) -> Result<T, Error> {
    error(node.pos(), "a `null` charset not supported yet")
}

fn default_null<T>(node: Node) -> Result<T, Error> {
    error(node.pos(), "a `Charset` field or array element that starts out null not supported yet")
}
