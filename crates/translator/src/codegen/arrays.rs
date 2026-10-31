//! Arrays in generated code. A `java::Array` is a shared handle, so each access goes through a
//! runtime helper that checks the index the way Java does; creation checks every size before it
//! allocates anything, as Java does. An array that no other name can see is a plain `Vec`,
//! written by `plain_arrays`. An array that two threads may reach is a `java::SharedArray`
//! (`Facts::shared_arrays`), which the same helpers take.

use super::array_writes::lends_array;
use super::*;

impl<'a> Gen<'a> {
    /// The Rust type of a Java type. An array is a `java::Array` of its elements.
    pub(super) fn ty_name(&mut self, ty: Type) -> String {
        match ty {
            Type::Array(elem) => {
                let shared = self.shared_array_type(ty);
                let elem = self.element_ty_name(*elem);
                let array = if shared { "SharedArray" } else { "Array" };
                format!("{}<{elem}>", self.runtime(array))
            }
            Type::Class(name, []) => self.object_type(name),
            Type::Class(name, args) => {
                let args: Vec<String> = args.iter().map(|&a| self.ty_name(a)).collect();
                self.object_type_with(name, &format!("<{}>", args.join(", ")))
            }
            Type::Enum(name) => struct_name(name),
            Type::Boxed(_) => self.boxed_name(ty),
            Type::StringBuilder => self.builder_type(),
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

    /// Whether the arrays of type `ty` are `java::SharedArray`s: two threads may reach arrays of
    /// its family (`analysis::array_family`), whose rows are of it too.
    pub(super) fn shared_array_type(&self, ty: Type) -> bool {
        self.facts.shared_arrays.contains(&crate::analysis::array_family(ty))
    }

    /// The runtime's function `name` that makes an array of type `ty` (`filled`, `from_vec`), or
    /// its `shared_` twin for a `java::SharedArray`.
    pub(super) fn array_maker(&mut self, name: &str, ty: Type) -> String {
        if self.shared_array_type(ty) { self.runtime(&format!("shared_{name}")) } else { self.runtime(name) }
    }

    /// `text`, a `java::Array` that a function of the runtime made, as an array of type `ty`.
    pub(super) fn made_array(&mut self, text: String, ty: Type) -> String {
        if self.shared_array_type(ty) { format!("{}({text})", self.runtime("to_shared")) } else { text }
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
            // A field is borrowed where it is, without a copy of the handle, and so is a static
            // that is a `java::Final`.
            ExprKind::Field { .. } => format!("&{}", self.object_place(e)),
            ExprKind::Static { .. } if let Some(place) = self.final_place(e) => format!("&{place}"),
            _ => format!("&{}", self.expr(e).at(UNARY)),
        }
    }

    /// `a[i]` as a value; a volatile read where Java makes it one (`Class::volatile_elements`).
    pub(super) fn index(&mut self, id: ExprId, array: &'a Expr, index: &'a Expr) -> Code {
        if let Some(item) = self.elements.get(&id) {
            return Code::atom(item.clone());
        }
        if let Some(path) = self.plain_path(array) {
            return self.plain_index(path, id, index, element(array.ty));
        }
        // A field's array is borrowed where it is, which a method that the index calls may not
        // borrow mutably (`table[nextIndex()]` in IdentityHashMap's iterators): its handle is
        // taken first, as Java evaluates the array before the index.
        let calls = |e: &Expr| {
            let mut found = false;
            visit_expr(e, &mut |x| {
                found |= matches!(x.kind, ExprKind::MethodCall { .. } | ExprKind::Call { .. })
            });
            found
        };
        let bound = matches!(array.kind, ExprKind::Field { .. }) && calls(index);
        let (binding, handle) = self.write_handle(array, bound);
        let volatile = self.class.volatile_elements.contains(&index.id);
        let i = self.value(index, Type::Int).text;
        let name = if volatile { "get_volatile" } else { "get" };
        let get = self.may_fail(id, name, "", &format!("{handle}, {i}"));
        match binding {
            Some(binding) => Code::atom(format!("{{ {binding} {} }}", get.text)),
            None => get,
        }
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
            return Code::atom(format!("{}({n}, {zero})", self.array_maker("filled", ty)));
        }
        let dims: Vec<String> = sizes.iter().map(|s| self.value(s, Type::Int).text).collect();
        let last = sizes.len() - 1;
        // The rows of an array of no rows are never made, so nothing else gives their elements a
        // type where the array goes to a generic function (`java::len`).
        let zero = match zero {
            "None" => format!("None::<{}>", self.ty_name(ty.element_at(sizes.len()))),
            _ => zero.to_string(),
        };
        let mut text = format!("{}(dims[{last}], {zero})", self.array_maker("filled", ty));
        for level in (0..last).rev() {
            // The rows built here are elements of level `level`; they are `Some` where that
            // element type is nullable, as the array they fill is.
            let row = match self.nullable_element(ty.element_at(level + 1)) {
                true => format!("Some({text})"),
                false => text,
            };
            text = format!("{}(dims[{level}], |_| {row})", self.array_maker("tabulate", ty));
        }
        let check = self.runtime("check_sizes");
        Code::new(format!("{{ let dims = [{}]; {check}(&dims); {text} }}", dims.join(", ")), LOWEST)
    }

    /// `new T[n][m]..` whose arrays at some level are `Object[]`s (`lower::covariant`), as a
    /// value of type `to`: the arrays of each level are made as their own type
    /// (`Class::object_rows`), the rows of an `Object[]` become `Object`s, and an `Object[]` of a
    /// narrower class carries it. All the lengths are checked before any array is made. Nothing
    /// else holds the new rows yet, so no identity is lost.
    pub(super) fn new_object_rows(&mut self, to: Type, made: &'a Expr, sizes: &'a [Expr]) -> Code {
        // A tree saved before `object_rows` has only the rows of the outermost level as `Object`s.
        let levels = match self.class.object_rows.get(&made.id) {
            Some(levels) => levels.clone(),
            None => std::iter::once((to, to))
                .chain((1..sizes.len()).map(|l| (made.ty.element_at(l), made.ty.element_at(l))))
                .collect(),
        };
        let dims: Vec<String> = sizes.iter().map(|s| self.value(s, Type::Int).text).collect();
        let last = sizes.len() - 1;
        let (ty, class) = levels[last];
        let elem = ty.element();
        // The rows of an array of no rows are never made, so nothing else gives their elements a
        // type where the array goes to a generic function (`java::len`).
        let zero = match self.element_zero(elem) {
            "None" => format!("None::<{}>", self.ty_name(elem)),
            zero => zero.to_string(),
        };
        let mut text = format!("{}(dims[{last}], {zero})", self.array_maker("filled", ty));
        text = self.carrying_class(ty, class, text);
        for level in (0..last).rev() {
            let (ty, class) = levels[level];
            let element = ty.element();
            let row = levels[level + 1].0;
            if element != row {
                let object = self.enum_type(element).expect("`Object` is an enum");
                let row = self.class.type_of(row).expect("rows that are `Object`s (see `lower::arrays`)");
                let variant = self.variant_of(object, row).expect("a variant of `Object` for the rows");
                text = format!("{variant}({text})");
            }
            // The rows are `Some` where the element type is nullable, as the array they fill is.
            if self.nullable_element(element) {
                text = format!("Some({text})");
            }
            text = format!("{}(dims[{level}], |_| {text})", self.array_maker("tabulate", ty));
            text = self.carrying_class(ty, class, text);
        }
        let check = self.runtime("check_sizes");
        Code::new(format!("{{ let dims = [{}]; {check}(&dims); {text} }}", dims.join(", ")), LOWEST)
    }

    /// `text`, a new array of type `ty` whose Java class is `class`, carrying that class where
    /// the translation holds it as an `Object[]` (`with_element_class`).
    fn carrying_class(&mut self, ty: Type, class: Type, text: String) -> String {
        match ty == class || !self.carries_class(ty) {
            true => text,
            false => self.with_element_class(class, Code::atom(text)).text,
        }
    }

    /// `{a, b}` and `new T[] {a, b}`.
    pub(super) fn array_lit(&mut self, ty: Type, items: &'a [Expr]) -> Code {
        let elem = element(ty);
        let texts: Vec<String> = items.iter().map(|item| self.element_value(item, elem)).collect();
        let maker = self.array_maker("from_vec", ty);
        let elements = self.table_lit(items, &texts).unwrap_or_else(|| format!("vec![{}]", texts.join(", ")));
        Code::atom(format!("{maker}({elements})"))
    }

    /// The `Vec` of the elements of a long array literal of string literals or of class
    /// literals (the tables of the
    /// reflection table, of resource bundles): a `static` of the constants and a loop that makes
    /// the elements from them, where `vec![..]` would be code that makes each one, 1.3 MB of
    /// machine code for a part of the server's reflection table, which takes long to compile and
    /// stays in memory after it ran once. `texts` are the elements as written one by one; each
    /// must be the same text around its constant.
    pub(super) fn table_lit(&mut self, items: &'a [Expr], texts: &[String]) -> Option<String> {
        const SHORTEST: usize = 32;
        if items.len() < SHORTEST {
            return None;
        }
        // An element of an `Object[]` or a `Class<?>[]` is the constant in a cast.
        fn constant(mut e: &Expr) -> &Expr {
            while let ExprKind::Cast(_, inner) = &e.kind {
                e = inner;
            }
            e
        }
        let strings = items.iter().all(|e| matches!(constant(e).kind, ExprKind::Str(_)));
        let classes =
            items.iter().all(|e| matches!(constant(e).kind, ExprKind::Builtin(Builtin::ClassLiteral(_), _)));
        let (static_type, read) = match (strings, classes) {
            (true, _) => ("&str", "*x"),
            (_, true) => ("&java::Class", "*x"),
            _ => return None,
        };
        let mut constants = Vec::with_capacity(items.len());
        let mut template: Option<String> = None;
        for (item, text) in items.iter().zip(texts) {
            let constant = match &constant(item).kind {
                ExprKind::Str(s) => rust_string(s),
                _ => self.expr(constant(item)).text,
            };
            // A class is a path to a constant or a static, or an array class, made by a `const fn`
            // that a static's initializer can call; named once in the element.
            let path = (constant.strip_prefix('&'))
                .is_some_and(|p| p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':'));
            let array = constant.starts_with("&const { java::Class::array(");
            if (classes && !path && !array) || text.matches(constant.as_str()).count() != 1 {
                return None;
            }
            let around = text.replacen(constant.as_str(), "\u{0}", 1);
            if template.get_or_insert_with(|| around.clone()) != &around {
                return None;
            }
            constants.push(constant);
        }
        let template = template?;
        // In parentheses only where a method is called on it, which `*` would take first.
        let read = match template.split_once('\u{0}') {
            Some((_, after)) if after.starts_with('.') => format!("({read})"),
            _ => read.to_string(),
        };
        let mut element = template.replacen('\u{0}', &read, 1);
        // An interned literal looks its string up once per place (`string_literal`): here once
        // per element, each with a cell of its own.
        let jstr = self.runtime("JStr");
        let cell = format!("static LITERAL: std::sync::OnceLock<{jstr}> = std::sync::OnceLock::new(); ");
        let mut cells = String::new();
        if element.contains(&cell) {
            element = element.replacen(&cell, "", 1).replacen("&LITERAL", "&CELLS[i]", 1);
            cells = format!(
                "static CELLS: [std::sync::OnceLock<{jstr}>; {n}] = [const {{ std::sync::OnceLock::new() }}; {n}]; ",
                n = items.len()
            );
        }
        // Any other place of the element's own would be one for all elements.
        if element.contains("static ") || element.contains("LITERAL") {
            return None;
        }
        Some(format!(
            "{{ static ITEMS: [{static_type}; {}] = [{}]; {cells}ITEMS.iter(){}.map(|{}| {element}).collect::<Vec<_>>() }}",
            items.len(),
            constants.join(", "),
            if cells.is_empty() { "" } else { ".enumerate()" },
            if cells.is_empty() { "x" } else { "(i, x)" },
        ))
    }

    /// `a[i] = v`, as a call of a runtime helper. Java evaluates the array, the index and then
    /// `v`, and checks the index last; the arguments come in that order. `used` says whether the
    /// assignment's value is needed, which picks `store` over `set`. A volatile write
    /// (`Class::volatile_elements`) is `set_volatile`, whose value is never used: it is the
    /// lowering of a `void` method.
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
        let (helper, mut v) = if self.class.volatile_elements.contains(&index.id) {
            ("set_volatile", self.element_value(rhs, ty))
        } else if used && self.stores_some(rhs, ty) {
            ("store_some", self.value(rhs, ty).text)
        } else {
            (if used { "store" } else { "set" }, self.element_value(rhs, ty))
        };
        // Into an `Object[]` that may be a covariant array, which checks what it holds.
        let checked;
        let helper = if self.class.checks_store(array.ty, rhs) {
            checked = format!("checked_{helper}");
            checked.as_str()
        } else {
            helper
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
        // A division that may fail passes its failure on with `?`, which a closure cannot hold.
        let dividing = matches!(op, BinOp::Div | BinOp::Rem) && self.facts.raising.contains(&id);
        if self.update_raises(op, ty, rhs) || dividing {
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
        // The element's type is named: `old.wrapping_add(1)` does not tell it, and rustc may not
        // see it through the array's borrow (`&this.borrow().digits`).
        let typed = if ty.is_numeric() { format!("{old}: {}", code::rust_type(ty)) } else { old.to_string() };
        let call = self.may_fail(id, helper, "", &format!("{handle}, {i}, |{typed}| {new}")).text;
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
                    // Text made with a call of a `toString` that throws.
                    _ if crate::analysis::generator_text(x) => {
                        crate::analysis::text_objects(x).into_iter().any(|o| self.text_raises(o))
                    }
                    _ => false,
                }
        });
        found
    }

    /// `raises_in` for the right operand `rhs` of `op=` on a value of type `ty`, which a string's
    /// `+=` turns into text.
    pub(super) fn update_raises(&self, op: BinOp, ty: Type, rhs: &Expr) -> bool {
        self.raises_in(rhs) || (op == BinOp::Add && ty == Type::String && self.text_raises(rhs))
    }

    /// Whether the text of `e` is made with a call of a method that throws (`object_text`).
    fn text_raises(&self, e: &Expr) -> bool {
        if !matches!(e.ty, Type::Class(..)) {
            return false;
        }
        if let Some(t) = self.enum_of(e) {
            return self.enum_text_throws(t);
        }
        let Some(c) = self.struct_class(e) else { return false };
        let text = hierarchy::to_string_impl(self.class, c).or_else(|| self.hash_index(c));
        text.is_some_and(|i| self.facts.throwing.contains(&i))
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
            let shown = format!("{old}.as_deref().unwrap_or({})", self.wtf8_literal("null"));
            let mut parts = vec![Part::Text(Code::atom(shown))];
            self.parts(rhs, &mut parts);
            return format!("Some({})", self.concat(parts).text);
        }
        if ty == Type::String {
            let mut parts = vec![Part::Text(Code::atom(old))];
            self.parts(rhs, &mut parts);
            return self.concat(parts).text;
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
        let taken =
            |name: &str| self.locals.values().any(|local| local == name) || self.fn_name_set.contains(name);
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
        // The temporaries of a `for`'s iterator live to the end of the loop: a borrow of the
        // object whose field holds the array would conflict with the body's.
        let source = if matches!(array.kind, ExprKind::Var(_)) { source } else { format!("{{ {source} }}") };
        let each = self.runtime("each");
        let wrap = if self.wrap_each(name, element(array.ty)) { ".map(Some)" } else { "" };
        self.line(&format!("for {mutable}{var} in {each}({source}){wrap} {{"));
        self.loop_body(Loop::Plain, body, &[]);
        self.line("}");
    }

    /// `Arrays.toString`, `fill`, `sort` and `copyOf` (the expression `id`).
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
        Type::StringBuilder => "stringbuilder".to_string(),
        Type::String => "string".to_string(),
        Type::Void => "void".to_string(),
        // A type of the runtime, such as `java::Class`, by its own name, of the characters that
        // a Rust name may have.
        _ => rust_type(ty)
            .rsplit("::")
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // A method told apart by its return type (`names::method_names`) may return nothing: its name
    // takes `void`, as xerces's two `createChunk`s of `DeferredDocumentImpl` do once their array
    // parameters are of one type.
    #[test]
    fn name_parts_are_rust_names() {
        assert_eq!(name_part(Type::Void), "void");
        assert_eq!(name_part(Type::array(Type::Int)), "i32_array");
        for ty in [Type::Void, Type::Int, Type::Boolean, Type::array(Type::Double)] {
            let part = name_part(ty);
            assert!(part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "{part}");
        }
    }
}
