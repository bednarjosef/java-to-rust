//! `BootLoader.loadLibrary(name)`, which the JDK calls to load one of its native libraries (`zip`
//! for `java.util.zip`, `net` for the sockets). A translated program has no native code to load,
//! so the call does nothing: the method is one of `REPLACED_METHODS`, and its body, which reaches
//! the native library loader, is never translated.
//!
//! And `CleanerImpl.start(cleaner, threadFactory)`, which starts the thread of a `Cleaner`: it
//! waits for the garbage collector to enqueue the phantom references of the objects registered
//! with the cleaner, and runs their cleaning actions. A translated program has no garbage
//! collector, so no reference is ever enqueued (see `references.rs`) and the thread would wait
//! for good; it is a daemon, which never keeps a program running, so a program without it does
//! the same. The call does nothing, and a program that only makes a cleaner (an `Inflater` does,
//! for the memory of its stream) starts no thread. A cleaning action still runs when `clean()`
//! is called, as `Inflater.end()` does.

use super::*;

impl<'t> Lowering<'t> {
    /// `BootLoader.loadLibrary(name)` as a call that does nothing, with the argument dropped. The
    /// argument must be a literal or a variable, which reads nothing else. And
    /// `CleanerImpl.start(cleaner, threadFactory)`, whose receiver must be a variable or a field
    /// of one, and its arguments variables or `null`. `None` for any other call.
    pub(super) fn library_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        receiver: Option<Node<'t>>,
        args: &[Expr],
    ) -> Result<Option<ExprKind>, Error> {
        if (symbol.owner, symbol.name) == ("jdk.internal.ref.CleanerImpl", "start") {
            let plain = |r: Node<'t>| match r.kind() {
                "IDENTIFIER" => true,
                "MEMBER_SELECT" => r.0["expression"]["kind"] == "IDENTIFIER",
                _ => false,
            };
            let reads_nothing = args.iter().all(|a| matches!(a.kind, ExprKind::Var(_) | ExprKind::Null));
            if !receiver.is_some_and(plain) || !reads_nothing {
                return unsupported(node, "`CleanerImpl.start` of anything but variables and their fields");
            }
            return Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())));
        }
        if (symbol.owner, symbol.name) != ("jdk.internal.loader.BootLoader", "loadLibrary") {
            return Ok(None);
        }
        if !matches!(args, [arg] if matches!(arg.kind, ExprKind::Str(_) | ExprKind::Var(_))) {
            return unsupported(node, "`BootLoader.loadLibrary` of anything but a literal or a variable");
        }
        Ok(Some(ExprKind::Builtin(Builtin::Native(Native::Nothing), Vec::new())))
    }
}
