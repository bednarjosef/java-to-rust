//! `synchronized (x) { ... }`, which lowers to the null check of `x` followed by the block.

use super::*;

impl<'t> Lowering<'t> {
    /// `synchronized (x) block`: `x` is evaluated and checked for null, then the block runs.
    pub(super) fn synchronized(&self, node: Node<'t>, out: &mut Vec<Stmt>) -> Result<(), Error> {
        let lock = self.expr(node.child("expression")?)?;
        let ty = lock.ty;
        let check = ExprKind::Builtin(Builtin::NullCheck, vec![lock]);
        out.push(Stmt::Expr(self.synth(check, node.pos(), ty)));
        self.stmt_into(node.child("block")?, out)
    }
}
