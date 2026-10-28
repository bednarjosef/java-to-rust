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
        args: Vec<Expr>,
    ) -> Result<Vec<Expr>, Error> {
        let Some(index) = self.method_index(symbol).filter(|_| symbol.varargs) else { return Ok(args) };
        self.packed_args_of(node, index, args)
    }

    /// `packed_args` for a call of method `index`, which is variable-arity if its declaration is:
    /// an enum constant with a body calls its enum's constructor through the body's anonymous
    /// one, which javac does not mark as variable-arity.
    pub(super) fn packed_args_of(
        &self,
        node: Node<'t>,
        index: usize,
        mut args: Vec<Expr>,
    ) -> Result<Vec<Expr>, Error> {
        if !self.methods[index].node.symbol().is_some_and(|s| s.varargs) {
            return Ok(args);
        }
        let params: Vec<Node<'t>> = self.methods[index].node.list("parameters").collect();
        let Some(last) = params.last().copied() else { return Ok(args) };
        let fixed = params.len() - 1;
        // The parameter's type is read where it is declared: `T...` is an array of the method's
        // own type variable.
        let declared = last.symbol().map_or("", |s| s.ty);
        let owner = self.methods[index].owner;
        let ty = self.declared_type(last, declared, owner, Some(index))?;
        // `f(xs)` with `xs` of the parameter's own array type passes it on, not as one element,
        // and so does an array of references or of an enum passed for `T...`, whose `T` javac
        // infers from it (`EnumSet.of(ib, rest)` with an `InferenceBound[] rest`).
        let of_variable = declared.strip_suffix("[]").is_some_and(|element| {
            let declares = |node: Node<'t>| node.list("typeParameters").any(|p| p.str("name") == element);
            declares(self.methods[index].node) || declares(self.types[owner].node)
        });
        let passed = args.get(fixed).map(|a| a.ty);
        // An array of references is an `Object[]` too, passed for `Object...` (see `covariant`).
        let objects = matches!(ty, Type::Array(e) if Self::is_object(*e));
        let direct = args.len() == params.len()
            && (passed == Some(Type::Null)
                || passed == Some(ty)
                || (of_variable || objects)
                    && matches!(passed, Some(Type::Array(e)) if Self::of_objects(*e)));
        if direct {
            return Ok(args);
        }
        let element = match ty {
            Type::Array(element) => *element,
            _ => Type::Void,
        };
        let rest = args.split_off(fixed).into_iter().map(|a| self.convert(a, element)).collect();
        let packed = self.synth(ExprKind::ArrayLit(rest), node.pos(), ty);
        // Java makes the array of the class javac gives its elements (`String[]` for
        // `Arrays.asList("a", "b")`), which it carries where the translation holds it as an
        // `Object[]` (`covariant`).
        let made = node.str("varargsElement");
        if !made.is_empty() && !self.covariant.known.is_empty() {
            self.made_as(&packed, self.exact_ty(node, &format!("{made}[]"))?);
        }
        args.push(packed);
        Ok(args)
    }
}
