//! `Math.ceilDiv`, `Math.ceilMod` and `Math.ceilDivExact` on `int` and `long`, with the
//! `(long, int)` forms of `ceilDiv` and `ceilMod`, and the other exact quotients `divideExact`
//! and `floorDivExact`, in `Math` and `StrictMath` alike. They are the runtime's `ceil_div`
//! family, in the checked form where a `catch` or a `finally` sees the exception.

use super::*;

impl<'a> Gen<'a> {
    /// The ceiling division methods of this file; `None` for any other method, which `math`
    /// translates.
    pub(super) fn ceil_division(
        &mut self,
        name: &str,
        args: &'a [Expr],
        ty: Type,
        id: ExprId,
    ) -> Option<Code> {
        let function = match name {
            "ceilDiv" => "ceil_div",
            "ceilMod" => "ceil_mod",
            "ceilDivExact" => "ceil_div_exact",
            "divideExact" => "divide_exact",
            "floorDivExact" => "floor_div_exact",
            _ => return None,
        };
        // The operands compute in `long` if either is one. The result is that type, except that
        // `ceilMod(long, int)` returns `int`.
        let operand = if args.iter().any(|a| a.ty == Type::Long) { Type::Long } else { Type::Int };
        // Typed literals: a generic `ceil_div(9223372036854775807, 0)` would default to `i32`.
        let texts: Vec<String> = args.iter().map(|a| self.value(a, operand).typed()).collect();
        let result = self.may_fail(id, function, "", &texts.join(", "));
        Some(if ty == operand { result } else { self.convert(result, operand, ty) })
    }
}
