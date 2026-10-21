//! The two checks of `Throwable.addSuppressed`: a throwable is not suppressed by itself, and `null`
//! is not suppressed at all. `Standard.java` cannot say `throw` (javac sees its `Throwable` as
//! unrelated to `java.lang.Throwable`), so the checks are added to the lowered method here, the way
//! Java's own method starts with them.

use super::*;

const SELF: &str = "java.lang.IllegalArgumentException";
const NULL: &str = "java.lang.NullPointerException";

/// The standard classes that the checks throw; `Standard::select` declares them with the method.
pub(super) const CLASSES: [&str; 2] = [SELF, NULL];

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
        let self_throw = self.throw_new(self_ctor, "Self-suppression not permitted", throwable, pos);
        let null_throw = self.throw_new(null_ctor, "Cannot suppress a null exception.", throwable, pos);
        let body = vec![
            Stmt::If { cond: self_cond, then: Box::new(Stmt::Block(vec![self_throw])), otherwise: None },
            Stmt::If { cond: null_cond, then: Box::new(Stmt::Block(vec![null_throw])), otherwise: None },
        ];
        methods[at].body.splice(0..0, body);
    }

    /// `throw new C(message)` as a `Throwable`, with `C` and its `(String)` constructor `(class, ctor)`.
    fn throw_new(&self, (class, ctor): (usize, usize), message: &str, throwable: usize, pos: Pos) -> Stmt {
        let text = self.synth(ExprKind::Str(message.to_string()), pos, Type::String);
        let class_ty = Type::class(&self.types[class].name);
        let new = self.synth(ExprKind::New { class, args: vec![text], ctor: Some(ctor) }, pos, class_ty);
        Stmt::Throw(self.convert(new, Type::class(&self.types[throwable].name)))
    }
}
