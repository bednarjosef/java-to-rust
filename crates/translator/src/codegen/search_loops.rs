//! Loops that search an array for a key and return when they find it:
//!
//! ```text
//! for (int i = 0; i < palette.length; i++) {
//!     if (palette[i] == key) {
//!         return i;
//!     }
//! }
//! ```
//!
//! become a call of the runtime's search, then the `return`:
//!
//! ```text
//! let found = java::search_same(java::non_null(PALETTE.cloned().as_ref()), 0, None, key.as_ref(), true, |item| ..);
//! if let Some(i) = found {
//!     return i;
//! }
//! ```
//!
//! `java::search_eq` compares integers, characters, booleans and enum constants with `==`;
//! `java::search_same` compares references by identity, as machine words where the key is the
//! same object as an element exactly when their bits are equal (a handle, or a value of an enum
//! of classes whose class is not compared by value, which the call tests: `by_bits`), and with
//! the test as Java writes it (`|item| ..`) otherwise. Either compares four elements to a turn
//! (array.rs, `search_eq`): LLVM does not unroll a loop it can leave in two places, so each element
//! paid the loop's own compare and branch too, where the JIT unrolls it. The key, the array and
//! the bounds are locals, constants, statics or fields of `this`, which nothing in the loop can
//! change, and are read once, before the search.

use super::*;
use arrays::element;

impl<'a> Gen<'a> {
    /// Writes the `for` loop as a search, if it is one (see the module's documentation).
    pub(super) fn search_loop(
        &mut self,
        init: &'a [Stmt],
        cond: Option<&'a Expr>,
        update: &'a [Expr],
        body: &'a Stmt,
    ) -> bool {
        let [Stmt::Local { ty: Type::Int, name: var, init: Some(start), .. }] = init else { return false };
        let Some(Expr { kind: ExprKind::Binary(BinOp::Lt, counter, end), .. }) = cond else { return false };
        if !matches!(&counter.kind, ExprKind::Var(v) if v == var) || end.ty != Type::Int {
            return false;
        }
        let [Expr { kind: ExprKind::IncDec { target: Place::Var(stepped), increment: true, .. }, .. }] =
            update
        else {
            return false;
        };
        if stepped != var {
            return false;
        }
        let Some((test, then)) = found_return(body) else { return false };
        let ExprKind::Binary(BinOp::Eq, l, r) = &test.kind else { return false };
        let (read, key) = match (&l.kind, &r.kind) {
            (ExprKind::Index(..), _) => (l.as_ref(), r.as_ref()),
            (_, ExprKind::Index(..)) => (r.as_ref(), l.as_ref()),
            _ => return false,
        };
        let ExprKind::Index(array, at) = &read.kind else { return false };
        if !matches!(&at.kind, ExprKind::Var(v) if v == var)
            || !names_array(array, var)
            || !plain_value(key, var)
            || !bound(start, var)
            || !bound(end, var)
        {
            return false;
        }
        // An index whose `ArrayIndexOutOfBoundsException` a `catch` may see is a `try_get` that
        // passes the exception on with `?`, which the runtime's search does not.
        if self.facts.raising.contains(&read.id) {
            return false;
        }
        let elem = element(array.ty);
        let by_value = matches!(
            elem,
            Type::Int | Type::Long | Type::Short | Type::Byte | Type::Char | Type::Boolean | Type::Enum(_)
        ) && key.ty == elem
            && !self.nullable_element(elem);
        let by_identity = matches!(elem, Type::Class(..))
            && (matches!(key.kind, ExprKind::Null) || self.ty_name(key.ty) == self.ty_name(elem));
        if !by_value && !by_identity {
            return false;
        }
        let to_length = matches!(&end.kind, ExprKind::Length(a) if same_array(a, array));
        let plain = self.plain_path(array);
        let mark = self.out.len();
        let handle = match &plain {
            // A plain array is a `Vec` or a slice of the values `a[i]` reads.
            Some(path) if path.rows.is_empty() && by_value => self.path_ref(path, false),
            Some(_) => return false,
            // A null array throws as the loop starts only where the condition reads its length.
            None if self.nullable(array) && !to_length => return false,
            None => self.handle(array),
        };
        let start_text = self.value(start, Type::Int).text;
        let end_text = match to_length {
            true => "None".to_string(),
            false => format!("Some({})", self.value(end, Type::Int).text),
        };
        let call = if by_value {
            let key_text = self.value(key, elem).text;
            let helper = self.runtime(if plain.is_some() { "slice_search_eq" } else { "search_eq" });
            format!("{helper}({handle}, {start_text}, {end_text}, {key_text})")
        } else {
            let enum_type = self.enum_of(read);
            let key_text = if enum_type.is_some() { self.enum_option(key) } else { self.object_option(key) };
            let by_bits = match enum_type.and_then(|t| self.unequal_bits_tags(t)) {
                Some(tags) if !matches!(key.kind, ExprKind::Null) => {
                    format!("{key_text}.is_none_or(|k| !matches!(k.0.tag(), {tags}))")
                }
                _ => "true".to_string(),
            };
            // The test as Java writes it, for the elements not compared as words.
            let item = self.fresh_name(&["item", "element", "value"]);
            self.elements.insert(read.id, item.to_string());
            let test_text = self.condition(test);
            self.elements.remove(&read.id);
            let helper = self.runtime("search_same");
            format!(
                "{helper}({handle}, {start_text}, {end_text}, {key_text}, {by_bits}, |{item}| {test_text})"
            )
        };
        // The call is one expression: anything that needed statements of its own first is left
        // to the loop as Java writes it.
        if self.out.len() != mark {
            self.out.truncate(mark);
            return false;
        }
        let found = self.temp("found");
        self.line(&format!("let {found} = {call};"));
        let mut used = false;
        visit_stmt(then, &mut |e| used |= mentions(e, var), &mut |_| {});
        if used {
            let index = self.local(var);
            self.line(&format!("if let Some({index}) = {found} {{"));
        } else {
            self.line(&format!("if {found}.is_some() {{"));
        }
        self.nested(then);
        self.line("}");
        true
    }
}

/// The condition and the `return` of a loop body that is only `if (cond) return ..;`.
fn found_return(body: &Stmt) -> Option<(&Expr, &Stmt)> {
    let stmt = match body {
        Stmt::Block(stmts) => match stmts.as_slice() {
            [stmt] => stmt,
            _ => return None,
        },
        stmt => stmt,
    };
    let Stmt::If { cond, then, otherwise: None } = stmt else { return None };
    let ret = match then.as_ref() {
        Stmt::Block(stmts) => match stmts.as_slice() {
            [stmt] => stmt,
            _ => return None,
        },
        stmt => stmt,
    };
    matches!(ret, Stmt::Return(..)).then_some((cond, then.as_ref()))
}

/// Whether `a` names an array the same way at each test: a local other than the loop's, a
/// static, or a field of `this`.
fn names_array(a: &Expr, var: &str) -> bool {
    match &a.kind {
        ExprKind::Var(v) => v != var,
        ExprKind::Static { .. } => true,
        ExprKind::Field { object, .. } => matches!(object.kind, ExprKind::This),
        _ => false,
    }
}

/// Whether two expressions name the same array: one local, one static or one field of `this`.
fn same_array(a: &Expr, b: &Expr) -> bool {
    match (&a.kind, &b.kind) {
        (ExprKind::Var(x), ExprKind::Var(y)) => x == y,
        (ExprKind::Static { class: c, name: x }, ExprKind::Static { class: d, name: y }) => c == d && x == y,
        (ExprKind::Field { object: o, name: x }, ExprKind::Field { object: p, name: y }) => {
            matches!(o.kind, ExprKind::This) && matches!(p.kind, ExprKind::This) && x == y
        }
        _ => false,
    }
}

/// Whether a bound of the loop is read the same before the search as at each test: a local
/// other than the loop's, a constant, the length of an array named so, or the operators of
/// those that cannot throw.
fn bound(e: &Expr, var: &str) -> bool {
    if matches!(e.ty, Type::Boxed(_)) {
        return false;
    }
    match &e.kind {
        ExprKind::Length(a) => {
            matches!(&a.kind, ExprKind::Var(v) if v != var)
                | matches!(&a.kind, ExprKind::Static { .. })
                | matches!(&a.kind, ExprKind::Field { object, .. } if matches!(object.kind, ExprKind::This))
        }
        ExprKind::Field { object, .. } => matches!(object.kind, ExprKind::This) && !e.ty.is_reference(),
        ExprKind::Unary(_, x) => bound(x, var),
        ExprKind::Binary(op, l, r) => cannot_throw(*op, r) && bound(l, var) && bound(r, var),
        _ => plain_value(e, var),
    }
}

/// A constant, or a local other than the loop's.
fn plain_value(e: &Expr, var: &str) -> bool {
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Long(_)
        | ExprKind::Char(_)
        | ExprKind::Bool(_)
        | ExprKind::Double(_)
        | ExprKind::Float(_)
        | ExprKind::Null => true,
        ExprKind::Var(v) => v != var,
        _ => false,
    }
}

/// Whether `l op r` cannot throw: anything but a division or remainder by what may be zero.
fn cannot_throw(op: BinOp, r: &Expr) -> bool {
    !matches!(op, BinOp::Div | BinOp::Rem) || matches!(r.kind, ExprKind::Int(k) if k != 0)
}
