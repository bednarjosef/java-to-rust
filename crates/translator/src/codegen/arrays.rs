//! Arrays in generated code. A `java::Array` is a shared handle, so each access goes through a
//! runtime helper that checks the index the way Java does; creation checks every size before it
//! allocates anything, as Java does. An array that no other name can see is a plain `Vec`,
//! written by `plain_arrays`. In a program that starts threads the handle is a
//! `java::SharedArray`, which the same helpers take.

use super::array_writes::lends_array;
use super::*;

impl<'a> Gen<'a> {
    /// The Rust type of a Java type. An array is a `java::Array` of its elements.
    pub(super) fn ty_name(&mut self, ty: Type) -> String {
        match ty {
            Type::Array(elem) => {
                let elem = self.element_ty_name(*elem);
                let array = if self.facts.threaded { "SharedArray" } else { "Array" };
                format!("{}<{elem}>", self.runtime(array))
            }
            Type::Class(name, []) => self.object_type(name),
            Type::Class(name, args) => {
                let args: Vec<String> = args.iter().map(|&a| self.ty_name(a)).collect();
                self.object_type_with(name, &format!("<{}>", args.join(", ")))
            }
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

    /// Whether the loop variable `name` over elements of type `elem` is an `Option` while the
    /// elements are not: the name is nullable elsewhere in the method, so the elements are wrapped
    /// in `Some` as the loop binds them.
    pub(super) fn wrap_each(&self, name: &str, elem: Type) -> bool {
        self.optional_var(self.method, elem, name) && !self.nullable_element(elem)
    }

    /// Whether `a = value` computes the value from `a` itself where `a` is a shared array: a
    /// borrow of its cell lent to a call (`&a.borrow()`) lives to the end of the statement, so the
    /// value is bound before the assignment.
    pub(super) fn lends_itself(&self, target: &str, ty: Type, value: &Expr) -> bool {
        matches!(ty, Type::Array(_)) && self.plain(target).is_none() && {
            let mut found = false;
            visit_expr(value, &mut |x| match &x.kind {
                ExprKind::Call { args, .. }
                | ExprKind::MethodCall { args, .. }
                | ExprKind::Builtin(_, args) => found |= args.iter().any(|a| mentions(a, target)),
                _ => {}
            });
            found
        }
    }

    /// Whether `a[i] = e` used as a value stores `Some(e)` and gives `e` back: the elements of
    /// type `elem` may be null, while `e` cannot be.
    pub(super) fn stores_some(&self, e: &Expr, elem: Type) -> bool {
        self.nullable_element(elem) && !self.nullable(e)
    }

    /// `e` as an element of type `elem`: in `Some` where the elements may be null.
    pub(super) fn element_value(&mut self, e: &'a Expr, elem: Type) -> String {
        if self.nullable_element(elem) { self.optional_to(e, elem).text } else { self.value(e, elem).text }
    }

    /// The runtime's function `name` that makes an array (`filled`, `from_vec`), or its `shared_`
    /// twin in a program with threads.
    pub(super) fn array_maker(&mut self, name: &str) -> String {
        if self.facts.threaded { self.runtime(&format!("shared_{name}")) } else { self.runtime(name) }
    }

    /// `text`, a `java::Array` that a function of the runtime made, as an array of the program.
    pub(super) fn made_array(&mut self, text: String) -> String {
        if self.facts.threaded { format!("{}({text})", self.runtime("to_shared")) } else { text }
    }

    /// The value of a new array's elements: zero, `false`, or `None` for references.
    pub(super) fn element_zero(&self, elem: Type) -> &'static str {
        if elem.is_reference() || self.nullable_element(elem) { "None" } else { zero_value(elem) }
    }

    /// `e` as a borrowed array handle, for the runtime helpers that take `&Array`.
    pub(super) fn handle(&mut self, e: &'a Expr) -> String {
        if self.nullable(e) {
            // Using a null array throws `NullPointerException`.
            let place = self.object_place(e);
            return self.dereference(&[e.id], &format!("{place}.as_ref()"));
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
        self.new_rows(ty, sizes, None)
    }

    /// `new T[n][m]..` of an array type that is translated as an `Object[]` (`to`, see
    /// `lower::covariant`), whose Java type is `exact`: the rows are made as `new_array` makes
    /// them, and each becomes an `Object`. Nothing else holds the new rows yet, so no identity is
    /// lost.
    pub(super) fn new_object_rows(&mut self, to: Type, exact: Type, sizes: &'a [Expr]) -> Code {
        let object = self.enum_type(to.element()).expect("`Object` is an enum");
        let row = self.class.type_of(exact.element()).expect("rows that are `Object`s (see `lower::arrays`)");
        let variant = self.variant_of(object, row).expect("a variant of `Object` for the rows");
        let wrapped = if self.nullable_element(to.element()) {
            format!("Some({variant}(#))")
        } else {
            format!("{variant}(#)")
        };
        self.new_rows(exact, sizes, Some(wrapped))
    }

    /// `new_array`, with each row of the outermost level put in place of the `#` of `wrap` when
    /// given.
    fn new_rows(&mut self, ty: Type, sizes: &'a [Expr], wrap: Option<String>) -> Code {
        let zero = self.element_zero(ty.element_at(sizes.len()));
        if let [size] = sizes {
            let n = self.value(size, Type::Int).text;
            return Code::atom(format!("{}({n}, {zero})", self.array_maker("filled")));
        }
        let dims: Vec<String> = sizes.iter().map(|s| self.value(s, Type::Int).text).collect();
        let last = sizes.len() - 1;
        let mut text = format!("{}(dims[{last}], {zero})", self.array_maker("filled"));
        for level in (0..last).rev() {
            // The rows built here are elements of level `level`; they are `Some` where that
            // element type is nullable, as the array they fill is.
            let row = match &wrap {
                Some(wrap) if level == 0 => wrap.replacen('#', &text, 1),
                _ if self.nullable_element(ty.element_at(level + 1)) => format!("Some({text})"),
                _ => text,
            };
            text = format!("{}(dims[{level}], |_| {row})", self.array_maker("tabulate"));
        }
        let check = self.runtime("check_sizes");
        Code::new(format!("{{ let dims = [{}]; {check}(&dims); {text} }}", dims.join(", ")), LOWEST)
    }

    /// `{a, b}` and `new T[] {a, b}`.
    pub(super) fn array_lit(&mut self, ty: Type, items: &'a [Expr]) -> Code {
        let elem = element(ty);
        let items: Vec<String> = items.iter().map(|item| self.element_value(item, elem)).collect();
        Code::atom(format!("{}(vec![{}])", self.array_maker("from_vec"), items.join(", ")))
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
        let (mut binding, handle) = self.write_handle(array, has_effects(index) || has_effects(rhs));
        let mut i = self.value(index, Type::Int).text;
        // The value of the assignment is an `Option` only where the stored value may be null.
        let (helper, mut v) = if used && self.stores_some(rhs, ty) {
            ("store_some", self.value(rhs, ty).text)
        } else {
            (if used { "store" } else { "set" }, self.element_value(rhs, ty))
        };
        if lends_array(index) || lends_array(rhs) {
            i = self.bind_first(&mut binding, i, &["index", "slot", "at"]);
            v = self.bind_first(&mut binding, v, &["value", "item", "new"]);
        }
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
        let (mut binding, handle) = self.write_handle(array, has_effects(index) || has_effects(rhs));
        let mut i = self.value(index, Type::Int).text;
        if lends_array(index) {
            i = self.bind_first(&mut binding, i, &["index", "slot", "at"]);
        }
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
        let (mut binding, handle) = self.write_handle(array, has_effects(index));
        let mut i = self.value(index, Type::Int).text;
        if lends_array(index) {
            i = self.bind_first(&mut binding, i, &["index", "slot", "at"]);
        }
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
        visit_expr(e, &mut |x| {
            found |= self.facts.raising.contains(&x.id)
                || self.facts.null_checked.contains_key(&x.id)
                || match &x.kind {
                    // A call of a method that throws, whose exception goes on with `?`.
                    ExprKind::Call { resolved: Some(m), .. }
                    | ExprKind::MethodCall { resolved: Some(m), .. }
                    | ExprKind::New { ctor: Some(m), .. } => self.facts.throwing.contains(m),
                    _ => false,
                }
        });
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
        // A null array throws `NullPointerException` as the loop starts.
        let source = if self.nullable(array) {
            format!("{}.clone()", self.handle(array))
        } else {
            self.expr(array).text
        };
        let each = self.runtime("each");
        let wrap = if self.wrap_each(name, element(array.ty)) { ".map(Some)" } else { "" };
        self.line(&format!("for {mutable}{var} in {each}({source}){wrap} {{"));
        self.loop_body(Loop::Plain, body, &[]);
        self.line("}");
    }

    /// `Arrays.toString`, `fill`, `sort` and `copyOf`.
    pub(super) fn arrays_builtin(&mut self, builtin: Builtin, args: &'a [Expr], id: ExprId) -> Code {
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
                let zero = self.element_zero(elem);
                self.may_fail(id, "copy_of", "", &format!("{handle}, {len}, {zero}"))
            }
            _ => unreachable!("only the Arrays builtins are handled here"),
        }
    }
}

/// The Rust name of a Java type in a method name: `int_array` for `int[]`.
pub(super) fn name_part(ty: Type) -> String {
    match ty {
        Type::Array(elem) => format!("{}_array", name_part(*elem)),
        Type::Class(name, _) | Type::Enum(name) => struct_name(name).to_lowercase(),
        Type::Boxed(_) => ty.to_string().to_lowercase(),
        // A type of the runtime, such as `java::Class`, by its own name.
        _ => rust_type(ty).rsplit("::").next().unwrap_or_default().to_lowercase(),
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
