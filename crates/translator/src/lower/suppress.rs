//! The two checks of `Throwable.addSuppressed`: a throwable is not suppressed by itself, and `null`
//! is not suppressed at all; and the two of `Throwable.initCause`: a cause is set once, and a
//! throwable is not its own cause. `Standard.java` cannot say `throw` (javac sees its `Throwable`
//! as unrelated to `java.lang.Throwable`), so the checks are added to the lowered methods here, the
//! way Java's own methods start with them.

use super::*;

const SELF: &str = "java.lang.IllegalArgumentException";
const NULL: &str = "java.lang.NullPointerException";
const SET: &str = "java.lang.IllegalStateException";

/// The standard classes that the checks of `addSuppressed` throw; `Standard::select` declares
/// them with the method.
pub(super) const CLASSES: [&str; 2] = [SELF, NULL];

/// The standard classes that the checks of `initCause` throw.
pub(super) const CAUSE_CLASSES: [&str; 2] = [SET, SELF];

impl<'t> Lowering<'t> {
    /// Puts `if (exception == this) throw ..; if (exception == null) throw ..;` at the start of
    /// `addSuppressed`, with Java's messages, when the program's classes include it.
    pub(super) fn suppress_checks(&self, methods: &mut [Method]) {
        let Some(throwable) = self.type_index("java.lang.Throwable") else { return };
        let Some(at) =
            methods.iter().position(|m| m.class == throwable && m.name == "addSuppressed" && !m.is_static)
        else {
            return;
        };
        let Some((name, ty)) = methods[at].params.first().map(|p| (p.name.clone(), p.ty)) else {
            return;
        };
        let ctor = |class: &str| {
            let class = self.type_index(class)?;
            let ctor = methods.iter().position(|m| {
                m.is_ctor && m.class == class && m.params.len() == 1 && m.params[0].ty == Type::String
            })?;
            Some((class, ctor))
        };
        let (Some(self_ctor), Some(null_ctor)) = (ctor(SELF), ctor(NULL)) else { return };
        let pos = methods[at].pos;
        let throwable_ty = Type::class(&self.types[throwable].name);
        let var = || self.synth(ExprKind::Var(name.clone()), pos, ty);
        let this = self.synth(ExprKind::This, pos, throwable_ty);
        let null = self.synth(ExprKind::Null, pos, Type::Null);
        let same = |right: Expr| ExprKind::Binary(BinOp::Eq, Box::new(var()), Box::new(right));
        let self_cond = self.synth(same(this), pos, Type::Boolean);
        let null_cond = self.synth(same(null), pos, Type::Boolean);
        let text = |message: &str| self.synth(ExprKind::Str(message.to_string()), pos, Type::String);
        let self_throw = self.throw_new(self_ctor, text("Self-suppression not permitted"), throwable, pos);
        let null_throw = self.throw_new(null_ctor, text("Cannot suppress a null exception."), throwable, pos);
        let body = vec![
            Stmt::If { cond: self_cond, then: Box::new(Stmt::Block(vec![self_throw])), otherwise: None },
            Stmt::If { cond: null_cond, then: Box::new(Stmt::Block(vec![null_throw])), otherwise: None },
        ];
        methods[at].body.splice(0..0, body);
    }

    /// Puts `if (overwrite != null) throw new IllegalStateException(overwrite); if (cause == this)
    /// throw ..;` after the first statement of `initCause` (which makes the message `overwrite` of
    /// a cause set before, see `Standard.java`), with Java's messages.
    pub(super) fn init_cause_checks(&self, methods: &mut [Method]) {
        let Some(throwable) = self.type_index("java.lang.Throwable") else { return };
        let Some(at) =
            methods.iter().position(|m| m.class == throwable && m.name == "initCause" && !m.is_static)
        else {
            return;
        };
        let Some((name, ty)) = methods[at].params.first().map(|p| (p.name.clone(), p.ty)) else {
            return;
        };
        let Some(Stmt::Local { name: overwrite, .. }) = methods[at].body.first() else { return };
        let overwrite = overwrite.clone();
        let ctor = |class: &str| {
            let class = self.type_index(class)?;
            let ctor = methods.iter().position(|m| {
                m.is_ctor && m.class == class && m.params.len() == 1 && m.params[0].ty == Type::String
            })?;
            Some((class, ctor))
        };
        let (Some(set_ctor), Some(self_ctor)) = (ctor(SET), ctor(SELF)) else { return };
        let pos = methods[at].pos;
        let throwable_ty = Type::class(&self.types[throwable].name);
        let message = || self.synth(ExprKind::Var(overwrite.clone()), pos, Type::String);
        let null = || self.synth(ExprKind::Null, pos, Type::Null);
        let set_cond = ExprKind::Binary(BinOp::Ne, Box::new(message()), Box::new(null()));
        let set_cond = self.synth(set_cond, pos, Type::Boolean);
        let cause = self.synth(ExprKind::Var(name), pos, ty);
        let this = self.synth(ExprKind::This, pos, throwable_ty);
        let self_cond =
            self.synth(ExprKind::Binary(BinOp::Eq, Box::new(cause), Box::new(this)), pos, Type::Boolean);
        let set_throw = self.throw_new(set_ctor, message(), throwable, pos);
        let text = self.synth(ExprKind::Str("Self-causation not permitted".to_string()), pos, Type::String);
        let self_throw = self.throw_new(self_ctor, text, throwable, pos);
        let body = vec![
            Stmt::If { cond: set_cond, then: Box::new(Stmt::Block(vec![set_throw])), otherwise: None },
            Stmt::If { cond: self_cond, then: Box::new(Stmt::Block(vec![self_throw])), otherwise: None },
        ];
        methods[at].body.splice(1..1, body);
    }

    /// `throw new C(text)` as a `Throwable`, with `C` and its `(String)` constructor `(class, ctor)`.
    fn throw_new(&self, (class, ctor): (usize, usize), text: Expr, throwable: usize, pos: Pos) -> Stmt {
        let class_ty = Type::class(&self.types[class].name);
        let new = self.synth(ExprKind::New { class, args: vec![text], ctor: Some(ctor) }, pos, class_ty);
        Stmt::Throw(self.convert(new, Type::class(&self.types[throwable].name)))
    }
}
