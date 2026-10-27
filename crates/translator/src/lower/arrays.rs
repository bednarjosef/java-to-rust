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
        self.sized_array(node, sizes)
    }

    /// A new array of `node`'s type with these lengths, one per dimension from the outermost.
    fn sized_array(&self, node: Node<'t>, sizes: Vec<Expr>) -> Result<ExprKind, Error> {
        let java = node.java_type();
        let ty = self.ty(node, java)?;
        if sizes.len() < 2 {
            return Ok(ExprKind::NewArray(sizes));
        }
        // The type of the arrays of each level, from the outermost, and their Java class. Where
        // the arrays of a level are `Object[]`s (see `covariant`), as in `new int[n][m]` where
        // `int[][]` is one, or `new C[a][b][c]` where `C[][]` is, the rows they hold are made as
        // their own arrays and become `Object`s (`Class::object_rows`).
        let mut javas = Vec::with_capacity(sizes.len());
        let mut level = java;
        for _ in 0..sizes.len() {
            javas.push(level);
            // javac's type has a level for each length.
            level = level.strip_suffix("[]").unwrap_or(level);
        }
        let tys = javas.iter().map(|&level| self.ty(node, level)).collect::<Result<Vec<_>, _>>()?;
        let object = Type::class(OBJECT);
        // An array type with a level for each length, which the analyses read as a new array's.
        let made = (1..tys.len()).fold(tys[tys.len() - 1], |made, _| Type::array(made));
        if made == ty || tys.iter().all(|&level| level.element() != object) {
            return Ok(ExprKind::NewArray(sizes));
        }
        let mut levels = Vec::with_capacity(tys.len());
        for (&level, &java) in tys.iter().zip(&javas) {
            levels.push((level, self.exact_ty(node, java)?));
        }
        for pair in levels.windows(2) {
            if pair[0].0.element() == object {
                self.note_object_array(pair[1].0);
            }
        }
        let rows = self.synth(ExprKind::NewArray(sizes), node.pos(), made);
        self.object_rows.borrow_mut().insert(rows.id, levels);
        Ok(ExprKind::Cast(ty, Box::new(rows)))
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
        // Over an object or an enum constant, the loop calls its iterator (see `foreach`).
        if matches!(array.ty, Type::Class(..) | Type::Enum(_)) {
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
        if Self::is_object(*element) && *element != ty && Self::of_objects(ty) {
            let pos = variable.pos();
            let each = self.fresh_name(&name);
            let value = self.synth(ExprKind::Var(each.clone()), pos, *element);
            let init = self.synth(ExprKind::Cast(ty, Box::new(value)), pos, ty);
            let declared = Stmt::Local { ty, name, init: Some(init), pos };
            body = Stmt::Block(vec![declared, body]);
            (ty, name) = (*element, each);
        } else if *element != ty {
            // An element of one class that the variable holds as another, an enum's constant as
            // an interface it implements (`for (Reader r : Policy.values())`), is converted to the
            // variable's type.
            let pos = variable.pos();
            let each = self.fresh_name(&name);
            let value = self.synth(ExprKind::Var(each.clone()), pos, *element);
            let init = self.convert(value, ty);
            if matches!(init.kind, ExprKind::Cast(..)) {
                let declared = Stmt::Local { ty, name, init: Some(init), pos };
                body = Stmt::Block(vec![declared, body]);
                (ty, name) = (*element, each);
            }
        }
        let looped = Stmt::ForEach { ty, name, array, body: Box::new(body) };
        Ok(match label {
            Some(label) => Stmt::Labeled { label: label.to_string(), body: Box::new(looped) },
            None => looped,
        })
    }

    /// `Arrays.setAll(array, generator)` as a statement: a loop that stores the generator's value
    /// at each index, as the JDK's does, with the array and the generator computed once, first.
    /// `None` for any other statement.
    pub(super) fn set_all(&self, node: Node<'t>) -> Result<Option<Stmt>, Error> {
        let call = node.child("expression")?;
        let Some(symbol) = call.symbol().filter(|_| call.kind() == "METHOD_INVOCATION") else {
            return Ok(None);
        };
        if symbol.owner != "java.util.Arrays" || symbol.name != "setAll" {
            return Ok(None);
        }
        let nodes: Vec<Node<'t>> = call.list("arguments").collect();
        let [array_node, generator_node] = nodes[..] else { return Ok(None) };
        let array = self.expr(array_node)?;
        let Type::Array(elem) = array.ty else { return Ok(None) };
        let elem = *elem;
        let (interface, name, sig) = match elem {
            Type::Int => ("java.util.function.IntUnaryOperator", "applyAsInt", "(int)int"),
            Type::Long => ("java.util.function.IntToLongFunction", "applyAsLong", "(int)long"),
            Type::Double => ("java.util.function.IntToDoubleFunction", "applyAsDouble", "(int)double"),
            _ => ("java.util.function.IntFunction", "apply", "(int)R"),
        };
        let resolved = self.template_method(interface, name, sig);
        if resolved.is_none() {
            return unsupported(call, "`Arrays.setAll` with this generator");
        }
        let generator = self.expr(generator_node)?;
        let pos = call.pos();
        let var = |name: &str, ty: Type| self.synth(ExprKind::Var(name.to_string()), pos, ty);
        let (a, g, i) = (
            self.fresh_name("setAllArray"),
            self.fresh_name("setAllGenerator"),
            self.fresh_name("setAllIndex"),
        );
        let (array_ty, generator_ty) = (array.ty, generator.ty);
        let length = self.synth(ExprKind::Length(Box::new(var(&a, array_ty))), pos, Type::Int);
        let cond = self.synth(
            ExprKind::Binary(BinOp::Lt, Box::new(var(&i, Type::Int)), Box::new(length)),
            pos,
            Type::Boolean,
        );
        let update = self.synth(
            ExprKind::IncDec { target: Place::Var(i.clone()), increment: true, prefix: false },
            pos,
            Type::Int,
        );
        let apply = ExprKind::MethodCall {
            receiver: Box::new(var(&g, generator_ty)),
            name: name.to_string(),
            args: vec![var(&i, Type::Int)],
            resolved,
            is_super: false,
        };
        // An erased generator's value is an `Object`, cast to the element type.
        let value = self.synth(self.erased_use(call, apply, elem)?, pos, elem);
        let target = Place::Index(Box::new(var(&a, array_ty)), Box::new(var(&i, Type::Int)));
        let store = self.synth(ExprKind::Assign { op: None, target, value: Box::new(value) }, pos, elem);
        let zero = self.synth(ExprKind::Int(0), pos, Type::Int);
        Ok(Some(Stmt::Block(vec![
            Stmt::Local { ty: array_ty, name: a.clone(), init: Some(array), pos },
            Stmt::Local { ty: generator_ty, name: g.clone(), init: Some(generator), pos },
            Stmt::For {
                init: vec![Stmt::Local { ty: Type::Int, name: i.clone(), init: Some(zero), pos }],
                cond: Some(cond),
                update: vec![update],
                body: Box::new(Stmt::Expr(store)),
            },
        ])))
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
    /// variable, or a variable that holds a class (cast or not), dropped unlowered; where the
    /// copy is a covariant array, the class it carries is noted from it
    /// (`covariant::note_element_class`). The copy must be of the array's own type, or an
    /// `Object[]` of a class literal.
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
        let ty = self.ty(node, node.java_type())?;
        // Into an `Object[]` (a literal's), which holds any reference: the copy of an array of
        // other references is the source's, once the two are `Object[]`s (`covariant`), and no
        // element throws `ArrayStoreException`. Into any other class an element might.
        let into_objects = literal
            && matches!(ty, Type::Array(e) if Self::is_object(*e))
            && matches!(args[0].ty, Type::Array(e) if Self::of_objects(*e));
        if args[0].ty != ty && !(into_objects && self.meets_object_array(args[0].ty, ty)) {
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

    /// `(C[]..[]) Array.newInstance(C.class, n1, .., nk)` (`node`, the cast;
    /// `program::literal_array`): `new C[n1]..[nk]`, which checks the lengths and fails on a
    /// negative one as `multiNewArray` does.
    pub(super) fn literal_array(&self, node: Node<'t>) -> Result<ExprKind, Error> {
        let Some(lengths) = crate::program::literal_array(node.0) else {
            unreachable!("only a cast of a new array of a class literal comes here")
        };
        let lengths = lengths.into_iter().map(|l| self.expr(Node(l))).collect::<Result<Vec<_>, _>>()?;
        self.sized_array(node, lengths)
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
/// strings, `copyOf` and `copyOfRange` of any array, and `setAll`, which becomes a loop. With the
/// JDK's sources, any other call is one of the
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
        // A statement of its own (see `Lowering::set_all`).
        ("setAll", 2) => true,
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
