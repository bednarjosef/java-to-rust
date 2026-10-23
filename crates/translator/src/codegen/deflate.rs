//! The natives of `java.util.zip.Deflater`: the runtime's compressor (`runtime/deflate.rs`),
//! whose state is keyed by the handle that `init` returns.

use super::*;

impl<'a> Gen<'a> {
    /// A native of `Deflater`. As for `Inflater` (`inflate.rs`), the receiver of the instance
    /// natives is not passed: `args[0]` is the handle that `init` returned.
    pub(super) fn deflate_native(&mut self, native: Native, args: &'a [Expr], id: ExprId) -> Code {
        match native {
            Native::DeflateInit => {
                let level = self.value(&args[0], Type::Int).text;
                let strategy = self.value(&args[1], Type::Int).text;
                let nowrap = self.value(&args[2], Type::Boolean).text;
                let f = self.runtime("deflate_init");
                self.exception_call(id, format!("{f}({level}, {strategy}, {nowrap})"))
            }
            Native::DeflateAdler | Native::DeflateReset | Native::DeflateEnd => {
                let addr = self.value(&args[0], Type::Long).text;
                let f = match native {
                    Native::DeflateAdler => "deflate_adler",
                    Native::DeflateReset => "deflate_reset",
                    _ => "deflate_end",
                };
                self.runtime_call(f, vec![addr])
            }
            Native::DeflateSetDictionary => {
                let addr = self.value(&args[0], Type::Long).text;
                let dict = self.array_arg(&args[1], false);
                let [off, len] = self.hoisted_ints(args, [2, 3]);
                let f = self.runtime("deflate_set_dictionary");
                self.exception_call(id, format!("{f}({addr}, {dict}, {off}, {len})"))
            }
            Native::DeflateSetDictionaryBuffer => {
                let addr = self.value(&args[0], Type::Long).text;
                let buffer = self.value(&args[1], Type::Long).text;
                let [len] = self.hoisted_ints(args, [2]);
                let f = self.runtime("deflate_set_dictionary_buffer");
                self.exception_call(id, format!("{f}({addr}, {buffer}, {len})"))
            }
            Native::DeflateBytes => {
                let addr = self.value(&args[0], Type::Long).text;
                let [in_off, in_len, out_off, out_len, flush, params] =
                    self.hoisted_ints(args, [2, 3, 5, 6, 7, 8]);
                let input = self.input_copy(&args[1], &in_off, &in_len);
                let output = self.array_arg(&args[4], true);
                let args = vec![addr, format!("&{input}"), output, out_off, out_len, flush, params];
                self.runtime_call("deflate_bytes", args)
            }
            Native::DeflateBytesBuffer => {
                let addr = self.value(&args[0], Type::Long).text;
                let [in_off, in_len] = self.hoisted_ints(args, [2, 3]);
                let input = self.input_copy(&args[1], &in_off, &in_len);
                let output = self.value(&args[4], Type::Long).text;
                let [out_len, flush, params] = self.hoisted_ints(args, [5, 6, 7]);
                let args = vec![addr, format!("&{input}"), output, out_len, flush, params];
                self.runtime_call("deflate_bytes_buffer", args)
            }
            Native::DeflateBufferBytes => {
                let addr = self.value(&args[0], Type::Long).text;
                let input = self.value(&args[1], Type::Long).text;
                let [in_len, out_off, out_len, flush, params] = self.hoisted_ints(args, [2, 4, 5, 6, 7]);
                let output = self.array_arg(&args[3], true);
                let args = vec![addr, input, in_len, output, out_off, out_len, flush, params];
                self.runtime_call("deflate_buffer_bytes", args)
            }
            Native::DeflateBufferBuffer => {
                let addr = self.value(&args[0], Type::Long).text;
                let input = self.value(&args[1], Type::Long).text;
                let [in_len] = self.hoisted_ints(args, [2]);
                let output = self.value(&args[3], Type::Long).text;
                let [out_len, flush, params] = self.hoisted_ints(args, [4, 5, 6]);
                let args = vec![addr, input, in_len, output, out_len, flush, params];
                self.runtime_call("deflate_buffer_buffer", args)
            }
            _ => unreachable!("not a native of java.util.zip.Deflater: {native:?}"),
        }
    }
}
