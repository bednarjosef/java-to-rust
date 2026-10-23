//! A call that reads a shared array by borrowing it (`&a.borrow()`) among its arguments. Java
//! reads the array only inside the call, after every argument, but a borrow made as an argument
//! lives until the end of the statement: a later argument that writes the array (`squeeze(a)`)
//! would then find it already borrowed. Such arguments are computed into locals first, in Java's
//! order, with the array's reference taken first, and the borrow is taken last, in the call.

use super::*;
use crate::ast::has_effects;

impl<'a> Gen<'a> {
    /// The call `build` makes from the texts of its arguments, where `args[array]` is the array
    /// that the call reads. `texts` holds the text of each argument in Java's order; its entry at
    /// `array` is not used, `build` gets the array's slice there. When no argument after the array
    /// has effects, the texts are used as they are. Otherwise the arguments up to the last one with
    /// effects are bound to locals in a block, in order: the array's reference first, as a clone,
    /// so that a later argument which reassigns the variable does not change which array is read.
    /// Literals stay in place: they read nothing.
    pub(super) fn array_call(
        &mut self,
        args: &'a [Expr],
        array: usize,
        mut texts: Vec<String>,
        build: impl FnOnce(&mut Self, &[String]) -> Code,
    ) -> Code {
        let source = &args[array];
        let plain = matches!(&source.kind, ExprKind::Var(name) if self.plain(name).is_some());
        let last = args.iter().rposition(has_effects);
        if plain || last.is_none_or(|last| last < array) {
            texts[array] = self.slice_of(source);
            return build(self, &texts);
        }
        let last = last.unwrap_or(array);
        let mut lets = Vec::new();
        for (i, arg) in args[..=last].iter().enumerate() {
            if i == array {
                let name = self.temp("array");
                let place = self.object_place(source);
                lets.push(format!("let {name} = {place}.clone();"));
                let handle = if self.nullable(source) {
                    self.dereference(&[source.id], &format!("{name}.as_ref()"))
                } else {
                    name
                };
                texts[i] = format!("&{handle}.borrow()");
            } else if !matches!(
                arg.kind,
                ExprKind::Int(_) | ExprKind::Long(_) | ExprKind::Char(_) | ExprKind::Bool(_)
            ) {
                let name = self.temp("arg");
                lets.push(format!("let {name} = {};", texts[i]));
                texts[i] = name;
            }
        }
        let call = build(self, &texts);
        Code::atom(format!("{{ {} {} }}", lets.join(" "), call.text))
    }
}
