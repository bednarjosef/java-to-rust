//! The unsigned methods (see `lower/unsigned.rs`). An operand is read as its unsigned bits: the
//! `int` or `long` is cast to the unsigned type of the same width, then widened to `u64` for the
//! runtime, which works on `u64` for both widths.

use super::*;

/// The unsigned Rust type of the same width as `kind`.
fn unsigned_type(kind: Type) -> &'static str {
    match kind {
        Type::Byte => "u8",
        Type::Short => "u16",
        Type::Int => "u32",
        _ => "u64",
    }
}

impl<'a> Gen<'a> {
    /// An operand's bits as a `u64`, for the runtime's `u64` functions.
    fn unsigned_operand(&mut self, e: &'a Expr, kind: Type) -> String {
        let code = self.value(e, kind);
        if kind == Type::Long {
            return format!("{} as u64", code.cast_operand());
        }
        format!("{} as {} as u64", code.cast_operand(), unsigned_type(kind))
    }

    /// The unsigned methods of `Integer`, `Long`, `Byte` and `Short`, for `op` on `kind`.
    pub(super) fn unsigned_builtin(
        &mut self,
        op: UnsignedOp,
        kind: Type,
        args: &'a [Expr],
        ty: Type,
    ) -> Code {
        match op {
            UnsignedOp::Divide | UnsignedOp::Remainder => {
                let a = self.unsigned_operand(&args[0], kind);
                let b = self.unsigned_operand(&args[1], kind);
                let function =
                    if op == UnsignedOp::Divide { "divide_unsigned" } else { "remainder_unsigned" };
                Code::new(format!("{}({a}, {b}) as {}", self.runtime(function), rust_type(kind)), AS)
            }
            UnsignedOp::Compare => {
                let a = self.unsigned_operand(&args[0], kind);
                let b = self.unsigned_operand(&args[1], kind);
                Code::atom(format!("{}({a}, {b})", self.runtime("compare_unsigned")))
            }
            UnsignedOp::ToString => {
                let value = self.unsigned_operand(&args[0], kind);
                let radix = match args.get(1) {
                    Some(radix) => self.value(radix, Type::Int).text,
                    None => "10".to_string(),
                };
                Code::atom(format!("{}({value}, {radix})", self.runtime("to_unsigned_string")))
            }
            UnsignedOp::Parse => {
                let text = &args[0];
                let text = if self.nullable(text) {
                    self.option_ref(text)
                } else {
                    format!("Some({})", self.str_ref(text))
                };
                let radix = match args.get(1) {
                    Some(radix) => self.value(radix, Type::Int).text,
                    None => "10".to_string(),
                };
                let bits = if kind == Type::Long { 64 } else { 32 };
                Code::new(
                    format!(
                        "{}({text}, {radix}, {bits}) as {}",
                        self.runtime("parse_unsigned"),
                        rust_type(kind)
                    ),
                    AS,
                )
            }
            UnsignedOp::Widen => {
                let value = self.value(&args[0], kind);
                Code::new(
                    format!("{} as {} as {}", value.cast_operand(), unsigned_type(kind), rust_type(ty)),
                    AS,
                )
            }
        }
    }
}
