//! The natives of `sun.nio.ch` that file channels call (`ast::CHANNEL_CALLS`), which act on a
//! file through its descriptor, as `FileDescriptor`'s `fd` field gives it.

use super::{Lowering, Node, Symbol, unsupported};
use crate::ast::*;

impl<'t> Lowering<'t> {
    /// A native of `sun.nio.ch` that the runtime implements, as `Native::Channel` with its
    /// arguments as Java gives them. A `UnixFileDispatcherImpl` native takes a `FileDescriptor`
    /// first; that argument becomes the descriptor's number, read from its `fd` field. The natives
    /// that do nothing become `Native::Nothing`. `None` for any other method.
    pub(super) fn channel_native_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if !symbol.native {
            return Ok(None);
        }
        if crate::ast::channel_nothing(symbol.owner, symbol.name) {
            return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())));
        }
        let Some(call) = crate::ast::channel_call(symbol.owner, symbol.name) else {
            return Ok(None);
        };
        if symbol.owner == crate::ast::UNIX_FILE_DISPATCHER {
            let Some(first) = args.first_mut() else {
                return unsupported(
                    node,
                    &format!("native method `{}.{}` without a descriptor", symbol.owner, symbol.name),
                );
            };
            let pos = first.pos;
            let file = Box::new(std::mem::replace(first, self.synth(ExprKind::Null, pos, Type::String)));
            let number = Place::Field { object: file, name: "fd".into() };
            *first = self.synth(super::objects::read(number), pos, Type::Int);
        }
        Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Channel(call)), std::mem::take(args))))
    }
}
