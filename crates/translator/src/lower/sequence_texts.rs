//! A builder made from, or appending, a `CharSequence` that may be an object of a class of the
//! program, as the JDK's `AbstractStringBuilder` copies one: by asking it for its `length()` and
//! its `charAt` of each index, never for its `toString()`. A string is appended as the string it
//! is, as Java's `append` does too (unit by unit, the runtime would look each unit up from the
//! start of the string); a builder gives its own text unit by unit.
//!
//! `b.append(s)` is otherwise `append(s, 0, s.length())`, whose range the generator reads with `length`
//! and `charAt` (`codegen::char_sequence`), in a block that holds the receiver and `s` as Java
//! computes them:
//!
//! ```text
//! switch (0) { default -> {
//!     StringBuilder receiver$ = b; CharSequence argument$ = s;
//!     if (argument$ instanceof String text$) yield receiver$.append(text$);
//!     yield receiver$.append(argument$, 0, receiver$ == null || argument$ == null ? 4 : argument$.length());
//! } }
//! ```
//!
//! A null `s` appends the range of "null" that is all of it; a null `b` throws once `s` is
//! computed, before `s` is asked for anything, as Java's call does. `new StringBuilder(s)` asks
//! `s` for its length first, then sizes the builder and appends `s` as above:
//!
//! ```text
//! switch (0) { default -> {
//!     CharSequence sequence$ = s; int length$ = sequence$.length();
//!     if (length$ < 0) throw new NegativeArraySizeException("Negative length: " + length$);
//!     StringBuilder built$ = new StringBuilder(length$ < 2147483631 ? length$ + 16 : 2147483647);
//!     if (sequence$ instanceof String text$) yield built$.append(text$);
//!     yield built$.append(sequence$, 0, sequence$.length());
//! } }
//! ```
//!
//! The `NullPointerException` of a null `s` there names the constructor's parameter, `seq`
//! (`npe_messages`, by the `SEQUENCE` local). Where no class of the program is a `CharSequence`,
//! one is a string or a builder, whose text the runtime copies directly.

use super::object::CHAR_SEQUENCE_METHODS;
use super::*;

/// The prefix of the local that holds the `CharSequence` of `new StringBuilder(seq)`.
pub(crate) const SEQUENCE: &str = "sequence$";

impl<'t> Lowering<'t> {
    /// Whether a `CharSequence` may be an object of a class of the program.
    pub(super) fn sequence_classes(&self) -> bool {
        self.object.is_some_and(|at| at.sequence_classes)
    }

    /// `builder.append(sequence)` (see the module's comment), of the call at `pos`.
    pub(super) fn append_sequence(&self, pos: Pos, builder: Expr, sequence: Expr) -> ExprKind {
        let ty = Type::class(CHAR_SEQUENCE);
        let sequence = self.convert(sequence, ty);
        let held = format!("{RECEIVER}{}", builder.id);
        let given = format!("{ARGUMENT}{}", sequence.id);
        let var = |name: &str, ty: Type| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let is_null = |name: &str, ty: Type| {
            let null = self.synth(ExprKind::Null, pos, Type::Null);
            self.synth(
                ExprKind::Binary(BinOp::Eq, Box::new(var(name, ty)), Box::new(null)),
                pos,
                Type::Boolean,
            )
        };
        let either = ExprKind::Binary(
            BinOp::Or,
            Box::new(is_null(&held, Type::StringBuilder)),
            Box::new(is_null(&given, ty)),
        );
        let end = ExprKind::Conditional(
            Box::new(self.synth(either, pos, Type::Boolean)),
            Box::new(self.synth(ExprKind::Int(4), pos, Type::Int)),
            Box::new(self.sequence_length(var(&given, ty))),
        );
        let builder_var = var(&held, Type::StringBuilder);
        let append = self.range_append(
            var(&held, Type::StringBuilder),
            var(&given, ty),
            self.synth(end, pos, Type::Int),
        );
        let body = vec![
            Stmt::Local { ty: Type::StringBuilder, name: held, init: Some(builder), pos },
            Stmt::Local { ty, name: given.clone(), init: Some(sequence), pos },
            self.string_shortcut(builder_var, var(&given, ty)),
            Stmt::Yield(append),
        ];
        self.default_block(body, pos)
    }

    /// `new StringBuilder(sequence)` (see the module's comment), of the constructor's call at
    /// `pos`, which throws with `negative`, `NegativeArraySizeException`'s class and its
    /// constructor from a message (`message_ctor`).
    pub(super) fn sequence_builder(&self, pos: Pos, sequence: Expr, negative: (usize, usize)) -> ExprKind {
        let ty = Type::class(CHAR_SEQUENCE);
        let sequence = self.convert(sequence, ty);
        let held = format!("{SEQUENCE}{}", sequence.id);
        let length = format!("length${}", sequence.id);
        let var = |name: &str, ty: Type| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let int = |v: i32| self.synth(ExprKind::Int(v), pos, Type::Int);
        let binary = |op: BinOp, l: Expr, r: Expr, ty: Type| {
            self.synth(ExprKind::Binary(op, Box::new(l), Box::new(r)), pos, ty)
        };
        let message = binary(
            BinOp::Add,
            self.synth(ExprKind::Str("Negative length: ".to_string()), pos, Type::String),
            var(&length, Type::Int),
            Type::String,
        );
        let negative = self.thrown(negative, message, pos);
        let capacity = ExprKind::Conditional(
            Box::new(binary(BinOp::Lt, var(&length, Type::Int), int(i32::MAX - 16), Type::Boolean)),
            Box::new(binary(BinOp::Add, var(&length, Type::Int), int(16), Type::Int)),
            Box::new(int(i32::MAX)),
        );
        let capacity = self.synth(capacity, pos, Type::Int);
        let new = ExprKind::Builtin(Builtin::Builder(BuilderOp::New), vec![capacity]);
        let new = self.synth(new, pos, Type::StringBuilder);
        let built = format!("built${}", sequence.id);
        let end = self.sequence_length(var(&held, ty));
        let body = vec![
            Stmt::Local { ty, name: held.clone(), init: Some(sequence), pos },
            Stmt::Local {
                ty: Type::Int,
                name: length.clone(),
                init: Some(self.sequence_length(var(&held, ty))),
                pos,
            },
            Stmt::If {
                cond: binary(BinOp::Lt, var(&length, Type::Int), int(0), Type::Boolean),
                then: Box::new(negative),
                otherwise: None,
            },
            Stmt::Local { ty: Type::StringBuilder, name: built.clone(), init: Some(new), pos },
            self.string_shortcut(var(&built, Type::StringBuilder), var(&held, ty)),
            Stmt::Yield(self.range_append(var(&built, Type::StringBuilder), var(&held, ty), end)),
        ];
        self.default_block(body, pos)
    }

    /// `if (sequence instanceof String text) yield builder.append(text);`: a string is appended as
    /// the string it is, as Java's `append(CharSequence)` does, rather than unit by unit.
    fn string_shortcut(&self, builder: Expr, sequence: Expr) -> Stmt {
        let pos = sequence.pos;
        let text = format!("text${}", sequence.id);
        let test =
            ExprKind::InstanceOf { value: Box::new(sequence), ty: Type::String, binding: Some(text.clone()) };
        let read = self.synth(ExprKind::Var(text), pos, Type::String);
        let append = ExprKind::Builtin(Builtin::Builder(BuilderOp::Append), vec![builder, read]);
        Stmt::If {
            cond: self.synth(test, pos, Type::Boolean),
            then: Box::new(Stmt::Yield(self.synth(append, pos, Type::StringBuilder))),
            otherwise: None,
        }
    }

    /// `builder.append(sequence, 0, end)`.
    fn range_append(&self, builder: Expr, sequence: Expr, end: Expr) -> Expr {
        let pos = builder.pos;
        let zero = self.synth(ExprKind::Int(0), pos, Type::Int);
        let append =
            ExprKind::Builtin(Builtin::Builder(BuilderOp::Append), vec![builder, sequence, zero, end]);
        self.synth(append, pos, Type::StringBuilder)
    }

    /// `sequence.length()` of the `CharSequence` `sequence`.
    fn sequence_length(&self, sequence: Expr) -> Expr {
        let s =
            self.object.and_then(|at| at.char_sequence).expect("a program with a CharSequence has its type");
        let i =
            CHAR_SEQUENCE_METHODS.iter().position(|(m, ..)| *m == "length").expect("`CharSequence.length`");
        let pos = sequence.pos;
        let call = ExprKind::MethodCall {
            receiver: Box::new(sequence),
            name: "length".to_string(),
            args: Vec::new(),
            resolved: Some(s.methods + i),
            is_super: false,
        };
        self.synth(call, pos, Type::Int)
    }

    /// The class `exception` and its constructor from a message, when the program has them.
    pub(super) fn message_ctor(&self, exception: &str) -> Option<(usize, usize)> {
        let class = self.type_index(exception)?;
        let ctor = self
            .methods_of(class)
            .into_iter()
            .find(|&m| self.methods[m].name == "<init>" && self.methods[m].sig == "(java.lang.String)void")?;
        Some((class, ctor))
    }

    /// `throw new E(message);` of the class and constructor `exception` (`message_ctor`).
    fn thrown(&self, (class, ctor): (usize, usize), message: Expr, pos: Pos) -> Stmt {
        let ty = Type::class(&self.types[class].name);
        let new = self.synth(ExprKind::New { class, args: vec![message], ctor: Some(ctor) }, pos, ty);
        Stmt::Throw(self.convert(new, Type::class("java.lang.Throwable")))
    }

    /// `switch (0) { default -> { body } }`.
    fn default_block(&self, body: Vec<Stmt>, pos: Pos) -> ExprKind {
        let selector = Box::new(self.synth(ExprKind::Int(0), pos, Type::Int));
        ExprKind::Switch { selector, cases: vec![Case { labels: vec![CaseLabel::Default], body }] }
    }
}
