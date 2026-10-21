//! The rest of `java.util.Arrays` on arrays of primitives and strings: `equals`, `hashCode`,
//! `sort` of strings and of ranges, `fill` of a range, `copyOfRange`, `binarySearch` and
//! `deepToString`; and `clone()` of an array, which `native_call` lowers.

use super::*;
use crate::types::assignable;

impl<'t> Lowering<'t> {
    /// The `java.util.Arrays` methods of `java_arrays.rs` and the builtin of each, for arrays whose
    /// elements are primitives or strings. Anything else is unsupported.
    pub(super) fn java_arrays_call(
        &self,
        node: Node<'t>,
        name: &str,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let elem = match args.first().map(|a| a.ty) {
            Some(Type::Array(elem)) => *elem,
            _ => return unsupported(node, "this call"),
        };
        let primitive = elem.is_numeric() || elem == Type::Boolean;
        let printable = primitive || elem == Type::String;
        let int = |i: usize| assignable(args[i].ty, Type::Int);
        let op = match (name, args.len()) {
            ("equals", 2) if printable && (args[1].ty == args[0].ty || args[1].ty == Type::Null) => {
                ArraysOp::Equals
            }
            ("hashCode", 1) if printable => ArraysOp::HashCode,
            ("sort", 1) if elem == Type::String => ArraysOp::SortAll,
            ("sort", 3) if printable && int(1) && int(2) => ArraysOp::SortRange,
            ("fill", 4) if printable && int(1) && int(2) && assignable(args[3].ty, elem) => {
                ArraysOp::FillRange
            }
            // Of references too, as `copyOf`.
            ("copyOfRange", 3) if int(1) && int(2) => ArraysOp::CopyOfRange,
            ("binarySearch", 2) if printable && key_ok(&args[1], elem) => ArraysOp::BinarySearch,
            ("binarySearch", 4) if printable && int(1) && int(2) && key_ok(&args[3], elem) => {
                ArraysOp::BinarySearchRange
            }
            ("deepToString", 1) if is_printable_rows(elem) => ArraysOp::DeepToString,
            _ => return unsupported(node, &format!("`Arrays.{name}` on this array")),
        };
        Ok(ExprKind::Builtin(Builtin::Arrays(op), args))
    }

    /// `a.clone()` on an array: its receiver is the first argument. Only arrays are cloned here;
    /// the `Array` owner is javac's class for an array's members.
    pub(super) fn array_clone(
        &self,
        node: Node<'t>,
        receiver: Option<Node<'t>>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<Builtin>, Error> {
        let Some(receiver) = receiver else { return unsupported(node, "`clone` without a receiver") };
        let array = self.expr(receiver)?;
        if !matches!(array.ty, Type::Array(_)) {
            return unsupported(node, "`clone` of anything but an array");
        }
        args.insert(0, array);
        Ok(Some(Builtin::Arrays(ArraysOp::Clone)))
    }
}

/// Whether `key` may be searched for in an array of `elem`: assignable to it, and not a null
/// literal, which Java rejects by throwing from `compareTo`.
fn key_ok(key: &Expr, elem: Type) -> bool {
    key.ty != Type::Null && assignable(key.ty, elem)
}

/// Whether `elem` is an array of primitives or strings, the rows `deepToString` formats.
fn is_printable_rows(elem: Type) -> bool {
    match elem {
        Type::Array(row) => row.is_numeric() || *row == Type::Boolean || *row == Type::String,
        _ => false,
    }
}
