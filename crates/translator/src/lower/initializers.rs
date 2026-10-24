//! Instance initializer blocks (`{ ... }` in a class body). Java runs them with the field
//! initializers, in textual order, at the start of every constructor that does not call another
//! one with `this(...)`. The field initializers before the first block are written into each
//! constructor before its body (see `codegen/objects.rs`), and the blocks run right after them,
//! each followed by the field initializers written between it and the next block, as
//! assignments: Java's order.

use super::*;

impl<'t> Lowering<'t> {
    /// Whether the instance field `field` of `class` has an initializer that runs among the
    /// initializer blocks: one written after the first of them.
    pub(super) fn runs_after_a_block(&self, class: Node<'t>, field: Node<'t>) -> bool {
        field.kind() == "VARIABLE"
            && !field.has_flag("STATIC")
            && !enums::is_enum_constant(field)
            && field.get("initializer").is_some()
            && class
                .list("members")
                .take_while(|m| m.0 != field.0)
                .any(|m| m.kind() == "BLOCK" && !Self::is_static_block(m))
    }

    /// Puts the instance initializer blocks of class `owner`, with the field initializers written
    /// after the first of them, at the start of a constructor's `body` (after its `super(...)`),
    /// unless the constructor first calls another one, which runs them.
    pub(super) fn run_initializer_blocks(&self, owner: usize, body: &mut Vec<Stmt>) -> Result<(), Error> {
        if delegates(body) {
            return Ok(());
        }
        let class = &self.types[owner];
        let members: Vec<Node<'t>> = class
            .node
            .list("members")
            .filter(|m| self.keeps(*m, &class.javac))
            .filter(|m| {
                (m.kind() == "BLOCK" && !Self::is_static_block(*m)) || self.runs_after_a_block(class.node, *m)
            })
            .collect();
        let mut lowered = Vec::new();
        for member in members {
            if member.kind() == "VARIABLE" {
                lowered.extend(self.late_field_initializer(owner, member)?);
                continue;
            }
            // In a census, a rejected block is left out of the constructor, or ends the program
            // where the census writes its translation.
            let initializing = self.initializing.replace(true);
            let stmts = self.block(member);
            self.initializing.set(initializing);
            let stmts = match stmts {
                Ok(stmts) => stmts,
                Err(e) => {
                    lowered.extend(
                        self.rejected_initializer(owner, &e, member.pos(), Type::Void).map(Stmt::Expr),
                    );
                    self.reject(owner, e)?;
                    continue;
                }
            };
            // A block that declares no locals needs no scope of its own.
            if stmts.iter().any(|s| matches!(s, Stmt::Local { .. })) {
                lowered.push(Stmt::Block(stmts));
            } else {
                lowered.extend(stmts);
            }
        }
        // They run after the superclass's constructor.
        let at = explicit_call(body).map_or(0, |i| i + 1);
        body.splice(at..at, lowered);
        Ok(())
    }

    /// The initializer of `field`, a field of class `owner` written after an initializer block,
    /// as an assignment to the field. In a census, a rejected initializer leaves the field at its
    /// default value, or ends the program where the census writes its translation.
    fn late_field_initializer(&self, owner: usize, field: Node<'t>) -> Result<Option<Stmt>, Error> {
        let ty = self.ty(field, field.symbol().map_or("", |s| s.ty))?;
        let initializing = self.initializing.replace(true);
        let value = field.child("initializer").and_then(|i| self.expr(i));
        self.initializing.set(initializing);
        let value = match value {
            Ok(value) => self.convert(value, ty),
            Err(e) => {
                let stub = self.rejected_initializer(owner, &e, field.pos(), Type::Void).map(Stmt::Expr);
                self.reject(owner, e)?;
                return Ok(stub);
            }
        };
        let pos = field.pos();
        let this = self.synth(ExprKind::This, pos, self.self_type(owner));
        let target = Place::Field { object: Box::new(this), name: field.str("name").to_string() };
        let kind = ExprKind::Assign { op: None, target, value: Box::new(value) };
        Ok(Some(Stmt::Expr(self.synth(kind, pos, ty))))
    }
}
