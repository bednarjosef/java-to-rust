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
        let body = self.single_stmt(node.child("statement")?)?;
        let looped =
            Stmt::ForEach { ty, name: variable.str("name").to_string(), array, body: Box::new(body) };
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
            ("copyOf", 2) if primitive && args[1].ty == Type::Int => Builtin::ArraysCopyOf,
            _ => return self.java_arrays_call(node, name, args),
        };
        Ok(ExprKind::Builtin(builtin, args))
    }

    /// `String[] args` of `main`, which the program never gets from the command line: an empty
    /// array, declared in the body in place of the parameter.
    pub(super) fn args_decl(&self, param: Node<'t>) -> Stmt {
        let ty = Type::array(Type::String);
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let init = Expr { kind: ExprKind::ArrayLit(Vec::new()), pos: param.pos(), ty, id };
        Stmt::Local { ty, name: param.str("name").to_string(), init: Some(init), pos: param.pos() }
    }
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
