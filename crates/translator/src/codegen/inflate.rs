//! The natives of `java.util.zip.Inflater`: the runtime's decompressor (`runtime/inflate.rs`),
//! whose state is keyed by the handle that `init` returns.

use super::*;

impl<'a> Gen<'a> {
    /// A native of `Inflater`. They are instance methods, but the receiver is not passed: `args[0]`
    /// is the handle that `init` returned, which the runtime keeps the stream by.
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
                let f = self.runtime("inflate_set_dictionary");
                self.exception_call(id, format!("{f}({addr}, {dict}, {off}, {len})"))
            }
            Native::InflateSetDictionaryBuffer => {
                let addr = self.value(&args[0], Type::Long).text;
                let buffer = self.value(&args[1], Type::Long).text;
                let [len] = self.hoisted_ints(args, [2]);
                let f = self.runtime("inflate_set_dictionary_buffer");
                self.exception_call(id, format!("{f}({addr}, {buffer}, {len})"))
            }
            Native::InflateBytes => {
                let addr = self.value(&args[0], Type::Long).text;
                let [in_off, in_len, out_off, out_len] = self.hoisted_ints(args, [2, 3, 5, 6]);
                let input = self.input_copy(&args[1], &in_off, &in_len);
                let output = self.array_arg(&args[4], true);
                self.fallible(id, "inflate_bytes", vec![addr, format!("&{input}"), output, out_off, out_len])
            }
            Native::InflateBytesBuffer => {
                let addr = self.value(&args[0], Type::Long).text;
                let [in_off, in_len] = self.hoisted_ints(args, [2, 3]);
                let input = self.input_copy(&args[1], &in_off, &in_len);
                let output = self.value(&args[4], Type::Long).text;
                let [out_len] = self.hoisted_ints(args, [5]);
                self.fallible(id, "inflate_bytes_buffer", vec![addr, format!("&{input}"), output, out_len])
            }
            Native::InflateBufferBytes => {
                let addr = self.value(&args[0], Type::Long).text;
                let input = self.value(&args[1], Type::Long).text;
                let [in_len, out_off, out_len] = self.hoisted_ints(args, [2, 4, 5]);
                let output = self.array_arg(&args[3], true);
                self.fallible(id, "inflate_buffer_bytes", vec![addr, input, in_len, output, out_off, out_len])
            }
            Native::InflateBufferBuffer => {
                let addr = self.value(&args[0], Type::Long).text;
                let input = self.value(&args[1], Type::Long).text;
                let [in_len, out_len] = self.hoisted_ints(args, [2, 4]);
                let output = self.value(&args[3], Type::Long).text;
                self.fallible(id, "inflate_buffer_buffer", vec![addr, input, in_len, output, out_len])
            }
            _ => unreachable!("not a native of java.util.zip.Inflater: {native:?}"),
        }
    }

    pub(super) fn runtime_call(&mut self, f: &str, args: Vec<String>) -> Code {
        Code::atom(format!("{}({})", self.runtime(f), args.join(", ")))
    }

    /// The input bytes of `inflateBytesBytes` or `inflateBytesBuffer`, copied into a temporary,
    /// which it returns: the output may be the same array, which cannot be borrowed twice. Only
    /// the bytes of the input are read (`java::copy_range`). The copy runs ahead of the statement
    /// (`ahead_of_statement`).
    pub(super) fn input_copy(&mut self, input: &'a Expr, off: &str, len: &str) -> String {
        let source = self.ahead_of_statement(|g| g.slice_of(input));
        let copy = self.temp("input");
        let range = self.runtime("copy_range");
        self.line(&format!("let {copy} = {range}({source}, {off}, {len});"));
        copy
    }

    /// A native whose runtime function answers a `Result` with Java's exception, such as
    /// `setDictionary`'s `IllegalArgumentException`: raised where a `catch` may see it
    /// (`Facts::raising`), else uncaught.
    pub(super) fn exception_call(&mut self, id: ExprId, call: String) -> Code {
        if self.facts.raising.contains(&id) {
            return Code::atom(self.passed_on(Code::atom(call)));
        }
        let throw = self.runtime("throw_exception");
        Code::atom(format!("{call}.unwrap_or_else(|e| {throw}(e))"))
    }
}
