//! Covariant arrays. Java converts an array of references to an array of any supertype of its
//! elements (a `Version[]` is an `Object[]`), and erasure makes the array of a type variable an
//! array of its bound: `list.toArray(new Version[0])` passes a `Version[]` where `T[]` is an
//! `Object[]`, and `toArray(a)` with an array big enough fills `a` and returns it. The
//! translation's arrays do not convert (`Version[]` is `java::Array<Option<Rc<Version>>>`), and
//! copying one at the boundary would lose that identity, so two different array types of
//! references that meet (an argument, an assignment, a return or a cast between the two, or a use
//! where javac substituted one for the array of a type variable) are each an `Object[]`
//! everywhere in the program. Lowering notes each such type, and those that would meet only once
//! the noted ones are `Object[]`s (`noted_later`), and the program is lowered again with them as
//! `Object[]`s (`Lowering::ty`), whose element reads get the checked cast back that
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
//! (`exact_ty`). An array of arrays (`String[][]` as an `Object[][]`, `int[][]` as an
//! `Object[]`) carries its class too, and so does each row of a new array of several dimensions
//! that the translation holds as an `Object[]` (`Class::object_rows`).
//!
//! An array that becomes an `Object` (`Object o = names`) meets each array type that an `Object`
//! is tested for or cast to where it may be one (`meets_through_object`): a `String[]` and
//! `instanceof Object[]` or `(Comparable<?>[]) o`, an `int[][]` and `(Object[]) o`. Both are then
//! `Object[]`s and the test reads the class the array carries.

use super::*;
use std::rc::Rc;

/// Type `ty` as lowering translates it with the array types `known` as `Object[]`s, from the
/// innermost element out (`Lowering::covariant_array`).
fn lowered_as(ty: Type, known: &std::collections::HashSet<Type>) -> Type {
    let Type::Array(element) = ty else { return ty };
    let array = Type::array(lowered_as(*element, known));
    if known.contains(&array) { Type::array(Type::class(OBJECT)) } else { array }
}

/// The array types that are `Object[]`s in this lowering (`known`), and those that it found
/// covariant (`noted`), shared with `real_generics::retry`, which lowers again with them; the
/// array types that met in this lowering (`meets_object_array`), which `noted_later` meets
/// again as the next lowering would; and the prediction made before the first lowering, shared
/// by all of them (`predict`).
#[derive(Clone, Default)]
pub(super) struct Covariant {
    pub(super) known: std::collections::HashSet<Type>,
    pub(super) noted: Rc<RefCell<Vec<Type>>>,
    pub(super) predicted: Rc<RefCell<Option<super::predict::Prediction>>>,
    met: RefCell<std::collections::HashSet<(Type, Type)>>,
}

impl Covariant {
    /// Forgets the array types that met, which a prediction of the covariant arrays made before
    /// this lowering lowered anything recorded (`predict::predict_arrays`).
    pub(super) fn forget_meets(&self) {
        self.met.take();
    }
}

impl<'t> Lowering<'t> {
    /// Whether array types `a` and `b` meet as two different arrays of references (`Version[]`
    /// and `Object[]`, or `Integer[]` and the `Comparable[]` that `T[]` erases to when
    /// `T extends Comparable<T>`), or as arrays of such arrays; each of them that is not an
    /// `Object[]` is then noted to be one. Once they are, they no longer differ: arrays that
    /// still do are not covariant.
    pub(super) fn meets_object_array(&self, a: Type, b: Type) -> bool {
        if a != b {
            self.covariant.met.borrow_mut().insert((a, b));
        }
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

    /// Notes array types `array`, which becomes an `Object`, and `tested`, which an `Object` is
    /// tested for or cast to, as covariant where `array` may be a `tested` (`meets_object_array`):
    /// `Object o = new String[] {..}` and `o instanceof Object[]` or `(Comparable<?>[]) o`, or an
    /// `int[][]` and `Object[]`. The test then reads the class the array carries.
    pub(super) fn meets_through_object(&self, array: Type, tested: Type) -> bool {
        array != tested && self.may_hold(array, tested) && self.meets_object_array(array, tested)
    }

    /// Notes the array types that the lowerings after this one would note once those it noted are
    /// `Object[]`s, as far as they meet as here: each pair of types that met here
    /// (`meets_object_array`, `meets_through_object`) meets again as lowering translates it then
    /// (`covariant_array`, from the innermost element), and so on until no more are noted. A
    /// `String[][]` that met an `Object[]` with `String[]` noted is an `Object[][]`, which meets
    /// that `Object[]` as well; an array that becomes an `Object` meets every `Object[]` that an
    /// `Object` is cast to once some type it was cast to is one. So the program is lowered again
    /// once for what one lowering finds, not once more for each step of it; a type that only a
    /// changed lowering meets is noted by the next attempt as before. The types meet as the next
    /// lowering has them without the Rust generics that `failed` here (`without_failed`): an array
    /// of a generic class with type arguments is another type once the class is erased, so a
    /// `MemoryModuleType<GlobalPos>[]` noted here is a `MemoryModuleType[]` there. A Rust generic
    /// class whose array a type noted here holds fails with it (`real_element_class`).
    pub(super) fn noted_later(&self, failed: &mut super::real_generics::Real) {
        if self.object.is_none() || self.covariant.noted.borrow().is_empty() && failed.is_empty() {
            return;
        }
        let mut known = self.covariant.known.clone();
        known.extend(self.covariant.noted.borrow().iter().copied());
        let met: Vec<(Type, Type)> = self.covariant.met.borrow().iter().copied().collect();
        loop {
            let lowered = |ty: Type| lowered_as(self.without_failed(ty, failed), &known);
            let mut more = Vec::new();
            for &(a, b) in &met {
                self.meet_again(lowered(a), lowered(b), &known, &mut more);
            }
            let arrays: Vec<Type> = self.object_arrays.borrow().iter().map(|&t| lowered(t)).collect();
            let tests: Vec<Type> = self.object_tests.borrow().iter().map(|&t| lowered(t)).collect();
            for &array in &arrays {
                for &tested in &tests {
                    if array != tested && self.may_hold(array, tested) {
                        self.meet_again(array, tested, &known, &mut more);
                    }
                }
            }
            if more.is_empty() {
                return;
            }
            failed.classes.extend(more.iter().filter_map(|&ty| self.real_element_class(ty)));
            known.extend(more.iter().copied());
            self.covariant.noted.borrow_mut().extend(more);
        }
    }

    /// `meets_object_array` of `a` and `b` with the array types `known` as `Object[]`s, each type
    /// it would note pushed onto `more`.
    fn meet_again(&self, a: Type, b: Type, known: &std::collections::HashSet<Type>, more: &mut Vec<Type>) {
        match (a, b) {
            (Type::Array(x), Type::Array(y)) if matches!((x, y), (Type::Array(_), Type::Array(_))) => {
                self.meet_again(*x, *y, known, more)
            }
            (Type::Array(x), Type::Array(y)) if x != y && Self::of_objects(*x) && Self::of_objects(*y) => {
                for (ty, element) in [(a, *x), (b, *y)] {
                    if !Self::is_object(element)
                        && !self.has_param(ty)
                        && !known.contains(&ty)
                        && !more.contains(&ty)
                    {
                        more.push(ty);
                    }
                }
            }
            _ => {}
        }
    }

    /// Whether an array of type `array` may be one of type `tested`: an array of references whose
    /// elements may be `tested`'s, or arrays of such arrays. A class of the runtime (a string, a
    /// box, a class, a builder) may be of any interface, and a box a `Number`.
    fn may_hold(&self, array: Type, tested: Type) -> bool {
        let (Type::Array(a), Type::Array(b)) = (array, tested) else { return false };
        if !Self::of_objects(*a) || !Self::of_objects(*b) {
            return false;
        }
        match (*a, *b) {
            (_, b) if Self::is_object(b) => true,
            (Type::Array(_), Type::Array(_)) => self.may_hold(*a, *b),
            (Type::Class(..) | Type::Enum(_), b @ Type::Class(..)) => self.upcast_to(*a, b),
            (Type::String | Type::Boxed(_) | Type::JavaClass | Type::StringBuilder, Type::Class(name, _)) => {
                name == NUMBER
                    || self.type_index(name).is_none_or(|t| self.types[t].node.kind() == "INTERFACE")
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
        if let Some(t) = self.real_element_class(ty) {
            self.fail(super::real_generics::Owner::Class(t));
        }
        true
    }

    /// The Rust generic class whose objects array type `ty` holds (`Box<String>[]`, or arrays of
    /// them): once `ty` is an `Object[]`, they are `Object`s, which a Rust generic's objects must
    /// not become, so the class fails with the array (see `real_generics`).
    fn real_element_class(&self, mut ty: Type) -> Option<usize> {
        while let Type::Array(element) = ty {
            ty = *element;
        }
        let Type::Class(name, args) = ty else { return None };
        let t = self.type_index(name)?;
        (!args.is_empty() && self.real.classes.contains(&t)).then_some(t)
    }

    /// Whether `node`, an `Arrays.sort` or `Arrays.fill` of a `String[]` that is an `Object[]`,
    /// calls the JDK's method for an `Object[]`, which reachability then kept
    /// (`Reach::string_array_writes`); the code that holds it is noted when it does
    /// (`Lowering::string_array_writes`).
    pub(super) fn jdk_string_array_write(&self, node: Node<'t>) -> bool {
        let jdk = self.strings_covariant()
            && self.reach.is_some_and(|r| r.string_array_writes)
            && crate::program::writes_string_array(node.0);
        if jdk {
            let code = match self.method_ctx.get() {
                Some(m) => (self.methods[m].owner, Some(m)),
                None => (self.current.get(), None),
            };
            self.string_array_writes.borrow_mut().push(code);
        }
        jdk
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
            // `(T[]) Array.newInstance(a.getClass().getComponentType(), n)`, of `a`'s class.
            "TYPE_CAST" if super::arrays::casts_component_array(node) => {
                let mut call = node.child("expression")?;
                while call.kind() == "PARENTHESIZED" {
                    call = call.child("expression")?;
                }
                if let Some((array, _)) = crate::program::component_array(call.0) {
                    self.made_like(x, Node(array))?;
                }
                return Ok(());
            }
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
                // `Arrays.copyOf(x, n, a.getClass())`, of `a`'s class.
                if class.kind() == "METHOD_INVOCATION"
                    && let Some(array) = class.get("methodSelect").and_then(|s| s.get("expression"))
                {
                    return self.made_like(made, array);
                }
                let java = match literal {
                    true => class.child("expression")?.java_type().to_string(),
                    false => "java.lang.Object[]".to_string(),
                };
                (java, made)
            }
            _ => return Ok(()),
        };
        if !java.ends_with("[]") || !Self::is_object_array(made.ty) {
            return Ok(());
        }
        // An array of arrays carries its own class too (`String[][]`, `int[][]`).
        let exact = self.exact_ty(node, &java)?;
        if !Self::of_objects(exact.element()) || exact == made.ty && node.kind() == "NEW_ARRAY" {
            return Ok(());
        }
        self.element_classes.borrow_mut().insert(made.id, exact);
        Ok(())
    }

    /// The array types that the translation holds as `Object[]`s, in the order of their names
    /// (`Class::covariant_arrays`).
    pub(super) fn covariant_arrays(&self) -> Vec<Type> {
        let mut types: Vec<Type> = self.covariant.known.iter().copied().collect();
        types.sort_by_cached_key(|t| t.to_string());
        types
    }

    /// Notes that `made`, a new array, is of the class of the array that `array` holds, a variable
    /// or a field of one (`Class::array_likes`), where the translation holds both as `Object[]`s.
    fn made_like(&self, made: &Expr, array: Node<'t>) -> Result<(), Error> {
        let named = match array.kind() {
            "IDENTIFIER" => true,
            "MEMBER_SELECT" => array.child("expression")?.kind() == "IDENTIFIER",
            _ => false,
        };
        if !named || !Self::is_object_array(made.ty) {
            return Ok(());
        }
        let source = self.expr(array)?;
        if source.ty == made.ty {
            self.array_likes.borrow_mut().insert(made.id, source);
        }
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
