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
        let convert = self.class_element_convert((s, d), (*es, *ed));
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

    /// The conversion of each element of an array of class `s` (element type `es`) into one of
    /// an array of class `d` (`ed`), for `try_arraycopy_with`: a closure that yields the element
    /// in the destination's representation when it passes, and else HotSpot's message. The
    /// element mismatch is named when the destination's class is a subtype of the source's (a
    /// `Named` array copied into a `Box` array); an unrelated pair is a type mismatch.
    pub(super) fn class_element_convert(&mut self, (s, d): (usize, usize), (es, ed): (Type, Type)) -> String {
        let (src_ty, dst_ty) = (self.element_ty_name(es), self.element_ty_name(ed));
        let subtype = self.class.hierarchy.is_subtype(s, d);
        let (from, to) = (self.binary_name(s), self.binary_name(d));
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
        // The arms of an enum's elements match its view.
        let scrutinee = match self.rep(s) {
            hierarchy::Rep::Enum(t) if src_nullable => {
                format!("e.as_ref().map({}::view)", struct_name(&self.class.types[t].name))
            }
            hierarchy::Rep::Enum(_) => "e.view()".to_string(),
            hierarchy::Rep::Object(_) => "e".to_string(),
        };
        format!(
            "|e: &{src_ty}| -> Result<{dst_ty}, &'static str> {{ match {scrutinee} {{ {}{fallthrough}}} }}",
            arms.concat()
        )
    }

    /// The arms converting each element of class `s` that is an instance of `d`. A pattern takes
    /// the object out of the source's representation, which is one class's object or an enum over
    /// several, and the arm yields it in the destination's: an enum variant when `d` has subclasses.
    /// A class that has no variant there, as no value of `d` is of it, ends the program at its
    /// element.
    fn element_arms(
        &mut self,
        s: usize,
        d: usize,
        (src_nullable, dst_nullable): (bool, bool),
    ) -> Vec<String> {
        let cases: Vec<(String, usize)> = match self.rep(s) {
            hierarchy::Rep::Object(c) => vec![("x".to_string(), c)],
            hierarchy::Rep::Enum(t) => self
                .enum_variants(t)
                .into_iter()
                .map(|c| (format!("{}(x)", self.view_pattern(t, c)), c))
                .collect(),
        };
        let missing = self.runtime("not_translated");
        cases
            .into_iter()
            .filter(|&(_, c)| self.class.hierarchy.is_subtype(c, d))
            .map(|(pattern, c)| {
                let pattern = if src_nullable { format!("Some({pattern})") } else { pattern };
                let object = match self.rep(d) {
                    hierarchy::Rep::Object(_) => "x.clone()".to_string(),
                    hierarchy::Rep::Enum(t) if self.is_enum_variant(t, c) => {
                        format!("{}(x.clone())", self.variant(t, c))
                    }
                    hierarchy::Rep::Enum(_) => {
                        let (from, to) = (self.binary_name(c), self.binary_name(d));
                        let pattern = pattern.replace("(x)", "(_)");
                        return format!(
                            "{pattern} => {missing}(\"`System.arraycopy` of a {from} into an array of {to}, which no {to} is elsewhere\"), "
                        );
                    }
                };
                let value = if dst_nullable { format!("Some({object})") } else { object };
                format!("{pattern} => Ok({value}), ")
            })
            .collect()
    }

    /// The call that copies the elements of `src` into `dest` with the conversion `convert`: over
    /// two shared arrays, which may be one array, or over slices when a plain array is one of them.
    fn class_copy_call(
        &mut self,
        src: &'a Expr,
        dest: &'a Expr,
        (p, q, n): (&str, &str, &str),
        convert: &str,
    ) -> String {
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
