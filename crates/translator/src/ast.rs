//! The Java syntax tree the translator works on: a typed subset of Java, lowered from javac's
//! attributed tree (`lower.rs`), so every expression has its Java type and calls are resolved.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Int,
    Long,
    Double,
    Boolean,
    String,
    Void,
    /// An array type such as `int[]` or `String[][]`.
    Array(&'static Type),
    /// A class declared in the translated program, by name.
    Class(&'static str),
    /// The type of `null`.
    Null,
}

impl Type {
    pub fn is_numeric(self) -> bool {
        matches!(self, Type::Int | Type::Long | Type::Double)
    }

    pub fn is_integral(self) -> bool {
        matches!(self, Type::Int | Type::Long)
    }

    /// `element[]`. Types are small and copied everywhere, so compound ones are interned for the
    /// life of the process (a few bytes per distinct type).
    pub fn array(element: Type) -> Type {
        Type::Array(intern(element))
    }

    pub fn class(name: &str) -> Type {
        static NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
        let mut names = NAMES.lock().unwrap();
        let name = match names.iter().find(|n| **n == name) {
            Some(n) => *n,
            None => {
                let n: &'static str = Box::leak(name.to_string().into_boxed_str());
                names.push(n);
                n
            }
        };
        Type::Class(name)
    }

    /// Whether values of this type are references (and so can be `null`).
    pub fn is_reference(self) -> bool {
        matches!(self, Type::String | Type::Array(_) | Type::Class(_) | Type::Null)
    }

    /// The wider of two integer types.
    pub fn max_integral(self, other: Type) -> Type {
        if self == Type::Long || other == Type::Long { Type::Long } else { Type::Int }
    }
}

fn intern(ty: Type) -> &'static Type {
    static TYPES: std::sync::Mutex<Vec<&'static Type>> = std::sync::Mutex::new(Vec::new());
    let mut types = TYPES.lock().unwrap();
    match types.iter().find(|t| ***t == ty) {
        Some(t) => t,
        None => {
            let t: &'static Type = Box::leak(Box::new(ty));
            types.push(t);
            t
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Int => f.write_str("int"),
            Type::Long => f.write_str("long"),
            Type::Double => f.write_str("double"),
            Type::Boolean => f.write_str("boolean"),
            Type::String => f.write_str("String"),
            Type::Void => f.write_str("void"),
            Type::Array(element) => write!(f, "{element}[]"),
            Type::Class(name) => f.write_str(name),
            Type::Null => f.write_str("null"),
        }
    }
}

/// A position in the Java source, 1-based, for error messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

/// The translated file: its classes, and every method and constructor of all of them in one list,
/// so that analyses can refer to any of them by index.
pub struct Class {
    /// The class that declares `main`, which names the program.
    pub name: String,
    pub types: Vec<TypeDecl>,
    pub methods: Vec<Method>,
}

/// A class of the file: its fields. Methods and constructors are in `Class::methods`.
pub struct TypeDecl {
    pub name: String,
    /// Instance fields, in declaration order.
    pub fields: Vec<Field>,
    /// Static fields, in declaration order.
    pub statics: Vec<Field>,
}

pub struct Field {
    pub name: String,
    pub ty: Type,
    pub init: Option<Expr>,
    pub pos: Pos,
}

pub struct Method {
    /// `<init>` for a constructor.
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Type,
    pub body: Vec<Stmt>,
    /// `public static void main(String[] args)`, the program's entry point.
    pub is_main: bool,
    pub is_static: bool,
    pub is_ctor: bool,
    /// The index of the declaring class in `Class::types`.
    pub class: usize,
    pub pos: Pos,
}

pub struct Param {
    pub name: String,
    pub ty: Type,
}

pub enum Stmt {
    Local { ty: Type, name: String, init: Option<Expr>, pos: Pos },
    Expr(Expr),
    If { cond: Expr, then: Box<Stmt>, otherwise: Option<Box<Stmt>> },
    While { cond: Expr, body: Box<Stmt> },
    DoWhile { body: Box<Stmt>, cond: Expr },
    For { init: Vec<Stmt>, cond: Option<Expr>, update: Vec<Expr>, body: Box<Stmt> },
    Return(Option<Expr>, Pos),
    Break,
    Continue,
    Block(Vec<Stmt>),
}

/// Identifies an expression, for facts that analyses attach to it.
pub type ExprId = usize;

pub struct Expr {
    pub kind: ExprKind,
    pub pos: Pos,
    /// Filled in by the checker.
    pub ty: Type,
    /// Unique within the class.
    pub id: ExprId,
}

pub enum ExprKind {
    Int(i32),
    Long(i64),
    Double(f64),
    Bool(bool),
    Str(String),
    Var(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `target = value`, or `target op= value` when `op` is set.
    Assign {
        op: Option<BinOp>,
        target: Place,
        value: Box<Expr>,
    },
    /// `++x`, `x++`, `--x`, `x--`.
    IncDec {
        target: Place,
        increment: bool,
        prefix: bool,
    },
    Cast(Type, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    /// A call of one of the class's static methods; `resolved` is the index of the chosen
    /// overload in `Class::methods`, set by the checker.
    Call {
        name: String,
        args: Vec<Expr>,
        resolved: Option<usize>,
    },
    /// A call of a library method such as `System.out.println` or `Math.max`.
    Builtin(Builtin, Vec<Expr>),
    /// `this`: the object an instance method or constructor runs on.
    This,
    /// `object.name`, an instance field.
    Field {
        object: Box<Expr>,
        name: String,
    },
    /// `Class.name`, a static field; `class` indexes `Class::types`.
    Static {
        class: usize,
        name: String,
    },
    /// `new C(args)`; `ctor` is the chosen constructor, an index into `Class::methods`.
    New {
        class: usize,
        args: Vec<Expr>,
        ctor: Option<usize>,
    },
    /// `receiver.name(args)`, a call of an instance method; `resolved` as for `Call`.
    MethodCall {
        receiver: Box<Expr>,
        name: String,
        args: Vec<Expr>,
        resolved: Option<usize>,
    },
    /// `this(args)`, a constructor calling another constructor of its class.
    ThisCall {
        args: Vec<Expr>,
        resolved: Option<usize>,
    },
}

/// What an assignment, `++` or `--` writes to.
pub enum Place {
    /// A local variable or parameter.
    Var(String),
    /// `object.name`.
    Field { object: Box<Expr>, name: String },
    /// `Class.name`; `class` indexes `Class::types`.
    Static { class: usize, name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Println,
    Print,
    Exit,
    MathAbs,
    MathMax,
    MathMin,
    MathSqrt,
    MathPow,
    /// `s.length()`; the receiver is the first argument.
    StringLength,
    /// `s.equals(t)`; the receiver is the first argument.
    StringEquals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Not,
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn java(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::UShr => ">>>",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }

    pub fn is_shift(self) -> bool {
        matches!(self, BinOp::Shl | BinOp::Shr | BinOp::UShr)
    }
}

/// A translation error: a syntax error, a type error or a Java feature not supported yet.
#[derive(Debug)]
pub struct Error {
    pub pos: Pos,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}: {}", self.pos.line, self.pos.column, self.message)
    }
}

impl std::error::Error for Error {}

pub fn error<T>(pos: Pos, message: impl Into<String>) -> Result<T, Error> {
    Err(Error { pos, message: message.into() })
}

// Queries over the syntax tree, shared by the analyses and the code generator.

/// Recognizes `for (int i = a; i < b; i++)` (or `long i`) where the body never assigns `i` and `b`
/// cannot change during the loop, which is exactly `for i in a..b`.
pub fn counting_loop<'e>(
    init: &'e [Stmt],
    cond: Option<&'e Expr>,
    update: &'e [Expr],
    body: &'e Stmt,
) -> Option<(&'e str, &'e Expr, &'e Expr)> {
    let [Stmt::Local { ty, name, init: Some(start), .. }] = init else { return None };
    let ExprKind::Binary(BinOp::Lt, var, end) = &cond?.kind else { return None };
    if !ty.is_integral() || !matches!(&var.kind, ExprKind::Var(v) if v == name) || end.ty != *ty {
        return None;
    }
    let steps_by_one = match update {
        [Expr { kind: ExprKind::IncDec { target: Place::Var(target), increment: true, .. }, .. }] => {
            target == name
        }
        [
            Expr {
                kind: ExprKind::Assign { op: Some(BinOp::Add), target: Place::Var(target), value }, ..
            },
        ] => target == name && matches!(value.kind, ExprKind::Int(1) | ExprKind::Long(1)),
        _ => false,
    };
    if !steps_by_one || assigns_stmt(body, name) || !stable(end, name, body) {
        return None;
    }
    Some((name, start, end))
}

/// Whether `e` evaluates to the same value on every iteration of a loop with this body.
pub fn stable(e: &Expr, loop_var: &str, body: &Stmt) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Long(_) => true,
        ExprKind::Var(v) => v != loop_var && !assigns_stmt(body, v),
        ExprKind::Unary(_, x) | ExprKind::Cast(_, x) => stable(x, loop_var, body),
        ExprKind::Binary(_, l, r) => stable(l, loop_var, body) && stable(r, loop_var, body),
        _ => false,
    }
}

/// Whether `e` assigns the local variable `name`.
pub fn assigns(e: &Expr, name: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Assign { target: Place::Var(target), .. }
        | ExprKind::IncDec { target: Place::Var(target), .. } => found |= target == name,
        _ => {}
    });
    found
}

pub fn assigns_any(stmts: &[Stmt], name: &str) -> bool {
    stmts.iter().any(|s| assigns_stmt(s, name))
}

pub fn assigns_stmt(stmt: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(stmt, &mut |e| found |= assigns(e, name), &mut |_| {});
    found
}

/// Whether `e` reads or writes the variable `name`.
pub fn mentions(e: &Expr, name: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Var(v)
        | ExprKind::Assign { target: Place::Var(v), .. }
        | ExprKind::IncDec { target: Place::Var(v), .. } => found |= v == name,
        _ => {}
    });
    found
}

/// Calls `on_expr` on every top-level expression and `on_stmt` on every statement in `stmt`.
pub fn visit_stmt(stmt: &Stmt, on_expr: &mut dyn FnMut(&Expr), on_stmt: &mut dyn FnMut(&Stmt)) {
    on_stmt(stmt);
    match stmt {
        Stmt::Local { init: Some(e), .. } => on_expr(e),
        Stmt::Local { init: None, .. } => {}
        Stmt::Expr(e) | Stmt::Return(Some(e), _) => on_expr(e),
        Stmt::If { cond, then, otherwise } => {
            on_expr(cond);
            visit_stmt(then, on_expr, on_stmt);
            if let Some(o) = otherwise {
                visit_stmt(o, on_expr, on_stmt);
            }
        }
        Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
            on_expr(cond);
            visit_stmt(body, on_expr, on_stmt);
        }
        Stmt::For { init, cond, update, body } => {
            init.iter().for_each(|s| visit_stmt(s, on_expr, on_stmt));
            if let Some(cond) = cond {
                on_expr(cond);
            }
            update.iter().for_each(&mut *on_expr);
            visit_stmt(body, on_expr, on_stmt);
        }
        Stmt::Block(stmts) => stmts.iter().for_each(|s| visit_stmt(s, on_expr, on_stmt)),
        Stmt::Return(None, _) | Stmt::Break | Stmt::Continue => {}
    }
}

/// Calls `f` on `e` and every expression inside it.
pub fn visit_expr(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, x) | ExprKind::Cast(_, x) | ExprKind::Field { object: x, .. } => visit_expr(x, f),
        ExprKind::Assign { target, value, .. } => {
            visit_place(target, f);
            visit_expr(value, f);
        }
        ExprKind::IncDec { target, .. } => visit_place(target, f),
        ExprKind::Binary(_, l, r) => {
            visit_expr(l, f);
            visit_expr(r, f);
        }
        ExprKind::Conditional(c, a, b) => {
            visit_expr(c, f);
            visit_expr(a, f);
            visit_expr(b, f);
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            visit_expr(receiver, f);
            args.iter().for_each(|a| visit_expr(a, f))
        }
        ExprKind::Call { args, .. }
        | ExprKind::Builtin(_, args)
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. } => args.iter().for_each(|a| visit_expr(a, f)),
        _ => {}
    }
}

/// Calls `f` on every expression inside the place (the object of a field place).
fn visit_place(place: &Place, f: &mut dyn FnMut(&Expr)) {
    if let Place::Field { object, .. } = place {
        visit_expr(object, f);
    }
}
