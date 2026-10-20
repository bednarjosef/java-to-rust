//! The Java syntax tree the translator works on: a small typed subset of Java, lowered from the
//! tree-sitter parse tree. The checker fills in the types (`Expr::ty`) and resolves calls.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Int,
    Long,
    Double,
    Boolean,
    String,
    Void,
}

impl Type {
    pub fn is_numeric(self) -> bool {
        matches!(self, Type::Int | Type::Long | Type::Double)
    }

    pub fn is_integral(self) -> bool {
        matches!(self, Type::Int | Type::Long)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Type::Int => "int",
            Type::Long => "long",
            Type::Double => "double",
            Type::Boolean => "boolean",
            Type::String => "String",
            Type::Void => "void",
        })
    }
}

/// A position in the Java source, 1-based, for error messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

pub struct Class {
    pub name: String,
    pub methods: Vec<Method>,
}

pub struct Method {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Type,
    pub body: Vec<Stmt>,
    /// `public static void main(String[] args)`, the program's entry point.
    pub is_main: bool,
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

pub struct Expr {
    pub kind: ExprKind,
    pub pos: Pos,
    /// Filled in by the checker.
    pub ty: Type,
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
        target: String,
        value: Box<Expr>,
    },
    /// `++x`, `x++`, `--x`, `x--`.
    IncDec {
        target: String,
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
