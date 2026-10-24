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
//! So is an array of an enum (`Day[]` passed for `E[]` of `<E extends Enum<E>>`, or as an
//! `Enum<?>[]`): the translation holds its constants as values, Java as objects, so where it is
//! an `Object[]` its constants are `Object`s (`values()` makes them so), and a read casts each back
//! to the enum, an `Option` of it where the element may be null, as any enum that may be null is
//! (`analysis::nullness`).
//!
//! Where the code makes such an array (`new String[n]`, `{a, b}`, `Arrays.copyOf` into a class
//! literal, an enum's `values()`), the array carries the class Java makes it of
//! (`Class::element_classes`, `java::ElementClass`): a store into an `Object[]` of an object the
//! array cannot hold throws `ArrayStoreException` (`Class::checks_store`), its `getClass()` is its
//! own class, and an `instanceof` or a cast to an array class tests that class
//! (`Class::array_tests`). A class literal of an array type is the array's own class
//! (`exact_ty`). An array of arrays (`String[][]` as an `Object[][]`) carries no class.

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
            (Type::Array(x), Type::Array(y)) if x != y && Self::of_objects(*x) && Self::of_objects(*y) => {
                let a = !Self::is_object(*x) && self.note_covariant(a);
                let b = !Self::is_object(*y) && self.note_covariant(b);
                a | b
            }
            _ => false,
        }
    }

    /// Whether an array of `element`s is an array of references, which Java converts to an
    /// array of a supertype: also an array of an enum, whose constants the translation holds as
    /// values but Java as objects (`Day[]` as `Enum<?>[]` or `Object[]`).
    pub(super) fn of_objects(element: Type) -> bool {
        element.is_reference() || matches!(element, Type::Enum(_))
    }

    /// Notes that array type `ty` is an `Object[]`, unless it already is or holds a Rust generic's
    /// type variable.
    pub(super) fn note_covariant(&self, ty: Type) -> bool {
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
    /// `Object[]` that `String[]` is where it is covariant. The JDK's code that runs a call
    /// (`lower::regex`) already makes the `Object[]`.
    pub(super) fn object_array_result(&self, node: Node<'t>, kind: ExprKind) -> ExprKind {
        if !self.strings_covariant()
            || node.java_type() != "java.lang.String[]"
            || !matches!(kind, ExprKind::Builtin(..))
        {
            return kind;
        }
        let made = self.synth(kind, node.pos(), Type::array(Type::String));
        ExprKind::Builtin(Builtin::ObjectArrayOf, vec![made])
    }

    /// Notes the class of the array that `x`, lowered from `node`, makes where the translation
    /// holds it as an `Object[]` though Java makes it of a narrower class of objects
    /// (`Class::element_classes`): a `new` of an array, or an `Arrays.copyOf` or `copyOfRange`
    /// into a class, which is the literal's or, for a class held in a variable, `Object[]`, as
    /// the translation cannot tell it (it is not the source's: `Arrays.copyOf(a, n, Object[].class)`
    /// of a `String[]` is an `Object[]`).
    pub(super) fn note_element_class(&self, node: Node<'t>, x: &Expr) -> Result<(), Error> {
        if self.covariant.known.is_empty() {
            return Ok(());
        }
        let (java, made) = match node.kind() {
            "NEW_ARRAY" => (node.java_type().to_string(), x),
            // An enum's `values()` and a class literal's `getInterfaces()`, which make a new
            // array of the call's type.
            "METHOD_INVOCATION"
                if matches!(x.kind, ExprKind::ArrayLit(_) | ExprKind::NewArray(_))
                    && node.symbol().is_some_and(|s| {
                        s.is_static && s.name == "values" && self.enum_index(s.owner).is_some()
                            || !s.is_static && s.name == "getInterfaces" && s.owner == "java.lang.Class"
                    }) =>
            {
                (node.java_type().to_string(), x)
            }
            "METHOD_INVOCATION" => {
                let mut made = x;
                while let ExprKind::Cast(_, inner) = &made.kind {
                    made = inner;
                }
                let copy = matches!(
                    made.kind,
                    ExprKind::Builtin(Builtin::ArraysCopyOf | Builtin::Arrays(ArraysOp::CopyOfRange), _)
                );
                let nodes: Vec<Node> = node.list("arguments").collect();
                if !copy
                    || !matches!(nodes.len(), 3 | 4)
                    || nodes.len() == 3 && !node.symbol().is_some_and(|s| s.name == "copyOf")
                {
                    return Ok(());
                }
                let mut class = nodes[nodes.len() - 1];
                while matches!(class.kind(), "TYPE_CAST" | "PARENTHESIZED") {
                    class = class.child("expression")?;
                }
                let literal = class.kind() == "MEMBER_SELECT" && class.str("identifier") == "class";
                let java = match literal {
                    true => class.child("expression")?.java_type().to_string(),
                    false => "java.lang.Object[]".to_string(),
                };
                (java, made)
            }
            _ => return Ok(()),
        };
        let Some(element) = java.strip_suffix("[]") else { return Ok(()) };
        if element.ends_with("[]") || !Self::is_object_array(made.ty) {
            return Ok(());
        }
        let exact = Type::array(self.ty(node, element)?);
        if !Self::of_objects(exact.element()) || exact == made.ty && node.kind() == "NEW_ARRAY" {
            return Ok(());
        }
        self.element_classes.borrow_mut().insert(made.id, exact);
        Ok(())
    }

    /// Notes that `x`, an array that lowering makes, is an array of class `exact`, where the
    /// translation holds it as an `Object[]` (`Class::element_classes`).
    pub(super) fn made_as(&self, x: &Expr, exact: Type) {
        if Self::is_object_array(x.ty) && exact != x.ty && Self::of_objects(exact.element()) {
            self.element_classes.borrow_mut().insert(x.id, exact);
        }
    }

    /// Whether `ty` is `Object[]`.
    fn is_object_array(ty: Type) -> bool {
        matches!(ty, Type::Array(element) if Self::is_object(*element))
    }

    /// The type that javac names `name` as Java has it, with a covariant array type kept as its
    /// own (`String[]`, not the `Object[]` that holds it): the class of a class literal, an
    /// `instanceof` or a cast, which is the array's own class.
    pub(super) fn exact_ty(&self, node: Node<'t>, name: &str) -> Result<Type, Error> {
        match name.strip_suffix("[]") {
            Some(element) => Ok(Type::array(self.exact_ty(node, element)?)),
            None => self.ty(node, name),
        }
    }

    /// `value`, the operand of an `instanceof` or a cast to the array type that javac names
    /// `name` and the translation holds as `ty`, an `Object[]`: noted to be tested against the
    /// array's own class (`Class::array_tests`) where that is narrower, as `String[]` is. An
    /// `Object[]` operand (`(String[]) objects`) is converted to `Object` for the test.
    pub(super) fn array_test(
        &self,
        node: Node<'t>,
        value: Expr,
        ty: Type,
        name: &str,
    ) -> Result<Expr, Error> {
        if self.covariant.known.is_empty() || !Self::is_object_array(ty) {
            return Ok(value);
        }
        let exact = self.exact_ty(node, name)?;
        if exact == ty || !(Self::is_object(value.ty) || value.ty == ty) {
            return Ok(value);
        }
        self.object_array(node, ty)?;
        let value = self.convert(value, Type::class(OBJECT));
        self.array_tests.borrow_mut().insert(value.id, exact);
        Ok(value)
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
