//! Monitors: `synchronized (x) { ... }`, which lowers to the lock `x` (`Builtin::MonitorEnter`)
//! followed by the block, and `Object`'s `wait`, `notify` and `notifyAll` and `Thread.holdsLock`,
//! which lower to natives. A synchronized method keeps its flag (`Method::is_synchronized`).

use super::*;

impl<'t> Lowering<'t> {
    /// `synchronized (x) block`: `x` is evaluated and checked for null, then the block runs, holding
    /// `x`'s monitor where the program can tell.
    pub(super) fn synchronized(&self, node: Node<'t>, out: &mut Vec<Stmt>) -> Result<(), Error> {
        let lock = self.expr(node.child("expression")?)?;
        let ty = lock.ty;
        let check = ExprKind::Builtin(Builtin::MonitorEnter, vec![lock]);
        out.push(Stmt::Expr(self.synth(check, node.pos(), ty)));
        self.stmt_into(node.child("block")?, out)
    }

    /// `o.wait()`, `o.wait(millis)`, `o.wait(millis, nanos)`, `o.notify()` and `o.notifyAll()`, with `this` as the object
    /// when the call names none; `None` for `Object`'s other methods.
    pub(super) fn monitor_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        receiver: Option<Node<'t>>,
        args: &mut Vec<Expr>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner != "java.lang.Object" || !matches!(symbol.name, "wait" | "notify" | "notifyAll") {
            return Ok(None);
        }
        let object = match receiver {
            Some(r) => self.expr(r)?,
            None => self.synth(ExprKind::This, node.pos(), self.self_type(self.current.get())),
        };
        self.check_lock(node, &object)?;
        let native = match (symbol.name, args.len()) {
            ("wait", 0) => {
                args.push(self.synth(ExprKind::Long(0), node.pos(), Type::Long));
                Native::MonitorWait
            }
            ("wait", _) => Native::MonitorWait,
            (name, _) => Native::MonitorNotify { all: name == "notifyAll" },
        };
        args.insert(0, object);
        Ok(Some(ExprKind::Builtin(Builtin::Native(native), std::mem::take(args))))
    }

    /// Rejects a monitor of what has no identity in the translation: a string or a box, which are
    /// values. A builder has its cell's monitor, and an array its elements'.
    pub(super) fn check_lock(&self, node: Node<'t>, lock: &Expr) -> Result<(), Error> {
        let what = match lock.ty {
            Type::String => "a string",
            Type::Boxed(_) => "a box",
            Type::Class(..) | Type::StringBuilder | Type::Array(_) => return Ok(()),
            _ => "a value",
        };
        unsupported(node, &format!("the monitor of {what}"))
    }
}
