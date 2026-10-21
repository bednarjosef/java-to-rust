//! Covariant arrays. Java converts an array of references to an array of any supertype of its
//! elements (a `Version[]` is an `Object[]`), and erasure makes the array of a type variable an
//! array of its bound: `list.toArray(new Version[0])` passes a `Version[]` where `T[]` is an
//! `Object[]`, and `toArray(a)` with an array big enough fills `a` and returns it. The
//! translation's arrays do not convert (`Version[]` is `java::Array<Option<Rc<Version>>>`), and
//! copying one at the boundary would lose that identity, so two different array types of
//! references that meet (an argument, an assignment, a return or a cast between the two, or a use
//! where javac substituted one for the array of a type variable) are each an `Object[]`
//! everywhere in the program. Lowering notes each such type, and the program is lowered again with
//! it as an `Object[]` (`Lowering::ty`), whose element reads get the checked cast back that
//! erasure writes (`generics::erased_use`), whose stores convert to `Object`, and whose for-each
//! variable is cast from `Object` (`arrays::for_each`). Every other array keeps its exact type.
//!
//! An array of strings is covariant the same way, though the runtime's string builtins make and
//! take `java::Array<String>`: where `String[]` is an `Object[]`, the builtins that read one get a
//! copy as a `String[]` (`Builtin::StringArrayOf`: `String.join`, `Arrays.toString`, `equals`,
//! `hashCode`, `binarySearch`), those that make one (`split`, `main`'s `args`) make the
//! `Object[]` (`Builtin::ObjectArrayOf`), and the runtime's `Arrays.sort` and `fill` of strings,
//! which change the array in place, are rejected on it.
//!
//! Such an array does not remember its element class: storing an object of another class in it
//! does not throw `ArrayStoreException`, and its `getClass()` is `Object[]`'s.

use super::*;
use std::rc::Rc;

/// The array types that are `Object[]`s in this lowering (`known`), and those that it found
/// covariant (`noted`), shared with `real_generics::retry`, which lowers again with them.
#[derive(Clone, Default)]
pub(super) struct Covariant {
    pub(super) known: Vec<Type>,
    pub(super) noted: Rc<RefCell<Vec<Type>>>,
}

impl<'t> Lowering<'t> {
    /// Whether array types `a` and `b` meet as two different arrays of references (`Version[]`
    /// and `Object[]`, or `Integer[]` and the `Comparable[]` that `T[]` erases to when
    /// `T extends Comparable<T>`), or as arrays of such arrays; each of them that is not an
    /// `Object[]` is then noted to be one. Once they are, they no longer differ: arrays that
    /// still do are not covariant.
    pub(super) fn meets_object_array(&self, a: Type, b: Type) -> bool {
        match (a, b) {
            (Type::Array(x), Type::Array(y)) if matches!((x, y), (Type::Array(_), Type::Array(_))) => {
                self.meets_object_array(*x, *y)
            }
            (Type::Array(x), Type::Array(y)) if x != y && x.is_reference() && y.is_reference() => {
                let a = !Self::is_object(*x) && self.note_covariant(a);
                let b = !Self::is_object(*y) && self.note_covariant(b);
                a | b
            }
            _ => false,
        }
    }

    /// An array of strings stays a `java::Array<String>`: the runtime's string methods make and
    /// take those (`split`, `String.join`, `main`'s arguments).
    fn note_covariant(&self, ty: Type) -> bool {
        let mut innermost = ty;
        while let Type::Array(element) = innermost {
            innermost = *element;
        }
        if self.object.is_none() || self.has_param(ty) || self.covariant.known.contains(&ty) {
            return false;
        }
        let mut noted = self.covariant.noted.borrow_mut();
        if !noted.contains(&ty) {
            noted.push(ty);
        }
        true
    }

    /// Whether `node`, an `Arrays.sort` or `Arrays.fill` of a `String[]` that is an `Object[]`,
    /// calls the JDK's method for an `Object[]`, which reachability then kept
    /// (`Reach::string_array_writes`).
    pub(super) fn jdk_string_array_write(&self, node: Node<'t>) -> bool {
        self.strings_covariant()
            && self.reach.is_some_and(|r| r.string_array_writes)
            && crate::program::writes_string_array(node.0)
    }

    /// Whether `String[]` is an `Object[]` in this lowering.
    pub(super) fn strings_covariant(&self) -> bool {
        self.covariant.known.contains(&Type::array(Type::String))
    }

    /// The arguments `args` of a call of the runtime's string or array builtins (`node`), each
    /// whose javac type is `String[]` given as the `String[]` copy that the builtin reads, where
    /// `String[]` is an `Object[]`. `writes`: the builtin changes the array, which a copy would
    /// not show, so it is rejected; `copies`: it takes any array, such as an `Object[]`.
    pub(super) fn string_array_args(
        &self,
        node: Node<'t>,
        args: Vec<Expr>,
        writes: bool,
        copies: bool,
    ) -> Result<Vec<Expr>, Error> {
        let nodes: Vec<Node> = node.list("arguments").collect();
        if !self.strings_covariant() || copies || nodes.len() != args.len() {
            return Ok(args);
        }
        let mut out = Vec::new();
        for (a, n) in args.into_iter().zip(nodes) {
            if n.java_type() != "java.lang.String[]" || !Self::is_object_array(a.ty) {
                out.push(a);
            } else if writes {
                return unsupported(node, STRING_ARRAY_WRITE);
            } else {
                let pos = a.pos;
                out.push(self.synth(
                    ExprKind::Builtin(Builtin::StringArrayOf, vec![a]),
                    pos,
                    Type::array(Type::String),
                ));
            }
        }
        Ok(out)
    }

    /// The call `kind` of a builtin that makes a `String[]` (javac's type of `node`), as the
    /// `Object[]` that `String[]` is where it is covariant.
    pub(super) fn object_array_result(&self, node: Node<'t>, kind: ExprKind) -> ExprKind {
        if !self.strings_covariant() || node.java_type() != "java.lang.String[]" {
            return kind;
        }
        let made = self.synth(kind, node.pos(), Type::array(Type::String));
        ExprKind::Builtin(Builtin::ObjectArrayOf, vec![made])
    }

    /// Whether `ty` is `Object[]`.
    fn is_object_array(ty: Type) -> bool {
        matches!(ty, Type::Array(element) if Self::is_object(*element))
    }

    /// Array type `array` as it is translated: an `Object[]` when it is covariant.
    pub(super) fn covariant_array(&self, array: Type) -> Type {
        if self.covariant.known.contains(&array) { Type::array(Type::class(OBJECT)) } else { array }
    }

    /// Erases the Rust generic that would take the `Object[]` of a covariant array (`arg`) for
    /// an array of its type variable (`param`), where it would infer `Object` for the variable.
    pub(super) fn fail_on_covariant(&self, arg: Type, param: Type) {
        if let (Type::Array(element), Type::Array(variable)) = (arg, param)
            && Self::is_object(*element)
            && !self.covariant.known.is_empty()
            && let Some(owner) = self.param_owner(*variable)
        {
            self.fail(owner);
        }
    }
}
