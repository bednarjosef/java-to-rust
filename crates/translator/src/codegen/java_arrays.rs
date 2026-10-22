//! The `java.util.Arrays` methods that the runtime's `java_arrays` helpers cover, and `clone()` of
//! an array. Each takes the array as a shared handle, as `arrays_builtin` does; `equals` and
//! `hashCode` also accept a null array, which is `None`.

use super::arrays::element;
use super::*;

impl<'a> Gen<'a> {
    /// One of the methods in `ArraysOp`, as a call of its runtime helper.
    pub(super) fn arrays_op(&mut self, op: ArraysOp, args: &'a [Expr], id: ExprId) -> Code {
        let elem = element(args[0].ty);
        // A search over elements that may be null whose `NullPointerException` a `catch` sees.
        if matches!(op, ArraysOp::BinarySearch | ArraysOp::BinarySearchRange)
            && self.facts.null_checked.contains_key(&id)
        {
            self.dereferenced.insert(id);
            let a = self.handle(&args[0]);
            let range = match op {
                ArraysOp::BinarySearchRange => {
                    let (from, to) =
                        (self.value(&args[1], Type::Int).text, self.value(&args[2], Type::Int).text);
                    format!("Some(({from}, {to}))")
                }
                _ => "None".to_string(),
            };
            let key = self.element_value(&args[args.len() - 1], elem);
            let message = |g: &Self, id: ExprId, default: &str| match g.facts.npe_messages.get(&id) {
                Some(Some(message)) => format!("{message:?}"),
                _ => format!("{default:?}"),
            };
            let null_element = message(self, id, crate::analysis::SEARCH_NULL_ELEMENT);
            let null_key = message(self, args[args.len() - 1].id, crate::analysis::SEARCH_NULL_KEY);
            let search = self.runtime("try_binary_search");
            return Code::atom(self.passed_on(Code::atom(format!(
                "{search}({a}, {range}, {key}, {null_element}, {null_key})"
            ))));
        }
        let text = match op {
            ArraysOp::Equals => {
                let (a, b) = (self.optional_handle(&args[0]), self.optional_handle(&args[1]));
                format!("{}({a}, {b})", self.runtime("equals"))
            }
            ArraysOp::HashCode => {
                let a = self.optional_handle(&args[0]);
                format!("{}({a})", self.runtime("hash_code"))
            }
            ArraysOp::SortAll => {
                let a = self.handle(&args[0]);
                format!("{}({a})", self.runtime("sort_all"))
            }
            ArraysOp::SortRange => {
                let a = self.handle(&args[0]);
                let (from, to) = (self.value(&args[1], Type::Int).text, self.value(&args[2], Type::Int).text);
                format!("{}({a}, {from}, {to})", self.runtime("sort_range"))
            }
            ArraysOp::FillRange => {
                let a = self.handle(&args[0]);
                let (from, to) = (self.value(&args[1], Type::Int).text, self.value(&args[2], Type::Int).text);
                let v = self.element_value(&args[3], elem);
                format!("{}({a}, {from}, {to}, {v})", self.runtime("fill_range"))
            }
            // An array that may be null, whose `NullPointerException` a `catch` sees: the JDK's
            // code checks the range first where `from` is not 0 (`try_copy_of_range`).
            ArraysOp::CopyOfRange
                if self.nullable(&args[0]) && self.facts.null_checked.contains_key(&args[0].id) =>
            {
                self.dereferenced.insert(args[0].id);
                let a = self.optional_handle(&args[0]);
                let (from, to) = (self.value(&args[1], Type::Int).text, self.value(&args[2], Type::Int).text);
                let zero = self.element_zero(elem);
                let message = match self.facts.npe_messages.get(&args[0].id) {
                    Some(Some(message)) => format!("Some({message:?})"),
                    _ => "None".to_string(),
                };
                let objects = elem.is_reference();
                let call = format!(
                    "{}({a}, {from}, {to}, {zero}, {message}, {objects})",
                    self.runtime("try_copy_of_range")
                );
                self.passed_on(Code::atom(call))
            }
            ArraysOp::CopyOfRange => {
                let a = self.handle(&args[0]);
                let (from, to) = (self.value(&args[1], Type::Int).text, self.value(&args[2], Type::Int).text);
                let zero = self.element_zero(elem);
                format!("{}({a}, {from}, {to}, {zero})", self.runtime("copy_of_range"))
            }
            // A key that may be null fails where Java compares it with an element, in the
            // element's `compareTo` (`binary_search_all_nullable`).
            ArraysOp::BinarySearch if self.nullable(&args[1]) && !self.nullable_element(elem) => {
                let a = self.handle(&args[0]);
                let key = self.optional_to(&args[1], elem).text;
                let search = format!("{}({a}, {key})", self.runtime("binary_search_all_nullable"));
                self.dereference(&[args[1].id], &search)
            }
            ArraysOp::BinarySearch => {
                let a = self.handle(&args[0]);
                let key = self.element_value(&args[1], elem);
                format!("{}({a}, {key})", self.runtime("binary_search_all"))
            }
            ArraysOp::BinarySearchRange if self.nullable(&args[3]) && !self.nullable_element(elem) => {
                let a = self.handle(&args[0]);
                let (from, to) = (self.value(&args[1], Type::Int).text, self.value(&args[2], Type::Int).text);
                let key = self.optional_to(&args[3], elem).text;
                let search =
                    format!("{}({a}, {from}, {to}, {key})", self.runtime("binary_search_range_nullable"));
                self.dereference(&[args[3].id], &search)
            }
            ArraysOp::BinarySearchRange => {
                let a = self.handle(&args[0]);
                let (from, to) = (self.value(&args[1], Type::Int).text, self.value(&args[2], Type::Int).text);
                let key = self.element_value(&args[3], elem);
                format!("{}({a}, {from}, {to}, {key})", self.runtime("binary_search_range"))
            }
            ArraysOp::DeepToString => {
                // The rows are arrays of `elem`; a row that is null prints as `null`, which the
                // inner call does for `None`.
                let row = elem;
                let inner = element(row);
                let show = self.show_closure(inner);
                let a = self.optional_handle(&args[0]);
                let row_arg = if self.nullable_element(row) { "row.as_ref()" } else { "Some(&row)" };
                let deep = self.runtime("deep_to_string");
                format!("{deep}({a}, |row| {deep}({row_arg}, {show}))")
            }
            ArraysOp::Clone => {
                let a = self.handle(&args[0]);
                format!("{}({a})", self.runtime("clone_array"))
            }
        };
        Code::atom(text)
    }

    /// An array argument that may be null, as an `Option` of a borrowed handle: `None` for the
    /// literal `null`.
    fn optional_handle(&mut self, e: &'a Expr) -> String {
        if e.ty == Type::Null {
            return "None".to_string();
        }
        if self.nullable(e) {
            return format!("{}.as_ref()", self.object_place(e));
        }
        format!("Some({})", self.handle(e))
    }

    /// `a == b` and `a != b` on arrays: whether they are the same array. Arrays are shared
    /// `Rc`s, and `Rc`'s `==` compares contents, so identity is `Rc::ptr_eq`, or `java::same`
    /// where either side may be null.
    pub(super) fn array_identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let same = if self.nullable(l) || self.nullable(r) {
            let (a, b) = (self.optional_handle(l), self.optional_handle(r));
            format!("{}({a}, {b})", self.runtime("same"))
        } else {
            let (a, b) = (self.handle(l), self.handle(r));
            let pointer = if self.shared_array_type(l.ty) { "Arc" } else { "Rc" };
            format!("{pointer}::ptr_eq({a}, {b})")
        };
        if op == BinOp::Ne { Code::new(format!("!{same}"), UNARY) } else { Code::atom(same) }
    }

    /// The closure that shows one element of an array, as `Arrays.toString` and `deepToString`
    /// print it.
    pub(super) fn show_closure(&mut self, elem: Type) -> String {
        match elem {
            Type::String if super::shared_strings() && self.nullable_element(elem) => {
                "|v| v.map_or_else(|| String::from(\"null\"), |v| v.text())".to_string()
            }
            Type::String if super::shared_strings() => "|v| v.text()".to_string(),
            Type::String if self.nullable_element(elem) => {
                "|v| v.unwrap_or_else(|| String::from(\"null\"))".to_string()
            }
            Type::String => "|v| v".to_string(),
            _ => format!("|v| {}.to_string()", self.shown(elem, "v".to_string())),
        }
    }
}
