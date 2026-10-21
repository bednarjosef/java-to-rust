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
    fn constant_text(&self, node: Option<Node<'t>>) -> Option<String> {
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
    fn as_text(&self, x: Expr) -> Expr {
        let empty = self.synth(ExprKind::Str(String::new()), x.pos, Type::String);
        let pos = x.pos;
        self.synth(ExprKind::Binary(BinOp::Add, Box::new(empty), Box::new(x)), pos, Type::String)
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
            "hashCode" => StringOp::HashCode,
            "toString" | "intern" => StringOp::Identity,
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
            return unsupported(node, "`String.format` with a locale");
        }
        // An array passed for the varargs is the arguments themselves (`Object[]`), or a single
        // array argument; neither is supported.
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

    /// `System.out.printf(format, args...)` and `System.out.format`: printing the formatted text.
    pub(super) fn printf_call(&self, node: Node<'t>, args: Vec<Expr>) -> Result<ExprKind, Error> {
        let arg_nodes: Vec<Node<'t>> = node.list("arguments").collect();
        let format = self.format_call(node, args, &arg_nodes, None)?;
        let text = self.synth(format, node.pos(), Type::String);
        Ok(ExprKind::Builtin(Builtin::Print, vec![text]))
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
            _ => return unsupported(node, &format!("method `StringBuilder.{name}`")),
        };
        let receiver = receiver.expect("an instance method has a receiver");
        let receiver = self.expr(receiver)?;
        if receiver.ty != Type::StringBuilder {
            return unsupported(node, &format!("method `{}.{name}`", symbol.owner));
        }
        let shown = match op {
            BuilderOp::Append if args.len() == 1 => Some(0),
            BuilderOp::Insert if args.len() == 2 => Some(1),
            BuilderOp::Append
                if args.len() == 3
                    && (char_sequence(args[0].ty) || args[0].ty == Type::array(Type::Char)) =>
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
            (
                "java.lang.String",
                [a]
                | [a, Type::Charset | Type::String]
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
            _ => return None,
        };
        symbol.is_static.then_some(Builtin::Bits(op, kind))
    }
}
