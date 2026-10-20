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
    fn element_zero(&self, elem: Type) -> &'static str {
        if elem.is_reference() { "None" } else { zero_value(elem) }
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

    /// The type of an array parameter that the aliasing analysis borrows: a slice.
    pub(super) fn borrowed_ty(&mut self, ty: Type, written: bool) -> String {
        let Type::Array(elem) = ty else { unreachable!("only arrays are borrowed") };
        let elem = self.element_ty_name(*elem);
        if written { format!("&mut [{elem}]") } else { format!("&[{elem}]") }
    }

    /// The argument for a borrowed array parameter: a borrow of the caller's plain array, of the
    /// contents of a shared one for the length of the call, or of a new array literal (the packed
    /// arguments of a varargs call), which needs no allocation.
    pub(super) fn borrowed_arg(&mut self, e: &'a Expr, written: bool) -> String {
        let ExprKind::Var(name) = &e.kind else {
            let ExprKind::ArrayLit(items) = &e.kind else {
                unreachable!("borrowed arguments are names or literals")
            };
            let elem = element(e.ty);
            let items: Vec<String> = items.iter().map(|item| self.element_value(item, elem)).collect();
            let mutably = if written { "mut " } else { "" };
            return format!("&{mutably}[{}]", items.join(", "));
        };
        let local = self.local(name);
        if self.is_borrowed(&local) {
            return local;
        }
        match (self.plain(name).is_some(), written) {
            (true, true) => format!("&mut {local}"),
            (true, false) => format!("&{local}"),
            (false, true) => format!("&mut {local}.borrow_mut()"),
            (false, false) => format!("&{local}.borrow()"),
        }
    }

    /// Whether the Rust name `a` is a borrowed array parameter of the current method: already a
    /// slice reference, so it is passed on as it is.
    fn is_borrowed(&self, a: &str) -> bool {
        self.class.methods[self.method].params.iter().enumerate().any(|(j, p)| {
            self.facts.borrowed_arrays.contains_key(&(self.method, j)) && self.local(&p.name) == a
        })
    }

    /// The `&` argument for the plain array `a`.
    fn slice_ref(&self, a: &str) -> String {
        if self.is_borrowed(a) { a.to_string() } else { format!("&{a}") }
    }

    /// An index known to be in bounds, as a `usize`.
    fn position(&mut self, index: &'a Expr) -> String {
        match index.kind {
            ExprKind::Int(n) => n.to_string(),
            _ => format!("{} as usize", self.value(index, Type::Int).at(AS)),
        }
    }

    /// The `&mut` argument for the plain array `a`.
    pub(super) fn mut_ref(&self, a: &str) -> String {
        if self.is_borrowed(a) { a.to_string() } else { format!("&mut {a}") }
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

    /// A plain array local, declared with its initializer as a `Vec`.
    pub(super) fn plain_local(&mut self, ty: Type, name: &str, init: Option<&'a Expr>, written: bool) {
        let mutable = if written { "mut " } else { "" };
        let rust = self.local(name);
        let elem = element(ty);
        let elem_ty = self.element_ty_name(elem);
        let init = init.expect("analysis gives plain arrays an initializer");
        let text = match &init.kind {
            ExprKind::NewArray(sizes) => {
                let [size] = sizes.as_slice() else { unreachable!("analysis keeps one-dimensional arrays") };
                match size.kind {
                    // A size that is not negative needs no check.
                    ExprKind::Int(n) if n >= 0 => format!("vec![{}; {n}]", self.element_zero(elem)),
                    _ => {
                        let n = self.value(size, Type::Int).text;
                        format!("{}({n}, {})", self.runtime("new_vec"), self.element_zero(elem))
                    }
                }
            }
            ExprKind::ArrayLit(items) => {
                let items: Vec<String> = items.iter().map(|item| self.element_value(item, elem)).collect();
                format!("vec![{}]", items.join(", "))
            }
            _ => unreachable!("analysis keeps only fresh arrays in plain locals"),
        };
        self.line(&format!("let {mutable}{rust}: Vec<{elem_ty}> = {text};"));
    }

    /// `a[i]` as a value.
    pub(super) fn index(&mut self, id: ExprId, array: &'a Expr, index: &'a Expr) -> Code {
        if let Some(item) = self.elements.get(&id) {
            return Code::atom(item.clone());
        }
        if let Some((_, a)) = self.plain_of(array) {
            if self.facts.in_bounds.contains(&id) {
                let k = self.position(index);
                return Code::atom(read_in_bounds(&a, &k, element(array.ty)));
            }
            let i = self.value(index, Type::Int).text;
            return Code::atom(format!("{}({}, {i})", self.runtime("at"), self.slice_ref(&a)));
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
        let zero = self.element_zero(ty.element_at(sizes.len()));
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
        if let Some((name, a)) = self.plain_of(array) {
            return self.plain_set(id, (name, &a), index, ty, rhs, used);
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let v = self.element_value(rhs, ty);
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
        if !used && self.facts.in_bounds.contains(&id) && rust_order_is_same(index, Some(rhs)) {
            let k = self.position(index);
            let v = self.element_value(rhs, ty);
            return format!("{a}[{k}] = {v}");
        }
        let i = self.value(index, Type::Int).text;
        let v = self.element_value(rhs, ty);
        let put = self.runtime("put");
        let mr = self.mut_ref(a);
        if !mentions(index, name) && !mentions(rhs, name) {
            return format!("{put}({mr}, {i}, {v})");
        }
        let slot = self.fresh_name(&["index", "slot", "at"]);
        let value = self.fresh_name(&["value", "item", "new"]);
        format!("{{ let {slot} = {i}; let {value} = {v}; {put}({mr}, {slot}, {value}) }}")
    }

    /// `a[i] op= v`, as a call of `update`, which reads the old element before it evaluates `v`.
    /// As a statement (`used` false) on a plain array, it is `a[i] op= v` where the index is in
    /// bounds, and `java::modify` otherwise.
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
        if let Some((name, a)) = self.plain_of(array)
            && !used
            && !mentions(rhs, name)
        {
            if self.facts.in_bounds.contains(&id)
                && simple_index(index)
                && rust_order_is_same(index, Some(rhs))
            {
                let k = self.position(index);
                let slot = format!("{a}[{k}]");
                if let Some(text) = self.compound_in_place(op, ty, rhs, id, &slot) {
                    return text;
                }
                let new = self.element_update(op, ty, rhs, id, &slot);
                return format!("{slot} = {new}");
            }
            let old = self.fresh_name(&["old", "previous", "before"]);
            let new = self.element_update(op, ty, rhs, id, old);
            return self.modify((name, &a), index, old, &new);
        }
        if let Some((_, a)) = self.plain_of(array) {
            let i = self.value(index, Type::Int).text;
            let slot = self.fresh_name(&["index", "slot", "at"]);
            let old = self.fresh_name(&["old", "previous", "before"]);
            let value = self.fresh_name(&["value", "item", "new"]);
            let new = self.element_update(op, ty, rhs, id, old);
            let (at, put) = (self.runtime("at"), self.runtime("put"));
            let mr = self.mut_ref(&a);
            return format!(
                "{{ let {slot} = {i}; let {old} = {at}(&{a}, {slot}); let {value} = {new}; {put}({mr}, {slot}, {value}) }}"
            );
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name(&["old", "previous", "before"]);
        let new = self.element_update(op, ty, rhs, id, old);
        format!("{}({handle}, {i}, |{old}| {new})", self.runtime("update"))
    }

    /// `a[i]++`, `--a[i]` and the like. As a statement (`used` false) on a plain array, it is
    /// `a[i] += 1` or a `wrapping_add` where the index is in bounds, and `java::modify` otherwise.
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
        if let Some((name, a)) = self.plain_of(array)
            && !used
        {
            if self.facts.in_bounds.contains(&id) && simple_index(index) {
                let k = self.position(index);
                let slot = format!("{a}[{k}]");
                let sign = if increment { "+" } else { "-" };
                if ty.is_floating() {
                    return format!("{slot} {sign}= 1.0");
                }
                if self.no_overflow(id) {
                    return format!("{slot} {sign}= 1");
                }
                return format!("{slot} = {}", self.step_value(ty, increment, id, &slot));
            }
            let old = self.fresh_name(&["old", "previous", "before"]);
            let new = self.step_value(ty, increment, id, old);
            return self.modify((name, &a), index, old, &new);
        }
        if let Some((_, a)) = self.plain_of(array) {
            let i = self.value(index, Type::Int).text;
            let slot = self.fresh_name(&["index", "slot", "at"]);
            let old = self.fresh_name(&["old", "previous", "before"]);
            let new = self.step_value(ty, increment, id, old);
            let (at, put) = (self.runtime("at"), self.runtime("put"));
            let mr = self.mut_ref(&a);
            if prefix {
                return format!(
                    "{{ let {slot} = {i}; let {old} = {at}(&{a}, {slot}); {put}({mr}, {slot}, {new}) }}"
                );
            }
            return format!(
                "{{ let {slot} = {i}; let {old} = {at}(&{a}, {slot}); {put}({mr}, {slot}, {new}); {old} }}"
            );
        }
        let handle = self.handle(array);
        let i = self.value(index, Type::Int).text;
        let old = self.fresh_name(&["old", "previous", "before"]);
        let new = self.step_value(ty, increment, id, old);
        let helper = if prefix { "update" } else { "post_update" };
        format!("{}({handle}, {i}, |{old}| {new})", self.runtime(helper))
    }

    /// `java::modify(&mut a, i, |old| new)` on the plain array `a`. The index is bound first when
    /// it mentions `a`, as in `a[Math.floorMod(k, a.length)] += 1`, since the call borrows `a`
    /// mutably before its arguments are evaluated.
    fn modify(&mut self, (name, a): (&str, &str), index: &'a Expr, old: &str, new: &str) -> String {
        let i = self.value(index, Type::Int).text;
        let modify = self.runtime("modify");
        let mr = self.mut_ref(a);
        if !mentions(index, name) {
            return format!("{modify}({mr}, {i}, |{old}| {new})");
        }
        let slot = self.fresh_name(&["index", "slot", "at"]);
        format!("{{ let {slot} = {i}; {modify}({mr}, {slot}, |{old}| {new}) }}")
    }

    /// `slot op= rhs` where Rust's compound assignment does what Java's does.
    fn compound_in_place(
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
    fn step_value(&self, ty: Type, increment: bool, id: ExprId, old: &str) -> String {
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
    fn element_update(&mut self, op: BinOp, ty: Type, rhs: &'a Expr, id: ExprId, old: &str) -> String {
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
    fn fresh_name(&self, candidates: &[&'static str]) -> &'static str {
        let taken = |name: &str| {
            self.locals.values().any(|local| local == name) || self.fn_names.iter().any(|f| f == name)
        };
        candidates.iter().copied().find(|name| !taken(name)).unwrap_or(candidates[0])
    }

    /// `for (T x : a) body`. The array is read one element at a time, as Java reads it.
    /// A counting loop over a plain array whose body only reads its elements becomes
    /// `for x in a.iter().cloned()`, and each `a[i]` in the body becomes `x`. Returns false when
    /// the loop is not one of those, or its elements are not plain values.
    pub(super) fn element_loop(&mut self, cond: &'a Expr, body: &'a Stmt) -> bool {
        let Some(reads) = self.facts.element_loops.get(&cond.id).cloned() else { return false };
        let ExprKind::Binary(_, _, end) = &cond.kind else { return false };
        let ExprKind::Length(array) = &end.kind else { return false };
        if element(array.ty).is_reference() {
            return false;
        }
        let Some((_, a)) = self.plain_of(array) else { return false };
        let item = self.fresh_name(&["item", "element", "value"]);
        self.elements.extend(reads.into_iter().map(|id| (id, item.to_string())));
        self.line(&format!("for {item} in {a}.iter().cloned() {{"));
        self.loop_body(Loop::Plain, body, &[]);
        self.line("}");
        true
    }

    pub(super) fn for_each(&mut self, name: &str, array: &'a Expr, body: &'a Stmt) {
        let mutable = if assigns_any(std::slice::from_ref(body), name) { "mut " } else { "" };
        let var = self.local(name);
        if let Some((java, a)) = self.plain_of(array) {
            if writes_elements(body, java) {
                // The body writes elements, so it cannot hold a borrow of the array: index it.
                let slot = self.fresh_name(&["index", "slot", "at"]);
                let element = read_in_bounds(&a, slot, element(array.ty));
                self.line(&format!("for {slot} in 0..{a}.len() {{"));
                self.indent += 1;
                self.line(&format!("let {mutable}{var} = {element};"));
                self.indent -= 1;
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

/// Whether `a[index] = rhs` and `a[index] op= rhs` can be Rust assignments, which evaluate `rhs`
/// before the index where Java evaluates the index first: the index has no effects, and `rhs`
/// assigns no variable that the index reads.
fn rust_order_is_same(index: &Expr, rhs: Option<&Expr>) -> bool {
    if has_effects(index) {
        return false;
    }
    let Some(rhs) = rhs else { return true };
    let mut same = true;
    visit_expr(index, &mut |x| {
        if let ExprKind::Var(v) = &x.kind {
            same &= !assigns(rhs, v);
        }
    });
    same
}

/// Whether an index is cheap and pure enough to be written twice, as in `a[i] = a[i] + 1`.
fn simple_index(index: &Expr) -> bool {
    matches!(index.kind, ExprKind::Int(_) | ExprKind::Var(_))
}

/// `a[i]` of a plain array for an index that is in bounds, where `index` is already a `usize`. A
/// string, array or object is cloned, since it cannot be moved out of the `Vec`.
fn read_in_bounds(a: &str, index: &str, elem: Type) -> String {
    let clone = if needs_clone(elem) { ".clone()" } else { "" };
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
