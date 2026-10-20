//! Arrays in generated code. A `java::Array` is a shared handle, so each access goes through a
//! runtime helper that checks the index the way Java does; creation checks every size before it
//! allocates anything, as Java does.

use super::*;

impl<'a> Gen<'a> {
    /// The Rust type of a Java type. An array is a `java::Array` of its elements.
    pub(super) fn ty_name(&mut self, ty: Type) -> String {
        match ty {
            Type::Array(elem) => {
                let elem = self.ty_name(*elem);
                format!("{}<{elem}>", self.runtime("Array"))
            }
            _ => rust_type(ty).to_string(),
        }
    }

    /// `e` as a borrowed array handle, for the runtime helpers that take `&Array`.
    fn handle(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// `a[i]` as a value.
    pub(super) fn index(&mut self, array: &'a Expr, index: &'a Expr) -> Code {
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        Code::atom(format!("{}({handle}, {i})", self.runtime("get")))
    }

    /// `a.length`.
    pub(super) fn length(&mut self, array: &'a Expr) -> Code {
        let handle = self.handle(array);
        Code::atom(format!("{}({handle})", self.runtime("len")))
    }

    /// `new T[n]`, or `new T[n][m]` and so on. Every size is evaluated, and checked, before any
    /// array is allocated, as Java does.
    pub(super) fn new_array(&mut self, ty: Type, sizes: &'a [Expr]) -> Code {
        let zero = zero_value(leaf(ty));
        if let [size] = sizes {
            let n = self.value(size, Type::Int).text;
            return Code::atom(format!("{}({n}, {zero})", self.runtime("filled")));
        }
        let dims: Vec<String> = sizes.iter().map(|s| self.value(s, Type::Int).text).collect();
        let last = sizes.len() - 1;
        let mut text = format!("{}(dims[{last}], {zero})", self.runtime("filled"));
        for level in (0..last).rev() {
            text = format!("{}(dims[{level}], |_| {text})", self.runtime("tabulate"));
        }
        let check = self.runtime("check_sizes");
        Code::new(format!("{{ let dims = [{}]; {check}(&dims); {text} }}", dims.join(", ")), LOWEST)
    }

    /// `{a, b}` and `new T[] {a, b}`.
    pub(super) fn array_lit(&mut self, ty: Type, items: &'a [Expr]) -> Code {
        let elem = element(ty);
        let items: Vec<String> = items.iter().map(|item| self.value(item, elem).text).collect();
        Code::atom(format!("{}(vec![{}])", self.runtime("from_vec"), items.join(", ")))
    }

    /// `a[i] = v`, as a call of a runtime helper. Java evaluates the array, the index and then
    /// `v`, and checks the index last; the arguments come in that order. `used` says whether the
    /// assignment's value is needed, which picks `store` over `set`.
    pub(super) fn index_set(
        &mut self,
        array: &'a Expr,
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        used: bool,
    ) -> String {
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let v = self.value(rhs, ty).text;
        let helper = if used { "store" } else { "set" };
        format!("{}({handle}, {i}, {v})", self.runtime(helper))
    }

    /// `a[i] op= v`, as a call of `update`, which reads the old element before it evaluates `v`.
    pub(super) fn index_update(
        &mut self,
        op: BinOp,
        array: &'a Expr,
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        id: ExprId,
    ) -> String {
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name();
        let new = self.element_update(op, ty, rhs, id, old);
        format!("{}({handle}, {i}, |{old}| {new})", self.runtime("update"))
    }

    /// `a[i]++`, `--a[i]` and the like.
    pub(super) fn index_step(
        &mut self,
        array: &'a Expr,
        index: &'a Expr,
        ty: Type,
        increment: bool,
        prefix: bool,
        id: ExprId,
    ) -> String {
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name();
        let sign = if increment { "+" } else { "-" };
        let new = if ty == Type::Double {
            format!("{old} {sign} 1.0")
        } else if self.no_overflow(id) {
            format!("{old} {sign} 1")
        } else {
            format!("{old}.wrapping_{}(1)", if increment { "add" } else { "sub" })
        };
        let helper = if prefix { "update" } else { "post_update" };
        format!("{}({handle}, {i}, |{old}| {new})", self.runtime(helper))
    }

    /// The new element of `a[i] op= rhs`, written in terms of the old element `old`.
    fn element_update(&mut self, op: BinOp, ty: Type, rhs: &'a Expr, id: ExprId, old: &str) -> String {
        if ty == Type::String {
            let mut parts = vec![Part::Arg(Code::atom(old))];
            self.parts(rhs, &mut parts);
            return self.format("format!", parts).text;
        }
        // As in `assignment`: the operation is computed in the promoted type, then cast back.
        let result = crate::types::binary_type(op, ty, rhs.ty).expect("checked");
        let right = self.operand(rhs);
        let code = self.binary(op, Operand { code: Code::atom(old), ty }, right, result, id);
        self.convert(code, result, ty).text
    }

    /// A name for the closure parameter that holds an element, which no local or method of the
    /// program already uses, since the right-hand side of the update may use them.
    fn fresh_name(&self) -> &'static str {
        let taken = |name: &str| {
            self.locals.values().any(|local| local == name) || self.fn_names.iter().any(|f| f == name)
        };
        ["old", "previous", "before"].into_iter().find(|name| !taken(name)).unwrap_or("before")
    }

    /// `for (T x : a) body`. The array is read one element at a time, as Java reads it.
    pub(super) fn for_each(&mut self, name: &str, array: &'a Expr, body: &'a Stmt) {
        let source = self.expr(array).text;
        let each = self.runtime("each");
        let mutable = if assigns_any(std::slice::from_ref(body), name) { "mut " } else { "" };
        let var = self.local(name);
        self.line(&format!("for {mutable}{var} in {each}({source}) {{"));
        self.loop_body(Loop::Plain, body, &[]);
        self.line("}");
    }

    /// `Arrays.toString`, `fill`, `sort` and `copyOf`.
    pub(super) fn arrays_builtin(&mut self, builtin: Builtin, args: &'a [Expr]) -> Code {
        let elem = element(args[0].ty);
        let handle = self.handle(&args[0]);
        match builtin {
            Builtin::ArraysToString => {
                let show = match elem {
                    Type::String => "|v| v".to_string(),
                    Type::Double => format!("|v| {}(v).to_string()", self.runtime("Double")),
                    _ => "|v| v.to_string()".to_string(),
                };
                Code::atom(format!("{}({handle}, {show})", self.runtime("to_string")))
            }
            Builtin::ArraysFill => {
                let v = self.value(&args[1], elem).text;
                Code::atom(format!("{}({handle}, {v})", self.runtime("fill")))
            }
            Builtin::ArraysSort => {
                let helper = if elem == Type::Double { "sort_doubles" } else { "sort" };
                Code::atom(format!("{}({handle})", self.runtime(helper)))
            }
            Builtin::ArraysCopyOf => {
                let len = self.value(&args[1], Type::Int).text;
                let zero = zero_value(elem);
                Code::atom(format!("{}({handle}, {len}, {zero})", self.runtime("copy_of")))
            }
            _ => unreachable!("only the Arrays builtins are handled here"),
        }
    }
}

/// The Rust name of a Java type in a method name: `int_array` for `int[]`.
pub(super) fn name_part(ty: Type) -> String {
    match ty {
        Type::Array(elem) => format!("{}_array", name_part(*elem)),
        _ => rust_type(ty).to_lowercase(),
    }
}

/// The element type of an array type.
fn element(ty: Type) -> Type {
    match ty {
        Type::Array(elem) => *elem,
        _ => unreachable!("lowering gives arrays array types"),
    }
}

/// The type of the innermost elements: `int` for `int[][]`.
fn leaf(ty: Type) -> Type {
    match ty {
        Type::Array(elem) => leaf(*elem),
        _ => ty,
    }
}

/// The default value of a primitive type, which is what a new array holds.
fn zero_value(ty: Type) -> &'static str {
    match ty {
        Type::Int => "0",
        Type::Long => "0i64",
        Type::Double => "0.0",
        Type::Boolean => "false",
        _ => unreachable!("lowering rejects arrays of nullable elements"),
    }
}
