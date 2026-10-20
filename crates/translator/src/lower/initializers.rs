//! Instance initializer blocks (`{ ... }` in a class body). Java runs them with the field
//! initializers, in textual order, at the start of every constructor that does not call another
//! one with `this(...)`. Field initializers are written into each constructor before its body
//! (see `codegen/objects.rs`), so a block runs right after them, before the rest of the body.
//! That is Java's order as long as no field initializer follows a block, which is rejected.

use super::*;

impl<'t> Lowering<'t> {
    /// Rejects the instance initializer block `block` of `class` when a field with an initializer
    /// is declared after it: that initializer would run before the block, not after.
    pub(super) fn check_initializer_block(&self, class: Node<'t>, block: Node<'t>) -> Result<(), Error> {
        let later = class
            .list("members")
            .skip_while(|m| m.0 != block.0)
            .find(|m| m.kind() == "VARIABLE" && !m.has_flag("STATIC") && m.get("initializer").is_some());
        match later {
            Some(field) => unsupported(field, "a field initializer after an instance initializer block"),
            None => Ok(()),
        }
    }

    /// Puts the instance initializer blocks of class `owner` at the start of a constructor's
    /// `body` (after its `super(...)`), unless the constructor first calls another one, which runs them.
    pub(super) fn run_initializer_blocks(&self, owner: usize, body: &mut Vec<Stmt>) -> Result<(), Error> {
        if matches!(body.first(), Some(Stmt::Expr(Expr { kind: ExprKind::ThisCall { .. }, .. }))) {
            return Ok(());
        }
        let class = &self.types[owner];
        let blocks: Vec<Node<'t>> = class
            .node
            .list("members")
            .filter(|m| m.kind() == "BLOCK" && !Self::is_static_block(*m) && self.keeps(*m, &class.name))
            .collect();
        let mut lowered = Vec::new();
        for block in blocks {
            let stmts = self.block(block)?;
            // A block that declares no locals needs no scope of its own.
            if stmts.iter().any(|s| matches!(s, Stmt::Local { .. })) {
                lowered.push(Stmt::Block(stmts));
            } else {
                lowered.extend(stmts);
            }
        }
        // They run after the superclass's constructor.
        let at = usize::from(matches!(
            body.first(),
            Some(Stmt::Expr(Expr { kind: ExprKind::SuperInit { .. }, .. }))
        ));
        body.splice(at..at, lowered);
        Ok(())
    }
}
