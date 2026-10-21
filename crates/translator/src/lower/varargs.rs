//! Varargs: a call of a method or constructor that takes a variable number of arguments packs the
//! trailing arguments into a new array, as Java does. A call that already passes an array for the
//! varargs parameter passes it on unchanged.

use super::*;

impl<'t> Lowering<'t> {
    /// The arguments of a call of a user method or constructor, with the trailing arguments of a
    /// variable-arity call packed into an array literal. A call that passes an array (or `null`)
    /// for the varargs parameter keeps its arguments as they are.
    pub(super) fn packed_args(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
        mut args: Vec<Expr>,
    ) -> Result<Vec<Expr>, Error> {
        let Some(index) = self.method_index(symbol).filter(|_| symbol.varargs) else { return Ok(args) };
        let params: Vec<Node<'t>> = self.methods[index].node.list("parameters").collect();
        let Some(last) = params.last().copied() else { return Ok(args) };
        let fixed = params.len() - 1;
        let ty = self.ty(last, last.symbol().map_or("", |s| s.ty))?;
        // `f(xs)` with `xs` of the parameter's own array type passes it on, not as one element.
        let direct = args.len() == params.len() && (args[fixed].ty == Type::Null || args[fixed].ty == ty);
        if direct {
            return Ok(args);
        }
        let element = match ty {
            Type::Array(element) => *element,
            _ => Type::Void,
        };
        let rest = args.split_off(fixed).into_iter().map(|a| self.convert(a, element)).collect();
        let packed = self.synth(ExprKind::ArrayLit(rest), node.pos(), ty);
        args.push(packed);
        Ok(args)
    }
}
