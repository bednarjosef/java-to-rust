//! Plain arrays in generated code (see `analysis::arrays`): a `Vec`, a `Vec` of `Vec`s for an
//! array of arrays, or a slice where a parameter borrows the array. An access goes through an
//! access path, the plain array at its root and the indexes of its rows (`m`, `m[i]`, `m[i][j]`):
//!
//! - an index known to be in bounds is Rust's own indexing, `m[i as usize][j as usize]`;
//! - any other index is checked the way Java checks it: `java::at`, `java::put` and
//!   `java::modify` for elements, `java::row` and `java::row_mut` for rows;
//! - where an index or the assigned value also uses the array, which a borrow taken first would
//!   lock out, the positions are computed into locals first, in Java's order, and the access
//!   uses them (`{ let row = java::position_of(&m, i); let index = ..; java::at(&m[row], index) }`).
//!
//! A plain array is made by `vec!`, returned and moved as the `Vec` itself, and becomes a shared
//! array (`java::share`) only where it meets code that needs one.

use super::*;
use arrays::element;

/// An access path rooted at a plain array: `a`, `m[i]` or `m[i][j]`.
pub(super) struct Path<'a> {
    /// The root's Java name.
    pub java: &'a str,
    /// The slice the path starts at instead, in a sliced loop.
    pub slice: Option<String>,
    /// The index of each row from the root outward, with the id of the row's indexing.
    pub rows: Vec<(&'a Expr, ExprId)>,
}

/// What a step of an access path is in the code: the root variable, a place (`m[k]`) or a
/// reference to a row (`java::row(&m, i)`).
#[derive(Clone, Copy, PartialEq)]
enum Form {
    Root,
    Place,
    Ref,
}

impl<'a> Gen<'a> {
    /// Whether the local `name` is a plain array: `Some` with whether its elements are written.
    pub(super) fn plain(&self, name: &str) -> Option<bool> {
        self.facts.plain_arrays.get(&(self.method, name.to_string())).copied()
    }

    /// The access path of `e`, if `e` is a plain array or a row of one. In a sliced loop, a path
    /// through the sliced rows starts at the slice.
    pub(super) fn plain_path(&self, e: &'a Expr) -> Option<Path<'a>> {
        let mut path = self.java_path(e)?;
        if let Some((slice, depth)) = self.sliced.get(path.java)
            && path.rows.len() >= *depth
        {
            path.rows.drain(..*depth);
            path.slice = Some(slice.clone());
        }
        Some(path)
    }

    fn java_path(&self, e: &'a Expr) -> Option<Path<'a>> {
        match &e.kind {
            ExprKind::Var(name) => {
                self.plain(name)?;
                Some(Path { java: name, slice: None, rows: Vec::new() })
            }
            ExprKind::Index(inner, index) => {
                let mut path = self.java_path(inner)?;
                path.rows.push((index, e.id));
                Some(path)
            }
            _ => None,
        }
    }

    /// The Rust name of the array a path starts at.
    fn path_root(&self, path: &Path<'a>) -> String {
        path.slice.clone().unwrap_or_else(|| self.local(path.java))
    }

    /// Whether the access or row `id` is known to be in bounds.
    fn in_bounds(&self, id: ExprId) -> bool {
        self.facts.in_bounds.contains(&id) || self.sliced_accesses.contains(&id)
    }

    /// The Rust type of a plain array of type `ty`: `Vec<i32>`, `Vec<Vec<i32>>`.
    pub(super) fn plain_ty(&mut self, ty: Type) -> String {
        match ty.element() {
            row @ Type::Array(_) => format!("Vec<{}>", self.plain_ty(row)),
            elem => format!("Vec<{}>", self.element_ty_name(elem)),
        }
    }

    /// The type of an array parameter that the aliasing analysis borrows: a slice.
    pub(super) fn borrowed_ty(&mut self, ty: Type, written: bool) -> String {
        let elem = match ty.element() {
            row @ Type::Array(_) => self.plain_ty(row),
            elem => self.element_ty_name(elem),
        };
        if written { format!("&mut [{elem}]") } else { format!("&[{elem}]") }
    }

    /// The argument for a borrowed array parameter: a borrow of the caller's plain array or of
    /// one of its rows, of the contents of a shared one for the length of the call, or of an
    /// array made for the call (a new array, an array a method returns, or the packed arguments
    /// of a varargs call).
    pub(super) fn borrowed_arg(&mut self, e: &'a Expr, written: bool) -> String {
        let mutably = if written { "mut " } else { "" };
        match &e.kind {
            ExprKind::Var(name) => {
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
            ExprKind::Index(..) => {
                let path = self.plain_path(e).expect("the analysis lends rows of plain arrays");
                self.path_ref(&path, written)
            }
            ExprKind::ArrayLit(items) if !matches!(element(e.ty), Type::Array(_)) => {
                let elem = element(e.ty);
                let items: Vec<String> = items.iter().map(|item| self.element_value(item, elem)).collect();
                format!("&{mutably}[{}]", items.join(", "))
            }
            _ => format!("&{mutably}{}", self.owned(e)),
        }
    }

    /// A call of static method `index`. When it lends a shared array through a `RefCell` borrow,
    /// the borrow is a temporary that would live to the end of the statement, past the call, so a
    /// call with a result binds it in a block of its own: what the statement does with the result
    /// may borrow the same array again.
    pub(super) fn static_call(&mut self, index: usize, args: &'a [Expr]) -> Code {
        let lends = args.iter().enumerate().any(|(i, a)| {
            self.facts.borrowed_arrays.contains_key(&(index, i))
                && matches!(&a.kind, ExprKind::Var(name)
                    if self.plain(name).is_none() && !self.is_borrowed(&self.local(name)))
        });
        let texts = self.call_args(index, args).join(", ");
        let call = format!("{}{}({texts})", self.static_fn(index), self.entry_suffix(index, args));
        if lends && self.class.methods[index].ret != Type::Void {
            return Code::atom(format!("{{ let value = {call}; value }}"));
        }
        Code::atom(call)
    }

    /// Whether the Rust name `a` is a borrowed array parameter of the current method or the
    /// variable of a for-each over the rows of a plain array or a slice of a sliced loop: already
    /// a reference, so it is passed on as it is.
    pub(super) fn is_borrowed(&self, a: &str) -> bool {
        self.class.methods[self.method].params.iter().enumerate().any(|(j, p)| {
            self.facts.borrowed_arrays.contains_key(&(self.method, j)) && self.local(&p.name) == a
        }) || self.facts.row_loops.iter().any(|(m, name)| *m == self.method && self.local(name) == a)
            || self.sliced.values().any(|(slice, _)| slice == a)
    }

    /// The `&` argument for the plain array `a`.
    pub(super) fn slice_ref(&self, a: &str) -> String {
        if self.is_borrowed(a) { a.to_string() } else { format!("&{a}") }
    }

    /// The `&mut` argument for the plain array `a`.
    pub(super) fn mut_ref(&self, a: &str) -> String {
        if self.is_borrowed(a) { a.to_string() } else { format!("&mut {a}") }
    }

    /// An index known to be in bounds, as a `usize`.
    pub(super) fn position(&mut self, index: &'a Expr) -> String {
        match index.kind {
            ExprKind::Int(n) => n.to_string(),
            _ => format!("{} as usize", self.value(index, Type::Int).at(AS)),
        }
    }

    /// The array a path denotes, as code and what that code is.
    fn path_form(&mut self, path: &Path<'a>, mutable: bool) -> (String, Form) {
        let mut text = self.path_root(path);
        let mut form = Form::Root;
        for &(index, id) in &path.rows {
            if self.in_bounds(id) {
                let k = self.position(index);
                text = format!("{text}[{k}]");
                form = Form::Place;
            } else {
                let reference = self.reference(text, form, mutable);
                let i = self.value(index, Type::Int).text;
                let helper = if mutable { "row_mut" } else { "row" };
                text = self.may_fail(id, helper, "", &format!("{reference}, {i}")).text;
                form = Form::Ref;
            }
        }
        (text, form)
    }

    /// `text` of the given form as a slice argument.
    fn reference(&self, text: String, form: Form, mutable: bool) -> String {
        match (form, mutable) {
            (Form::Root, false) => self.slice_ref(&text),
            (Form::Root, true) => self.mut_ref(&text),
            (Form::Place, false) => format!("&{text}"),
            (Form::Place, true) => format!("&mut {text}"),
            (Form::Ref, _) => text,
        }
    }

    /// The array a path denotes, as something to index or call `len()` on.
    fn path_place(&mut self, path: &Path<'a>, mutable: bool) -> String {
        self.path_form(path, mutable).0
    }

    /// The array a path denotes, as a slice argument.
    pub(super) fn path_ref(&mut self, path: &Path<'a>, mutable: bool) -> String {
        let (text, form) = self.path_form(path, mutable);
        self.reference(text, form, mutable)
    }

    /// Whether every row of a path is known to be in bounds.
    fn rows_in_bounds(&self, path: &Path<'a>) -> bool {
        path.rows.iter().all(|(_, id)| self.in_bounds(*id))
    }

    /// The positions of a path's rows computed into locals first, checked unless they are known
    /// to be in bounds: the bindings, and the place of the array the path denotes with them.
    fn bound_rows(&mut self, path: &Path<'a>) -> (Vec<String>, String) {
        let mut place = self.path_root(path);
        let mut lets = Vec::new();
        for (level, &(index, id)) in path.rows.iter().enumerate() {
            let name = self.temp(["row", "column", "plane"].get(level).copied().unwrap_or("level"));
            let position = if self.in_bounds(id) {
                self.position(index)
            } else {
                let reference = if level == 0 { self.slice_ref(&place) } else { format!("&{place}") };
                let i = self.value(index, Type::Int).text;
                self.may_fail(id, "position_of", "", &format!("{reference}, {i}")).text
            };
            lets.push(format!("let {name} = {position};"));
            place = format!("{place}[{name}]");
        }
        (lets, place)
    }

    /// The bound place of a path as a slice argument.
    fn bound_ref(&self, path: &Path<'a>, place: &str, mutable: bool) -> String {
        let form = if path.rows.is_empty() { Form::Root } else { Form::Place };
        self.reference(place.to_string(), form, mutable)
    }

    /// A plain array local, declared with its initializer as a `Vec`.
    pub(super) fn plain_local(&mut self, ty: Type, name: &str, init: Option<&'a Expr>, mutable: bool) {
        let mutable = if mutable { "mut " } else { "" };
        let rust = self.local(name);
        let ty_text = self.plain_ty(ty);
        match init {
            Some(init) => {
                let value = self.owned(init);
                self.line(&format!("let {mutable}{rust}: {ty_text} = {value};"));
            }
            // Assigned in a `try` block, which Rust does not follow (see `local_decl`).
            None if self.try_assigned.contains(name) => {
                self.line("#[allow(unused_assignments)]");
                self.line(&format!("let {mutable}{rust}: {ty_text} = Vec::new();"));
            }
            None => self.line(&format!("let {mutable}{rust}: {ty_text};")),
        }
    }

    /// An array that nobody else holds, as a plain `Vec`: a new array, a literal, the result of a
    /// method that returns its own arrays, or a plain local moved out of its variable.
    pub(super) fn owned(&mut self, e: &'a Expr) -> String {
        if self.facts.catching.contains_key(&e.id) && self.catching_now.insert(e.id) {
            let mark = self.out.len();
            let text = self.owned(e);
            self.catching_now.remove(&e.id);
            return self.caught(e.id, Code::atom(text), mark).text;
        }
        match &e.kind {
            ExprKind::NewArray(sizes) => self.owned_new(e.ty, sizes),
            ExprKind::ArrayLit(items) => {
                let elem = element(e.ty);
                let texts: Vec<String> = if matches!(elem, Type::Array(_)) {
                    items.iter().map(|item| self.owned(item)).collect()
                } else {
                    items.iter().map(|item| self.element_value(item, elem)).collect()
                };
                self.table_lit(items, &texts).unwrap_or_else(|| format!("vec![{}]", texts.join(", ")))
            }
            ExprKind::Var(name) => self.local(name),
            ExprKind::Builtin(Builtin::MainArgs, _) => self.main_args_vec(e.ty),
            ExprKind::Call { args, resolved: Some(index), .. } => {
                let call = self.static_call(*index, args);
                self.raising(call, *index).text
            }
            ExprKind::MethodCall { receiver, args, resolved: Some(index), is_super, .. } => {
                let call = if self.closure_type(receiver.ty).is_some() {
                    self.closure_call(receiver, *index, args)
                } else if self.inlined(receiver, *index) {
                    self.inline_call(*index, args)
                } else if self.class.methods[*index].is_static {
                    self.static_call(*index, args)
                } else if !*is_super && let Some(t) = self.enum_of(receiver) {
                    self.enum_call(t, e.pos, receiver, *index, args)
                } else {
                    // The method of the receiver's class, which an interface's method is not.
                    let target = self.dispatch(receiver, *index, *is_super);
                    if self.runs_nowhere(receiver, target) {
                        return self.no_object_runs(receiver, target, *index).text;
                    }
                    self.method_call(receiver, target, args)
                };
                // A method that throws returns its array in a `Result`.
                self.raising(call, *index).text
            }
            _ => unreachable!("the analysis makes plain arrays only of owned values"),
        }
    }

    /// `new T[n]` or `new T[n][m]...` as nested `vec!`s. A size that may be negative is checked
    /// first, every size before anything is allocated, as Java does.
    fn owned_new(&mut self, ty: Type, sizes: &'a [Expr]) -> String {
        let zero = self.element_zero(ty.element_at(sizes.len()));
        let not_negative = |g: &Self, s: &Expr| match &s.kind {
            ExprKind::Int(n) => *n >= 0,
            // A constant such as `static final int ROWS = 12`.
            ExprKind::Static { class, name } => g.class.types[*class].statics.iter().any(|f| {
                &f.name == name
                    && f.constant
                    && matches!(f.init, Some(Expr { kind: ExprKind::Int(n), .. }) if n >= 0)
            }),
            _ => g.facts.index_ranges.get(&s.id).is_some_and(|&(lo, _)| lo >= 0),
        };
        // `vec![vec![0; m]; n]` computes `m` before `n`, so several sizes must have no effects.
        let in_order = sizes.len() == 1 || !sizes.iter().any(has_effects);
        if in_order && sizes.iter().all(|s| not_negative(self, s)) {
            let mut text = zero.to_string();
            for size in sizes.iter().rev() {
                let n = match size.kind {
                    ExprKind::Int(n) => n.to_string(),
                    _ => format!("{} as usize", self.value(size, Type::Int).at(AS)),
                };
                text = format!("vec![{text}; {n}]");
            }
            return text;
        }
        if let [size] = sizes {
            let n = self.value(size, Type::Int).text;
            return format!("{}({n}, {zero})", self.runtime("new_vec"));
        }
        let dims: Vec<String> = sizes.iter().map(|s| self.value(s, Type::Int).text).collect();
        let mut text = zero.to_string();
        for level in (0..sizes.len()).rev() {
            text = format!("vec![{text}; dims[{level}] as usize]");
        }
        format!("{{ let dims = [{}]; {}(&dims); {text} }}", dims.join(", "), self.runtime("check_sizes"))
    }

    /// A plain array where a shared one is needed: a plain local moved into it, or the result of
    /// a method that returns its own arrays, made shared with `java::share`.
    pub(super) fn shared_array(&mut self, e: &'a Expr) -> Option<Code> {
        let owned = match &e.kind {
            ExprKind::Var(name) => self.plain(name).is_some() && !self.is_borrowed(&self.local(name)),
            ExprKind::Call { resolved: Some(index), .. }
            | ExprKind::MethodCall { resolved: Some(index), .. } => self.facts.owned_returns.contains(index),
            _ => false,
        };
        if !owned {
            return None;
        }
        let value = self.owned(e);
        let share = self.runtime(if self.shared_array_type(e.ty) { "share_across" } else { "share" });
        Some(Code::atom(format!("{share}({value})")))
    }

    /// `a[i]` of a plain array or row, as a value.
    pub(super) fn plain_index(&mut self, path: Path<'a>, id: ExprId, index: &'a Expr, elem: Type) -> Code {
        let root = path.java;
        if path.rows.iter().any(|(i, _)| writes_root(i, root)) || writes_root(index, root) {
            let (mut lets, place) = self.bound_rows(&path);
            let slot = self.fresh_name(&["index", "slot", "at"]);
            let i = self.value(index, Type::Int).text;
            lets.push(format!("let {slot} = {i};"));
            let reference = self.bound_ref(&path, &place, false);
            let at = self.may_fail(id, "at", "", &format!("{reference}, {slot}")).text;
            return Code::atom(format!("{{ {} {at} }}", lets.join(" ")));
        }
        if self.in_bounds(id) {
            let place = self.path_place(&path, false);
            let k = self.position(index);
            return Code::atom(read_in_bounds(&place, &k, elem));
        }
        let reference = self.path_ref(&path, false);
        let i = self.value(index, Type::Int).text;
        self.may_fail(id, "at", "", &format!("{reference}, {i}"))
    }

    /// `a.length` of a plain array or row.
    pub(super) fn plain_length(&mut self, path: Path<'a>) -> Code {
        let place = self.path_place(&path, false);
        Code::new(format!("{place}.len() as i32"), AS)
    }

    /// `a[i] = v` on a plain array or row: a plain assignment where the index is in bounds,
    /// `put` (or `assign`, for a row that is moved in) otherwise. Where the index or the value
    /// mentions the array, they are computed first, since the call would borrow it mutably while
    /// they read it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn plain_set(
        &mut self,
        path: Path<'a>,
        id: ExprId,
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        used: bool,
    ) -> String {
        let root = path.java;
        let row = matches!(ty, Type::Array(_));
        let in_order = rust_order_is_same(index, Some(rhs))
            && path.rows.iter().all(|(i, _)| rust_order_is_same(i, Some(rhs)));
        // `place[k] = v` borrows the array mutably before it computes the positions in it: a row's
        // position that reads the array takes the way below, and an index that reads it
        // (`a[n % a.length]`) is computed first.
        let rows_read_root = path.rows.iter().any(|(i, _)| mentions(i, root));
        if !used && self.in_bounds(id) && self.rows_in_bounds(&path) && in_order && !rows_read_root {
            let place = self.path_place(&path, true);
            let k = self.position(index);
            let v = self.assigned_element(rhs, ty);
            if mentions(index, root) {
                let slot = self.fresh_name(&["index", "slot", "at"]);
                return format!("{{ let {slot} = {k}; {place}[{slot}] = {v} }}");
            }
            return format!("{place}[{k}] = {v}");
        }
        let mentioned =
            path.rows.iter().any(|(i, _)| mentions(i, root)) || mentions(index, root) || mentions(rhs, root);
        // The value of the assignment is an `Option` only where the stored value may be null.
        let some = used && !row && self.stores_some(rhs, ty);
        let put = if some { "put_some" } else { "put" };
        if !mentioned {
            let reference = self.path_ref(&path, true);
            let i = self.value(index, Type::Int).text;
            let v = if some { self.value(rhs, ty).text } else { self.assigned_element(rhs, ty) };
            let helper = if row && !used { "assign" } else { put };
            return self.may_fail(id, helper, "", &format!("{reference}, {i}, {v}")).text;
        }
        let (mut lets, place) = self.bound_rows(&path);
        let i = self.value(index, Type::Int).text;
        // A variable or literal index that the value leaves alone needs no binding of its own.
        let slot = if path.rows.is_empty() && simple_index(index) && in_order {
            i
        } else {
            let slot = self.fresh_name(&["index", "slot", "at"]);
            lets.push(format!("let {slot} = {i};"));
            slot.to_string()
        };
        let value = self.fresh_name(&["value", "item", "new"]);
        let v = if some { self.value(rhs, ty).text } else { self.assigned_element(rhs, ty) };
        lets.push(format!("let {value} = {v};"));
        let reference = self.bound_ref(&path, &place, true);
        let call = self.may_fail(id, put, "", &format!("{reference}, {slot}, {value}")).text;
        format!("{{ {} {call} }}", lets.join(" "))
    }

    /// The value stored by `a[i] = rhs` into a plain array: an element, or a row moved in.
    fn assigned_element(&mut self, rhs: &'a Expr, ty: Type) -> String {
        if matches!(ty, Type::Array(_)) { self.owned(rhs) } else { self.element_value(rhs, ty) }
    }

    /// `a[i] op= v` on a plain array or row. As a statement (`used` false), it is `a[i] op= v`
    /// where the index is in bounds, and `java::modify` otherwise; where the value mentions the
    /// array, or the result is used, the old element is read and the new one written through
    /// positions computed first.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn plain_update(
        &mut self,
        path: Path<'a>,
        op: BinOp,
        index: &'a Expr,
        ty: Type,
        rhs: &'a Expr,
        id: ExprId,
        used: bool,
    ) -> String {
        let root = path.java;
        let rows_mention = path.rows.iter().any(|(i, _)| mentions(i, root));
        let simple = simple_index(index) && path.rows.iter().all(|(i, _)| simple_index(i));
        let in_order = rust_order_is_same(index, Some(rhs))
            && path.rows.iter().all(|(i, _)| rust_order_is_same(i, Some(rhs)));
        if !used
            && !writes_root(rhs, root)
            && self.in_bounds(id)
            && self.rows_in_bounds(&path)
            && simple
            && in_order
        {
            // Rust reads the old element before a value that only reads the array.
            let place = self.path_place(&path, true);
            let k = self.position(index);
            let slot = format!("{place}[{k}]");
            if !mentions(rhs, root)
                && let Some(text) = self.compound_in_place(op, ty, rhs, id, &slot)
            {
                return text;
            }
            let new = self.element_update(op, ty, rhs, id, &slot);
            return format!("{slot} = {new}");
        }
        // A value that may fail is computed outside a closure, where its failure can go on.
        if !used
            && !mentions(rhs, root)
            && !mentions(index, root)
            && !rows_mention
            && !self.update_raises(op, ty, rhs)
        {
            let reference = self.path_ref(&path, true);
            let i = self.value(index, Type::Int).text;
            let old = self.fresh_name(&["old", "previous", "before"]);
            let new = self.element_update(op, ty, rhs, id, old);
            let new = self.closure_value(id, new);
            return self.may_fail(id, "modify", "", &format!("{reference}, {i}, |{old}| {new}")).text;
        }
        let (mut lets, place) = self.bound_rows(&path);
        let slot = self.fresh_name(&["index", "slot", "at"]);
        let old = self.fresh_name(&["old", "previous", "before"]);
        let value = self.fresh_name(&["value", "item", "new"]);
        let i = self.value(index, Type::Int).text;
        lets.push(format!("let {slot} = {i};"));
        let read = self.bound_ref(&path, &place, false);
        let at = self.may_fail(id, "at", "", &format!("{read}, {slot}")).text;
        lets.push(format!("let {old} = {at};"));
        let new = self.element_update(op, ty, rhs, id, old);
        lets.push(format!("let {value} = {new};"));
        let write = self.bound_ref(&path, &place, true);
        let put = self.may_fail(id, "put", "", &format!("{write}, {slot}, {value}")).text;
        format!("{{ {} {put} }}", lets.join(" "))
    }

    /// `a[i]++`, `--a[i]` and the like on a plain array or row. As a statement (`used` false),
    /// it is `a[i] += 1` or a `wrapping_add` where the index is in bounds, and `java::modify`
    /// otherwise.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn plain_step(
        &mut self,
        path: Path<'a>,
        index: &'a Expr,
        ty: Type,
        increment: bool,
        prefix: bool,
        id: ExprId,
        used: bool,
    ) -> String {
        let root = path.java;
        let rows_mention = path.rows.iter().any(|(i, _)| mentions(i, root));
        if !used && !mentions(index, root) && !rows_mention {
            let simple = simple_index(index) && path.rows.iter().all(|(i, _)| simple_index(i));
            if self.in_bounds(id) && self.rows_in_bounds(&path) && simple {
                let place = self.path_place(&path, true);
                let k = self.position(index);
                let slot = format!("{place}[{k}]");
                let sign = if increment { "+" } else { "-" };
                if ty.is_floating() {
                    return format!("{slot} {sign}= 1.0");
                }
                if self.no_overflow(id) {
                    return format!("{slot} {sign}= 1");
                }
                return format!("{slot} = {}", self.step_value(ty, increment, id, &slot));
            }
            let reference = self.path_ref(&path, true);
            let i = self.value(index, Type::Int).text;
            let old = self.fresh_name(&["old", "previous", "before"]);
            let new = self.step_value(ty, increment, id, old);
            let new = self.closure_value(id, new);
            return self.may_fail(id, "modify", "", &format!("{reference}, {i}, |{old}| {new}")).text;
        }
        let (mut lets, place) = self.bound_rows(&path);
        let slot = self.fresh_name(&["index", "slot", "at"]);
        let old = self.fresh_name(&["old", "previous", "before"]);
        let i = self.value(index, Type::Int).text;
        lets.push(format!("let {slot} = {i};"));
        let read = self.bound_ref(&path, &place, false);
        let at = self.may_fail(id, "at", "", &format!("{read}, {slot}")).text;
        lets.push(format!("let {old} = {at};"));
        let new = self.step_value(ty, increment, id, old);
        let write = self.bound_ref(&path, &place, true);
        let put = self.may_fail(id, "put", "", &format!("{write}, {slot}, {new}")).text;
        if prefix {
            format!("{{ {} {put} }}", lets.join(" "))
        } else {
            format!("{{ {} {put}; {old} }}", lets.join(" "))
        }
    }

    /// A counting loop over a plain array whose body only reads its elements becomes
    /// `for x in a.iter().cloned()`, and each `a[i]` in the body becomes `x`. Returns false when
    /// the loop is not one of those, or its elements are not plain values.
    pub(super) fn element_loop(&mut self, cond: &'a Expr, body: &'a Stmt) -> bool {
        let Some(reads) = self.facts.element_loops.get(&cond.id).cloned() else { return false };
        let ExprKind::Binary(_, _, end) = &cond.kind else { return false };
        let ExprKind::Length(array) = &end.kind else { return false };
        if element(array.ty).is_reference() || self.nullable_element(element(array.ty)) {
            return false;
        }
        let ExprKind::Var(name) = &array.kind else { return false };
        if self.plain(name).is_none() {
            return false;
        }
        let a = self.local(name);
        let item = self.fresh_name(&["item", "element", "value"]);
        self.elements.extend(reads.into_iter().map(|id| (id, item.to_string())));
        self.line(&format!("for {item} in {a}.iter().cloned() {{"));
        self.loop_body(Loop::Plain, body, &[]);
        self.line("}");
        true
    }

    /// `for (T x : a) body` over a plain one-dimensional array: over `a.iter().cloned()`, or by
    /// index where the body writes elements, as it cannot hold a borrow of the array then.
    /// Over a plain array of arrays, the variable borrows each row: `for row in m.iter()`, or
    /// `iter_mut()` when the body writes through it.
    pub(super) fn plain_for_each(&mut self, name: &str, java: &str, array: &'a Expr, body: &'a Stmt) {
        let mutable = if assigns_any(std::slice::from_ref(body), name) { "mut " } else { "" };
        let var = self.local(name);
        let a = self.local(java);
        if let Some(written) = self.plain(name).filter(|_| matches!(element(array.ty), Type::Array(_))) {
            let iter = if written { "iter_mut" } else { "iter" };
            self.line(&format!("for {var} in {a}.{iter}() {{"));
        } else if writes_elements(body, java) {
            let slot = self.fresh_name(&["index", "slot", "at"]);
            let wrap = self.wrap_each(name, element(array.ty));
            let element = read_in_bounds(&a, slot, element(array.ty));
            let element = if wrap { format!("Some({element})") } else { element };
            self.line(&format!("for {slot} in 0..{a}.len() {{"));
            // The loop's frame opens at its header, where a label goes, before the element's line.
            self.open(Kind::Loop(Loop::Plain));
            self.indent += 1;
            self.line(&format!("let {mutable}{var} = {element};"));
            self.indent -= 1;
            self.nested(body);
            self.close();
            self.line("}");
            return;
        } else {
            let wrap = if self.wrap_each(name, element(array.ty)) { ".map(Some)" } else { "" };
            self.line(&format!("for {mutable}{var} in {a}.iter().cloned(){wrap} {{"));
        }
        self.loop_body(Loop::Plain, body, &[]);
        self.line("}");
    }
}

/// Whether evaluating `e` may write to the array `root`: assign one of its elements, pass it to
/// a call, or assign the variable.
fn writes_root(e: &Expr, root: &str) -> bool {
    let mut found = false;
    visit_expr(e, &mut |x| match &x.kind {
        ExprKind::Assign { target: Place::Index(array, _), .. }
        | ExprKind::IncDec { target: Place::Index(array, _), .. } => found |= root_is(array, root),
        ExprKind::Assign { target: Place::Var(v), .. } => found |= v == root,
        ExprKind::Call { args, .. }
        | ExprKind::MethodCall { args, .. }
        | ExprKind::New { args, .. }
        | ExprKind::ThisCall { args, .. }
        | ExprKind::SuperInit { args, .. }
        | ExprKind::Builtin(_, args) => found |= args.iter().any(|a| mentions(a, root)),
        _ => {}
    });
    found
}

/// Whether the access path `e` is rooted at the variable `root`.
fn root_is(e: &Expr, root: &str) -> bool {
    match &e.kind {
        ExprKind::Var(v) => v == root,
        ExprKind::Index(inner, _) => root_is(inner, root),
        _ => false,
    }
}

/// Whether `a[index] = rhs` and `a[index] op= rhs` can be Rust assignments, which evaluate `rhs`
/// before the index where Java evaluates the index first: the index has no effects, and `rhs`
/// assigns no variable that the index reads.
pub(super) fn rust_order_is_same(index: &Expr, rhs: Option<&Expr>) -> bool {
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
                    found |= root_is(array, name);
                }
            })
        },
        &mut |_| {},
    );
    found
}
