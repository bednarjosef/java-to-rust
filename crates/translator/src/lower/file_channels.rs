//! The natives of `UnixFileDispatcherImpl` that take a `FileDescriptor` object. `map0` takes the
//! descriptor's number, the `fd` of the object, as the stream natives of `descriptor_call` do, so
//! the runtime's descriptor table finds the file. The other arguments are passed as they are.

use super::*;

impl<'t> Lowering<'t> {
    /// A call of `UnixFileDispatcherImpl.map0`, as `Native::MapFile` with the descriptor's number
    /// in place of the object; `None` for any other method.
    pub(super) fn file_channel_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if !symbol.native || symbol.owner != "sun.nio.ch.UnixFileDispatcherImpl" || symbol.name != "map0" {
            return Ok(None);
        }
        let pos = node.pos();
        let descriptor = args.remove(0);
        let field = Place::Field { object: Box::new(descriptor), name: "fd".into() };
        let number = self.synth(super::objects::read(field), pos, Type::Int);
        args.insert(0, number);
        Ok(Some(ExprKind::Builtin(Builtin::Native(Native::MapFile), std::mem::take(args))))
    }
}
