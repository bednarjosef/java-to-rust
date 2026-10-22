//! The natives of `java.util.zip.Inflater`: the runtime's decompressor (`runtime/inflate.rs`),
//! whose state is keyed by the handle that `init` returns.

use super::*;

impl<'a> Gen<'a> {
    /// A native of `Inflater`. `inflateBytesBytes` is an instance method, so its receiver is
    /// `args[0]`; the runtime keeps the stream by its handle and does not need the receiver.
    pub(super) fn inflate_native(&mut self, native: Native, args: &'a [Expr], id: ExprId) -> Code {
        match native {
            Native::InflateInit => {
                let nowrap = self.value(&args[0], Type::Boolean).text;
                self.runtime_call("inflate_init", vec![nowrap])
            }
            Native::InflateAdler | Native::InflateReset | Native::InflateEnd => {
                let addr = self.value(&args[0], Type::Long).text;
                let f = match native {
                    Native::InflateAdler => "inflate_adler",
                    Native::InflateReset => "inflate_reset",
                    _ => "inflate_end",
                };
                self.runtime_call(f, vec![addr])
            }
            Native::InflateSetDictionary => {
                let addr = self.value(&args[0], Type::Long).text;
                let dict = self.array_arg(&args[1], false);
                let [off, len] = self.hoisted_ints(args, [2, 3]);
                let call =
                    format!("{}({addr}, {dict}, {off}, {len})", self.runtime("inflate_set_dictionary"));
                if self.facts.raising.contains(&id) {
                    Code::atom(self.passed_on(Code::atom(call)))
                } else {
                    let throw = self.runtime("throw_exception");
                    Code::atom(format!("{call}.unwrap_or_else(|e| {throw}(e))"))
                }
            }
            Native::InflateBytes => {
                let addr = self.value(&args[1], Type::Long).text;
                let [in_off, in_len, out_off, out_len] = self.hoisted_ints(args, [3, 4, 6, 7]);
                // The input is copied: the output may be the same array, which cannot be borrowed
                // twice.
                let source = self.slice_of(&args[2]);
                let input = self.temp("input");
                self.line(&format!(
                    "let {input} = ({source})[{in_off} as usize..({in_off} + {in_len}) as usize].to_vec();"
                ));
                let output = self.array_arg(&args[5], true);
                self.fallible(id, "inflate_bytes", vec![addr, format!("&{input}"), output, out_off, out_len])
            }
            _ => unreachable!("not a native of java.util.zip.Inflater: {native:?}"),
        }
    }

    fn runtime_call(&mut self, f: &str, args: Vec<String>) -> Code {
        Code::atom(format!("{}({})", self.runtime(f), args.join(", ")))
    }
}
