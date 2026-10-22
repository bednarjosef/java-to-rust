//! `System.arraycopy` from an array of one element class to an array of another, where the
//! source's class is not a subtype of the destination's. HotSpot checks each element as it is
//! stored: null always passes, and an element passes when it is an instance of the destination's
//! class. The elements before the first one that fails are stored, and then the
//! `ArrayStoreException` is thrown, with the message HotSpot gives for the pair.

use super::*;

impl<'a> Gen<'a> {
    /// The checked copy of `System.arraycopy` between arrays of two element classes, or `None`
    /// when the source's class is a subtype of the destination's, which is a plain copy.
    pub(super) fn checked_class_copy(&mut self, args: &'a [Expr], id: ExprId) -> Option<Code> {
        let [src, src_pos, dest, dest_pos, length] = args else {
            unreachable!("arraycopy has five arguments")
        };
        let (Type::Array(es), Type::Array(ed)) = (src.ty, dest.ty) else { return None };
        let (Type::Class(..), Type::Class(..)) = (*es, *ed) else { return None };
        let (Some(s), Some(d)) = (self.type_of(*es), self.type_of(*ed)) else {
            self.rejected.push(Error {
                pos: src.pos,
                message: format!("`System.arraycopy` from {} to {} not supported yet", src.ty, dest.ty),
            });
            return Some(Code::atom("unreachable!()"));
        };
        // A subtype's elements all pass, so they are stored as they are when both sides hold them
        // in the same type; otherwise they are converted without a check.
        let (src_ty, dst_ty) = (self.element_ty_name(*es), self.element_ty_name(*ed));
        let subtype = self.class.hierarchy.is_subtype(s, d);
        if subtype && src_ty == dst_ty {
            return None;
        }
        // The element mismatch is named when the destination's class is a subtype of the source's
        // (a `Named` array copied into a `Box` array); an unrelated pair is a type mismatch.
        // `Object` is named as the runtime names it, which is not its binary name.
        let binary = |t: usize| match self.class.types[t].name.as_str() {
            OBJECT => "java.lang.Object".to_string(),
            _ => self.binary_name(t),
        };
        let (from, to) = (binary(s), binary(d));
        let message = if self.class.hierarchy.is_subtype(d, s) {
            format!(
                "arraycopy: element type mismatch: can not cast one of the elements of {from}[] to the type of the destination array, {to}"
            )
        } else {
            format!("arraycopy: type mismatch: can not copy {from}[] into {to}[]")
        };
        // An element is stored when it passes, and a null always does: each arm converts an
        // element of a class that passes from the source's representation to the destination's.
        let (src_nullable, dst_nullable) = (src_ty.starts_with("Option<"), dst_ty.starts_with("Option<"));
        let mut arms = self.element_arms(s, d, (src_nullable, dst_nullable));
        if src_nullable && dst_nullable {
            arms.insert(0, "None => Ok(None), ".to_string());
        }
        let fallthrough = if subtype { String::new() } else { format!("_ => Err(\"{message}\"), ") };
        let convert = format!(
            "|e: &{src_ty}| -> Result<{dst_ty}, &'static str> {{ match e {{ {}{fallthrough}}} }}",
            arms.concat()
        );
        let [p, q, n] = ["src_pos", "dest_pos", "length"].map(|base| self.temp(base));
        let ints: Vec<String> =
            [src_pos, dest_pos, length].iter().map(|e| self.value(e, Type::Int).text).collect();
        let checked = self.facts.raising.contains(&id);
        let call = self.class_copy_call(src, dest, (&p, &q, &n), &convert);
        let body = if checked {
            call
        } else {
            let uncaught = self.runtime("uncaught");
            format!("{call}.unwrap_or_else({uncaught})")
        };
        let code = Code::atom(format!(
            "{{ let ({p}, {q}, {n}) = ({}, {}, {}); {body} }}",
            ints[0], ints[1], ints[2]
        ));
        if !checked {
            return Some(code);
        }
        Some(Code::atom(self.passed_on(code)))
    }

    /// The arms converting each element of class `s` that is an instance of `d`. A pattern takes
    /// the object out of the source's representation, which is one class's object or an enum over
    /// several, and the arm yields it in the destination's: an enum variant when `d` has subclasses.
    fn element_arms(&self, s: usize, d: usize, (src_nullable, dst_nullable): (bool, bool)) -> Vec<String> {
        let cases: Vec<(String, usize)> = match self.rep(s) {
            hierarchy::Rep::Object(c) => vec![("x".to_string(), c)],
            hierarchy::Rep::Enum(t) => self
                .enum_variants(t)
                .into_iter()
                .map(|c| (format!("{}(x)", self.variant(t, c)), c))
                .collect(),
        };
        cases
            .into_iter()
            .filter(|&(_, c)| self.class.hierarchy.is_subtype(c, d))
            .map(|(pattern, c)| {
                let object = match self.rep(d) {
                    hierarchy::Rep::Object(_) => "x.clone()".to_string(),
                    hierarchy::Rep::Enum(t) => format!("{}(x.clone())", self.variant(t, c)),
                };
                let pattern = if src_nullable { format!("Some({pattern})") } else { pattern };
                let value = if dst_nullable { format!("Some({object})") } else { object };
                format!("{pattern} => Ok({value}), ")
            })
            .collect()
    }

    /// The call that copies the elements of `src` into `dest` with the conversion `convert`: over
    /// two shared arrays, which may be one array, or over slices when a plain array is one of them.
    fn class_copy_call(&mut self, src: &'a Expr, dest: &'a Expr, (p, q, n): (&str, &str, &str), convert: &str) -> String {
        let plain = |g: &Self, e: &Expr| matches!(&e.kind, ExprKind::Var(name) if g.plain(name).is_some());
        if !plain(self, src) && !plain(self, dest) {
            let (s, d) = (self.handle(src), self.handle(dest));
            let copy = self.runtime("try_arraycopy_with");
            return format!("{copy}({s}, {p}, {d}, {q}, {n}, {convert})");
        }
        let s = self.slice_of(src);
        let d = match &dest.kind {
            ExprKind::Var(name) if self.plain(name).is_some() => self.mut_ref(&self.local(name)),
            _ => format!("&mut {}.borrow_mut()", self.array_receiver(dest)),
        };
        let copy = self.runtime("try_copy_refs");
        format!("{copy}({s}, {p}, {d}, {q}, {n}, {convert})")
    }
}
