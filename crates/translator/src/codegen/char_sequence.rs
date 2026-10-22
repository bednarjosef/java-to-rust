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

    /// The text of `s` in `StringBuilder.append(s, start, end)`, as a `&str`, for a `s` that holds
    /// only strings and builders (see `plain_sequence`). A null `s` is `"null"`, as Java's
    /// `append` makes it.
    pub(super) fn range_text(&mut self, e: &'a Expr) -> String {
        match e.ty {
            Type::String | Type::StringBuilder if !self.nullable(e) => self.text_ref(e),
            _ => format!("&{}", self.text_of(e)),
        }
    }

    /// `sb.append(s, start, end)` for the operation `id`, where `s` is a `CharSequence` with a
    /// program class among its variants. Java evaluates `s` before the range, and then asks `s`
    /// for its `length` and its `charAt` of each index, which are the enum's own methods; the
    /// runtime does the rest (`append_seq_range`). What those methods throw goes on from here, as
    /// from a call (`Class::callees` names them for the analyses), and so does the range's failure
    /// where a `catch` or a `finally` may see it. A null `s` appends a range of "null", as Java's
    /// `append` makes it.
    pub(super) fn program_range_call(&mut self, id: ExprId, sb: &str, s: &'a Expr, range: &str) -> Code {
        let t = self.type_of(s.ty).expect("a CharSequence has an enum");
        let sequence =
            self.class.type_index(CHAR_SEQUENCE).expect("a program with a CharSequence has its type");
        let length = self.char_sequence_method(sequence, "length", 0);
        let char_at = self.char_sequence_method(sequence, "charAt", 1);
        let throws = [length, char_at].map(|m| self.facts.throwing.contains(&m));
        let (seq, index) = (self.temp("seq"), self.temp("index"));
        let length_call = format!("{seq}.{}()", self.dispatch_name(t, length));
        let char_call = format!("{seq}.{}({index})", self.dispatch_name(t, char_at));
        let length_call = if throws[0] { length_call } else { format!("Ok({length_call})") };
        let char_call = if throws[1] { char_call } else { format!("Ok({char_call})") };
        let error =
            if throws.contains(&true) { self.throwable_type() } else { "java::Exception".to_string() };
        let args = format!("{range}, {length_call}, |{index}| {char_call}");
        let call = if self.facts.raising.contains(&id) || throws.contains(&true) {
            let call = format!("{sb}.try_append_seq_range::<{error}>({args})");
            Code::atom(self.passed_on(Code::atom(call)))
        } else {
            Code::atom(format!("{sb}.append_seq_range::<{error}>({args})"))
        };
        if self.nullable(s) {
            let option = self.expr(s);
            let null = self.append_range_call(id, sb, &format!("\"null\", {range}"));
            return Code::atom(format!(
                "match &{} {{ Some({seq}) => {}, None => {} }}",
                option.at(ATOM),
                call.text,
                null.text
            ));
        }
        let obj = self.object(s);
        Code::atom(format!("{{ let {seq} = &{obj}; {} }}", call.text))
    }

    /// The index of the method `name` with `params` parameters of the `CharSequence` at `sequence`.
    fn char_sequence_method(&self, sequence: usize, name: &str, params: usize) -> usize {
        self.class
            .methods
            .iter()
            .position(|m| m.class == sequence && m.name == name && !m.is_static && m.params.len() == params)
            .expect("`CharSequence` declares the method")
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
    pub(super) fn plain_sequence(&self, e: &Expr) -> bool {
        self.type_of(e.ty).is_some_and(|t| {
            self.enum_variants(t)
                .into_iter()
                .all(|c| matches!(self.runtime_of(c), Some(Type::String | Type::StringBuilder)))
        })
    }
}
