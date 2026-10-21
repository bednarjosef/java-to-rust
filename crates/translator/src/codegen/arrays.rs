//! Arrays in generated code. A `java::Array` is a shared handle, so each access goes through a
//! runtime helper that checks the index the way Java does; creation checks every size before it
//! allocates anything, as Java does. An array that no other name can see is a plain `Vec`,
//! written by `plain_arrays`.

use super::*;

impl<'a> Gen<'a> {
    /// The Rust type of a Java type. An array is a `java::Array` of its elements.
    pub(super) fn ty_name(&mut self, ty: Type) -> String {
        match ty {
            Type::Array(elem) => {
                let elem = self.element_ty_name(*elem);
                format!("{}<{elem}>", self.runtime("Array"))
            }
            Type::Class(name) => self.object_type(name),
            Type::Enum(name) => struct_name(name),
            Type::Boxed(_) => self.boxed_name(ty),
            Type::StringBuilder => self.runtime("StringBuilder"),
            _ => rust_type(ty).to_string(),
        }
    }

    /// The Rust type of an array element of type `elem`: an `Option` where arrays of it may hold
    /// null.
    pub(super) fn element_ty_name(&mut self, elem: Type) -> String {
        let name = self.ty_name(elem);
        if self.nullable_element(elem) { format!("Option<{name}>") } else { name }
    }

    /// Whether elements of type `elem` may be null, in any array.
    pub(super) fn nullable_element(&self, elem: Type) -> bool {
        self.facts.nullable_elements.contains(&elem)
    }

    /// `e` as an element of type `elem`: in `Some` where the elements may be null.
    pub(super) fn element_value(&mut self, e: &'a Expr, elem: Type) -> String {
        if self.nullable_element(elem) { self.optional_to(e, elem).text } else { self.value(e, elem).text }
    }

    /// The value of a new array's elements: zero, `false`, or `None` for references.
    pub(super) fn element_zero(&self, elem: Type) -> &'static str {
        if elem.is_reference() { "None" } else { zero_value(elem) }
    }

    /// `e` as a borrowed array handle, for the runtime helpers that take `&Array`.
    pub(super) fn handle(&mut self, e: &'a Expr) -> String {
        if self.nullable(e) {
            // Using a null array throws `NullPointerException`.
            let place = self.object_place(e);
            return format!("{}({place}.as_ref())", self.runtime("non_null"));
        }
        match &e.kind {
            ExprKind::Var(name) => format!("&{}", self.local(name)),
            // A field is borrowed where it is, without a copy of the handle.
            ExprKind::Field { .. } => format!("&{}", self.object_place(e)),
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// `a[i]` as a value.
    pub(super) fn index(&mut self, id: ExprId, array: &'a Expr, index: &'a Expr) -> Code {
        if let Some(item) = self.elements.get(&id) {
            return Code::atom(item.clone());
        }
        if let Some(path) = self.plain_path(array) {
            return self.plain_index(path, id, index, element(array.ty));
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        self.may_fail(id, "get", "", &format!("{handle}, {i}"))
    }

    /// `a.length`.
    pub(super) fn length(&mut self, array: &'a Expr) -> Code {
        if let Some(path) = self.plain_path(array) {
            return self.plain_length(path);
        }
        let handle = self.handle(array);
        Code::atom(format!("{}({handle})", self.runtime("len")))
    }
    /// `new T[n]`, or `new T[n][m]` and so on. Every size is evaluated, and checked, before any
    /// array is allocated, as Java does.
    pub(super) fn new_array(&mut self, ty: Type, sizes: &'a [Expr]) -> Code {
        let zero = self.element_zero(ty.element_at(sizes.len()));
        if let [size] = sizes {
            let n = self.value(size, Type::Int).text;
            return Code::atom(format!("{}({n}, {zero})", self.runtime("filled")));
        }
        let dims: Vec<String> = sizes.iter().map(|s| self.value(s, Type::Int).text).collect();
        let last = sizes.len() - 1;
        let mut text = format!("{}(dims[{last}], {zero})", self.runtime("filled"));
        for level in (0..last).rev() {
            // The rows built here are elements of level `level`; they are `Some` where that
            // element type is nullable, as the array they fill is.
            let row =
                if self.nullable_element(ty.element_at(level + 1)) { format!("Some({text})") } else { text };
            text = format!("{}(dims[{level}], |_| {row})", self.runtime("tabulate"));
        }
        let check = self.runtime("check_sizes");
        Code::new(format!("{{ let dims = [{}]; {check}(&dims); {text} }}", dims.join(", ")), LOWEST)
    }

    /// `{a, b}` and `new T[] {a, b}`.
    pub(super) fn array_lit(&mut self, ty: Type, items: &'a [Expr]) -> Code {
        let elem = element(ty);
        let items: Vec<String> = items.iter().map(|item| self.element_value(item, elem)).collect();
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
        if let Some(path) = self.plain_path(array) {
            return self.plain_set(path, id, index, ty, rhs, used);
        }
        let (binding, handle) = self.write_handle(array, has_effects(index) || has_effects(rhs));
        let i = self.value(index, Type::Int).text;
        let v = self.element_value(rhs, ty);
        let helper = if used { "store" } else { "set" };
        let call = self.may_fail(id, helper, "", &format!("{handle}, {i}, {v}")).text;
        Self::bound_write(binding, call)
    }

    /// `a[i] op= v`, as a call of `update`, which reads the old element before it evaluates `v`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn index_update(
        &mut self,
        op: BinOp,
        array: &'a Expr,
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        id: ExprId,
        used: bool,
    ) -> String {
        if let Some(path) = self.plain_path(array) {
            return self.plain_update(path, op, index, ty, rhs, id, used);
        }
        if self.raises_in(rhs) {
            return self.update_in_steps(op, array, index, ty, rhs, id, used);
        }
        let (binding, handle) = self.write_handle(array, has_effects(index) || has_effects(rhs));
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name(&["old", "previous", "before"]);
        let new = self.element_update(op, ty, rhs, id, old);
        let new = self.closure_value(id, new);
        let call = self.may_fail(id, "update", "", &format!("{handle}, {i}, |{old}| {new}")).text;
        Self::bound_write(binding, call)
    }

    /// `a[i]++`, `--a[i]` and the like.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn index_step(
        &mut self,
        array: &'a Expr,
        index: &'a Expr,
        ty: Type,
        increment: bool,
        prefix: bool,
        id: ExprId,
        used: bool,
    ) -> String {
        if let Some(path) = self.plain_path(array) {
            return self.plain_step(path, index, ty, increment, prefix, id, used);
        }
        let (binding, handle) = self.write_handle(array, has_effects(index));
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name(&["old", "previous", "before"]);
        let new = self.step_value(ty, increment, id, old);
        let new = self.closure_value(id, new);
        let helper = if prefix { "update" } else { "post_update" };
        let call = self.may_fail(id, helper, "", &format!("{handle}, {i}, |{old}| {new}")).text;
        Self::bound_write(binding, call)
    }

    /// `a[i] op= v` on a shared array whose value `v` may fail where a handler sees it: the
    /// element is read, the value computed and the element written one after the other, in
    /// Java's order, so that the failure of `v` can go on (it could not from inside the closure
    /// of `update`).
    #[allow(clippy::too_many_arguments)]
    fn update_in_steps(
        &mut self,
        op: BinOp,
        array: &'a Expr,
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        id: ExprId,
        used: bool,
    ) -> String {
        let (binding, handle) = self.write_handle(array, true);
        let i = self.value(index, Type::Int).text;
        let slot = self.fresh_name(&["index", "slot", "at"]);
        let old = self.fresh_name(&["old", "previous", "before"]);
        let value = self.fresh_name(&["value", "item", "new"]);
        let new = self.element_update(op, ty, rhs, id, old);
        let get = self.may_fail(id, "get", "", &format!("{handle}, {slot}")).text;
        let mut lets = vec![
            binding.unwrap_or_default(),
            format!("let {slot} = {i};"),
            format!("let {old} = {get};"),
            format!("let {value} = {new};"),
        ];
        let stored = if used { format!("{value}.clone()") } else { value.to_string() };
        let set = self.may_fail(id, "set", "", &format!("{handle}, {slot}, {stored}")).text;
        lets.push(format!("{set};"));
        if used {
            lets.push(value.to_string());
        }
        format!("{{ {} }}", lets.join(" "))
    }

    /// Whether some operation in `e` may fail where a handler sees it, so that `e` cannot be
    /// computed inside a closure.
    pub(super) fn raises_in(&self, e: &Expr) -> bool {
        let mut found = false;
        visit_expr(e, &mut |x| found |= self.facts.raising.contains(&x.id));
        found
    }

    /// The new element `new` as the body of the closure of an element update: `Ok(new)` where
    /// the update is the checked form (`may_fail`), whose closure returns a `Result`.
    pub(super) fn closure_value(&self, id: ExprId, new: String) -> String {
        if self.facts.raising.contains(&id) { format!("Ok({new})") } else { new }
    }

    /// `slot op= rhs` where Rust's compound assignment does what Java's does.
    pub(super) fn compound_in_place(
        &mut self,
        op: BinOp,
        ty: Type,
        rhs: &'a Expr,
        id: ExprId,
        slot: &str,
    ) -> Option<String> {
        if ty == Type::String {
            return None;
        }
        let symbol = self.in_place_op(op, ty, rhs, id)?;
        let v = self.value(rhs, if op.is_shift() { rhs.ty } else { ty });
        Some(format!("{slot} {symbol} {}", v.text))
    }

    /// The element after `++` or `--`, written in terms of the old element `old`.
    pub(super) fn step_value(&self, ty: Type, increment: bool, id: ExprId, old: &str) -> String {
        let sign = if increment { "+" } else { "-" };
        if ty.is_floating() {
            format!("{old} {sign} 1.0")
        } else if self.no_overflow(id) {
            format!("{old} {sign} 1")
        } else {
            format!("{old}.wrapping_{}(1)", if increment { "add" } else { "sub" })
        }
    }

    /// The new element of `a[i] op= rhs`, written in terms of the old element `old`.
    pub(super) fn element_update(
        &mut self,
        op: BinOp,
        ty: Type,
        rhs: &'a Expr,
        id: ExprId,
        old: &str,
    ) -> String {
        if ty == Type::String && self.nullable_element(ty) {
            // A null element shows as `null`.
            let shown = format!("{}({old}.as_deref())", self.runtime("or_null"));
            let mut parts = vec![Part::Arg(Code::atom(shown))];
            self.parts(rhs, &mut parts);
            return format!("Some({})", self.format("format!", parts).text);
        }
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
    pub(super) fn fresh_name(&self, candidates: &[&'static str]) -> &'static str {
        let taken = |name: &str| {
            self.locals.values().any(|local| local == name) || self.fn_names.iter().any(|f| f == name)
        };
        candidates.iter().copied().find(|name| !taken(name)).unwrap_or(candidates[0])
    }

    /// `for (T x : a) body`. The array is read one element at a time, as Java reads it.
    pub(super) fn for_each(&mut self, name: &str, array: &'a Expr, body: &'a Stmt) {
        if let ExprKind::Var(java) = &array.kind
            && self.plain(java).is_some()
        {
            return self.plain_for_each(name, java, array, body);
        }
        let mutable = if assigns_any(std::slice::from_ref(body), name) { "mut " } else { "" };
        let var = self.local(name);
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
                let show = self.show_closure(elem);
                Code::atom(format!("{}({handle}, {show})", self.runtime("to_string")))
            }
            Builtin::ArraysFill => {
                let v = self.element_value(&args[1], elem);
                Code::atom(format!("{}({handle}, {v})", self.runtime("fill")))
            }
            Builtin::ArraysSort => {
                let helper = match elem {
                    Type::Double => "sort_doubles",
                    Type::Float => "sort_floats",
                    _ => "sort",
                };
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
        Type::Class(name) | Type::Enum(name) => struct_name(name).to_lowercase(),
        _ => rust_type(ty).to_lowercase(),
    }
}

/// The element type of an array type.
pub(super) fn element(ty: Type) -> Type {
    match ty {
        Type::Array(elem) => *elem,
        _ => unreachable!("lowering gives arrays array types"),
    }
}

/// The default value of a primitive type, which is what a new array holds.
pub(super) fn zero_value(ty: Type) -> &'static str {
    match ty {
        Type::Int => "0",
        Type::Long => "0i64",
        Type::Byte => "0i8",
        Type::Short => "0i16",
        Type::Char => "0u16",
        Type::Float => "0.0f32",
        Type::Double => "0.0",
        Type::Boolean => "false",
        _ => unreachable!("lowering rejects arrays of nullable elements"),
    }
}
