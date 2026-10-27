//! The natives of `sun.nio.ch` that file channels call (`Native::Channel`), as calls of the
//! runtime's `java_runtime::file_channel`. Their arguments are ints, longs, booleans, the
//! descriptor's number, which lowering reads from the `FileDescriptor`, and the `long[]` that
//! `FileKey.init` fills, borrowed mutably. A call that can fail goes through `fallible`, as the
//! file natives do.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn channel_native(&mut self, call: &ChannelCall, args: &'a [Expr], id: ExprId) -> Code {
        let values: Vec<String> = args
            .iter()
            .map(|arg| match arg.ty {
                Type::Array(_) => self.array_arg(arg, true),
                Type::Long | Type::Boolean => self.value(arg, arg.ty).text,
                _ => self.value(arg, Type::Int).text,
            })
            .collect();
        if call.fails {
            return self.fallible(id, call.function, values);
        }
        Code::atom(format!("{}({})", self.runtime(call.function), values.join(", ")))
    }
}
