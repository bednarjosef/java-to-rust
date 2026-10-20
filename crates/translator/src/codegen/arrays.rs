//! Arrays in generated code. A `java::Array` is a shared handle, so each access goes through a
//! runtime helper that checks the index the way Java does; creation checks every size before it
//! allocates anything, as Java does. An array that no other name can see is a plain `Vec` (see
//! `analysis::arrays`): it is indexed directly, or through `java::at` and `java::put` where the
//! index is not known to be in bounds.

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

    /// Whether the local `name` is a plain array: `Some` with whether its elements are written.
    pub(super) fn plain(&self, name: &str) -> Option<bool> {
        self.facts.plain_arrays.get(&(self.method, name.to_string())).copied()
    }

    /// `e` as a plain array local: its Java name and its Rust name.
    fn plain_of(&self, e: &'a Expr) -> Option<(&'a str, String)> {
        let ExprKind::Var(name) = &e.kind else { return None };
        self.plain(name)?;
        Some((name, self.local(name)))
    }

    /// `e` as a borrowed array handle, for the runtime helpers that take `&Array`.
    fn handle(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// A plain array local, declared with its initializer as a `Vec`.
    pub(super) fn plain_local(&mut self, ty: Type, name: &str, init: Option<&'a Expr>, written: bool) {
        let mutable = if written { "mut " } else { "" };
        let rust = self.local(name);
        let elem = element(ty);
        let elem_ty = self.ty_name(elem);
        let init = init.expect("analysis gives plain arrays an initializer");
        let text = match &init.kind {
            ExprKind::NewArray(sizes) => {
                let [size] = sizes.as_slice() else { unreachable!("analysis keeps one-dimensional arrays") };
                let n = self.value(size, Type::Int).text;
                format!("{}({n}, {})", self.runtime("new_vec"), zero_value(elem))
            }
            ExprKind::ArrayLit(items) => {
                let items: Vec<String> = items.iter().map(|item| self.value(item, elem).text).collect();
                format!("vec![{}]", items.join(", "))
            }
            _ => unreachable!("analysis keeps only fresh arrays in plain locals"),
        };
        self.line(&format!("let {mutable}{rust}: Vec<{elem_ty}> = {text};"));
    }

    /// `a[i]` as a value.
    pub(super) fn index(&mut self, id: ExprId, array: &'a Expr, index: &'a Expr) -> Code {
        if let Some((_, a)) = self.plain_of(array) {
            let i = self.value(index, Type::Int).text;
            if self.facts.in_bounds.contains(&id) {
                return Code::atom(read_in_bounds(&a, &format!("{i} as usize"), element(array.ty)));
            }
            return Code::atom(format!("{}(&{a}, {i})", self.runtime("at")));
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        Code::atom(format!("{}({handle}, {i})", self.runtime("get")))
    }

    /// `a.length`.
    pub(super) fn length(&mut self, array: &'a Expr) -> Code {
        if let Some((_, a)) = self.plain_of(array) {
            return Code::new(format!("{a}.len() as i32"), AS);
        }
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
        id: ExprId,
        array: &'a Expr,
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        used: bool,
    ) -> String {
        if let Some((name, a)) = self.plain_of(array) {
            return self.plain_set(id, (name, &a), index, ty, rhs, used);
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let v = self.value(rhs, ty).text;
        let helper = if used { "store" } else { "set" };
        format!("{}({handle}, {i}, {v})", self.runtime(helper))
    }

    /// `a[i] = v` on a plain array: `put`, or a plain assignment where the index is in bounds.
    /// The index and the value are bound first when they mention `a`, since the call would borrow
    /// `a` mutably while they read it.
    fn plain_set(
        &mut self,
        id: ExprId,
        (name, a): (&'a str, &str),
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        used: bool,
    ) -> String {
        let i = self.value(index, Type::Int).text;
        let v = self.value(rhs, ty).text;
        if !used && self.facts.in_bounds.contains(&id) {
            return format!("{a}[{i} as usize] = {v}");
        }
        let put = self.runtime("put");
        if !mentions(index, name) && !mentions(rhs, name) {
            return format!("{put}(&mut {a}, {i}, {v})");
        }
        let slot = self.fresh_name(&["index", "slot", "at"]);
        let value = self.fresh_name(&["value", "item", "new"]);
        format!("{{ let {slot} = {i}; let {value} = {v}; {put}(&mut {a}, {slot}, {value}) }}")
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
        if let Some((_, a)) = self.plain_of(array) {
            let i = self.value(index, Type::Int).text;
            let slot = self.fresh_name(&["index", "slot", "at"]);
            let old = self.fresh_name(&["old", "previous", "before"]);
            let value = self.fresh_name(&["value", "item", "new"]);
            let new = self.element_update(op, ty, rhs, id, old);
            let (at, put) = (self.runtime("at"), self.runtime("put"));
            return format!(
                "{{ let {slot} = {i}; let {old} = {at}(&{a}, {slot}); let {value} = {new}; {put}(&mut {a}, {slot}, {value}) }}"
            );
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name(&["old", "previous", "before"]);
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
        if let Some((_, a)) = self.plain_of(array) {
            let i = self.value(index, Type::Int).text;
            let slot = self.fresh_name(&["index", "slot", "at"]);
            let old = self.fresh_name(&["old", "previous", "before"]);
            let new = self.step_value(ty, increment, id, old);
            let (at, put) = (self.runtime("at"), self.runtime("put"));
            if prefix {
                return format!(
                    "{{ let {slot} = {i}; let {old} = {at}(&{a}, {slot}); {put}(&mut {a}, {slot}, {new}) }}"
                );
            }
            return format!(
                "{{ let {slot} = {i}; let {old} = {at}(&{a}, {slot}); {put}(&mut {a}, {slot}, {new}); {old} }}"
            );
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name(&["old", "previous", "before"]);
        let new = self.step_value(ty, increment, id, old);
        let helper = if prefix { "update" } else { "post_update" };
        format!("{}({handle}, {i}, |{old}| {new})", self.runtime(helper))
    }

    /// The element after `++` or `--`, written in terms of the old element `old`.
    fn step_value(&self, ty: Type, increment: bool, id: ExprId, old: &str) -> String {
        let sign = if increment { "+" } else { "-" };
        if ty == Type::Double {
            format!("{old} {sign} 1.0")
        } else if self.no_overflow(id) {
            format!("{old} {sign} 1")
        } else {
            format!("{old}.wrapping_{}(1)", if increment { "add" } else { "sub" })
        }
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

    /// A name for a temporary, the first of `candidates` that no local or method of the program
    /// already uses, since the code the temporary is bound around may use those.
    fn fresh_name(&self, candidates: &[&'static str]) -> &'static str {
        let taken = |name: &str| {
            self.locals.values().any(|local| local == name) || self.fn_names.iter().any(|f| f == name)
        };
        candidates.iter().copied().find(|name| !taken(name)).unwrap_or(candidates[0])
    }

    /// `for (T x : a) body`. The array is read one element at a time, as Java reads it.
    pub(super) fn for_each(&mut self, name: &str, array: &'a Expr, body: &'a Stmt) {
        let mutable = if assigns_any(std::slice::from_ref(body), name) { "mut " } else { "" };
        let var = self.local(name);
        if let Some((java, a)) = self.plain_of(array) {
            if writes_elements(body, java) {
                // The body writes elements, so it cannot hold a borrow of the array: index it.
                let slot = self.fresh_name(&["index", "slot", "at"]);
                let element = read_in_bounds(&a, slot, element(array.ty));
                self.line(&format!("for {slot} in 0..{a}.len() {{"));
                self.line(&format!("let {mutable}{var} = {element};"));
            } else {
                self.line(&format!("for {mutable}{var} in {a}.iter().cloned() {{"));
            }
            self.loop_body(Loop::Plain, body, &[]);
            self.line("}");
            return;
        }
        let source = self.expr(array).text;
        let each = self.runtime("each");
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

/// `a[i]` of a plain array for an index that is in bounds, where `index` is already a `usize`. A
/// `String` is cloned, since it cannot be moved out of the `Vec`.
fn read_in_bounds(a: &str, index: &str, elem: Type) -> String {
    let clone = if elem == Type::String { ".clone()" } else { "" };
    format!("{a}[{index}]{clone}")
}

/// Whether `body` assigns an element of the array `name`.
fn writes_elements(body: &Stmt, name: &str) -> bool {
    let mut found = false;
    visit_stmt(
        body,
        &mut |e| {
            visit_expr(e, &mut |x| {
                if let ExprKind::Assign { target: Place::Index(array, _), .. }
                | ExprKind::IncDec { target: Place::Index(array, _), .. } = &x.kind
                {
                    found |= matches!(&array.kind, ExprKind::Var(v) if v == name);
                }
            })
        },
        &mut |_| {},
    );
    found
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
