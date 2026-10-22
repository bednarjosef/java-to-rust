//! Lowering of the methods of `String` and `StringBuilder`, `String.format` and `printf`, and the
//! bit and radix methods of `Integer` and `Long`. Each becomes a `Builtin` whose arguments are the
//! call's with the receiver first; the overload is told apart later by the arguments' types.
//! Calls the runtime cannot do exactly are rejected here: a regular expression or format that
//! is a constant is checked against what the runtime supports.

use super::*;

/// Whether `ty` is a `CharSequence` that the translation knows: a string or a builder (or `null`).
fn char_sequence(ty: Type) -> bool {
    matches!(ty, Type::String | Type::StringBuilder | Type::Null)
}

/// Whether a value of type `ty` can be turned into text the way `String.valueOf(Object)` does.
fn textual(ty: Type) -> bool {
    !matches!(ty, Type::Array(_) | Type::Void)
}

impl<'t> Lowering<'t> {
    /// The text of a constant string argument: a literal, or a constant variable.
    pub(super) fn constant_text(&self, node: Option<Node<'t>>) -> Option<String> {
        let node = node?;
        match node.kind() {
            "STRING_LITERAL" => node.0["value"].as_str().map(str::to_string),
            "PARENTHESIZED" => self.constant_text(node.get("expression")),
            "IDENTIFIER" | "MEMBER_SELECT" if node.java_type() == "java.lang.String" => {
                node.symbol().and_then(|s| s.constant).map(str::to_string)
            }
            _ => None,
        }
    }

    /// `x` as `"" + x`, which shows it as `String.valueOf` does.
    pub(super) fn as_text(&self, x: Expr) -> Expr {
        let empty = self.synth(ExprKind::Str(String::new()), x.pos, Type::String);
        let pos = x.pos;
        self.synth(ExprKind::Binary(BinOp::Add, Box::new(empty), Box::new(x)), pos, Type::String)
    }

    /// `s.chars()` and `s.lines()`: the JDK's methods over an array the runtime makes, of the
    /// string's UTF-16 code units or of its lines, which stream the same values with the same
    /// characteristics as the JDK's own spliterators. `chars()` is `Arrays.stream(units)`, which is
    /// `SIZED` as the JDK's is; `lines()` is not, so it is
    ///
    /// ```text
    /// StreamSupport.stream(Spliterators.spliteratorUnknownSize(
    ///     Spliterators.iterator(Spliterators.spliterator(lines, 0)), ORDERED | IMMUTABLE | NONNULL), false)
    /// ```
    fn string_stream(
        &self,
        node: Node<'t>,
        method: &str,
        receiver: Option<Node<'t>>,
    ) -> Result<ExprKind, Error> {
        let (_, calls) = crate::program::STRING_STREAMS
            .iter()
            .find(|(m, _)| *m == method)
            .expect("a method of `String` that streams");
        let mut resolved = Vec::new();
        for &(owner, name, sig) in calls.iter() {
            let Some(m) = self
                .methods
                .iter()
                .position(|m| self.types[m.owner].javac == owner && m.name == name && m.sig == sig)
            else {
                return error(
                    node.pos(),
                    format!(
                        "`String.{method}` streams with the JDK's code: translate the program with the \
                         JDK's sources (`--main` and `--library`) for it"
                    ),
                );
            };
            resolved.push(m);
        }
        let receiver = self.expr(receiver.expect("an instance method has a receiver"))?;
        let pos = receiver.pos;
        let (op, ty) = match method {
            "chars" => (StringOp::CharCodes, Type::array(Type::Int)),
            _ => (StringOp::Lines, Type::array(Type::class(OBJECT))),
        };
        let mut value = self.synth(ExprKind::Builtin(Builtin::String(op), vec![receiver]), pos, ty);
        // Spliterator.ORDERED | Spliterator.IMMUTABLE | Spliterator.NONNULL, as the JDK's lines.
        let extra = |name: &str| match name {
            "spliterator" => Some(ExprKind::Int(0)),
            "spliteratorUnknownSize" => Some(ExprKind::Int(0x0000_0010 | 0x0000_0400 | 0x0000_0100)),
            "stream" if method == "lines" => Some(ExprKind::Bool(false)),
            _ => None,
        };
        let last = resolved.len() - 1;
        for (i, m) in resolved.into_iter().enumerate() {
            let info = &self.methods[m];
            let mut args = vec![value];
            if let Some(kind) = extra(info.name) {
                let ty = if matches!(kind, ExprKind::Bool(_)) { Type::Boolean } else { Type::Int };
                args.push(self.synth(kind, pos, ty));
            }
            let kind = ExprKind::Call { name: info.name.to_string(), args, resolved: Some(m) };
            if i == last {
                return Ok(kind);
            }
            let ret = info.node.child("returnType")?;
            let ty = self.declared_type(ret, ret.java_type(), info.owner, Some(m))?;
            value = self.synth(kind, pos, ty);
        }
        unreachable!("a stream of a string has a last call")
    }

    /// `s.toUpperCase(locale)` or `s.toLowerCase(locale)`: the case mapping of the root locale,
    /// given the locale's language, `locale.getLanguage()`, as the JDK's tells the languages with
    /// their own (Turkish, Azeri and Lithuanian), which the runtime does not have and ends the
    /// program for (`java_runtime::string::uppercase_in`). The call throws on a null locale.
    fn case_in_locale(
        &self,
        node: Node<'t>,
        name: &str,
        receiver: Option<Node<'t>>,
        arg_nodes: &[Node<'t>],
    ) -> Result<ExprKind, Error> {
        let (Some(receiver), [locale]) = (receiver, arg_nodes) else {
            return unsupported(node, &format!("this `String.{name}`"));
        };
        let (class, method, signature) = crate::program::LOCALE_LANGUAGE;
        let (line, column) = (&locale.0["line"], &locale.0["column"]);
        let language = serde_json::json!({"kind": "METHOD_INVOCATION", "line": line, "column": column,
            "javaType": "java.lang.String", "arguments": [],
            "methodSelect": {"kind": "MEMBER_SELECT", "line": line, "column": column,
                "identifier": method, "expression": locale.0,
                "symbol": {"kind": "METHOD", "name": method, "owner": class, "static": false,
                    "type": signature}},
            "symbol": {"kind": "METHOD", "name": method, "owner": class, "static": false,
                "type": signature}});
        let language: &'t serde_json::Value = Box::leak(Box::new(language));
        // The receiver is evaluated before the argument, as in Java.
        let s = self.expr(receiver)?;
        let language = self.expr(Node(language))?;
        let op = if name == "toUpperCase" { StringOp::ToUpperCase } else { StringOp::ToLowerCase };
        Ok(ExprKind::Builtin(Builtin::String(op), vec![s, language]))
    }

    /// A call of a method of `String`. `args` are the lowered arguments, without the receiver.
    pub(super) fn string_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
        mut args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let name = symbol.name;
        let arg_nodes: Vec<Node<'t>> = node.list("arguments").collect();
        let op = match name {
            "substring" => StringOp::Substring,
            "indexOf" => StringOp::IndexOf,
            "lastIndexOf" => StringOp::LastIndexOf,
            "contains" => StringOp::Contains,
            "startsWith" => StringOp::StartsWith,
            "endsWith" => StringOp::EndsWith,
            "isEmpty" => StringOp::IsEmpty,
            "isBlank" => StringOp::IsBlank,
            "trim" => StringOp::Trim,
            "strip" => StringOp::Strip,
            "stripLeading" => StringOp::StripLeading,
            "stripTrailing" => StringOp::StripTrailing,
            "toUpperCase" if args.is_empty() => StringOp::ToUpperCase,
            "toLowerCase" if args.is_empty() => StringOp::ToLowerCase,
            "toUpperCase" | "toLowerCase" if symbol.ty == "(java.util.Locale)java.lang.String" => {
                return self.case_in_locale(node, name, receiver, &arg_nodes);
            }
            "replace" => StringOp::Replace,
            "replaceAll" => StringOp::ReplaceAll,
            "replaceFirst" => StringOp::ReplaceFirst,
            "matches" => StringOp::Matches,
            "split" => StringOp::Split,
            "join" => StringOp::Join,
            "repeat" => StringOp::Repeat,
            "compareTo" => StringOp::CompareTo,
            "compareToIgnoreCase" => StringOp::CompareToIgnoreCase,
            "equalsIgnoreCase" => StringOp::EqualsIgnoreCase,
            "concat" => StringOp::Concat,
            "toCharArray" => StringOp::ToCharArray,
            "getChars" if symbol.ty == "(int,int,char[],int)void" => StringOp::GetChars,
            "getBytes" if symbol.ty == "(int,int,byte[],int)void" => StringOp::GetLowBytes,
            "codePointAt" => StringOp::CodePointAt,
            "codePointBefore" => StringOp::CodePointBefore,
            "codePointCount" => StringOp::CodePointCount,
            "offsetByCodePoints" => StringOp::OffsetByCodePoints,
            "regionMatches" => StringOp::RegionMatches,
            "stripIndent" => StringOp::StripIndent,
            // A `CharSequence` is compared by its text, which its `toString()` gives.
            "contentEquals" if matches!(args[0].ty, Type::Class(..)) => {
                let x = args.pop().expect("one argument");
                let pos = x.pos;
                args.push(self.synth(ExprKind::Builtin(Builtin::ObjectText, vec![x]), pos, Type::String));
                StringOp::ContentEquals
            }
            "contentEquals" => StringOp::ContentEquals,
            "subSequence" => {
                let receiver = receiver.expect("an instance method has a receiver");
                args.insert(0, self.expr(receiver)?);
                return self.as_char_sequence(node, Builtin::String(StringOp::Substring), args);
            }
            "chars" | "lines" if args.is_empty() => return self.string_stream(node, name, receiver),
            "hashCode" => StringOp::HashCode,
            "toString" => StringOp::ToString,
            "intern" => StringOp::Intern,
            "equals" => StringOp::EqualsOther,
            "valueOf" | "copyValueOf" if args.first().is_some_and(|a| a.ty == Type::array(Type::Char)) => {
                StringOp::FromChars
            }
            "valueOf" if args.len() == 1 => {
                let x = args.pop().expect("one argument");
                // `String.valueOf(null)` is the `char[]` overload, which throws.
                if !textual(x.ty) || x.ty == Type::Null {
                    return unsupported(node, "`String.valueOf` of an array");
                }
                return Ok(self.as_text(x).kind);
            }
            "format" => return self.format_call(node, args, &arg_nodes, None),
            "formatted" => {
                let receiver = receiver.expect("an instance method has a receiver");
                let format = self.expr(receiver)?;
                return self.format_call(node, args, &arg_nodes, Some((format, receiver)));
            }
            _ => return unsupported(node, &format!("method `String.{name}`")),
        };
        match op {
            StringOp::ContentEquals if args[0].ty == Type::Null => {
                return unsupported(node, "`String.contentEquals(null)`");
            }
            StringOp::Contains | StringOp::Replace
                if args.iter().any(|a| !char_sequence(a.ty) && a.ty != Type::Char) =>
            {
                return unsupported(node, &format!("`String.{name}` of this kind of text"));
            }
            StringOp::Join => {
                let array_form = args.len() == 2 && args[1].ty == Type::array(Type::String);
                if args.len() == 2 && matches!(args[1].ty, Type::Class(..)) && args[0].ty == Type::String {
                    let mut args = args.into_iter();
                    let (delimiter, elements) =
                        (args.next().expect("two arguments"), args.next().expect("two arguments"));
                    return self.join_iterable(node, delimiter, elements);
                }
                if !array_form && args.iter().any(|a| !char_sequence(a.ty)) {
                    return unsupported(node, "`String.join` of anything but strings");
                }
            }
            StringOp::Split | StringOp::ReplaceAll | StringOp::ReplaceFirst | StringOp::Matches => {
                if let Some(pattern) = self.constant_text(arg_nodes.first().copied()) {
                    let checked = match (op, self.constant_text(arg_nodes.get(1).copied())) {
                        (StringOp::ReplaceAll | StringOp::ReplaceFirst, Some(r)) => {
                            java_runtime::regex::check_replacement(&pattern, &r)
                        }
                        _ => java_runtime::regex::check(&pattern),
                    };
                    if let Err(why) = checked {
                        return error(node.pos(), format!("{why} not supported yet"));
                    }
                }
            }
            StringOp::IndexOf | StringOp::LastIndexOf if args.len() > 2 => {
                return unsupported(node, &format!("`String.{name}` with an end index"));
            }
            _ => {}
        }
        if !symbol.is_static {
            let receiver = receiver.expect("an instance method has a receiver");
            args.insert(0, self.expr(receiver)?);
        }
        Ok(ExprKind::Builtin(Builtin::String(op), args))
    }

    /// `subSequence` of a string or a builder, `builtin` with `args`: its `substring`, as the
    /// `CharSequence` that the call's type is.
    fn as_char_sequence(&self, node: Node<'t>, builtin: Builtin, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let text = self.synth(ExprKind::Builtin(builtin, args), node.pos(), Type::String);
        let ty = self.ty(node, node.java_type())?;
        Ok(self.convert(text, ty).kind)
    }

    /// `String.format(format, args...)`, or `format.formatted(args...)` when `formatted` holds the
    /// receiver.
    fn format_call(
        &self,
        node: Node<'t>,
        mut args: Vec<Expr>,
        arg_nodes: &[Node<'t>],
        formatted: Option<(Expr, Node<'t>)>,
    ) -> Result<ExprKind, Error> {
        let format_node = match formatted {
            Some((format, receiver)) => {
                args.insert(0, format);
                Some(receiver)
            }
            None => arg_nodes.first().copied(),
        };
        if args[0].ty != Type::String {
            return self.format_in_locale(node, arg_nodes);
        }
        // An array of objects passed for the varargs is the arguments themselves (`Object[]`, as
        // `LocaleResources.trace` passes its own); any other array is a single array argument,
        // which is not supported.
        let objects = |ty: Type| matches!(ty, Type::Array(element) if element.is_reference());
        if args.len() == 2 && objects(args[1].ty) {
            return Ok(ExprKind::Builtin(Builtin::Format, args));
        }
        if let Some(array) = args[1..].iter().find(|a| !textual(a.ty)) {
            return error(array.pos, "an array as an argument of `String.format` not supported yet");
        }
        if let Some(format) = self.constant_text(format_node)
            && let Err(why) = java_runtime::format::check_supported(&format)
        {
            return error(node.pos(), format!("{why} not supported yet: \"{format}\""));
        }
        Ok(ExprKind::Builtin(Builtin::Format, args))
    }

    /// `String.format(locale, format, args...)` (and `printf` with a locale): what the JDK's code
    /// runs, `new Formatter(locale).format(format, args).toString()`, with the JDK's `Formatter`,
    /// which formats as the locale does.
    fn format_in_locale(&self, node: Node<'t>, arg_nodes: &[Node<'t>]) -> Result<ExprKind, Error> {
        let [new, format, text] = crate::program::FORMAT_IN_LOCALE;
        let found = |(owner, name, sig): (&str, &str, &str)| {
            self.methods.iter().any(|m| self.types[m.owner].javac == owner && m.name == name && m.sig == sig)
        };
        let Some((locale, rest)) =
            arg_nodes.split_first().filter(|_| [new, format, text].into_iter().all(found))
        else {
            return error(
                node.pos(),
                "`String.format` with a locale formats with the JDK's `Formatter`: translate the program \
                 with the JDK's sources (`--main` and `--library`) for it",
            );
        };
        let (line, column) = (&node.0["line"], &node.0["column"]);
        let symbol = |(owner, name, sig): (&str, &str, &str)| {
            let kind = if name == "<init>" { "CONSTRUCTOR" } else { "METHOD" };
            serde_json::json!({"kind": kind, "name": name, "owner": owner, "static": false, "type": sig,
                "varargs": name == "format"})
        };
        let call = |receiver: serde_json::Value,
                    method: (&str, &str, &str),
                    args: Vec<serde_json::Value>,
                    ty: &str| {
            serde_json::json!({"kind": "METHOD_INVOCATION", "line": line, "column": column, "javaType": ty,
                "arguments": args,
                "methodSelect": {"kind": "MEMBER_SELECT", "line": line, "column": column,
                    "identifier": method.1, "expression": receiver, "symbol": symbol(method)},
                "symbol": symbol(method)})
        };
        let formatter = serde_json::json!({"kind": "NEW_CLASS", "line": line, "column": column,
            "javaType": new.0, "arguments": [locale.0], "symbol": symbol(new)});
        let formatted = call(formatter, format, rest.iter().map(|a| a.0.clone()).collect(), new.0);
        let text = call(formatted, text, Vec::new(), "java.lang.String");
        let text: &'t serde_json::Value = Box::leak(Box::new(text));
        Ok(self.expr(Node(text))?.kind)
    }

    /// `System.out.printf(format, args...)` and `System.out.format`: printing the formatted text.
    pub(super) fn printf_call(&self, node: Node<'t>, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let arg_nodes: Vec<Node<'t>> = node.list("arguments").collect();
        let format = self.format_call(node, args, &arg_nodes, None)?;
        let text = self.synth(format, node.pos(), Type::String);
        Ok(ExprKind::Builtin(Builtin::Print, vec![text]))
    }

    /// `ps.printf(format, args...)` and `ps.format` of a `PrintStream` object: the text is
    /// formatted here, as `System.out.printf`'s is, and printed by the translator's
    /// `PrintStream.formattedText`, which returns the stream as `printf` does.
    pub(super) fn print_stream_format(
        &self,
        node: Node<'t>,
        receiver: Option<Node<'t>>,
        args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let arg_nodes: Vec<Node<'t>> = node.list("arguments").collect();
        let format = self.format_call(node, args, &arg_nodes, None)?;
        let text = self.synth(format, node.pos(), Type::String);
        let (owner, name, signature) = crate::program::PRINT_FORMATTED;
        let resolved = self
            .methods
            .iter()
            .position(|m| self.types[m.owner].javac == owner && m.name == name && m.sig == signature);
        if resolved.is_none() {
            return unsupported(node, "`printf` of this `PrintStream`");
        }
        let receiver = match receiver {
            Some(r) => self.receiver(r, owner)?,
            None => self.this(node),
        };
        Ok(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: name.to_string(),
            args: vec![text],
            resolved,
            is_super: false,
        })
    }

    /// A call of a method of `StringBuilder` (or of `AbstractStringBuilder`, which declares some of
    /// them).
    pub(super) fn builder_call(
        &self,
        node: Node<'t>,
        symbol: &Symbol,
        receiver: Option<Node<'t>>,
        mut args: Vec<Expr>,
    ) -> Result<ExprKind, Error> {
        let name = symbol.name;
        let op = match name {
            "append" => BuilderOp::Append,
            "insert" => BuilderOp::Insert,
            "reverse" => BuilderOp::Reverse,
            "setCharAt" => BuilderOp::SetCharAt,
            "charAt" => BuilderOp::CharAt,
            "deleteCharAt" => BuilderOp::DeleteCharAt,
            "delete" => BuilderOp::Delete,
            "replace" => BuilderOp::Replace,
            "indexOf" => BuilderOp::IndexOf,
            "lastIndexOf" => BuilderOp::LastIndexOf,
            "length" => BuilderOp::Length,
            "setLength" => BuilderOp::SetLength,
            "toString" => BuilderOp::ToString,
            "substring" => BuilderOp::Substring,
            "isEmpty" => BuilderOp::IsEmpty,
            "compareTo" => BuilderOp::CompareTo,
            "appendCodePoint" => BuilderOp::AppendCodePoint,
            "repeat" => BuilderOp::Repeat,
            "codePointAt" => BuilderOp::CodePointAt,
            "codePointBefore" => BuilderOp::CodePointBefore,
            "codePointCount" => BuilderOp::CodePointCount,
            "offsetByCodePoints" => BuilderOp::OffsetByCodePoints,
            "getChars" => BuilderOp::GetChars,
            "subSequence" => BuilderOp::Substring,
            _ => return unsupported(node, &format!("method `StringBuilder.{name}`")),
        };
        let receiver = receiver.expect("an instance method has a receiver");
        let receiver = self.expr(receiver)?;
        if receiver.ty != Type::StringBuilder {
            return unsupported(node, &format!("method `{}.{name}`", symbol.owner));
        }
        // `repeat(text, count)` of a string or a builder; `repeat(codePoint, count)` takes an `int`.
        if op == BuilderOp::Repeat
            && symbol.ty.starts_with("(java.lang.CharSequence")
            && !char_sequence(args[0].ty)
        {
            return unsupported(node, "`StringBuilder.repeat` of this kind of text");
        }
        if name == "subSequence" {
            args.insert(0, receiver);
            return self.as_char_sequence(node, Builtin::Builder(op), args);
        }
        let shown = match op {
            BuilderOp::Append if args.len() == 1 => Some(0),
            BuilderOp::Insert if args.len() == 2 => Some(1),
            BuilderOp::Append
                if args.len() == 3
                    && (char_sequence(args[0].ty)
                        || args[0].ty == Type::class(CHAR_SEQUENCE)
                        || args[0].ty == Type::array(Type::Char)) =>
            {
                None
            }
            BuilderOp::Append | BuilderOp::Insert => {
                return unsupported(node, &format!("this overload of `StringBuilder.{name}`"));
            }
            _ => None,
        };
        if let Some(i) = shown {
            let ty = args[i].ty;
            if matches!(ty, Type::Array(_)) && ty != Type::array(Type::Char) {
                return unsupported(node, "appending an array that is not a `char[]`");
            }
            // An object shows its `toString`, which runs before the builder is changed.
            if matches!(ty, Type::Class(..)) {
                let x = args.remove(i);
                args.insert(i, self.as_text(x));
            }
        }
        args.insert(0, receiver);
        Ok(ExprKind::Builtin(Builtin::Builder(op), args))
    }

    /// `new String(..)` and `new StringBuilder(..)`; `None` for any other class.
    pub(super) fn new_text(&self, node: Node<'t>) -> Result<Option<ExprKind>, Error> {
        let Some(symbol) = node.symbol() else { return Ok(None) };
        if !matches!(symbol.owner, "java.lang.String" | "java.lang.StringBuilder" | "java.lang.StringBuffer")
        {
            return Ok(None);
        }
        let args: Vec<Expr> = node.list("arguments").map(|a| self.expr(a)).collect::<Result<_, _>>()?;
        let tys: Vec<Type> = args.iter().map(|a| a.ty).collect();
        let chars = Type::array(Type::Char);
        let bytes = Type::array(Type::Byte);
        Ok(Some(match (symbol.owner, tys.as_slice()) {
            ("java.lang.String", []) => ExprKind::Str(String::new()),
            ("java.lang.String", [Type::String]) => {
                ExprKind::Builtin(Builtin::String(StringOp::Identity), args)
            }
            ("java.lang.String", [Type::StringBuilder]) => {
                ExprKind::Builtin(Builtin::Builder(BuilderOp::ToString), args)
            }
            ("java.lang.String", [a] | [a, Type::Int, Type::Int]) if *a == chars => {
                ExprKind::Builtin(Builtin::String(StringOp::FromChars), args)
            }
            ("java.lang.String", [a, Type::Int, Type::Int]) if *a == Type::array(Type::Int) => {
                ExprKind::Builtin(Builtin::String(StringOp::FromCodePoints), args)
            }
            ("java.lang.String", [a, Type::Int, Type::Int, Type::Int]) if *a == bytes => {
                ExprKind::Builtin(Builtin::String(StringOp::FromHighByte), args)
            }
            (
                "java.lang.String",
                [a]
                | [a, Type::Charset | Type::String]
                | [a, Type::Int, Type::Int]
                | [a, Type::Int, Type::Int, Type::Charset | Type::String],
            ) if *a == bytes => ExprKind::Builtin(Builtin::String(StringOp::FromBytes), args),
            (
                "java.lang.StringBuilder" | "java.lang.StringBuffer",
                [] | [Type::Int | Type::String | Type::StringBuilder],
            ) => ExprKind::Builtin(Builtin::Builder(BuilderOp::New), args),
            (owner, _) => return unsupported(node, &format!("this constructor of `{owner}`")),
        }))
    }

    /// A bit or radix method of `Integer` or `Long`; `None` for any other method.
    pub(super) fn bits_call(&self, symbol: &Symbol, args: &[Expr]) -> Option<Builtin> {
        let kind = match symbol.owner {
            "java.lang.Integer" => Type::Int,
            "java.lang.Long" => Type::Long,
            _ => return None,
        };
        let op = match (symbol.name, args.len()) {
            ("toHexString", 1) => BitsOp::ToHexString,
            ("toBinaryString", 1) => BitsOp::ToBinaryString,
            ("toOctalString", 1) => BitsOp::ToOctalString,
            ("numberOfLeadingZeros", 1) => BitsOp::NumberOfLeadingZeros,
            ("numberOfTrailingZeros", 1) => BitsOp::NumberOfTrailingZeros,
            ("bitCount", 1) => BitsOp::BitCount,
            ("highestOneBit", 1) => BitsOp::HighestOneBit,
            ("lowestOneBit", 1) => BitsOp::LowestOneBit,
            ("reverse", 1) => BitsOp::Reverse,
            ("reverseBytes", 1) => BitsOp::ReverseBytes,
            ("rotateLeft", 2) => BitsOp::RotateLeft,
            ("rotateRight", 2) => BitsOp::RotateRight,
            ("signum", 1) => BitsOp::Signum,
            ("toString", 2) => BitsOp::ToStringRadix,
            ("parseInt" | "parseLong", 2) if args[0].ty == Type::String => BitsOp::ParseRadix,
            ("parseInt" | "parseLong", 4) if args[0].ty == Type::String => BitsOp::ParseRange,
            _ => return None,
        };
        symbol.is_static.then_some(Builtin::Bits(op, kind))
    }
}
