//! Gives every expression its Java type and resolves calls to overloads, following the Java
//! Language Specification for the supported subset. The input is assumed to be valid Java, so
//! the checker only reports what it needs in order to translate.

use crate::ast::*;
use std::collections::HashMap;

pub fn check(class: &mut Class) -> Result<(), Error> {
    let signatures: Vec<Signature> = class
        .methods
        .iter()
        .map(|m| Signature {
            name: m.name.clone(),
            params: m.params.iter().map(|p| p.ty).collect(),
            ret: m.ret,
        })
        .collect();
    for method in &mut class.methods {
        let mut checker = Checker {
            methods: &signatures,
            scopes: vec![method.params.iter().map(|p| (p.name.clone(), p.ty)).collect()],
            ret: method.ret,
        };
        checker.stmts(&mut method.body)?;
    }
    Ok(())
}

struct Signature {
    name: String,
    params: Vec<Type>,
    ret: Type,
}

/// Whether a value of type `from` can be passed or assigned where `to` is expected: identity or
/// widening primitive conversion.
pub fn assignable(from: Type, to: Type) -> bool {
    from == to
        || matches!(
            (from, to),
            (Type::Int, Type::Long) | (Type::Int, Type::Double) | (Type::Long, Type::Double)
        )
}

/// Binary numeric promotion of two numeric types.
pub fn promote(a: Type, b: Type) -> Type {
    if a == Type::Double || b == Type::Double {
        Type::Double
    } else if a == Type::Long || b == Type::Long {
        Type::Long
    } else {
        Type::Int
    }
}

/// The type of `left op right`, or `None` when the operator does not apply to these types.
pub fn binary_type(op: BinOp, left: Type, right: Type) -> Option<Type> {
    let numeric = left.is_numeric() && right.is_numeric();
    let integral = left.is_integral() && right.is_integral();
    let boolean = left == Type::Boolean && right == Type::Boolean;
    match op {
        BinOp::Add
            if (left == Type::String || right == Type::String)
                && left != Type::Void
                && right != Type::Void =>
        {
            Some(Type::String)
        }
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem if numeric => {
            Some(promote(left, right))
        }
        BinOp::Shl | BinOp::Shr | BinOp::UShr if integral => Some(promote(left, Type::Int)),
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor if integral => Some(promote(left, right)),
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::And | BinOp::Or if boolean => {
            Some(Type::Boolean)
        }
        BinOp::Eq | BinOp::Ne if numeric || boolean => Some(Type::Boolean),
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge if numeric => Some(Type::Boolean),
        _ => None,
    }
}

struct Checker<'a> {
    methods: &'a [Signature],
    scopes: Vec<HashMap<String, Type>>,
    ret: Type,
}

impl Checker<'_> {
    fn lookup(&self, name: &str, pos: Pos) -> Result<Type, Error> {
        match self.scopes.iter().rev().find_map(|s| s.get(name)) {
            Some(&ty) => Ok(ty),
            None => error(pos, format!("unknown variable `{name}`")),
        }
    }

    fn scoped<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        self.scopes.push(HashMap::new());
        let result = f(self);
        self.scopes.pop();
        result
    }

    fn stmts(&mut self, stmts: &mut [Stmt]) -> Result<(), Error> {
        stmts.iter_mut().try_for_each(|s| self.stmt(s))
    }

    /// A statement that is the whole body of an `if` or a loop gets its own scope.
    fn body(&mut self, stmt: &mut Stmt) -> Result<(), Error> {
        self.scoped(|c| c.stmt(stmt))
    }

    fn stmt(&mut self, stmt: &mut Stmt) -> Result<(), Error> {
        match stmt {
            Stmt::Local { ty, name, init, pos } => {
                if let Some(init) = init {
                    self.expect(init, *ty)?;
                }
                if self.scopes.iter().any(|s| s.contains_key(name)) {
                    return error(*pos, format!("variable `{name}` is already defined"));
                }
                self.scopes.last_mut().unwrap().insert(name.clone(), *ty);
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
            }
            Stmt::If { cond, then, otherwise } => {
                self.expect(cond, Type::Boolean)?;
                self.body(then)?;
                if let Some(otherwise) = otherwise {
                    self.body(otherwise)?;
                }
            }
            Stmt::While { cond, body } => {
                self.expect(cond, Type::Boolean)?;
                self.body(body)?;
            }
            Stmt::DoWhile { body, cond } => {
                self.body(body)?;
                self.expect(cond, Type::Boolean)?;
            }
            Stmt::For { init, cond, update, body } => self.scoped(|c| {
                c.stmts(init)?;
                if let Some(cond) = cond {
                    c.expect(cond, Type::Boolean)?;
                }
                for u in update.iter_mut() {
                    c.expr(u)?;
                }
                c.body(body)
            })?,
            Stmt::Return(value, pos) => match value {
                Some(value) => self.expect(value, self.ret)?,
                None if self.ret == Type::Void => {}
                None => return error(*pos, "missing return value"),
            },
            Stmt::Break | Stmt::Continue => {}
            Stmt::Block(stmts) => self.scoped(|c| c.stmts(stmts))?,
        }
        Ok(())
    }

    fn expect(&mut self, e: &mut Expr, ty: Type) -> Result<(), Error> {
        let actual = self.expr(e)?;
        if !assignable(actual, ty) {
            return error(e.pos, format!("expected {ty}, found {actual}"));
        }
        Ok(())
    }

    fn expr(&mut self, e: &mut Expr) -> Result<Type, Error> {
        let pos = e.pos;
        let ty = match &mut e.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Long(_) => Type::Long,
            ExprKind::Double(_) => Type::Double,
            ExprKind::Bool(_) => Type::Boolean,
            ExprKind::Str(_) => Type::String,
            ExprKind::Var(name) => self.lookup(name, pos)?,
            ExprKind::Unary(op, operand) => {
                let t = self.expr(operand)?;
                match op {
                    UnOp::Neg | UnOp::Plus if t.is_numeric() => promote(t, Type::Int),
                    UnOp::BitNot if t.is_integral() => promote(t, Type::Int),
                    UnOp::Not if t == Type::Boolean => Type::Boolean,
                    _ => return error(pos, format!("bad operand type {t} for unary operator")),
                }
            }
            ExprKind::Binary(op, left, right) => {
                let (l, r) = (self.expr(left)?, self.expr(right)?);
                if l == Type::String && r == Type::String && matches!(op, BinOp::Eq | BinOp::Ne) {
                    return error(pos, "== on strings compares references; not supported yet");
                }
                match binary_type(*op, l, r) {
                    Some(t) => t,
                    None => {
                        return error(pos, format!("bad operand types {l} {} {r}", op.java()));
                    }
                }
            }
            ExprKind::Assign { op, target, value } => {
                let t = self.lookup(target, pos)?;
                let v = self.expr(value)?;
                let ok = match op {
                    None => assignable(v, t),
                    Some(op) => binary_type(*op, t, v)
                        .is_some_and(|r| assignable(r, t) || (r.is_numeric() && t.is_numeric())),
                };
                if !ok {
                    return error(pos, format!("cannot assign {v} to {t} `{target}`"));
                }
                t
            }
            ExprKind::IncDec { target, .. } => {
                let t = self.lookup(target, pos)?;
                if !t.is_numeric() {
                    return error(pos, format!("bad operand type {t} for ++ or --"));
                }
                t
            }
            ExprKind::Cast(to, value) => {
                let from = self.expr(value)?;
                if !(from == *to || from.is_numeric() && to.is_numeric()) {
                    return error(pos, format!("cannot cast {from} to {to}"));
                }
                *to
            }
            ExprKind::Conditional(cond, a, b) => {
                self.expect(cond, Type::Boolean)?;
                let (a, b) = (self.expr(a)?, self.expr(b)?);
                if a == b {
                    a
                } else if a.is_numeric() && b.is_numeric() {
                    promote(a, b)
                } else {
                    return error(pos, format!("incompatible types {a} and {b} in ?:"));
                }
            }
            ExprKind::Call { name, args, resolved } => {
                let arg_types: Vec<Type> = args.iter_mut().map(|a| self.expr(a)).collect::<Result<_, _>>()?;
                let index = self.resolve(name, &arg_types, pos)?;
                *resolved = Some(index);
                self.methods[index].ret
            }
            ExprKind::Builtin(builtin, args) => {
                let arg_types: Vec<Type> = args.iter_mut().map(|a| self.expr(a)).collect::<Result<_, _>>()?;
                builtin_type(*builtin, &arg_types)
                    .ok_or_else(|| Error { pos, message: format!("bad arguments to {builtin:?}") })?
            }
        };
        e.ty = ty;
        Ok(ty)
    }

    /// Picks the most specific applicable overload, as in JLS 15.12.2.
    fn resolve(&self, name: &str, args: &[Type], pos: Pos) -> Result<usize, Error> {
        let applicable: Vec<usize> = (0..self.methods.len())
            .filter(|&i| {
                let m = &self.methods[i];
                m.name == name
                    && m.params.len() == args.len()
                    && args.iter().zip(&m.params).all(|(&a, &p)| assignable(a, p))
            })
            .collect();
        let more_specific = |a: usize, b: usize| {
            let (a, b) = (&self.methods[a].params, &self.methods[b].params);
            a.iter().zip(b).all(|(&x, &y)| assignable(x, y))
        };
        let best: Vec<usize> =
            applicable.iter().copied().filter(|&a| applicable.iter().all(|&b| more_specific(a, b))).collect();
        match best[..] {
            [one] => Ok(one),
            [] if applicable.is_empty() => error(pos, format!("no method `{name}` for these arguments")),
            _ => error(pos, format!("call of `{name}` is ambiguous")),
        }
    }
}

fn builtin_type(builtin: Builtin, args: &[Type]) -> Option<Type> {
    use Type::*;
    match (builtin, args) {
        (Builtin::Println, []) => Some(Void),
        (Builtin::Println | Builtin::Print, [t]) if *t != Void => Some(Void),
        (Builtin::Exit, [Int]) => Some(Void),
        (Builtin::MathAbs, [t]) if t.is_numeric() => Some(*t),
        (Builtin::MathMax | Builtin::MathMin, [a, b]) if a.is_numeric() && b.is_numeric() => {
            Some(promote(*a, *b))
        }
        (Builtin::MathSqrt, [t]) if t.is_numeric() => Some(Double),
        (Builtin::MathPow, [a, b]) if a.is_numeric() && b.is_numeric() => Some(Double),
        (Builtin::StringLength, [String]) => Some(Int),
        (Builtin::StringEquals, [String, String]) => Some(Boolean),
        _ => None,
    }
}
