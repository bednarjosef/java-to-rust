//! The arguments of calls of Rust generics. A parameter whose type is a type variable of a real
//! generic takes any value as it is (`convert`), but Rust instantiates the generic with the
//! type of what it is given, so an argument of a subclass would make a `Box<Square>` where Java
//! has a `Box<Shape>`. Each such argument is converted to the type javac substituted for the
//! variable at the call (`b.set(new Line(3))` on a `Box<Shape>`, `pick(c, new Line(1), q)` with
//! `T` inferred as `Shape`), so that every argument has that type.

use super::generics::split_args;
use super::*;

impl<'t> Lowering<'t> {
    /// `convert_args` for the call or `new` at `node`, with the arguments for parameters of a
    /// type variable converted to the type javac substituted for it there.
    pub(super) fn convert_generic_args(
        &self,
        node: Node<'t>,
        index: Option<usize>,
        args: Vec<Expr>,
    ) -> Result<Vec<Expr>, Error> {
        let args = self.convert_args(index, args)?;
        let Some(index) = index else { return Ok(args) };
        let declared = self.param_types(index)?;
        if !declared.iter().any(|&ty| self.param_owner(ty).is_some()) {
            return Ok(args);
        }
        let substituted = self.substituted_params(node, index);
        if substituted.len() != args.len() || declared.len() != args.len() {
            return Ok(args);
        }
        Ok(args
            .into_iter()
            .zip(declared)
            .zip(substituted)
            .map(|((arg, ty), s)| {
                if self.param_owner(ty).is_none() {
                    return arg;
                }
                match self.ty(node, &s) {
                    Ok(to @ Type::Class(..)) if !self.has_param(to) => self.convert(arg, to),
                    _ => arg,
                }
            })
            .collect())
    }

    /// The parameter types of the method or constructor called at `node`, as javac substituted
    /// them: the method's type at the call, or the constructor's parameters with the class's
    /// type variables replaced by the type arguments of the object made. Empty when unknown.
    fn substituted_params(&self, node: Node<'t>, index: usize) -> Vec<String> {
        if node.kind() == "NEW_CLASS" {
            let ty = node.java_type();
            let Some((_, args)) = ty.split_once('<') else { return Vec::new() };
            let args = split_args(&args[..args.len() - 1]);
            let class = &self.types[self.methods[index].owner].node;
            let vars: Vec<&str> = class.list("typeParameters").map(|p| p.str("name")).collect();
            let sig = node.symbol().map_or("", |s| s.ty);
            return params_of(sig)
                .into_iter()
                .map(|p| match vars.iter().position(|v| *v == p) {
                    Some(k) if vars.len() == args.len() => args[k].to_string(),
                    _ => p.to_string(),
                })
                .collect();
        }
        let Ok(select) = node.child("methodSelect") else { return Vec::new() };
        params_of(select.java_type()).into_iter().map(str::to_string).collect()
    }
}

/// The parameter types of javac's method type `<T>(boolean,T,T)T`.
fn params_of(sig: &str) -> Vec<&str> {
    let mut depth = 0;
    let mut start = None;
    for (i, c) in sig.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            '(' if depth == 0 => start = Some(i + 1),
            ')' if depth == 0 => {
                let inner = &sig[start.unwrap_or(i)..i];
                return if inner.is_empty() { Vec::new() } else { split_args(inner) };
            }
            _ => {}
        }
    }
    Vec::new()
}
