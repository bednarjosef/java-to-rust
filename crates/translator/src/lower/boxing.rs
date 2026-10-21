//! Lowering of the boxed classes `Integer`, `Long`, `Double`, `Boolean`, `Character`, `Byte`,
//! `Short` and `Float`: their type names, and their static and instance methods. The boxing and
//! unboxing themselves are not in the tree; javac leaves them implicit (see `codegen::boxing`).

use super::*;

/// The boxed type named by the qualified class name, such as `java.lang.Integer`.
pub(super) fn boxed_type(name: &str) -> Option<Type> {
    let primitive = match name {
        "java.lang.Integer" => Type::Int,
        "java.lang.Long" => Type::Long,
        "java.lang.Double" => Type::Double,
        "java.lang.Boolean" => Type::Boolean,
        "java.lang.Character" => Type::Char,
        "java.lang.Byte" => Type::Byte,
        "java.lang.Short" => Type::Short,
        "java.lang.Float" => Type::Float,
        _ => return None,
    };
    Some(Type::boxed(primitive))
}

/// Methods that the boxed classes share with `Character`, or have of their own: those of
/// `Character` that are not in this list (`isDigit` and the like) go to `character_method`.
pub(super) fn is_boxed_method(name: &str) -> bool {
    matches!(
        name,
        "parseInt"
            | "parseLong"
            | "parseShort"
            | "parseByte"
            | "parseDouble"
            | "parseFloat"
            | "parseBoolean"
            | "valueOf"
            | "toString"
            | "compare"
            | "hashCode"
            | "equals"
            | "compareTo"
            | "sum"
            | "max"
            | "min"
            | "isNaN"
            | "isInfinite"
            | "isFinite"
            | "logicalAnd"
            | "logicalOr"
            | "logicalXor"
    ) || matches!(
        name,
        "intValue"
            | "longValue"
            | "doubleValue"
            | "floatValue"
            | "shortValue"
            | "byteValue"
            | "charValue"
            | "booleanValue"
    )
}

impl<'t> Lowering<'t> {
    /// `Boolean.TRUE` and `Boolean.FALSE`: the cached boxed values, boxed from a literal.
    pub(super) fn boolean_constant(&self, node: Node) -> Option<ExprKind> {
        let symbol =
            node.symbol().filter(|s| s.kind == "FIELD" && s.is_static && s.owner == "java.lang.Boolean")?;
        let value = match symbol.name {
            "TRUE" => true,
            "FALSE" => false,
            _ => return None,
        };
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let literal = Expr { kind: ExprKind::Bool(value), pos: Pos::default(), ty: Type::Boolean, id };
        Some(ExprKind::Builtin(Builtin::Boxed(BoxedOp::Box, Type::Boolean), vec![literal]))
    }

    /// A call to a method of a boxed class. `args` holds the arguments, and the receiver of an
    /// instance method is put first, as `Builtin::Boxed` takes it.
    pub(super) fn boxed_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        args: &mut Vec<Expr>,
        receiver: Option<Node<'t>>,
    ) -> Result<Builtin, Error> {
        let name = symbol.name;
        let op = match (name, symbol.is_static) {
            ("max", true) => return Ok(Builtin::MathMax),
            ("min", true) => return Ok(Builtin::MathMin),
            ("parseInt" | "parseLong" | "parseShort" | "parseByte" | "valueOf", true) if args.len() == 2 => {
                BoxedOp::ParseRadix
            }
            ("parseInt" | "parseLong" | "parseShort" | "parseByte" | "parseDouble" | "parseFloat" | "parseBoolean", true) => {
                BoxedOp::Parse
            }
            ("valueOf", true) if args.first().is_some_and(|a| a.ty == Type::String) => BoxedOp::Parse,
            ("valueOf", true) => BoxedOp::Box,
            ("toString" | "hashCode", true) => {
                if name == "toString" {
                    BoxedOp::ToString
                } else {
                    BoxedOp::HashCode
                }
            }
            ("compare", true) => BoxedOp::Compare,
            ("sum", true) => BoxedOp::Sum,
            ("isNaN", _) => BoxedOp::FloatTest("is_nan"),
            ("isInfinite", _) => BoxedOp::FloatTest("is_infinite"),
            ("isFinite", _) => BoxedOp::FloatTest("is_finite"),
            ("logicalAnd", true) => BoxedOp::Logical(BinOp::BitAnd),
            ("logicalOr", true) => BoxedOp::Logical(BinOp::BitOr),
            ("logicalXor", true) => BoxedOp::Logical(BinOp::BitXor),
            (_, false) if name.ends_with("Value") => BoxedOp::Unbox,
            ("toString", false) => BoxedOp::ToString,
            ("hashCode", false) => BoxedOp::HashCode,
            ("compareTo", false) => BoxedOp::Compare,
            ("equals", false) => {
                // `Integer.equals(5L)` is false in Java, so only an argument of the same kind is lowered.
                let same_kind =
                    args[0].ty.unboxed() == boxed_type(symbol.owner).expect("a boxed method").unboxed();
                if args[0].ty != Type::Null && !same_kind {
                    return unsupported(node, "equals with an argument of another type");
                }
                BoxedOp::Equals
            }
            _ => return unsupported(node, &format!("method `{}.{name}`", symbol.owner)),
        };
        if !symbol.is_static {
            let receiver = receiver.expect("an instance method has a receiver");
            args.insert(0, self.expr(receiver)?);
        }
        let expected = match op {
            BoxedOp::Parse
            | BoxedOp::Box
            | BoxedOp::Unbox
            | BoxedOp::ToString
            | BoxedOp::HashCode
            | BoxedOp::FloatTest(_) => 1,
            BoxedOp::ParseRadix | BoxedOp::Compare | BoxedOp::Equals | BoxedOp::Sum | BoxedOp::Logical(_) => {
                2
            }
        };
        if args.len() != expected {
            return unsupported(node, &format!("the overload of `{name}` with {} arguments", args.len()));
        }
        let kind = boxed_type(symbol.owner).expect("a boxed method").unboxed();
        // No `Character` parses a string.
        if matches!(op, BoxedOp::Parse) && matches!(kind, Type::Char) {
            return unsupported(node, &format!("`{}.{name}` of a string", symbol.owner));
        }
        Ok(Builtin::Boxed(op, kind))
    }
}
