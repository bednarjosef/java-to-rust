//! Lowering for a census of what the translator rejects (`program::census`): an error in a piece
//! of code is recorded with the type it is in, and lowering goes on past it. A rejected method,
//! constructor or lambda becomes a stub, with its signature and a body that never runs, so the
//! code around it is still lowered and analysed; a rejected field initializer or initializer
//! block is left out. A class whose declaration is rejected has all of its code rejected with it
//! (`reject_class`).

use super::*;

impl<'t> Lowering<'t> {
    /// Records `error`, in code of the type with index `t`, when lowering for a census, with the
    /// position of the method or lambda being lowered; passes it on otherwise. The code left out
    /// is part of that method's body alone.
    pub(super) fn reject(&self, t: usize, error: Error) -> Result<(), Error> {
        let method = self.method_ctx.get();
        self.record(t, method.map(|m| self.methods[m].node.pos()), method, error)
    }

    /// `reject` in the code at `anchor`, the position of a method, a lambda or a class, which
    /// may be code of more than one method.
    pub(super) fn reject_at(&self, t: usize, anchor: Option<Pos>, error: Error) -> Result<(), Error> {
        self.record(t, anchor, None, error)
    }

    /// `reject_at`, in the body of method `method` alone when it is given.
    fn record(
        &self,
        t: usize,
        anchor: Option<Pos>,
        method: Option<usize>,
        error: Error,
    ) -> Result<(), Error> {
        match &self.census {
            Some(rejected) => {
                rejected.borrow_mut().push((t, anchor, method, error));
                Ok(())
            }
            None => Err(error),
        }
    }

    /// Records, in a census, that the declaration of class `t` is rejected for `error` (the
    /// first error of its declaration, when it has several), which the class's code is rejected
    /// for too (`method_or_stub`, `type_decl_or_stub`); passes the error on otherwise.
    pub(super) fn reject_class(&self, t: usize, error: Result<(), Error>) -> Result<(), Error> {
        let Err(error) = error else { return Ok(()) };
        if self.census.is_none() {
            return Err(error);
        }
        self.rejected_classes.borrow_mut().entry(t).or_insert(error);
        Ok(())
    }

    /// The error that the declaration of class `t` was rejected for in a census, if it was.
    fn class_rejection(&self, t: usize) -> Option<Error> {
        let rejected = self.rejected_classes.borrow();
        rejected.get(&t).map(|e| Error { pos: e.pos, message: e.message.clone() })
    }

    /// The rejection, in a census, of a piece of code that javac reported a compile error in,
    /// which javac marks when it goes on past compile errors (`Program::keep_going`): a method,
    /// a lambda, an initializer block of a class, a field's initializer, or a class whose
    /// declaration has the error.
    pub(super) fn javac_error(&self, node: Node) -> Result<(), Error> {
        if self.census.is_none() {
            return Ok(());
        }
        match node.0.get("javacError").and_then(Value::as_str) {
            Some(code) => {
                Err(Error { pos: node.pos(), message: format!("javac reports a compile error ({code})") })
            }
            None => Ok(()),
        }
    }

    /// Checks the declarations of the classes in a census, before anything else asks for their
    /// superclasses and interfaces: those that are rejected are known before their code is
    /// lowered.
    pub(super) fn check_class_headers(&self) {
        if self.census.is_some() {
            for t in 0..self.types.len() {
                let _ = self.reject_class(t, self.javac_error(self.types[t].node));
                let _ = self.superclass(t);
                let _ = self.interfaces(t);
            }
        }
    }

    /// Method `index`, or in a census, when lowering rejects it, its stub.
    pub(super) fn method_or_stub(&self, index: usize) -> Result<Method, Error> {
        if self.census.is_none() {
            return self.method(index);
        }
        // The code of a class whose declaration is rejected is rejected with it.
        let owner = self.methods[index].owner;
        if let Some(error) = self.class_rejection(owner) {
            let message = self.stub_message(index, &error);
            self.reject_at(owner, Some(self.methods[index].node.pos()), error)?;
            self.stubbed.borrow_mut().insert(index);
            return self.stub(index, message);
        }
        if let Err(error) = self.javac_error(self.methods[index].node) {
            return self.stub_if_rejected(index, Err(error));
        }
        // A panic of lowering is a rejection of the method like any other.
        let ctx = (self.method_ctx.get(), self.aliases.borrow().len());
        if !self.catch_panics {
            return self.stub_if_rejected(index, self.method(index));
        }
        let lowered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.method(index)));
        let lowered = lowered.unwrap_or_else(|panic| {
            self.method_ctx.set(ctx.0);
            self.aliases.borrow_mut().truncate(ctx.1);
            let pos = self.methods[index].node.pos();
            Err(Error { pos, message: format!("the translator panicked: {}", panic_message(&*panic)) })
        });
        self.stub_if_rejected(index, lowered)
    }

    /// Method `index` as lowering gave it, `lowered`, or when lowering rejected it, its stub.
    fn stub_if_rejected(&self, index: usize, lowered: Result<Method, Error>) -> Result<Method, Error> {
        match lowered {
            Err(error) => {
                let message = self.stub_message(index, &error);
                let anchor = Some(self.methods[index].node.pos());
                self.record(self.methods[index].owner, anchor, Some(index), error)?;
                self.stubbed.borrow_mut().insert(index);
                self.stub(index, message)
            }
            method => method,
        }
    }

    /// The stub of method `index`, whose body names the rejection `message`.
    fn stub(&self, index: usize, message: String) -> Result<Method, Error> {
        self.stubbing.set(true);
        *self.stub_message.borrow_mut() = Some(message);
        let stub = self.method(index);
        self.stub_message.borrow_mut().take();
        self.stubbing.set(false);
        stub
    }

    /// The body of a stub of method `index` of class `class`, rejected for `error`.
    pub(super) fn stub_body(&self, class: usize, index: usize, error: &Error) -> Vec<Stmt> {
        *self.stub_message.borrow_mut() = Some(self.stub_message(index, error));
        let body = self.never(class, self.methods[index].node.pos());
        self.stub_message.borrow_mut().take();
        body
    }

    /// What the stub of method `index`, rejected for `error`, says where it runs: the method, and
    /// the rejection with its line.
    fn stub_message(&self, index: usize, error: &Error) -> String {
        let method = &self.methods[index];
        format!(
            "{}.{}: {} (line {})",
            self.types[method.owner].javac, method.name, error.message, error.pos.line
        )
    }

    /// The code in place of a rejected field initializer or initializer block of type `t` where a
    /// census writes its translation (`ends_rejected`): a stub of type `ty` that ends the program
    /// with the rejection, `error`.
    pub(super) fn rejected_initializer(&self, t: usize, error: &Error, pos: Pos, ty: Type) -> Option<Expr> {
        if !self.ends_rejected {
            return None;
        }
        let message = format!("{}: {} (line {})", self.types[t].javac, error.message, error.pos.line);
        let message = self.synth(ExprKind::Str(message), pos, Type::String);
        Some(self.synth(ExprKind::Builtin(Builtin::NoObject { class: t }, vec![message]), pos, ty))
    }

    /// The declaration of type `t`, or in a census, when lowering rejects it, the declaration with
    /// its fields and captured locals of a type that is not supported as `int`s: the code that
    /// uses them lowers the same types, and is rejected too.
    pub(super) fn type_decl_or_stub(&self, t: usize) -> Result<TypeDecl, Error> {
        // The field initializers and initializer blocks of a class whose declaration is rejected
        // are rejected with it, and left out (`type_decl`).
        if let Some(error) = self.class_rejection(t) {
            let javac = &self.types[t].javac;
            let node = self.types[t].node;
            let initializers = node.list("members").filter(|m| match m.kind() {
                "VARIABLE" => m.get("initializer").is_some() && !enums::is_enum_constant(*m),
                kind => kind == "BLOCK",
            });
            for member in initializers.filter(|m| self.keeps(*m, javac)) {
                let error = Error { pos: error.pos, message: error.message.clone() };
                self.reject_at(t, Some(member.pos()), error)?;
            }
            self.stubbing.set(true);
            let stub = self.type_decl(t);
            self.stubbing.set(false);
            return stub;
        }
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

    /// The body of a method that never runs: a dormant method's, or a stub's, which names its
    /// rejection.
    pub(super) fn never(&self, class: usize, pos: Pos) -> Vec<Stmt> {
        let message = self.stub_message.borrow().clone();
        let message = message.map(|m| self.synth(ExprKind::Str(m), pos, Type::String));
        let never = ExprKind::Builtin(Builtin::NoObject { class }, message.into_iter().collect());
        vec![Stmt::Expr(self.synth(never, pos, Type::Void))]
    }
}

/// The message of a panic.
pub fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    let text = panic.downcast_ref::<String>().cloned();
    let text = text.or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
    match PANIC_PLACE.with(|p| p.take()) {
        Some(place) => format!("{text} (at {place})"),
        None => text,
    }
}

thread_local! {
    /// Where in the translator the last panic of this thread happened (`quiet_panics`).
    static PANIC_PLACE: std::cell::Cell<Option<String>> = const { std::cell::Cell::new(None) };
}

/// Where in the translator the last panic of this thread happened, taken: for a panic that
/// another thread goes on with (`panic_carried`).
pub fn panic_place() -> Option<String> {
    PANIC_PLACE.with(|p| p.take())
}

/// Makes `place` where the last panic of this thread happened: one of another thread, which this
/// one goes on with (`std::panic::resume_unwind`, which does not run the hook).
pub fn panic_carried(place: Option<String>) {
    PANIC_PLACE.with(|p| p.set(place));
}

/// Makes panics quiet, as rejections in a census are: each is reported with the rest, not
/// printed, with the place in the translator where it happened (`panic_message`).
pub fn quiet_panics() {
    std::panic::set_hook(Box::new(|info| {
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line()));
        PANIC_PLACE.with(|p| p.set(place));
    }));
}
