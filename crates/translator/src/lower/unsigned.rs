//! Lowering of the unsigned methods of `Integer` and `Long` (`divideUnsigned`, `toUnsignedString`,
//! `parseUnsignedInt` and the like) and of the unsigned views of `Integer`, `Byte` and `Short`
//! (`toUnsignedLong`, `toUnsignedInt`), and `compareUnsigned` of `Byte` and `Short`. All of them
//! are static.

use super::*;

/// Whether the static method `name` of the class `owner` is one of the unsigned methods.
pub(super) fn is_unsigned_method(owner: &str, name: &str) -> bool {
    match owner {
        "java.lang.Integer" | "java.lang.Long" => matches!(
            name,
            "divideUnsigned"
                | "remainderUnsigned"
                | "compareUnsigned"
                | "toUnsignedString"
                | "parseUnsignedInt"
                | "parseUnsignedLong"
                | "toUnsignedLong"
        ),
        "java.lang.Byte" | "java.lang.Short" => matches!(name, "toUnsignedInt" | "toUnsignedLong"),
        _ => false,
    }
}

impl<'t> Lowering<'t> {
    /// A call of an unsigned method, see `is_unsigned_method`. The kind is the primitive of the
    /// class, or for `Byte` and `Short` the type they widen from.
    pub(super) fn unsigned_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &[Expr],
    ) -> Result<Builtin, Error> {
        let name = symbol.name;
        let op = match name {
            "divideUnsigned" => UnsignedOp::Divide,
            "remainderUnsigned" => UnsignedOp::Remainder,
            "compareUnsigned" => UnsignedOp::Compare,
            "toUnsignedString" => UnsignedOp::ToString,
            "parseUnsignedInt" | "parseUnsignedLong" if args.len() == 4 => UnsignedOp::ParseRange,
            "parseUnsignedInt" | "parseUnsignedLong" => UnsignedOp::Parse,
            _ => UnsignedOp::Widen,
        };
        let arity = match op {
            UnsignedOp::Divide | UnsignedOp::Remainder | UnsignedOp::Compare => 2..=2,
            UnsignedOp::ToString | UnsignedOp::Parse => 1..=2,
            UnsignedOp::ParseRange => 4..=4,
            UnsignedOp::Widen => 1..=1,
        };
        if !arity.contains(&args.len()) {
            return unsupported(node, &format!("the overload of `{name}` with {} arguments", args.len()));
        }
        let kind = match symbol.owner {
            "java.lang.Integer" => Type::Int,
            "java.lang.Long" => Type::Long,
            "java.lang.Byte" => Type::Byte,
            _ => Type::Short,
        };
        Ok(Builtin::Unsigned(op, kind))
    }

    /// `Byte.compareUnsigned(x, y)` or `Short`'s: `toUnsignedInt(x) - toUnsignedInt(y)`, which is
    /// what the JDK computes.
    pub(super) fn small_compare_unsigned(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: Vec<Expr>,
    ) -> ExprKind {
        let kind = if symbol.owner == "java.lang.Byte" { Type::Byte } else { Type::Short };
        let pos = node.pos();
        let mut widened = args.into_iter().map(|a| {
            Box::new(self.synth(
                ExprKind::Builtin(Builtin::Unsigned(UnsignedOp::Widen, kind), vec![a]),
                pos,
                Type::Int,
            ))
        });
        let (x, y) = (widened.next().expect("two arguments"), widened.next().expect("two arguments"));
        ExprKind::Binary(BinOp::Sub, x, y)
    }
}
