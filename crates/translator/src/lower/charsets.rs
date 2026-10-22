//! Charsets, which are values of `Type::Charset`: a `StandardCharsets` constant, `Charset.forName`
//! and the methods of a charset are `Builtin::Charset` expressions, and `==` on two charsets is
//! a comparison of the two. A charset may be null as a reference may: where the nullness
//! analysis finds that it may, it is an `Option` of the runtime's charset (`Copy`, as a
//! `java::Class`), which a method of the charset dereferences. Only an array of charsets that may
//! hold null is rejected, here, before lowering: converting a value cannot report an error.
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
        // A decoder or an encoder of the charset: the JDK's, made by the translator's method
        // (`program::CHARSET_CODERS`).
        if let Some(receiver) = receiver
            && let Some(method) = crate::program::charset_coder(symbol.owner, symbol.name, symbol.ty)
        {
            let args = vec![self.expr(receiver)?];
            return self.charset_call(node, symbol.name, method, args);
        }
        // A method that the JDK does by a coder with its arguments (`program::CHARSET_CODINGS`).
        if let Some(receiver) = receiver
            && let Some(method) = crate::program::charset_coding(symbol.owner, symbol.name, symbol.ty)
        {
            let operands: Vec<Node> = std::iter::once(receiver).chain(args).collect();
            let operands: Vec<Expr> = operands.into_iter().map(|n| self.expr(n)).collect::<Result<_, _>>()?;
            return self.charset_call(node, symbol.name, method, operands);
        }
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
        if op == CharsetOp::Equals && !is_charset(operands[1]) && operands[1].kind() != "NULL_LITERAL" {
            return unsupported(node, "`equals` with other than a charset");
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

    /// A call of the translator's method `method` (`program::charset_coder` or `charset_coding`)
    /// for the charset method `name`, which must be in the program.
    fn charset_call(
        &self,
        node: Node<'t>,
        name: &str,
        (class, to, sig): (&str, &str, &str),
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let resolved = self
            .methods
            .iter()
            .position(|m| self.types[m.owner].javac == class && m.name == to && m.sig == sig);
        if resolved.is_none() {
            return unsupported(node, &format!("`Charset.{name}` outside a whole program"));
        }
        Ok(ExprKind::Call { name: to.to_string(), args, resolved })
    }

    /// `==` or `!=` on charsets, or of a charset and `null`: a comparison of the two values, which
    /// are the same object exactly when they are the same charset or both null.
    pub(super) fn charset_comparison(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let (left, right) = (node.child("leftOperand")?, node.child("rightOperand")?);
        let operand = |n: Node| is_charset(n) || n.kind() == "NULL_LITERAL";
        if !operand(left) || !operand(right) {
            return unsupported(node, "a comparison of a `Charset` with other than a charset");
        }
        let (left, right) = (self.expr(left)?, self.expr(right)?);
        let op = if node.kind() == "EQUAL_TO" { BinOp::Eq } else { BinOp::Ne };
        Ok(ExprKind::Binary(op, Box::new(left), Box::new(right)))
    }
}

/// Rejects the null that an array of charsets cannot hold: one that is created without values,
/// whose elements would start out null, or with a value that may be null.
pub(super) fn check(unit: Node) -> Result<(), Error> {
    check_sink(unit)?;
    for child in children(unit) {
        check(child)?;
    }
    Ok(())
}

fn check_sink(node: Node) -> Result<(), Error> {
    match node.kind() {
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

/// The children of `node` in the tree: its object fields and the objects in its arrays, but not
/// its symbol, which names declarations rather than holding code.
pub(super) fn children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
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
    error(node.pos(), "a `Charset` array element that starts out null not supported yet")
}
