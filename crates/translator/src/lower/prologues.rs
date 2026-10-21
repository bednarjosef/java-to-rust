//! Java 25's flexible constructor bodies: statements before a constructor's `this(..)` or
//! `super(..)`, its prologue (the JDK's `SortedOps.OfRef` computes its comparator there). They
//! cannot use `this`, except to assign the class's fields that have no initializer.
//!
//! The prologue stays before the call: the generator writes it before it builds the object, so
//! its locals are in scope for the call's arguments and the rest of the body. An assignment of a
//! field becomes a local, `early$f`, which `this.f = early$f` right after the call stores in the
//! object once it exists: the superclass's constructor of a plain fields struct cannot see the
//! subclass's fields, and the second constructor form, where it can, moves those stores before
//! the superclass's part (see `analysis::whole_objects`).

use super::*;

impl<'t> Lowering<'t> {
    /// Prepares the prologue of constructor `node` of class `owner`, whose lowered statements are
    /// `body`.
    pub(super) fn constructor_prologue(
        &self,
        node: Node<'t>,
        owner: usize,
        body: &mut Vec<Stmt>,
    ) -> Result<(), Error> {
        let Some(javac_body) = node.get("body") else { return Ok(()) };
        let explicit = javac_body.list("statements").position(|s| is_super_call(s) || is_this_call(s));
        let Some(written) = explicit.filter(|&i| i > 0) else { return Ok(()) };
        let Some(at) = explicit_call(body) else {
            // `super()` of `Object`, which does nothing and is left out: the prologue is the start
            // of the body. The fields' initializers would run before it, not after.
            let class = self.types[owner].node;
            let initialized = class.list("members").any(|m| {
                m.kind() == "VARIABLE" && !m.has_flag("STATIC") && m.get("initializer").is_some()
                    || m.kind() == "BLOCK" && !Self::is_static_block(m)
            });
            if initialized {
                let stmt = javac_body.list("statements").nth(written).unwrap_or(node);
                return unsupported(stmt, "statements before `super()` in a class with field initializers");
            }
            return Ok(());
        };
        let delegating = delegates(body);
        let mut stores = Vec::new();
        for stmt in &mut body[..at] {
            let Stmt::Expr(Expr {
                kind: ExprKind::Assign { op: None, target: Place::Field { object, name }, value },
                pos,
                ty,
                ..
            }) = stmt
            else {
                continue;
            };
            if !matches!(object.kind, ExprKind::This) || mentions_this(value) {
                continue;
            }
            if delegating {
                return unsupported(node, "a field assigned before `this(..)`");
            }
            let (pos, ty, name) = (*pos, *ty, name.clone());
            let local = early_field_local(&name);
            let value = std::mem::replace(value.as_mut(), self.synth(ExprKind::Null, pos, Type::Null));
            *stmt = Stmt::Local { ty, name: local.clone(), init: Some(value), pos };
            let object = Box::new(self.this_at(pos, owner));
            let value = Box::new(self.synth(ExprKind::Var(local), pos, ty));
            let kind = ExprKind::Assign { op: None, target: Place::Field { object, name }, value };
            stores.push(Stmt::Expr(self.synth(kind, pos, ty)));
        }
        // Anything else that uses `this` (an enclosing instance, a field assigned inside a
        // statement) is not translated yet.
        for stmt in &body[..at] {
            let mut uses = false;
            visit_stmt(stmt, &mut |e| uses |= mentions_this(e), &mut |_| {});
            if uses {
                return unsupported(node, "this use of `this` before `this(..)` or `super(..)`");
            }
        }
        body.splice(at + 1..at + 1, stores);
        Ok(())
    }

    /// `this` of class `owner`.
    fn this_at(&self, pos: Pos, owner: usize) -> Expr {
        self.synth(ExprKind::This, pos, self.self_type(owner))
    }
}

/// `this(...)`, a constructor calling another one of its class.
fn is_this_call(stmt: Node) -> bool {
    stmt.get("expression").is_some_and(|call| {
        call.kind() == "METHOD_INVOCATION"
            && call.get("methodSelect").is_some_and(|s| s.kind() == "IDENTIFIER" && s.str("name") == "this")
    })
}
