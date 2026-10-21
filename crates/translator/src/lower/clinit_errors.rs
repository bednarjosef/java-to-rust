//! The static initializers that fail, as Java runs them: the first use of the class throws the
//! failure (an `Error` as it is, anything else wrapped in `ExceptionInInitializerError`), and every
//! later use throws `NoClassDefFoundError`. The `<clinit>` of a class keeps that state in two static
//! fields and rethrows, so that a `try` sees the failures as ordinary exceptions. Only programs that
//! declare these classes, because they catch one of them (see `Standard::select`), get this; the
//! others keep the uncaught translation (see `analysis/exceptions.rs`).

use super::*;

/// The static field of a class's initialization: 0 not started, 1 started or done, 2 failed.
pub(crate) const STATE: &str = "init$state";
/// The static field with the text of the failure, which `NoClassDefFoundError`'s cause shows.
const FAILURE: &str = "init$failure";
const EIIE: &str = "java.lang.ExceptionInInitializerError";
const NCDFE: &str = "java.lang.NoClassDefFoundError";
const ERROR: &str = "java.lang.Error";
const THROWABLE: &str = "java.lang.Throwable";

/// Whether the `<clinit>` of a class with `decl` is rewritten (see `guard_clinit`).
pub(crate) fn rewritten_clinit(decl: &TypeDecl) -> bool {
    decl.statics.iter().any(|f| f.name == STATE)
}

impl<'t> Lowering<'t> {
    /// Rewrites the `<clinit>` `method` of the class `index` to the state machine of the module
    /// comment, when the program declares the exception classes it needs; the two state fields go
    /// in `decl`. `methods` holds the constructors that the new code calls.
    pub(super) fn guard_clinit(
        &self,
        index: usize,
        method: &mut Method,
        decl: &mut TypeDecl,
        methods: &[Method],
    ) {
        let throwable = Type::class(THROWABLE);
        let (Some(to_eiie), Some(eiie_text), Some(ncdfe), Some(error), Some(any)) = (
            self.ctor(methods, EIIE, &[throwable]),
            self.ctor(methods, EIIE, &[Type::String]),
            self.ctor(methods, NCDFE, &[Type::String, throwable]),
            self.type_index(ERROR),
            self.type_index(THROWABLE),
        ) else {
            return;
        };
        let pos = method.pos;
        let int = |v: i32| self.synth(ExprKind::Int(v), pos, Type::Int);
        let text = |s: &str| self.synth(ExprKind::Str(s.to_string()), pos, Type::String);
        let var = |name: &str, ty: Type| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let state = || self.synth(ExprKind::Static { class: index, name: STATE.to_string() }, pos, Type::Int);
        let failure =
            || self.synth(ExprKind::Static { class: index, name: FAILURE.to_string() }, pos, Type::String);
        let new = |(class, ctor): (usize, usize), args: Vec<Expr>| {
            let ty = Type::class(&self.types[class].name);
            self.synth(ExprKind::New { class, args, ctor: Some(ctor) }, pos, ty)
        };
        let set = |name: &str, ty: Type, value: Expr| {
            let target = Place::Static { class: index, name: name.to_string() };
            Stmt::Expr(self.synth(ExprKind::Assign { op: None, target, value: Box::new(value) }, pos, ty))
        };
        let throw = |e: Expr| Stmt::Throw(self.convert(e, throwable));
        let when = |cond: Expr, body: Vec<Stmt>| Stmt::If {
            cond,
            then: Box::new(Stmt::Block(body)),
            otherwise: None,
        };
        let is = |v: i32| {
            let cond = ExprKind::Binary(BinOp::Eq, Box::new(state()), Box::new(int(v)));
            self.synth(cond, pos, Type::Boolean)
        };
        let concat = |left: Expr, right: Expr| {
            self.synth(ExprKind::Binary(BinOp::Add, Box::new(left), Box::new(right)), pos, Type::String)
        };
        // The text the JVM keeps for a failure: `Exception ` and the failure's `toString`.
        let failure_of = |e: Expr| concat(concat(text("Exception "), e), text(" [in thread \"main\"]"));
        let error_catch = Catch {
            types: vec![error],
            name: "failure".to_string(),
            ty: Type::class(ERROR),
            body: vec![
                set(STATE, Type::Int, int(2)),
                set(FAILURE, Type::String, failure_of(var("failure", Type::class(ERROR)))),
                throw(var("failure", Type::class(ERROR))),
            ],
            pos,
        };
        let any_catch = Catch {
            types: vec![any],
            name: "failure".to_string(),
            ty: throwable,
            body: vec![
                set(STATE, Type::Int, int(2)),
                set(FAILURE, Type::String, failure_of(var("failure", throwable))),
                throw(new(to_eiie, vec![var("failure", throwable)])),
            ],
            pos,
        };
        // `Could not initialize class` and the class's binary name, which the runtime gives.
        let binary = self.synth(
            ExprKind::Builtin(Builtin::ClassLiteralName { class: index, simple: false }, Vec::new()),
            pos,
            Type::String,
        );
        let message = concat(text("Could not initialize class "), binary);
        let cause = self.convert(new(eiie_text, vec![failure()]), throwable);
        let original = std::mem::take(&mut method.body);
        method.body = vec![
            when(is(2), vec![throw(new(ncdfe, vec![message, cause]))]),
            when(is(1), vec![Stmt::Return(None, pos)]),
            set(STATE, Type::Int, int(1)),
            Stmt::Try { body: original, catches: vec![error_catch, any_catch], finally: None },
        ];
        decl.statics.push(Field { name: STATE.to_string(), ty: Type::Int, init: None, constant: false, pos });
        decl.statics.push(Field {
            name: FAILURE.to_string(),
            ty: Type::String,
            init: None,
            constant: false,
            pos,
        });
    }

    /// The constructor `class(params)` of a standard class, as its index in `methods`, when the
    /// program declares the class.
    fn ctor(&self, methods: &[Method], class: &str, params: &[Type]) -> Option<(usize, usize)> {
        let class = self.type_index(class)?;
        let ctor = methods.iter().position(|m| {
            m.is_ctor
                && m.class == class
                && m.params.len() == params.len()
                && m.params.iter().zip(params).all(|(p, t)| p.ty == *t)
        })?;
        Some((class, ctor))
    }
}
