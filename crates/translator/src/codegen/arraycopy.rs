//! `System.arraycopy` between two `Object`s, pair by pair. A pair of arrays of one type is a plain
//! copy. A pair of arrays of other types is what Java does for it, or the `ArrayStoreException`
//! that HotSpot throws, with HotSpot's message, which a program may print. The element classes
//! that a message or a check would name are `String` and `Object`; another element class is
//! rejected, as no check against it is written here.

use super::*;

/// The name of a primitive type as HotSpot's messages give it: `int`.
fn primitive_name(ty: Type) -> Option<&'static str> {
    Some(match ty {
        Type::Int => "int",
        Type::Long => "long",
        Type::Byte => "byte",
        Type::Short => "short",
        Type::Char => "char",
        Type::Float => "float",
        Type::Double => "double",
        Type::Boolean => "boolean",
        _ => return None,
    })
}

impl<'a> Gen<'a> {
    /// The arms of `match (&src, &dest)` for `System.arraycopy`, over the runtime classes
    /// `variants` of the enum of `t`. They cover every pair of classes, each in one arm: a
    /// non-array source, then a non-array destination, then a pair of arrays. `names` are the
    /// locals of the positions and the length.
    pub(super) fn object_copy_arms(
        &mut self,
        t: usize,
        variants: &[usize],
        pos: Pos,
        checked: bool,
        names: [&str; 3],
    ) -> Vec<String> {
        let arrays: Vec<(usize, Type)> =
            variants.iter().filter_map(|&c| self.element_of(c).map(|element| (c, element))).collect();
        let others: Vec<usize> = variants.iter().copied().filter(|&c| self.element_of(c).is_none()).collect();
        let mut arms = Vec::new();
        for &s in &others {
            let message = format!("arraycopy: source type {} is not an array", self.binary_name(s));
            let body = self.mismatch(checked, &message);
            arms.push(format!("({}(_), _) => {body},", self.variant(t, s)));
        }
        for &(s, _) in &arrays {
            for &d in &others {
                let message = format!("arraycopy: destination type {} is not an array", self.binary_name(d));
                let body = self.mismatch(checked, &message);
                arms.push(format!("({}(_), {}(_)) => {body},", self.variant(t, s), self.variant(t, d)));
            }
        }
        for &(s, es) in &arrays {
            for &(d, ed) in &arrays {
                arms.push(self.array_pair_arm(t, variants, (s, d), (es, ed), pos, checked, names));
            }
        }
        arms
    }

    /// The arm for a source array of class `s` and a destination array of class `d`.
    fn array_pair_arm(
        &mut self,
        t: usize,
        variants: &[usize],
        (s, d): (usize, usize),
        (es, ed): (Type, Type),
        pos: Pos,
        checked: bool,
        [p, q, n]: [&str; 3],
    ) -> String {
        let (vs, vd) = (self.variant(t, s), self.variant(t, d));
        let (pattern, body) = if s == d {
            let copy = self.runtime(if checked { "try_arraycopy" } else { "arraycopy" });
            (format!("({vs}(src), {vd}(dest))"), format!("{copy}(src, {p}, dest, {q}, {n})"))
        } else {
            self.object_copy_between(t, variants, (s, d), (es, ed), pos, checked, [p, q, n])
        };
        format!("{pattern} => {body},")
    }

    /// The element type of the array class `c`, when it is an array class.
    fn element_of(&self, c: usize) -> Option<Type> {
        match self.class.array_of(c) {
            Some(Type::Array(element)) => Some(*element),
            _ => None,
        }
    }

    /// A copy between arrays of two different types: the pattern of its arm and its body. The
    /// pattern binds `src` and `dest` only where the body reads them.
    fn object_copy_between(
        &mut self,
        t: usize,
        variants: &[usize],
        (s, d): (usize, usize),
        (es, ed): (Type, Type),
        pos: Pos,
        checked: bool,
        names: [&str; 3],
    ) -> (String, String) {
        let [p, q, n] = names;
        let (vs, vd) = (self.variant(t, s), self.variant(t, d));
        let mismatch = "arraycopy: type mismatch: can not copy";
        let message = match (primitive_name(es), primitive_name(ed)) {
            (Some(a), Some(b)) => Some(format!("{mismatch} {a}[] into {b}[]")),
            (Some(a), None) => Some(format!("{mismatch} {a}[] into object array[]")),
            (None, Some(b)) => Some(format!("{mismatch} object array[] into {b}[]")),
            (None, None) => None,
        };
        if let Some(message) = message {
            return (format!("({vs}(_), {vd}(_))"), self.mismatch(checked, &message));
        }
        let is_object = |ty: &Type| matches!(ty, Type::Class(name, _) if *name == OBJECT);
        let string = variants.iter().copied().find(|&c| self.binary_name(c) == "java.lang.String");
        let object = struct_name(&self.class.types[t].name);
        let copy = self.runtime("try_arraycopy_with");
        let uncaught = self.runtime("uncaught");
        let (string, is_string_to_object, is_object_to_string) = match string {
            Some(c) => (c, es == Type::String && is_object(&ed), is_object(&es) && ed == Type::String),
            None => (0, false, false),
        };
        // Each element is stored as the `Object` it is: every `String` is an `Object`.
        let convert = if is_string_to_object {
            let string_variant = self.variant(t, string);
            format!(
                "|e: &Option<String>| -> Result<Option<{object}>, &'static str> {{ Ok(e.clone().map({string_variant})) }}"
            )
        } else if is_object_to_string {
            // An element is stored only if it is a `String`, which no other `Object` is.
            let string_variant = self.variant(t, string);
            let message = "arraycopy: element type mismatch: can not cast one of the elements of java.lang.Object[] to the type of the destination array, java.lang.String";
            format!(
                "|e: &Option<{object}>| -> Result<Option<String>, &'static str> {{ match e {{ None => Ok(None), Some({string_variant}(s)) => Ok(Some(s.clone())), Some(_) => Err(\"{message}\") }} }}"
            )
        } else {
            let (from, to) = (self.binary_name(s), self.binary_name(d));
            self.rejected.push(Error {
                pos,
                message: format!("`System.arraycopy` from {from} to {to} not supported yet"),
            });
            return (format!("({vs}(_), {vd}(_))"), "unreachable!()".to_string());
        };
        let call = format!("{copy}(src, {p}, dest, {q}, {n}, {convert})");
        let body = if checked { call } else { format!("{call}.unwrap_or_else({uncaught})") };
        (format!("({vs}(src), {vd}(dest))"), body)
    }

    /// The `ArrayStoreException` that HotSpot throws with `message`: the copy's `Err` when it is
    /// checked, and a throw otherwise.
    fn mismatch(&mut self, checked: bool, message: &str) -> String {
        if checked {
            format!(
                "Err({}(\"java.lang.ArrayStoreException\", \"{message}\"))",
                self.runtime("Exception::new")
            )
        } else {
            format!("{}(\"java.lang.ArrayStoreException\", \"{message}\")", self.runtime("throw"))
        }
    }
}
