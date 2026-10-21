//! `BootLoader.loadLibrary(name)`, which the JDK calls to load one of its native libraries (`zip`
//! for `java.util.zip`, `net` for the sockets). A translated program has no native code to load,
//! so the call does nothing: the method is one of `REPLACED_METHODS`, and its body, which reaches
//! the native library loader, is never translated.

use super::*;

impl<'t> Lowering<'t> {
    /// `BootLoader.loadLibrary(name)` as a call that does nothing, with the argument dropped. The
    /// argument must be a literal or a variable, which reads nothing else. `None` for any other call.
    pub(super) fn library_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        args: &[Expr],
    ) -> Result<Option<ExprKind>, Error> {
        if (symbol.owner, symbol.name) != ("jdk.internal.loader.BootLoader", "loadLibrary") {
            return Ok(None);
        }
        if !matches!(args, [arg] if matches!(arg.kind, ExprKind::Str(_) | ExprKind::Var(_))) {
            return unsupported(node, "`BootLoader.loadLibrary` of anything but a literal or a variable");
        }
        Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())))
    }
}
