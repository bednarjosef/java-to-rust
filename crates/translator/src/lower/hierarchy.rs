//! Lowering of the type hierarchy: superclasses, abstract classes and methods, overrides,
//! `super(...)` and `super.m(...)`, and the implicit conversions between class types, which are
//! made explicit as `Cast` nodes so that the analyses and the generator see every upcast.

use super::*;

impl<'t> Lowering<'t> {
    /// The superclass of class `t` when the program declares it; `None` for `Object`. A class
    /// that extends a class of the runtime is rejected.
    pub(super) fn superclass(&self, t: usize) -> Result<Option<usize>, Error> {
        let node = self.types[t].node;
        let Some(clause) = node.get("extendsClause") else { return Ok(None) };
        let name = clause.java_type();
        if name.contains('<') {
            return unsupported(clause, "generic classes");
        }
        match self.type_index(name) {
            Some(s) if self.types[s].node.kind() == "CLASS" => Ok(Some(s)),
            Some(_) => unsupported(clause, &format!("extending `{name}`")),
            None if name == "java.lang.Object" => Ok(None),
            None => unsupported(clause, &format!("extending `{name}`")),
        }
    }

    /// Whether class `a` is class `b` or a subclass of it.
    pub(super) fn is_subclass(&self, a: usize, b: usize) -> bool {
        let mut at = Some(a);
        while let Some(t) = at {
            if t == b {
                return true;
            }
            at = self.superclass(t).ok().flatten();
        }
        false
    }

    /// The methods of the program that a method declaration overrides, from the list javac
    /// gives with its symbol.
    pub(super) fn overrides(&self, node: Node<'t>) -> Vec<usize> {
        let Some(list) = node.get("symbol").and_then(|s| s.0["overrides"].as_array()) else {
            return Vec::new();
        };
        let name = node.str("name");
        list.iter()
            .filter_map(|o| {
                let owner = o["owner"].as_str()?;
                let sig = o["type"].as_str()?;
                self.methods
                    .iter()
                    .position(|m| self.types[m.owner].name == owner && m.name == name && m.sig == sig)
            })
            .collect()
    }

    /// `super(args)` at the start of a constructor: nothing when the superclass is `Object`,
    /// whose constructor does nothing, else `SuperInit`.
    pub(super) fn super_init(&self, stmt: Node<'t>) -> Result<Option<Stmt>, Error> {
        let call = stmt.child("expression")?;
        if self.superclass(self.current.get())?.is_none() {
            return Ok(None);
        }
        let Some(symbol) = call.symbol() else { return unsupported(call, "this `super` call") };
        let args: Vec<Expr> = call.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let args = self.packed_args(call, &symbol, args)?;
        let resolved = self.method_index(&symbol);
        let args = self.convert_args(resolved, args)?;
        let kind = ExprKind::SuperInit { args, resolved };
        Ok(Some(Stmt::Expr(self.synth(kind, call.pos(), Type::Void))))
    }

    /// The types of the parameters of method `index`.
    pub(super) fn param_types(&self, index: usize) -> Result<Vec<Type>, Error> {
        self.methods[index]
            .node
            .list("parameters")
            .map(|p| self.ty(p, p.symbol().map_or("", |s| s.ty)))
            .collect()
    }

    /// The arguments of a call of method `index`, each converted to its parameter's type.
    pub(super) fn convert_args(&self, index: Option<usize>, args: Vec<Expr>) -> Result<Vec<Expr>, Error> {
        let Some(index) = index else { return Ok(args) };
        let types = self.param_types(index)?;
        Ok(args
            .into_iter()
            .zip(types.into_iter().chain(std::iter::repeat(Type::Void)))
            .map(|(a, ty)| self.convert(a, ty))
            .collect())
    }

    /// `e` as a value of type `to`: an object of one class type that Java uses as another (an
    /// upcast) gets a `Cast`, which the generator turns into the conversion.
    pub(super) fn convert(&self, e: Expr, to: Type) -> Expr {
        let differs = match (e.ty, to) {
            (Type::Class(a), Type::Class(b)) => a != b,
            _ => false,
        };
        if !differs {
            return e;
        }
        let pos = e.pos;
        self.synth(ExprKind::Cast(to, Box::new(e)), pos, to)
    }

    /// The operands of `==` or `!=` on objects of two class types, the one of the subclass
    /// converted to the other's type.
    pub(super) fn convert_operands(&self, left: Expr, right: Expr) -> (Expr, Expr) {
        let (Some(a), Some(b)) = (self.class_of(left.ty), self.class_of(right.ty)) else {
            return (left, right);
        };
        if a == b {
            (left, right)
        } else if self.is_subclass(a, b) {
            (self.convert(left, right.ty), right)
        } else if self.is_subclass(b, a) {
            let ty = left.ty;
            (left, self.convert(right, ty))
        } else {
            (left, right)
        }
    }

    fn class_of(&self, ty: Type) -> Option<usize> {
        match ty {
            Type::Class(name) => self.type_index(name),
            _ => None,
        }
    }
}
