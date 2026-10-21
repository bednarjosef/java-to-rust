//! Making a new exception of another's class, as `ForkJoinTask.getException` does by reflection
//! for an exception that a task threw in another thread, so that the one the joining thread
//! throws has its own stack trace (`program::recreated_exception` finds the code):
//!
//! ```text
//! Constructor<?> noArgCtor = null, oneArgCtor = null;
//! for (Constructor<?> c : ex.getClass().getConstructors()) { .. }
//! if (oneArgCtor != null) ex = (Throwable) oneArgCtor.newInstance(ex);
//! else if (noArgCtor != null) { Throwable rx = (Throwable) noArgCtor.newInstance(); rx.initCause(ex); ex = rx; }
//! ```
//!
//! A translated program has no reflection, but it knows every class whose objects it creates. So
//! the `try` block is a test of the class of `ex` against each exception class of the program
//! that reflection could use, with the constructor that reflection would choose:
//!
//! ```text
//! if (ex.getClass() == A.class) ex = new A(ex);
//! else if (ex.getClass() == B.class) { Throwable rx = new B(); rx.initCause(ex); ex = rx; }
//! ..
//! ```
//!
//! Reflection uses a public constructor that takes one `Throwable`, else a public one without
//! parameters, of a class that the caller may use: a public one (or protected, for a member
//! class), or one in the caller's package. An inner, local or anonymous class has none such, as
//! its constructors take the enclosing instance or captured variables first. A class with neither
//! constructor keeps `ex` unchanged, as the JDK does. The `catch` stays, for a constructor or an
//! `initCause` that throws. Reachability (`program::reach`) and the standard classes'
//! translation (`exceptions::recreating_constructor`) keep the constructors that this needs.

use super::*;

/// The constructors that reflection chooses from, in its order of preference.
const RECREATING: [&str; 2] = ["(java.lang.Throwable)void", "()void"];

impl<'t> Lowering<'t> {
    /// The statements that stand for the block of the `try` statement `node` when it makes a new
    /// exception of another's class (see the module's comment); `None` for any other `try`.
    pub(super) fn recreation(&self, node: Node<'t>) -> Result<Option<Vec<Stmt>>, Error> {
        if crate::program::recreated_exception(node.0).is_none() {
            return Ok(None);
        }
        let pos = node.pos();
        // The `ex` of `ex.getClass().getConstructors()`, the loop's expression.
        let loop_ = node.child("block")?.list("statements").nth(2).expect("recreated_exception's loop");
        let get_class = loop_.child("expression")?.child("methodSelect")?.child("expression")?;
        let exception = get_class.child("methodSelect")?.child("expression")?;
        let throwable_ty = self.ty(exception, exception.java_type())?;
        let throwable = self.type_index("java.lang.Throwable");
        let init_cause = throwable.and_then(|t| {
            self.methods.iter().position(|m| {
                m.owner == t && m.name == "initCause" && m.sig == "(java.lang.Throwable)java.lang.Throwable"
            })
        });
        let (Some(throwable), Some(init_cause)) = (throwable, init_cause) else {
            return unsupported(
                node,
                "making a new exception of another's class without `Throwable.initCause`",
            );
        };
        let caller_package = self.package_of(self.current.get());
        let mut chain: Option<Stmt> = None;
        for t in (0..self.types.len()).rev() {
            let decl = self.types[t].node;
            let flags = |n: Node| n.0["modifiers"]["flags"].as_array().cloned().unwrap_or_default();
            let has = |n: Node, flag: &str| flags(n).iter().any(|f| f == flag);
            let javac = &self.types[t].javac;
            if !self.is_subclass(t, throwable)
                || decl.kind() != "CLASS"
                || has(decl, "ABSTRACT")
                || self.nesting(t).is_some()
            {
                continue;
            }
            let accessible = standard_subclass(javac, "java.lang.Throwable")
                || has(decl, "PUBLIC")
                || has(decl, "PROTECTED")
                || self.package_of(t) == caller_package;
            // A standard exception class that the program never creates has no objects.
            let name = &self.types[t].name;
            let unused_standard = self.standard.has(name) && !self.standard.instantiated.contains(name);
            if !accessible || unused_standard {
                continue;
            }
            let constructor = RECREATING.into_iter().find_map(|sig| {
                self.methods
                    .iter()
                    .position(|m| m.owner == t && m.name == "<init>" && m.sig == sig && has(m.node, "PUBLIC"))
                    .map(|ctor| (sig, ctor))
            });
            let Some((sig, ctor)) = constructor else { continue };
            let read = || self.expr(exception);
            let class_ty = Type::class(&self.types[t].name);
            let assign = |value: Expr| -> Result<Stmt, Error> {
                let kind =
                    ExprKind::Assign { op: None, target: self.target(exception)?, value: Box::new(value) };
                Ok(Stmt::Expr(self.synth(kind, pos, throwable_ty)))
            };
            let then = if sig == RECREATING[0] {
                let created = self.synth(
                    ExprKind::New { class: t, args: vec![read()?], ctor: Some(ctor) },
                    pos,
                    class_ty,
                );
                assign(self.convert(created, throwable_ty))?
            } else {
                let created =
                    self.synth(ExprKind::New { class: t, args: Vec::new(), ctor: Some(ctor) }, pos, class_ty);
                let name = self.fresh_name("recreated");
                let var = || self.synth(ExprKind::Var(name.clone()), pos, throwable_ty);
                let cause = ExprKind::MethodCall {
                    receiver: Box::new(var()),
                    name: "initCause".to_string(),
                    args: vec![read()?],
                    resolved: Some(init_cause),
                    is_super: false,
                };
                Stmt::Block(vec![
                    Stmt::Local {
                        ty: throwable_ty,
                        name: name.clone(),
                        init: Some(self.convert(created, throwable_ty)),
                        pos,
                    },
                    Stmt::Expr(self.synth(cause, pos, throwable_ty)),
                    assign(var())?,
                ])
            };
            let name_of = ExprKind::Builtin(Builtin::ClassName { simple: false }, vec![read()?]);
            let literal =
                ExprKind::Builtin(Builtin::ClassLiteralName { class: t, simple: false }, Vec::new());
            let same = ExprKind::Builtin(
                Builtin::SameClass { same: true },
                vec![self.synth(name_of, pos, Type::String), self.synth(literal, pos, Type::String)],
            );
            chain = Some(Stmt::If {
                cond: self.synth(same, pos, Type::Boolean),
                then: Box::new(then),
                otherwise: chain.map(Box::new),
            });
        }
        // A null `ex` throws in `getClass`, which the `catch` ignores: it stays null.
        let null = Box::new(self.synth(ExprKind::Null, pos, Type::Null));
        let not_null = self.synth(
            ExprKind::Binary(BinOp::Ne, Box::new(self.expr(exception)?), null),
            pos,
            Type::Boolean,
        );
        Ok(Some(match chain {
            Some(chain) => vec![Stmt::If { cond: not_null, then: Box::new(chain), otherwise: None }],
            None => Vec::new(),
        }))
    }
}
