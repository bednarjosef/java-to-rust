//! The text that `append(s, start, end)` takes a range of: Java checks the range on `s.length()`
//! and appends `s.subSequence(start, end)`, so the text must be what those two give for `s`.

use super::*;

impl<'a> Gen<'a> {
    /// `sb.append_range(text, start, end)` for the operation `id`: where a `catch` or a `finally`
    /// may see its `IndexOutOfBoundsException` (`Facts::raising`), the checked form, whose failure
    /// goes on from here as a thrown exception does.
    pub(super) fn append_range_call(&mut self, id: ExprId, sb: &str, args: &str) -> Code {
        if self.facts.raising.contains(&id) {
            return Code::atom(self.passed_on(Code::atom(format!("{sb}.try_append_range({args})"))));
        }
        Code::atom(format!("{sb}.append_range({args})"))
    }

    /// The text of `s` in `StringBuilder.append(s, start, end)`, as a `&str`. A null `s` is
    /// `"null"`, as Java's `append` makes it. A `CharSequence` whose variants include a program
    /// class is rejected: the class's `length` and `charAt` need not agree with its `toString`.
    pub(super) fn range_text(&mut self, e: &'a Expr) -> String {
        match e.ty {
            Type::String | Type::StringBuilder if !self.nullable(e) => self.text_ref(e),
            Type::Class(..) => {
                let message = "appending a range of a `CharSequence` that a program class implements";
                if !self.plain_sequence(e) {
                    self.rejected.push(Error { pos: e.pos, message: message.to_string() });
                    return "\"\"".to_string();
                }
                format!("&{}", self.text_of(e))
            }
            _ => format!("&{}", self.text_of(e)),
        }
    }

    /// The text that `new StringBuilder(seq)` copies, as a `&str`. Java's constructor asks `seq`
    /// for its length, so a null `seq` throws `NullPointerException` rather than giving "null".
    /// A `CharSequence` with a program class among its variants is rejected, as `range_text` does.
    pub(super) fn copied_text(&mut self, e: &'a Expr) -> String {
        if !self.plain_sequence(e) {
            let message = "creating a builder from a `CharSequence` that a program class implements";
            self.rejected.push(Error { pos: e.pos, message: message.to_string() });
            return "\"\"".to_string();
        }
        let t = self.type_of(e.ty).expect("a CharSequence has an enum");
        self.need(Need::Display(t));
        let obj = self.object(e);
        format!("&{obj}.to_string()")
    }

    /// Whether the `CharSequence` `e` holds only strings and builders: no program class is one of
    /// its variants, so its text is the text of the string or builder it holds.
    fn plain_sequence(&self, e: &Expr) -> bool {
        self.type_of(e.ty).is_some_and(|t| {
            self.enum_variants(t)
                .into_iter()
                .all(|c| matches!(self.runtime_of(c), Some(Type::String | Type::StringBuilder)))
        })
    }
}
