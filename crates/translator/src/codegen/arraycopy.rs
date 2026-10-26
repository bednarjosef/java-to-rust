//! `System.arraycopy` between two `Object`s. A pair of arrays of one type is a plain copy. Between
//! arrays of two classes or of strings each element is checked against the destination's class as
//! it is stored (`arraycopy_classes.rs`), and so between arrays of `String` and of `Object`; a pair
//! of arrays of other reference types, such as arrays of arrays, is rejected. Every other pair (a
//! source or destination that is not an array, or a primitive element type on either side and
//! different types) fails with the `ArrayStoreException` that HotSpot throws, with HotSpot's
//! message, which a program may print: one arm makes it from what each side is
//! (`array_copy_kind`), so the arms grow with the pairs of array types, not of all classes.

use super::*;

/// One `System.arraycopy` call, as its arms of the match need it: where the call is, whether a
/// failure goes on from it (`checked`), and the names of the positions and the length.
#[derive(Clone, Copy)]
pub(super) struct CopyCall<'n> {
    pub(super) pos: Pos,
    pub(super) checked: bool,
    pub(super) names: [&'n str; 3],
}

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
    /// `variants` of the enum of `t`: a pair of arrays of one class, a pair of arrays of two
    /// reference types, and last every pair that fails.
    pub(super) fn object_copy_arms(&mut self, t: usize, variants: &[usize], call: CopyCall) -> Vec<String> {
        let arrays: Vec<(usize, Type)> =
            variants.iter().filter_map(|&c| self.element_of(c).map(|element| (c, element))).collect();
        // `String`'s variant, found once for all the pairs.
        let string = variants.iter().copied().find(|&c| self.binary_name(c) == "java.lang.String");
        let mut arms = Vec::new();
        for &(s, es) in &arrays {
            for &(d, ed) in &arrays {
                if s == d || (primitive_name(es).is_none() && primitive_name(ed).is_none()) {
                    arms.push(self.array_pair_arm(t, string, (s, d), (es, ed), call));
                }
            }
        }
        self.need(Need::CopyKind(t));
        let failure =
            format!("{}(src.array_copy_kind(), dest.array_copy_kind())", self.runtime("arraycopy_mismatch"));
        let body = if call.checked {
            format!("Err({failure})")
        } else {
            format!("{}({failure})", self.runtime("throw_exception"))
        };
        // The arms bind the arrays; the values themselves are `src` and `dest` (`object_array_copy`).
        arms.push(format!("_ => {body},"));
        arms
    }

    /// What an object of class `c` is to `System.arraycopy` (`java::CopyKind`).
    pub(super) fn copy_kind(&mut self, c: usize) -> String {
        let kind = self.runtime("CopyKind");
        match self.element_of(c) {
            None => format!("{kind}::NotArray({})", rust_string(&self.binary_name(c))),
            Some(element) => match primitive_name(element) {
                Some(name) => format!("{kind}::Primitive(\"{name}\")"),
                None => format!("{kind}::References"),
            },
        }
    }

    /// The arm for a source array of class `s` and a destination array of class `d`, where
    /// `string` is the variant of `String` among the classes, if it is one.
    fn array_pair_arm(
        &mut self,
        t: usize,
        string: Option<usize>,
        (s, d): (usize, usize),
        (es, ed): (Type, Type),
        call: CopyCall,
    ) -> String {
        let (vs, vd) = (self.view_pattern(t, s), self.view_pattern(t, d));
        let (pattern, body) = if s == d {
            let copy = self.runtime(if call.checked { "try_arraycopy" } else { "arraycopy" });
            let [p, q, n] = call.names;
            (format!("({vs}(src), {vd}(dest))"), format!("{copy}(src, {p}, dest, {q}, {n})"))
        } else {
            self.object_copy_between(t, string, (s, d), (es, ed), call)
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

    /// A copy between arrays of two different reference types: the pattern of its arm and its body. The
    /// pattern binds `src` and `dest` only where the body reads them.
    fn object_copy_between(
        &mut self,
        t: usize,
        string: Option<usize>,
        (s, d): (usize, usize),
        (es, ed): (Type, Type),
        call: CopyCall,
    ) -> (String, String) {
        let CopyCall { pos, checked, names: [p, q, n] } = call;
        let (vs, vd) = (self.view_pattern(t, s), self.view_pattern(t, d));
        let is_object = |ty: &Type| matches!(ty, Type::Class(name, _) if *name == OBJECT);
        let object = struct_name(&self.class.types[t].name);
        let copy = self.runtime("try_arraycopy_with");
        let uncaught = self.runtime("uncaught");
        let (string, is_string_to_object, is_object_to_string) = match string {
            Some(c) => (c, es == Type::String && is_object(&ed), is_object(&es) && ed == Type::String),
            None => (0, false, false),
        };
        let text = rust_type(Type::String);
        // Each element is stored as the `Object` it is: every `String` is an `Object`.
        let convert = if is_string_to_object {
            let string_variant = self.variant(t, string);
            format!(
                "|e: &Option<{text}>| -> Result<Option<{object}>, &'static str> {{ Ok(e.clone().map({string_variant})) }}"
            )
        } else if is_object_to_string {
            // An element is stored only if it is a `String`, which no other `Object` is.
            let string_variant = self.view_pattern(t, string);
            let message = "arraycopy: element type mismatch: can not cast one of the elements of java.lang.Object[] to the type of the destination array, java.lang.String";
            format!(
                "|e: &Option<{object}>| -> Result<Option<{text}>, &'static str> {{ match e.as_ref().map({object}::view) {{ None => Ok(None), Some({string_variant}(s)) => Ok(Some(s.clone())), Some(_) => Err(\"{message}\") }} }}"
            )
        } else if let (Some(a), Some(b)) = (self.class_element(es), self.class_element(ed)) {
            // Arrays of two classes: each element is checked against the destination's class.
            self.class_element_convert((a, b), (es, ed))
        } else {
            let (from, to) = (self.binary_name(s), self.binary_name(d));
            self.rejected.push(Error {
                pos,
                message: format!("`System.arraycopy` from {from} to {to} not supported yet"),
            });
            return (format!("({vs}(_), {vd}(_))"), "unreachable!()".to_string());
        };
        let copy_call = format!("{copy}(src, {p}, dest, {q}, {n}, {convert})");
        let body = if checked { copy_call } else { format!("{copy_call}.unwrap_or_else({uncaught})") };
        (format!("({vs}(src), {vd}(dest))"), body)
    }

    /// The type of the elements of type `element`, when they are of a class or strings.
    fn class_element(&self, element: Type) -> Option<usize> {
        match element {
            Type::Class(..) | Type::String => self.type_of(element),
            _ => None,
        }
    }
}
