//! Lowering for a census of what the translator rejects (`program::census`): an error in a piece
//! of code is recorded with the type it is in, and lowering goes on past it. A rejected method,
//! constructor or lambda becomes a stub, with its signature and a body that never runs, so the
//! code around it is still lowered and analysed; a rejected field initializer or initializer
//! block is left out.

use super::*;

impl<'t> Lowering<'t> {
    /// Records `error`, in code of the type with index `t`, when lowering for a census, with the
    /// position of the method or lambda being lowered; passes it on otherwise.
    pub(super) fn reject(&self, t: usize, error: Error) -> Result<(), Error> {
        self.reject_at(t, self.method_ctx.get().map(|m| self.methods[m].node.pos()), error)
    }

    /// `reject` in the code at `anchor`, the position of a method, a lambda or a class.
    pub(super) fn reject_at(&self, t: usize, anchor: Option<Pos>, error: Error) -> Result<(), Error> {
        match &self.census {
            Some(rejected) => {
                rejected.borrow_mut().push((t, anchor, error));
                Ok(())
            }
            None => Err(error),
        }
    }

    /// Method `index`, or in a census, when lowering rejects it, its stub.
    pub(super) fn method_or_stub(&self, index: usize) -> Result<Method, Error> {
        if self.census.is_none() {
            return self.method(index);
        }
        // A panic of lowering is a rejection of the method like any other.
        let ctx = (self.method_ctx.get(), self.aliases.borrow().len());
        let lowered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.method(index)));
        let lowered = lowered.unwrap_or_else(|panic| {
            self.method_ctx.set(ctx.0);
            self.aliases.borrow_mut().truncate(ctx.1);
            let pos = self.methods[index].node.pos();
            Err(Error { pos, message: format!("the translator panicked: {}", panic_message(&*panic)) })
        });
        match lowered {
            Err(error) => {
                self.reject_at(self.methods[index].owner, Some(self.methods[index].node.pos()), error)?;
                self.stubbed.borrow_mut().insert(index);
                self.stubbing.set(true);
                let stub = self.method(index);
                self.stubbing.set(false);
                stub
            }
            method => method,
        }
    }

    /// The declaration of type `t`, or in a census, when lowering rejects it, the declaration with
    /// its fields and captured locals of a type that is not supported as `int`s: the code that
    /// uses them lowers the same types, and is rejected too.
    pub(super) fn type_decl_or_stub(&self, t: usize) -> Result<TypeDecl, Error> {
        match self.type_decl(t) {
            Err(error) if self.census.is_some() => {
                self.reject_at(t, Some(self.types[t].node.pos()), error)?;
                self.stubbing.set(true);
                let stub = self.type_decl(t);
                self.stubbing.set(false);
                stub
            }
            decl => decl,
        }
    }

    /// A type of a method's signature. A stub keeps the parameters and the result of a type that
    /// is not supported, as `int`s: a call of it is rejected anyway, as its caller lowers the same
    /// types.
    pub(super) fn signature_type(&self, node: Node, name: &str) -> Result<Type, Error> {
        match self.ty(node, name) {
            Err(_) if self.stubbing.get() => Ok(Type::Int),
            ty => ty,
        }
    }

    /// The body of a method that never runs: a dormant method's, or a stub's.
    pub(super) fn never(&self, class: usize, pos: Pos) -> Vec<Stmt> {
        let never = ExprKind::Builtin(Builtin::NoObject { class }, Vec::new());
        vec![Stmt::Expr(self.synth(never, pos, Type::Void))]
    }
}

/// The message of a panic.
pub fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    let text = panic.downcast_ref::<String>().cloned();
    text.or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}
