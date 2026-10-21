//! The methods of `String` and `StringBuilder`, `String.format`, and the bit and radix methods of
//! `Integer` and `Long`, as calls of the runtime's versions of them (`java::substring`,
//! `java::StringBuilder`, `java::format`, ...).
//!
//! A builder is a `java::StringBuilder`, a shared handle whose methods take `&self` and return
//! `&Self` for chaining. A chain is written as Rust chains it, `sb.append_str("a").append(1)`; only
//! where the builder is used as a value is the handle cloned, which names the same builder.

use super::*;

impl<'a> Gen<'a> {
    /// A string, or any `CharSequence`, as a `&str` that a runtime function takes: null throws
    /// `NullPointerException`, and a builder gives its current text.
    pub(super) fn text_ref(&mut self, e: &'a Expr) -> String {
        match e.ty {
            Type::StringBuilder => format!("&{}.to_string()", self.builder_recv(e)),
            _ if self.nullable(e) => self.deref(e),
            _ => self.str_ref(e),
        }
    }

    /// A string as the receiver of a method of `str`.
    fn str_recv(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            _ if self.nullable(e) => self.deref(e),
            ExprKind::Str(s) => rust_string(s),
            ExprKind::Var(name) => self.local(name),
            ExprKind::Field { object, .. } if self.field_in_place(object) => self.object_place(e),
            _ => self.expr(e).at(ATOM),
        }
    }

    fn int_arg(&mut self, e: &'a Expr) -> String {
        self.value(e, Type::Int).text
    }

    fn char_arg(&mut self, e: &'a Expr) -> String {
        self.value(e, Type::Char).typed()
    }

    pub(super) fn string_builtin(&mut self, op: StringOp, args: &'a [Expr], id: ExprId) -> Code {
        let call = |g: &mut Self, f: &str, parts: Vec<String>| {
            Code::atom(format!("{}({})", g.runtime(f), parts.join(", ")))
        };
        let char_like =
            |e: &Expr| matches!(e.ty.unboxed(), Type::Char | Type::Int | Type::Short | Type::Byte);
        match op {
            StringOp::Substring => {
                let s = self.text_ref(&args[0]);
                let begin = self.int_arg(&args[1]);
                match args.get(2) {
                    None => self.may_fail(id, "substring_from", "", &format!("{s}, {begin}")),
                    Some(end) => {
                        let end = self.int_arg(end);
                        self.may_fail(id, "substring", "", &format!("{s}, {begin}, {end}"))
                    }
                }
            }
            StringOp::IndexOf | StringOp::LastIndexOf => {
                let first = op == StringOp::IndexOf;
                let s = self.text_ref(&args[0]);
                let (target, f) = if char_like(&args[1]) {
                    let v = self.value(&args[1], Type::Int).text;
                    (v, if first { "index_of_char" } else { "last_index_of_char" })
                } else {
                    (self.text_ref(&args[1]), if first { "index_of" } else { "last_index_of" })
                };
                let from = match args.get(2) {
                    Some(from) => self.int_arg(from),
                    None if first => "0".to_string(),
                    None => "i32::MAX".to_string(),
                };
                call(self, f, vec![s, target, from])
            }
            StringOp::Contains | StringOp::EndsWith | StringOp::IsEmpty => {
                let s = self.str_recv(&args[0]);
                let method = match op {
                    StringOp::Contains => "contains",
                    StringOp::EndsWith => "ends_with",
                    _ => "is_empty",
                };
                let arg = match args.get(1) {
                    Some(a) => self.text_ref(a),
                    None => String::new(),
                };
                Code::atom(format!("{s}.{method}({arg})"))
            }
            StringOp::StartsWith if args.len() == 2 => {
                let s = self.str_recv(&args[0]);
                let prefix = self.text_ref(&args[1]);
                Code::atom(format!("{s}.starts_with({prefix})"))
            }
            StringOp::StartsWith => {
                let s = self.text_ref(&args[0]);
                let prefix = self.text_ref(&args[1]);
                let offset = self.int_arg(&args[2]);
                call(self, "starts_with_at", vec![s, prefix, offset])
            }
            StringOp::ToUpperCase | StringOp::ToLowerCase if args.len() == 2 => {
                let f = if op == StringOp::ToUpperCase { "uppercase_in" } else { "lowercase_in" };
                let s = self.text_ref(&args[0]);
                let language = self.text_ref(&args[1]);
                call(self, f, vec![s, language])
            }
            StringOp::IsBlank
            | StringOp::Trim
            | StringOp::Strip
            | StringOp::StripLeading
            | StringOp::StripTrailing
            | StringOp::ToUpperCase
            | StringOp::ToLowerCase
            | StringOp::HashCode => {
                let f = match op {
                    StringOp::IsBlank => "is_blank",
                    StringOp::Trim => "trim",
                    StringOp::Strip => "strip",
                    StringOp::StripLeading => "strip_leading",
                    StringOp::StripTrailing => "strip_trailing",
                    StringOp::ToUpperCase => "uppercase",
                    StringOp::ToLowerCase => "lowercase",
                    _ => "string_hash",
                };
                let s = self.text_ref(&args[0]);
                call(self, f, vec![s])
            }
            StringOp::Replace if args[1].ty == Type::Char => {
                let s = self.text_ref(&args[0]);
                let (old, new) = (self.char_arg(&args[1]), self.char_arg(&args[2]));
                call(self, "replace_char", vec![s, old, new])
            }
            StringOp::Replace
            | StringOp::ReplaceAll
            | StringOp::ReplaceFirst
            | StringOp::Matches
            | StringOp::Repeat
            | StringOp::CompareTo
            | StringOp::CompareToIgnoreCase => {
                let f = match op {
                    StringOp::Replace => "replace",
                    StringOp::ReplaceAll => "replace_all",
                    StringOp::ReplaceFirst => "replace_first",
                    StringOp::Matches => "matches_all",
                    StringOp::Repeat => "repeat",
                    StringOp::CompareTo => "compare_to",
                    _ => "compare_to_ignore_case",
                };
                let parts = args
                    .iter()
                    .map(|a| if a.ty.unboxed() == Type::Int { self.int_arg(a) } else { self.text_ref(a) })
                    .collect();
                call(self, f, parts)
            }
            StringOp::Split => {
                let s = self.text_ref(&args[0]);
                let regex = self.text_ref(&args[1]);
                let limit = match args.get(2) {
                    Some(limit) => self.int_arg(limit),
                    None => "0".to_string(),
                };
                let parts = call(self, "split", vec![s, regex, limit]).text;
                let parts = if self.nullable_element(Type::String) {
                    format!("{parts}.into_iter().map(Some).collect()")
                } else {
                    parts
                };
                Code::atom(format!("{}({parts})", self.array_maker("from_vec")))
            }
            StringOp::Join => self.join(args),
            StringOp::EqualsIgnoreCase => {
                let s = self.text_ref(&args[0]);
                let f = self.runtime("equals_ignore_case");
                if self.nullable(&args[1]) {
                    let other = self.option_ref(&args[1]);
                    Code::atom(format!("{other}.is_some_and(|o| {f}({s}, o))"))
                } else {
                    let other = self.str_ref(&args[1]);
                    Code::atom(format!("{f}({s}, {other})"))
                }
            }
            StringOp::Concat => {
                let (a, b) = (self.text_ref(&args[0]), self.text_ref(&args[1]));
                Code::atom(format!("[{a}, {b}].concat()"))
            }
            StringOp::ToCharArray => {
                let s = self.text_ref(&args[0]);
                let units = call(self, "units", vec![s]).text;
                Code::atom(format!("{}({units})", self.array_maker("from_vec")))
            }
            StringOp::GetChars => {
                let s = self.text_ref(&args[0]);
                let [begin, end, dst_begin] = self.hoisted_ints(args, [1, 2, 4]);
                let dst = self.array_arg(&args[3], true);
                self.may_fail(id, "get_chars", "", &format!("{s}, {begin}, {end}, {dst}, {dst_begin}"))
            }
            StringOp::CharCodes => {
                let s = self.text_ref(&args[0]);
                let units = call(self, "units", vec![s]).text;
                Code::atom(format!(
                    "{}({units}.into_iter().map(i32::from).collect())",
                    self.runtime("from_vec")
                ))
            }
            StringOp::JoinRange => {
                let texts: Vec<String> = args[..3].iter().map(|a| self.text_ref(a)).collect();
                let elements = self.slice_of(&args[3]);
                let size = self.int_arg(&args[4]);
                let f = self.runtime("join_range");
                Code::atom(format!("{f}({}, {elements}, {size})", texts.join(", ")))
            }
            StringOp::Lines => {
                let s = self.text_ref(&args[0]);
                let lines = call(self, "lines", vec![s]).text;
                let object = self.class.type_index(OBJECT).expect("lowering adds `Object` for `lines`");
                let string = self.class.type_of(Type::String).expect("a runtime class of `Object`");
                let variant = self.variant(object, string);
                let line = if self.nullable_element(Type::class(OBJECT)) {
                    format!("Some({variant}(line))")
                } else {
                    format!("{variant}(line)")
                };
                Code::atom(format!(
                    "{}({lines}.into_iter().map(|line| {line}).collect())",
                    self.array_maker("from_vec")
                ))
            }
            StringOp::FromChars => {
                let chars = self.slice_of(&args[0]);
                match args.get(1..) {
                    Some([offset, count]) => {
                        let (offset, count) = (self.int_arg(offset), self.int_arg(count));
                        self.may_fail(id, "from_chars_range", "", &format!("{chars}, {offset}, {count}"))
                    }
                    _ => call(self, "from_units", vec![chars]),
                }
            }
            StringOp::FromCodePoints | StringOp::FromHighByte => {
                let mut values = vec![self.slice_of(&args[0])];
                values.extend(args[1..].iter().map(|x| self.int_arg(x)));
                let name = if op == StringOp::FromCodePoints { "from_code_points" } else { "from_high_byte" };
                call(self, name, values)
            }
            StringOp::GetBytes => {
                let s = self.text_ref(&args[0]);
                let charset = match args.get(1) {
                    Some(charset) => self.charset_arg(charset),
                    None => format!("{}::Utf8", self.runtime("Charset")),
                };
                let bytes = call(self, "get_bytes", vec![s, charset]).text;
                Code::atom(self.made_array(bytes))
            }
            StringOp::FromBytes => {
                let bytes = self.slice_of(&args[0]);
                match args.get(1..) {
                    Some([offset, length, rest @ ..]) if rest.len() <= 1 => {
                        let (offset, length) = (self.int_arg(offset), self.int_arg(length));
                        // Without a charset, the default one, which is UTF-8.
                        let charset = match rest {
                            [charset] => self.charset_arg(charset),
                            _ => format!("{}::Utf8", self.runtime("Charset")),
                        };
                        call(self, "new_string_range", vec![bytes, offset, length, charset])
                    }
                    Some([charset]) => {
                        let charset = self.charset_arg(charset);
                        call(self, "new_string", vec![bytes, charset])
                    }
                    _ => {
                        let charset = format!("{}::Utf8", self.runtime("Charset"));
                        call(self, "new_string", vec![bytes, charset])
                    }
                }
            }
            StringOp::Identity if self.nullable(&args[0]) => {
                Code::atom(format!("{}.to_string()", self.deref(&args[0])))
            }
            StringOp::Identity => self.expr(&args[0]),
            StringOp::EqualsOther => {
                // Never equal; the receiver still throws when it is null, after the argument runs.
                let (receiver, other) = (&args[0], &args[1]);
                let mut steps = Vec::new();
                let receiver_null = self.nullable(receiver);
                if receiver_null {
                    let option = self.option_ref(receiver);
                    steps.push(format!("let receiver_ = {option};"));
                } else if has_effects(receiver) {
                    steps.push(format!("{};", self.expr(receiver).text));
                }
                match &other.kind {
                    // A variable is still used, or Rust would warn that it is not.
                    ExprKind::Var(name) => steps.push(format!("let _ = &{};", self.local(name))),
                    _ if has_effects(other) => steps.push(format!("{};", self.expr(other).text)),
                    _ => {}
                }
                if receiver_null {
                    let checked = self.dereference(&[receiver.id], "receiver_");
                    steps.push(format!("{checked};"));
                }
                if steps.is_empty() {
                    Code::atom("false")
                } else {
                    Code::atom(format!("{{ {} false }}", steps.join(" ")))
                }
            }
        }
    }

    /// `String.join(delimiter, ...)` of an array or of separate arguments.
    fn join(&mut self, args: &'a [Expr]) -> Code {
        let delimiter = self.text_ref(&args[0]);
        let f = self.runtime("join");
        if args.len() == 2 && matches!(args[1].ty, Type::Array(_)) {
            let elements = self.slice_of(&args[1]);
            return Code::atom(format!("{f}({delimiter}, {elements})"));
        }
        let rest = &args[1..];
        let optional = rest.iter().any(|a| self.nullable(a));
        let items: Vec<String> = rest
            .iter()
            .map(|a| match a.ty {
                Type::StringBuilder if optional && self.nullable(a) => {
                    let option = self.builder_option(a);
                    format!("{option}.map(|b| b.to_string()).as_deref()")
                }
                _ if optional && self.nullable(a) => self.option_ref(a),
                _ if optional => format!("Some({})", self.text_ref(a)),
                _ => self.text_ref(a),
            })
            .collect();
        if items.is_empty() {
            return Code::atom(format!("{f}({delimiter}, &[] as &[&str])"));
        }
        Code::atom(format!("{f}({delimiter}, &[{}])", items.join(", ")))
    }

    /// A builder that may be null as an `Option<&java::StringBuilder>`.
    pub(super) fn builder_option(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Null => format!("None::<&{}>", self.runtime("StringBuilder")),
            _ => format!("{}.as_ref()", self.object_place(e)),
        }
    }

    /// A builder as the receiver of one of its methods, without a clone: null throws
    /// `NullPointerException`, and a chained call is the `&Self` it returns.
    pub(super) fn builder_recv(&mut self, e: &'a Expr) -> String {
        match &e.kind {
            ExprKind::Builtin(Builtin::Builder(op), args) if op.chains() => self.builder_call(*op, args).text,
            _ if self.nullable(e) => {
                let option = self.builder_option(e);
                self.dereference(&[e.id], &option)
            }
            ExprKind::Var(name) => self.local(name),
            ExprKind::Field { .. } | ExprKind::Index(..) => {
                let place = self.object_place(e);
                if place.starts_with('{') { format!("({place})") } else { place }
            }
            _ => self.expr(e).at(ATOM),
        }
    }

    /// `a == b` and `a != b` on builders: whether they are the same builder.
    pub(super) fn builder_identity(&mut self, op: BinOp, l: &'a Expr, r: &'a Expr) -> Code {
        let same = if self.nullable(l) || self.nullable(r) {
            let (a, b) = (self.some_builder(l), self.some_builder(r));
            format!("{}({a}, {b})", self.runtime("same_builder"))
        } else {
            let (a, b) = (self.builder_recv(l), self.builder_recv(r));
            format!("{a}.same(&{b})")
        };
        if op == BinOp::Ne { Code::new(format!("!{same}"), UNARY) } else { Code::atom(same) }
    }

    fn some_builder(&mut self, e: &'a Expr) -> String {
        if self.nullable(e) || matches!(e.kind, ExprKind::Null) {
            self.builder_option(e)
        } else {
            format!("Some(&{})", self.builder_recv(e))
        }
    }

    /// A builder method as it is called; a chaining method gives the `&Self` it returns, which
    /// `builtin` clones where the builder is used as a value.
    pub(super) fn builder_call(&mut self, op: BuilderOp, args: &'a [Expr]) -> Code {
        let ty = self.runtime("StringBuilder");
        if op == BuilderOp::New {
            return Code::atom(match args.first() {
                None => format!("{ty}::new()"),
                Some(n) if n.ty == Type::Int => format!("{ty}::with_capacity({})", self.int_arg(n)),
                Some(s) => format!("{ty}::from_text({})", self.text_ref(s)),
            });
        }
        let sb = self.builder_recv(&args[0]);
        let rest = &args[1..];
        let method =
            |name: &str, parts: Vec<String>| Code::atom(format!("{sb}.{name}({})", parts.join(", ")));
        match op {
            BuilderOp::Append if rest.len() == 1 => {
                let (name, value) = self.appended("append", &rest[0]);
                method(&name, vec![value])
            }
            BuilderOp::Append => {
                let (start, end) = (self.int_arg(&rest[1]), self.int_arg(&rest[2]));
                if matches!(rest[0].ty, Type::Array(_)) {
                    let chars = self.slice_of(&rest[0]);
                    method("append_chars_range", vec![chars, start, end])
                } else {
                    let text = self.text_ref(&rest[0]);
                    method("append_range", vec![text, start, end])
                }
            }
            BuilderOp::Insert => {
                let offset = self.int_arg(&rest[0]);
                let (name, value) = self.appended("insert", &rest[1]);
                method(&name, vec![offset, value])
            }
            BuilderOp::Reverse => method("reverse", vec![]),
            BuilderOp::Length => method("length", vec![]),
            BuilderOp::IsEmpty => method("is_empty", vec![]),
            BuilderOp::ToString => method("to_string", vec![]),
            BuilderOp::CharAt | BuilderOp::DeleteCharAt | BuilderOp::SetLength => {
                let name = match op {
                    BuilderOp::CharAt => "char_at",
                    BuilderOp::DeleteCharAt => "delete_char_at",
                    _ => "set_length",
                };
                let i = self.int_arg(&rest[0]);
                method(name, vec![i])
            }
            BuilderOp::SetCharAt => {
                let (i, c) = (self.int_arg(&rest[0]), self.char_arg(&rest[1]));
                method("set_char_at", vec![i, c])
            }
            BuilderOp::Delete => {
                let (start, end) = (self.int_arg(&rest[0]), self.int_arg(&rest[1]));
                method("delete", vec![start, end])
            }
            BuilderOp::Replace => {
                let (start, end) = (self.int_arg(&rest[0]), self.int_arg(&rest[1]));
                let text = self.text_ref(&rest[2]);
                method("replace", vec![start, end, text])
            }
            BuilderOp::IndexOf | BuilderOp::LastIndexOf => {
                let text = self.text_ref(&rest[0]);
                let from = match rest.get(1) {
                    Some(from) => self.int_arg(from),
                    None if op == BuilderOp::IndexOf => "0".to_string(),
                    None => "i32::MAX".to_string(),
                };
                method(if op == BuilderOp::IndexOf { "index_of" } else { "last_index_of" }, vec![text, from])
            }
            BuilderOp::Substring => {
                let start = self.int_arg(&rest[0]);
                match rest.get(1) {
                    None => method("substring_from", vec![start]),
                    Some(end) => {
                        let end = self.int_arg(end);
                        method("substring", vec![start, end])
                    }
                }
            }
            BuilderOp::CompareTo => {
                let other = self.builder_recv(&rest[0]);
                method("compare_to", vec![format!("&{other}")])
            }
            BuilderOp::New => unreachable!("handled above"),
        }
    }

    /// The method (`append_str`, `append_char`, ...) of the `append` or `insert` family named
    /// `base` that adds `x` as Java shows it, and `x` as its argument.
    fn appended(&mut self, base: &str, x: &'a Expr) -> (String, String) {
        let suffixed = |s: &str| format!("{base}_{s}");
        match x.ty {
            Type::Null => (suffixed("str"), "\"null\"".to_string()),
            Type::String if self.nullable(x) => {
                let option = self.option_ref(x);
                (suffixed("str"), format!("{option}.unwrap_or(\"null\")"))
            }
            Type::String => (suffixed("str"), self.str_ref(x)),
            Type::Char => (suffixed("char"), self.char_arg(x)),
            Type::Array(_) => (suffixed("chars"), self.slice_of(x)),
            Type::StringBuilder if self.nullable(x) => {
                let option = self.builder_option(x);
                let text = format!("&{}({option}).to_string()", self.runtime("or_null"));
                (suffixed("str"), text)
            }
            Type::StringBuilder => {
                let other = self.builder_recv(x);
                (suffixed("builder"), format!("&{other}"))
            }
            Type::Boxed(kind) if self.nullable(x) => (base.to_string(), self.boxed_text(x, *kind)),
            Type::Boxed(kind) => {
                let value = self.value(x, *kind).text;
                (base.to_string(), self.shown(*kind, value))
            }
            Type::Double | Type::Float => {
                let value = self.expr(x).text;
                (base.to_string(), self.shown(x.ty, value))
            }
            // Enums show their names; numbers and booleans show as Rust shows them.
            _ => (base.to_string(), self.expr(x).generic_arg()),
        }
    }

    /// `String.format(format, args...)` as `java::format`, each argument as a `java::Arg`.
    pub(super) fn format_builtin(&mut self, args: &'a [Expr]) -> Code {
        let format = self.text_ref(&args[0]);
        let values: Vec<String> = args[1..].iter().map(|a| self.format_arg(a)).collect();
        let f = self.runtime("format");
        Code::atom(format!("{f}({format}, &[{}])", values.join(", ")))
    }

    fn format_arg(&mut self, e: &'a Expr) -> String {
        let arg = self.runtime("Arg");
        let variant = |ty: Type| match ty {
            Type::Boolean => "Bool",
            Type::Char => "Char",
            Type::Byte => "Byte",
            Type::Short => "Short",
            Type::Int => "Int",
            Type::Long => "Long",
            Type::Float => "Float",
            _ => "Double",
        };
        match e.ty {
            Type::Null => format!("{arg}::Null"),
            Type::String if self.nullable(e) => {
                let option = self.option_ref(e);
                format!("{option}.map_or({arg}::Null, {arg}::Str)")
            }
            Type::String => format!("{arg}::Str({})", self.str_ref(e)),
            Type::Boxed(kind) if self.nullable(e) => {
                let option = self.expr(e).at(ATOM);
                let value = self.unbox_code(Code::atom("v_"), *kind, false).text;
                if value == "v_" {
                    format!("{option}.map_or({arg}::Null, {arg}::{})", variant(*kind))
                } else {
                    format!("{option}.map_or({arg}::Null, |v_| {arg}::{}({value}))", variant(*kind))
                }
            }
            ty if ty.unboxed().is_numeric() || ty.unboxed() == Type::Boolean => {
                let kind = ty.unboxed();
                let value = self.value(e, kind).text;
                format!("{arg}::{}({value})", variant(kind))
            }
            // An object, enum or builder shows the text of its `toString`.
            _ => format!("{arg}::Text(&{})", self.text_of(e)),
        }
    }

    /// `e` as a `String` of the text that concatenating it shows.
    fn text_of(&mut self, e: &'a Expr) -> String {
        let mut parts = Vec::new();
        self.parts(e, &mut parts);
        match parts.as_slice() {
            [Part::Arg(code)] if code.text.ends_with(".to_string()") => code.text.clone(),
            [Part::Arg(code)] => format!("{}.to_string()", code.receiver()),
            _ => self.format("format!", parts).text,
        }
    }

    /// A bit or radix method of `Integer` (`kind` is `int`) or `Long`.
    pub(super) fn bits_builtin(&mut self, op: BitsOp, kind: Type, args: &'a [Expr], id: ExprId) -> Code {
        let bits = if kind == Type::Long { 64 } else { 32 };
        let rust = rust_type(kind);
        if op == BitsOp::ParseRadix {
            let s = if self.nullable(&args[0]) {
                self.option_ref(&args[0])
            } else {
                format!("Some({})", self.str_ref(&args[0]))
            };
            let radix = self.int_arg(&args[1]);
            return self.may_fail(id, "parse_radix", &format!("::<{rust}>"), &format!("{s}, {radix}"));
        }
        if op == BitsOp::ParseRange {
            return self.parse_range_builtin(kind, args, id);
        }
        let v = self.value(&args[0], kind);
        let recv = self.recv(&v, kind);
        let format = |spec: &str| {
            let text = if v.literal.is_some() { v.typed() } else { v.text.clone() };
            Code::atom(format!("format!(\"{{:{spec}}}\", {text})"))
        };
        let int = |text: String| Code::new(format!("{text} as i32"), AS);
        // The value as an `i64`, which the runtime's functions take for both kinds.
        let wide = match v.literal {
            _ if kind == Type::Long => v.text.clone(),
            Some(Literal::Int(n)) => n.to_string(),
            _ => format!("{} as i64", v.cast_operand()),
        };
        match op {
            BitsOp::ToHexString => format("x"),
            BitsOp::ToBinaryString => format("b"),
            BitsOp::ToOctalString => format("o"),
            BitsOp::NumberOfLeadingZeros => int(format!("{recv}.leading_zeros()")),
            BitsOp::NumberOfTrailingZeros => int(format!("{recv}.trailing_zeros()")),
            BitsOp::BitCount => int(format!("{recv}.count_ones()")),
            BitsOp::Signum if kind == Type::Long => int(format!("{recv}.signum()")),
            BitsOp::Signum => Code::atom(format!("{recv}.signum()")),
            BitsOp::Reverse => Code::atom(format!("{recv}.reverse_bits()")),
            BitsOp::ReverseBytes => Code::atom(format!("{recv}.swap_bytes()")),
            BitsOp::LowestOneBit if is_identifier(&v.text) => {
                Code::new(format!("{0} & {0}.wrapping_neg()", v.text), BIT_AND)
            }
            BitsOp::LowestOneBit => {
                Code::atom(format!("{{ let v: {rust} = {}; v & v.wrapping_neg() }}", v.text))
            }
            BitsOp::HighestOneBit => {
                let f = self.runtime("highest_one_bit");
                let text = format!("{f}({wide}, {bits})");
                if kind == Type::Long { Code::atom(text) } else { Code::new(format!("{text} as i32"), AS) }
            }
            BitsOp::RotateLeft | BitsOp::RotateRight => {
                let distance = self.value(&args[1], Type::Int);
                let name = if op == BitsOp::RotateLeft { "rotate_left" } else { "rotate_right" };
                Code::atom(format!("{recv}.{name}({} as u32)", distance.cast_operand()))
            }
            BitsOp::ToStringRadix => {
                let radix = self.int_arg(&args[1]);
                let f = self.runtime("to_string_radix");
                Code::atom(format!("{f}({wide}, {radix})"))
            }
            BitsOp::ParseRadix | BitsOp::ParseRange => unreachable!("handled above"),
        }
    }
}
