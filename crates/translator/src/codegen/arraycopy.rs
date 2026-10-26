//! `System.arraycopy` between two `Object`s. A pair of arrays of one class is a plain copy, with
//! an arm of its own at the call. Every other pair goes to `java::try_arraycopy_across`, which
//! does what HotSpot does: a source or destination that is not an array, or an array of a
//! primitive type, fails with HotSpot's `ArrayStoreException`; between arrays of references each
//! element is checked against the class of the destination's elements as it is stored. It asks the
//! program's `Object` for what it needs through `java::CopyAcross`, which is written once for the
//! program with one arm for each array class: the elements of a source as `Object`s, and the
//! storing of `Object`s into a destination, each converted to the destination's elements. So the
//! arms grow with the array classes, not with their pairs, and every pair is translated.

use super::*;

/// One `System.arraycopy` call, as its arms of the match need it: whether a failure goes on from
/// it (`checked`), and the names of the positions and the length.
#[derive(Clone, Copy)]
pub(super) struct CopyCall<'n> {
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
    /// The arms of `match (src.view(), dest.view())` for `System.arraycopy`, over the runtime
    /// classes `variants` of the enum of `t`: a pair of arrays of one class, and last every other
    /// pair, which binds `src` and `dest` as the objects (`object_array_copy`).
    pub(super) fn object_copy_arms(&mut self, t: usize, variants: &[usize], call: CopyCall) -> Vec<String> {
        let [p, q, n] = call.names;
        let copy = self.runtime(if call.checked { "try_arraycopy" } else { "arraycopy" });
        let mut arms = Vec::new();
        for &c in variants {
            if self.element_of(c).is_some() {
                let v = self.view_pattern(t, c);
                arms.push(format!("({v}(src), {v}(dest)) => {copy}(src, {p}, dest, {q}, {n}),"));
            }
        }
        self.need(Need::CopyAcross(t));
        // Named, so that objects held by reference (`&&Object`) are dereferenced to the enum.
        let name = struct_name(&self.class.types[t].name);
        let across = format!("{}::<{name}>(src, {p}, dest, {q}, {n})", self.runtime("try_arraycopy_across"));
        let body = if call.checked {
            across
        } else {
            format!("{across}.unwrap_or_else(|e| {}(e))", self.runtime("throw_exception"))
        };
        arms.push(format!("_ => {body},"));
        arms
    }

    /// `impl java::CopyAcross` for the enum of `t`, whose runtime classes are `variants`: for
    /// each array of references, its length, its elements as values of the enum, and the storing
    /// of values of the enum, which the runtime has checked against the class of its elements.
    pub(super) fn copy_across_impl(&mut self, t: usize, variants: &[usize]) {
        let arrays: Vec<usize> = variants
            .iter()
            .copied()
            .filter(|&c| self.element_of(c).is_some_and(|e| primitive_name(e).is_none()))
            .collect();
        let name = struct_name(&self.class.types[t].name);
        let elements = self.runtime("Elements");
        let (kind, class) = (self.runtime("CopyKind"), self.runtime("Class"));
        self.gap();
        let across = self.runtime("CopyAcross");
        self.line(&format!("impl {across} for {name} {{"));
        self.indent += 1;
        self.line(&format!("fn copy_kind(&self) -> {kind} {{"));
        self.line("    self.array_copy_kind()");
        self.line("}");
        self.line(&format!("fn copy_class(&self) -> {class} {{"));
        self.line("    self.runtime_class()");
        self.line("}");
        let methods: [(&str, &str, &str); 3] = [
            ("fn copy_length(&self) -> usize", "length", "0"),
            ("fn copy_out(&self, range: std::ops::Range<usize>) -> Vec<Option<Self>>", "out", "Vec::new()"),
            ("fn copy_in(&self, at: usize, items: &[Option<Self>])", "in", "{}"),
        ];
        for (signature, method, otherwise) in methods {
            self.line(&format!("{signature} {{"));
            self.indent += 1;
            if arrays.is_empty() {
                self.line(otherwise);
            } else {
                let elements = elements.clone();
                self.match_arms_or(&arrays, Some(otherwise.to_string()), &move |g: &mut Self, c| {
                    let element = g.element_of(c).expect("an array class");
                    let text = match method {
                        "length" => format!("{elements}::length(o)"),
                        "out" => {
                            let each = g.element_as_value(t, element);
                            format!("{elements}::reading(o, range, |items| items.iter().map(|e| {each}).collect())")
                        }
                        _ => {
                            let each = g.value_as_element(t, element);
                            format!("{elements}::store_from(o, at, &items.iter().map(|e| {each}).collect::<Vec<_>>())")
                        }
                    };
                    (text, true)
                });
            }
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// An element `e` (a reference) of an array of `element`s as an `Option` of a value of the
    /// enum of `t`, `Object`'s: null as `None`.
    fn element_as_value(&mut self, t: usize, element: Type) -> String {
        let Some(c) = self.type_of(element) else {
            return self.copy_missing(&format!("a {element} as an `Object`"));
        };
        // The element, owned, is `x`.
        let value = match self.rep(c) {
            _ if c == t => "x".to_string(),
            hierarchy::Rep::Enum(e) if e == t => "x".to_string(),
            hierarchy::Rep::Enum(e) => {
                // A value of another enum has the same bits, where `Object`'s has its class.
                self.need(Need::Convert(e, t));
                let convert = format!("x.{}()", self.to_name(t));
                let (mine, objects) = (self.enum_variants(e), self.enum_variants(t));
                let shared: Vec<usize> = mine.iter().copied().filter(|c| objects.contains(c)).collect();
                if shared.len() == mine.len() {
                    convert
                } else {
                    let missing = self.copy_missing("an object of a class that is no `Object` elsewhere");
                    if shared.is_empty() {
                        format!("{{ let _ = x; {missing} }}")
                    } else {
                        let tags = self.tags(&shared, &mine);
                        format!("match x.0.tag() {{ {tags} => {convert}, _ => {missing} }}")
                    }
                }
            }
            hierarchy::Rep::Object(k) => match self.variant_of(t, k) {
                Some(variant) => format!("{variant}(x)"),
                None => {
                    let missing = self.copy_missing(&format!("a {} as an `Object`", self.binary_name(k)));
                    format!("{{ let _ = x; {missing} }}")
                }
            },
        };
        if self.nullable_element(element) {
            format!("e.clone().map(|x| {value})")
        } else {
            format!("{{ let x = e.clone(); Some({value}) }}")
        }
    }

    /// A value `e` (an `&Option` of a value of the enum of `t`) as an element of an array of
    /// `element`s, which the runtime has found it to be an instance of, or null.
    fn value_as_element(&mut self, t: usize, element: Type) -> String {
        let nullable = self.nullable_element(element);
        let null = self.copy_missing(&format!("a null into an array of {element} that holds none"));
        let Some(c) = self.type_of(element) else {
            return self.copy_missing(&format!("an `Object` as a {element}"));
        };
        let narrowed = if c == t {
            "o.clone()".to_string()
        } else {
            self.need(Need::Narrow(t, c));
            let missing = self.copy_missing(&format!("an `Object` as a {}", self.binary_name(c)));
            format!("o.{}().unwrap_or_else(|| {missing})", self.as_name(c))
        };
        if nullable {
            format!("e.as_ref().map(|o| {narrowed})")
        } else {
            format!("match e {{ Some(o) => {narrowed}, None => {null} }}")
        }
    }

    /// The end of a program at a copy that the translation cannot make: `what`, which no object
    /// of the program was found to be.
    fn copy_missing(&mut self, what: &str) -> String {
        let message = format!("`System.arraycopy` of {what}");
        format!("{}({})", self.runtime("not_translated"), rust_string(&message))
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

    /// The element type of the array class `c`, when it is an array class.
    fn element_of(&self, c: usize) -> Option<Type> {
        match self.class.array_of(c) {
            Some(Type::Array(element)) => Some(*element),
            _ => None,
        }
    }
}
