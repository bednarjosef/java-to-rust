//! Arrays: creation, element access, `length`, for-each loops, `main`'s `args`, and the methods
//! of `java.util.Arrays` that the translator knows.

use super::*;

impl<'t> Lowering<'t> {
    /// `new T[n][m]`, or an initializer such as `{1, 2}` and `new int[] {1, 2}`.
    pub(super) fn new_array(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        if node.get("initializers").is_some() {
            // Each element converts to the element type, as an assignment to it would.
            let element = match self.ty(node, node.java_type())? {
                Type::Array(element) => *element,
                _ => Type::Void,
            };
            let items = node
                .list("initializers")
                .map(|item| Ok(self.convert(self.expr(item)?, element)))
                .collect::<Result<_, Error>>()?;
            return Ok(ExprKind::ArrayLit(items));
        }
        let sizes = node.list("dimensions").map(|d| self.expr(d)).collect::<Result<Vec<_>, _>>()?;
        // An array of arrays that is an `Object[]` (see `covariant`), `new int[n][m]` where
        // `int[][]` is: the rows are made as `int[]`s and become `Object`s.
        let java = node.java_type();
        if sizes.len() > 1
            && let Some(row) = java.strip_suffix("[]")
            && let exact = Type::array(self.ty(node, row)?)
            && exact != self.ty(node, java)?
        {
            self.note_object_array(exact.element());
            let rows = self.synth(ExprKind::NewArray(sizes), node.pos(), exact);
            return Ok(ExprKind::Cast(self.ty(node, java)?, Box::new(rows)));
        }
        Ok(ExprKind::NewArray(sizes))
    }

    /// `a[i]` as an expression or, on the left of an assignment, as a place.
    pub(super) fn index(&self, node: Node<'t>) -> Result<(Expr, Expr), Error> {
        let array = self.expr(node.child("expression")?)?;
        let index = self.expr(node.child("index")?)?;
        Ok((array, index))
    }

    /// `for (T x : a) body`.
    pub(super) fn for_each(&self, node: Node<'t>, label: Option<&str>) -> Result<Stmt, Error> {
        let variable = node.child("variable")?;
        let ty = self.ty(variable, variable.symbol().map_or("", |s| s.ty))?;
        let array = self.expr(node.child("expression")?)?;
        // Over an object, the loop calls its iterator (see `foreach`).
        if matches!(array.ty, Type::Class(..)) {
            return self.for_each_iterable(node, array, label);
        }
        if !matches!(array.ty, Type::Array(_)) {
            return unsupported(node, "for-each loops over anything but arrays");
        }
        let mut body = self.single_stmt(node.child("statement")?)?;
        let mut name = variable.str("name").to_string();
        let Type::Array(element) = array.ty else { unreachable!() };
        // The elements of an `Object[]` that a covariant array is are cast to the variable's
        // type.
        let mut ty = ty;
        if Self::is_object(*element) && *element != ty && ty.is_reference() {
            let pos = variable.pos();
            let each = self.fresh_name(&name);
            let value = self.synth(ExprKind::Var(each.clone()), pos, *element);
            let init = self.synth(ExprKind::Cast(ty, Box::new(value)), pos, ty);
            let declared = Stmt::Local { ty, name, init: Some(init), pos };
            body = Stmt::Block(vec![declared, body]);
            (ty, name) = (*element, each);
        }
        let looped = Stmt::ForEach { ty, name, array, body: Box::new(body) };
        Ok(match label {
            Some(label) => Stmt::Labeled { label: label.to_string(), body: Box::new(looped) },
            None => looped,
        })
    }

    /// `Arrays.toString`, `fill`, `sort` and `copyOf`, for the element types the runtime covers.
    pub(super) fn arrays_call(&self, node: Node<'t>, name: &str, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let elem = match args.first().map(|a| a.ty) {
            Some(Type::Array(elem)) => *elem,
            _ => return unsupported(node, "this call"),
        };
        let primitive = elem.is_numeric() || elem == Type::Boolean;
        let printable = primitive || elem == Type::String;
        let builtin = match (name, args.len()) {
            ("toString", 1) if printable => Builtin::ArraysToString,
            ("fill", 2) if printable => Builtin::ArraysFill,
            ("sort", 1) if elem.is_numeric() => Builtin::ArraysSort,
            // Of references too: the elements are copied as they are, and the new ones are null.
            ("copyOf", 2) if crate::types::assignable(args[1].ty, Type::Int) => Builtin::ArraysCopyOf,
            _ => return self.java_arrays_call(node, name, args),
        };
        Ok(ExprKind::Builtin(builtin, args))
    }

    /// `Arrays.copyOf(a, n, type)` and `copyOfRange(a, from, to, type)` as the runtime's
    /// `copyOf(a, n)` and `copyOfRange(a, from, to)`: an array's elements do not depend on the
    /// class it is made of, which is a class literal (`Object[].class`), the `getClass()` of a
    /// variable, or a variable that holds a class (cast or not), dropped unlowered. The copy must
    /// be of the array's own type.
    pub(super) fn copy_of_with_class(
        &self,
        node: Node<'t>,
        symbol: &Symbol<'t>,
    ) -> Result<Option<ExprKind>, Error> {
        if symbol.owner != "java.util.Arrays" || !matches!(symbol.name, "copyOf" | "copyOfRange") {
            return Ok(None);
        }
        let mut nodes: Vec<Node<'t>> = node.list("arguments").collect();
        if nodes.len() != if symbol.name == "copyOf" { 3 } else { 4 } {
            return Ok(None);
        }
        let mut class = nodes.pop().expect("a class argument");
        while matches!(class.kind(), "TYPE_CAST" | "PARENTHESIZED") {
            class = class.child("expression")?;
        }
        let literal = class.kind() == "MEMBER_SELECT" && class.str("identifier") == "class";
        let of_variable = class.kind() == "METHOD_INVOCATION"
            && class.symbol().is_some_and(|s| s.name == "getClass")
            && class.get("arguments").is_none_or(|a| a.0.as_array().is_none_or(Vec::is_empty))
            && class
                .get("methodSelect")
                .and_then(|s| s.get("expression"))
                .is_some_and(|e| e.kind() == "IDENTIFIER");
        // A variable that holds a class (`Class<T[]> klazz`, as `ArrayDeque.toArray` passes on).
        let held = class.kind() == "IDENTIFIER" && class.java_type().starts_with("java.lang.Class");
        if !literal && !of_variable && !held {
            return unsupported(node, &format!("`Arrays.{}` into a class that is computed", symbol.name));
        }
        let args: Vec<Expr> = nodes.into_iter().map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        if args[0].ty != self.ty(node, node.java_type())? {
            return unsupported(node, &format!("`Arrays.{}` into an array of another type", symbol.name));
        }
        self.arrays_call(node, symbol.name, args).map(Some)
    }

    /// `(T[]) Array.newInstance(a.getClass().getComponentType(), n)` (`node`, the cast), the new
    /// array of `a`'s own class that the JDK's `toArray(T[])` and sorts make: `n` nulls of `a`'s
    /// type, which is the cast's, an `Object[]` when it is covariant (see `covariant`), whose
    /// arrays keep no class of their own. `a` is a variable or a field, not read: a null `a`
    /// does not throw `NullPointerException` here.
    pub(super) fn component_array(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let ty = self.ty(node, node.java_type())?;
        let mut call = node.child("expression")?;
        while call.kind() == "PARENTHESIZED" {
            call = call.child("expression")?;
        }
        let Some((array, length)) = crate::program::component_array(call.0) else {
            unreachable!("only a cast of a component array comes here")
        };
        let (array, length) = (Node(array), Node(length));
        if !matches!(array.kind(), "IDENTIFIER" | "MEMBER_SELECT") {
            return unsupported(node, "`Array.newInstance` of the class of a computed array");
        }
        let source = self.expr(array)?;
        if source.ty != ty && !self.meets_object_array(source.ty, ty) {
            return unsupported(node, "`Array.newInstance` into an array of another type");
        }
        Ok(ExprKind::NewArray(vec![self.expr(length)?]))
    }

    /// `String[] args` of `main`: the program's command-line arguments, declared in the body in
    /// place of the parameter.
    pub(super) fn args_decl(&self, param: Node<'t>) -> Stmt {
        let ty = self.covariant_array(Type::array(Type::String));
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let init = Expr { kind: ExprKind::Builtin(Builtin::MainArgs, Vec::new()), pos: param.pos(), ty, id };
        Stmt::Local { ty, name: param.str("name").to_string(), init: Some(init), pos: param.pos() }
    }
}

/// Whether the runtime does the call `Arrays.name(..)` whose arguments have javac's types
/// `args`: the calls that `arrays_call` and `java_arrays_call` cover, on arrays of primitives and
/// strings, and `copyOf` and `copyOfRange` of any array. With the JDK's sources, any other call is one of the
/// translated `java.util.Arrays`, and reachability enters only those (see `program::reach`).
pub fn runtime_arrays_call(name: &str, args: &[&str]) -> bool {
    let Some(elem) = args.first().and_then(|a| a.strip_suffix("[]")) else { return false };
    let primitive =
        |t: &str| matches!(t, "int" | "long" | "short" | "byte" | "char" | "float" | "double" | "boolean");
    let printable = |t: &str| primitive(t) || t == "java.lang.String";
    match (name, args.len()) {
        ("toString" | "hashCode" | "sort", 1)
        | ("fill" | "equals" | "binarySearch", 2)
        | ("sort", 3)
        | ("fill" | "binarySearch", 4) => printable(elem),
        ("copyOf", 2 | 3) | ("copyOfRange", 3 | 4) => true,
        ("deepToString", 1) => elem.strip_suffix("[]").is_some_and(printable),
        _ => false,
    }
}

/// Whether `node` is a cast of `Array.newInstance(a.getClass().getComponentType(), n)` (see
/// `Lowering::component_array`).
pub(super) fn casts_component_array(node: Node) -> bool {
    let mut call = &node.0["expression"];
    while call["kind"] == "PARENTHESIZED" {
        call = &call["expression"];
    }
    node.kind() == "TYPE_CAST" && crate::program::component_array(call).is_some()
}

/// `a.length`, recognised by the symbol javac gives an array's `length` field.
pub(super) fn is_array_length(node: Node) -> bool {
    node.symbol().is_some_and(|s| s.kind == "FIELD" && s.name == "length" && s.owner == "Array")
}

/// Whether any statement of `body` mentions the variable `name`. An unused `args` is not declared.
pub(super) fn reads_name(body: &[Stmt], name: &str) -> bool {
    let mut found = false;
    for stmt in body {
        visit_stmt(stmt, &mut |e| found |= mentions(e, name), &mut |_| {});
    }
    found
}
