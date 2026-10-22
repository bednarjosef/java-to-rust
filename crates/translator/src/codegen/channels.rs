//! The natives of `sun.nio.ch` that file channels call (`Native::Channel`), as calls of the
//! runtime's `java_runtime::file_channel`. Their arguments are ints, longs and the descriptor's
//! number, which lowering reads from the `FileDescriptor`. A call that can fail goes through
//! `fallible`, as the file natives do.

use super::*;

impl<'a> Gen<'a> {
    pub(super) fn channel_native(&mut self, call: &ChannelCall, args: &'a [Expr], id: ExprId) -> Code {
        let values: Vec<String> = args
            .iter()
            .map(|arg| {
                let ty = if arg.ty == Type::Long { Type::Long } else { Type::Int };
                self.value(arg, ty).text
            })
            .collect();
        if call.fails {
            return self.fallible(id, call.function, values);
        }
        Code::atom(format!("{}({})", self.runtime(call.function), values.join(", ")))
    }
}
