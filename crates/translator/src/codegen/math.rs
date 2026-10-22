//! The `Math` methods that throw on overflow or on bad bounds, and the ones that need the runtime's
//! exact helpers: `addExact` and the rest, `clamp`, `copySign`, `nextDown` and the products
//! `multiplyHigh`, `unsignedMultiplyHigh` and `multiplyFull`.

use super::*;

impl<'a> Gen<'a> {
    /// The methods of this file; `None` for any other method, which `math` translates.
    pub(super) fn math_extra(&mut self, name: &str, args: &'a [Expr], ty: Type) -> Option<Code> {
        // `scalb(x, n)` takes an `int` after its operand, and is the same in `Math` and
        // `StrictMath` (`scalb_f32` for `float`).
        if name == "scalb" {
            let float = if ty == Type::Float { "_f32" } else { "" };
            let x = self.value(&args[0], ty).text;
            let n = self.value(&args[1], Type::Int).text;
            return Some(Code::atom(format!("{}({x}, {n})", self.runtime(&format!("scalb{float}")))));
        }
        // `toIntExact` takes a `long`, and `clamp(long, int, int)` computes in `long`. The products
        // take `long`s, except `multiplyFull`, which takes `int`s and returns the exact `long`.
        let operand = match name {
            "toIntExact" | "multiplyHigh" | "unsignedMultiplyHigh" => Type::Long,
            "multiplyFull" => Type::Int,
            "clamp" if ty == Type::Int => Type::Long,
            "addExact" | "subtractExact" | "multiplyExact" | "negateExact" | "incrementExact"
            | "decrementExact" | "absExact" | "clamp" | "copySign" | "nextDown" => ty,
            _ => return None,
        };
        let codes: Vec<Code> = args.iter().map(|a| self.value(a, operand)).collect();
        // Typed literals: a generic `add_exact(9223372036854775807, 0)` would default to `i32`.
        let mut texts: Vec<String> = codes.iter().map(|c| c.typed()).collect();
        let float = if operand == Type::Float { "_f32" } else { "" };
        let code = match name {
            "copySign" => Code::atom(format!("{}.copysign({})", self.recv(&codes[0], operand), texts[1])),
            "nextDown" => Code::atom(format!("{}({})", self.runtime(&format!("next_down{float}")), texts[0])),
            // The high and full halves of a product, which the runtime computes in `i128`.
            "multiplyHigh" => self.exact_call("multiply_high", &texts),
            "unsignedMultiplyHigh" => self.exact_call("unsigned_multiply_high", &texts),
            "multiplyFull" => self.exact_call("multiply_full", &texts),
            "clamp" => {
                let function = match operand {
                    Type::Float => "clamp_f32",
                    Type::Double => "clamp_f64",
                    _ => "clamp",
                };
                let result = self.exact_call(function, &texts);
                if ty == operand { result } else { self.convert(result, operand, ty) }
            }
            _ => {
                let function = match name {
                    "addExact" | "incrementExact" => "add_exact",
                    "subtractExact" | "decrementExact" => "sub_exact",
                    "multiplyExact" => "mul_exact",
                    "negateExact" => "neg_exact",
                    "absExact" => "abs_exact",
                    _ => "to_int_exact",
                };
                // `x + 1` and `x - 1`: the literal takes the type of `x`.
                if name.starts_with("increment") || name.starts_with("decrement") {
                    texts.push("1".to_string());
                }
                self.exact_call(function, &texts)
            }
        };
        Some(code)
    }

    fn exact_call(&mut self, function: &str, texts: &[String]) -> Code {
        Code::atom(format!("{}({})", self.runtime(function), texts.join(", ")))
    }
}
