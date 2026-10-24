//! Generic classes and methods, erased as javac erases them. A type variable is the erasure of
//! its first bound (`Object` without one), a parameterized type is its class (`Box<Integer>` is
//! `Box`), and a wildcard or a captured type is its bound. The members of a generic class are
//! declared with the erased types, so a use whose type javac substituted (`Box<Integer>.get()` is
//! an `Integer`) gets a `Cast` from the declared type to the substituted one: the checked
//! conversion javac inserts there. Arguments are converted to the erased parameter types by the
//! conversions every call makes (`convert_args`).

use super::real_generics::Owner;
use super::*;

/// The type arguments in javac's `java.lang.Integer,Box<T>`, split at the commas outside `<>`.
pub(super) fn split_args(args: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in args.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(args[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(args[start..].trim());
    out
}

/// The bounds of an intersection type as `JavaTree` writes it, its erasure first
/// (`Main.Base & Main.Named`), split at the `&`s outside `<>`; `None` for any other type.
pub(super) fn intersection_bounds(name: &str) -> Option<Vec<&str>> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in name.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            '&' if depth == 0 => {
                out.push(name[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(name[start..].trim());
    (out.len() > 1).then_some(out)
}

impl<'t> Lowering<'t> {
    /// The declaration of the type variable `name` in scope: the method's being lowered (or the
    /// one `scope` names), then its class's and those of the code it is nested in. One of the
    /// code around an inner, local or anonymous class is erased there: its struct has no type
    /// parameters of its own for it.
    fn type_var(&self, name: &str) -> Option<(Node<'t>, Owner)> {
        let (class, method) = self.scope.get().unwrap_or((self.current.get(), self.method_ctx.get()));
        let found = self.type_var_around(name, class, method)?;
        let around = match found.1 {
            Owner::Method(m) => Some(m) != method,
            Owner::Class(t) => t != class,
        };
        if around && self.nesting(class).is_some() && self.is_real(found.1) {
            self.fail(found.1);
        }
        Some(found)
    }

    fn type_var_around(&self, name: &str, class: usize, method: Option<usize>) -> Option<(Node<'t>, Owner)> {
        let declares = |node: Node<'t>| node.list("typeParameters").find(|p| p.str("name") == name);
        if let Some(m) = method
            && let Some(found) = declares(self.methods[m].node)
        {
            return Some((found, Owner::Method(m)));
        }
        if let Some(nested) = self.nesting(class) {
            if let Some(found) = declares(self.types[class].node) {
                return Some((found, Owner::Class(class)));
            }
            return self.type_var_around(name, nested.enclosing, nested.method);
        }
        let mut outer = Some(self.types[class].name.as_str());
        while let Some(name) = outer {
            if let Some(t) = self.type_index(name)
                && let Some(found) = declares(self.types[t].node)
            {
                return Some((found, Owner::Class(t)));
            }
            outer = name.rsplit_once('.').map(|(outer, _)| outer);
        }
        None
    }

    /// The erasure of the type variable `name` of method `method` of class `class` (or of a
    /// class around it), as javac writes it: that of its first bound, or `java.lang.Object`.
    pub(super) fn var_erasure_name(&self, name: &str, class: usize, method: Option<usize>) -> Option<String> {
        let (param, _) = self.type_var_around(name, class, method)?;
        let Some(bound) = param.list("bounds").next() else { return Some("java.lang.Object".to_string()) };
        let bound = bound.java_type();
        let raw = bound.split('<').next().unwrap_or(bound);
        if raw.contains('.') { Some(raw.to_string()) } else { self.var_erasure_name(raw, class, method) }
    }

    /// The type of a type variable: its own type in a Rust generic, else its erasure.
    fn var_type(&self, node: Node, (param, owner): (Node<'t>, Owner)) -> Result<Type, Error> {
        if self.is_real(owner) {
            return Ok(self.param_type(owner, param.str("name")));
        }
        self.var_erasure(node, param)
    }

    /// A generic class with the type arguments javac wrote, `args` (`java.lang.Integer,T`), when it
    /// is a Rust generic.
    fn real_class(&self, node: Node, raw: &str, args: &str) -> Option<Result<Type, Error>> {
        let t = self.type_index(raw)?;
        if !self.is_real(Owner::Class(t)) {
            return None;
        }
        let mut types = Vec::new();
        for arg in split_args(args) {
            // A wildcard or a primitive (which javac never writes) has no Rust type argument.
            if arg.starts_with('?') || arg.starts_with("capture#") || intersection_bounds(arg).is_some() {
                self.fail(Owner::Class(t));
                return Some(Ok(Type::class(raw)));
            }
            match self.ty(node, arg) {
                Ok(ty) => types.push(ty),
                Err(e) => return Some(Err(e)),
            }
        }
        Some(Ok(Type::generic(raw, &types)))
    }

    /// The erasure of a type variable: that of its first bound, or `Object`. A value of it is
    /// cast to a later bound where it is used as one (`receiver`).
    fn var_erasure(&self, node: Node, param: Node<'t>) -> Result<Type, Error> {
        match param.list("bounds").next() {
            Some(bound) => self.ty(bound, bound.java_type()),
            None => self.object_type(node),
        }
    }

    /// `Object`, which a program that declares a type variable without a bound always has.
    fn object_type(&self, node: Node) -> Result<Type, Error> {
        match self.object {
            Some(_) => Ok(Type::class(OBJECT)),
            None => unsupported(node, "Object"),
        }
    }

    /// The erasure of a type that javac writes with type arguments, a wildcard, a capture or a
    /// type variable; `None` for any other type.
    pub(super) fn erased(&self, node: Node, name: &str) -> Option<Result<Type, Error>> {
        // An intersection's values are of its erasure, as on the JVM; a call of a later bound's
        // method casts the receiver to that bound (`receiver`).
        if let Some(bounds) = intersection_bounds(name) {
            return Some(self.ty(node, bounds[0]));
        }
        let without_outer = without_outer_args(name);
        if !without_outer.contains('<') && without_outer != name {
            return Some(self.ty(node, &without_outer));
        }
        let name = without_outer.as_str();
        // `capture#1 of ? extends Shape`, the type of a value read through a wildcard.
        if let Some(rest) = name.strip_prefix("capture#") {
            let bound = rest.split_once(" of ").map_or("?", |(_, b)| b);
            return Some(self.wildcard(node, bound));
        }
        if name.starts_with('?') {
            return Some(self.wildcard(node, name));
        }
        // `Box<java.lang.Integer>` and `Box<T>[]`: the class, with any array dimensions after.
        if let Some(open) = name.find('<')
            && !name.starts_with('<')
        {
            let close = name.rfind('>').unwrap_or(name.len() - 1);
            let suffix = &name[close + 1..];
            if suffix.is_empty()
                && let Some(real) = self.real_class(node, &name[..open], &name[open + 1..close])
            {
                return Some(real);
            }
            let raw = format!("{}{}", &name[..open], suffix);
            return Some(self.ty(node, &raw));
        }
        let var = self.type_var(name)?;
        Some(self.var_type(node, var))
    }

    /// The receiver of a call of a method that class `owner` declares. A type variable with
    /// several bounds (`T extends Object & Comparable<? super T>`) erases to its first, and an
    /// intersection type (`(Base & Named) o`) to its erasure, so a value of it calling a method
    /// of a later bound is cast to that bound, as javac casts it.
    pub(super) fn receiver(&self, receiver: Node<'t>, owner: &str) -> Result<Expr, Error> {
        let value = self.expr(receiver)?;
        let ty = receiver.java_type();
        let var = ty
            .strip_prefix("capture#")
            .map_or(ty, |rest| rest.split_once(" of ? extends ").map_or("", |(_, bound)| bound));
        let several = intersection_bounds(var).is_some()
            || !var.contains(['.', '<', '[', ' ', '?'])
                && self.type_var(var).is_some_and(|(param, _)| param.list("bounds").nth(1).is_some());
        if !several {
            return Ok(value);
        }
        let bound = self.ty(receiver, owner)?;
        Ok(self.convert(value, bound))
    }

    /// A cast to an intersection type, `(Base & Named) o`, as javac erases it (`TransTypes`): a
    /// checked cast to each bound in turn but the erasure and the type that the place converts
    /// the value to (`expected`, from `JavaTree`), then to the erasure, then to that type when it
    /// is a bound, so that the first bound the value is not of is the one a `ClassCastException`
    /// names. `None` for a lambda or a method reference, whose cast is the one interface of the
    /// intersection with methods.
    pub(super) fn intersection_cast(&self, node: Node<'t>) -> Result<Option<Expr>, Error> {
        let Some(bounds) = node.get("type").filter(|t| t.kind() == "INTERSECTION_TYPE") else {
            return Ok(None);
        };
        let mut expression = node.child("expression")?;
        while expression.kind() == "PARENTHESIZED" {
            expression = expression.child("expression")?;
        }
        if matches!(expression.kind(), "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE") {
            if intersection_bounds(node.java_type()).is_some() {
                return unsupported(node, "a lambda of an intersection of interfaces");
            }
            return Ok(None);
        }
        // Where the place is not one `JavaTree` knows, the order of the casts is not known.
        let expected = node.str("expected");
        if expected.is_empty() {
            return unsupported(node, "a cast to an intersection type in this place");
        }
        let expected = self.ty(node, expected).ok();
        let erasure = self.ty(node, node.java_type())?;
        let mut value = self.expr(node.child("expression")?)?;
        // The bound that is the place's type, whose check is the place's conversion.
        let mut last = None;
        for bound in bounds.list("bounds") {
            let ty = self.ty(bound, bound.java_type())?;
            // At the bound's position: a cast at its value's is one that cannot fail
            // (`failures::raising`).
            if ty != erasure && Some(ty) == expected {
                last = Some((ty, bound.pos()));
            } else if ty != erasure && ty != value.ty {
                value = self.synth(ExprKind::Cast(ty, Box::new(value)), bound.pos(), ty);
            }
        }
        let erased = self.synth(ExprKind::Cast(erasure, Box::new(value)), node.pos(), erasure);
        // The place's conversion is at the value's position, where a cast cannot fail, so the
        // check of the bound that is the place's type is made here, last, at the bound's; the
        // value is then of that type.
        Ok(Some(match last {
            Some((ty, pos)) => self.synth(ExprKind::Cast(ty, Box::new(erased)), pos, ty),
            None => erased,
        }))
    }

    /// The erasure of the type variable `name`, if one is in scope.
    pub(super) fn type_variable(&self, node: Node, name: &str) -> Option<Result<Type, Error>> {
        if name.contains(['.', '<', '[', ' ', '?']) {
            return None;
        }
        let var = self.type_var(name)?;
        Some(self.var_type(node, var))
    }

    /// The erasure of a wildcard: its upper bound, or `Object`.
    fn wildcard(&self, node: Node, wildcard: &str) -> Result<Type, Error> {
        match wildcard.strip_prefix("? extends ") {
            Some(bound) => self.ty(node, bound),
            None => self.object_type(node),
        }
    }

    /// The type that javac's `name` declares in the scope of class `class` and its method
    /// `method`: a member's declared type, read at a use elsewhere.
    pub(super) fn declared_type(
        &self,
        node: Node,
        name: &str,
        class: usize,
        method: Option<usize>,
    ) -> Result<Type, Error> {
        let outer = self.scope.replace(Some((class, method)));
        let ty = self.ty(node, name);
        self.scope.set(outer);
        ty
    }

    /// The declared type of the field that `node` names, when that differs from the type javac
    /// gave the use: a field of a type variable, whose erasure it holds.
    pub(super) fn field_type(&self, node: Node<'t>) -> Result<Option<Type>, Error> {
        let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD") else { return Ok(None) };
        let Some(class) = self.type_index(symbol.owner) else { return Ok(None) };
        let declared = self.declared_type(node, symbol.ty, class, None)?;
        Ok((declared != self.ty(node, node.java_type())?).then_some(declared))
    }

    /// The type that a use of a member has where it is declared, when javac gave the use a
    /// substituted type: a call's return type, a field's type, an element's type.
    fn declared_use_type(&self, node: Node<'t>, kind: &ExprKind) -> Result<Option<Type>, Error> {
        Ok(match kind {
            ExprKind::MethodCall { resolved: Some(index), .. }
            | ExprKind::Call { resolved: Some(index), .. }
                if *index < self.methods.len() =>
            {
                let info = &self.methods[*index];
                let Some(ret) = info.node.get("returnType") else { return Ok(None) };
                Some(self.declared_type(ret, ret.java_type(), info.owner, Some(*index))?)
            }
            // (A call that reads a field, as one through a field offset does, has the field's type
            // already.)
            ExprKind::Field { .. } => {
                let Some(symbol) = node.symbol().filter(|s| s.kind == "FIELD") else { return Ok(None) };
                let Some(class) = self.type_index(symbol.owner) else { return Ok(None) };
                Some(self.declared_type(node, symbol.ty, class, None)?)
            }
            ExprKind::Index(array, _) => match array.ty {
                Type::Array(element) => Some(*element),
                _ => None,
            },
            _ => None,
        })
    }

    /// `kind`, of javac's type `ty`, with the checked conversion from its declared type that
    /// erasure needs (see the module's comment).
    pub(super) fn erased_use(&self, node: Node<'t>, kind: ExprKind, ty: Type) -> Result<ExprKind, Error> {
        let Some(declared) = self.declared_use_type(node, &kind)? else { return Ok(kind) };
        // Rust substitutes a Rust generic's type variables itself.
        if declared == ty || ty == Type::Void || self.has_param(declared) {
            return Ok(kind);
        }
        if matches!(declared, Type::Array(_)) {
            // The array of a type variable is an `Object[]`, and so is its argument's, which
            // covariant arrays are once lowered again.
            if self.meets_object_array(ty, declared) {
                return Ok(kind);
            }
            return unsupported(node, "an array of a type variable used as an array of its argument");
        }
        // An array held as an `Object` (`Deque<int[]>`) is cast back to its type.
        if Self::is_object(declared) {
            self.object_array(node, ty)?;
        } else if matches!(ty, Type::Array(_)) {
            return unsupported(node, "an array of a type variable used as an array of its argument");
        }
        let inner = self.synth(kind, node.pos(), declared);
        self.erased_casts.borrow_mut().insert(inner.id);
        Ok(ExprKind::Cast(ty, Box::new(inner)))
    }
}

/// javac's type `name` without the type arguments of the classes that an inner class's type
/// names it through: `Outer<E>.Inner<F>` is `Outer.Inner<F>`, whose arguments are the inner
/// class's own. (Taken whole, it read as `Outer` with the arguments `E>.Inner<F`.)
fn without_outer_args(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(open) = rest.find('<') {
        let mut depth = 0;
        let close = rest[open..]
            .char_indices()
            .find_map(|(i, c)| {
                match c {
                    '<' => depth += 1,
                    '>' => depth -= 1,
                    _ => {}
                }
                (depth == 0).then_some(open + i)
            })
            .unwrap_or(rest.len() - 1);
        let outer = rest[close + 1..].starts_with('.');
        out.push_str(if outer { &rest[..open] } else { &rest[..=close] });
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}
