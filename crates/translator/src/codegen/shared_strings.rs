//! Strings in a program that compares them by reference (`Class::shared_strings`): every string
//! is a `java::JStr`, and what makes one says which object it is, as Java makes it
//! (`analysis::string_identity::made`): an interned literal, a new string, or one kept per key.

use super::*;
use crate::analysis::{Made, made};

impl<'a> Gen<'a> {
    /// The string of a literal, `text` a Rust string literal or the name of a string constant (a
    /// `&'static java::Wtf8`): a `java::JString`, or the interned `java::JStr`, looked up once
    /// where the code has it, where the program's strings are shared.
    pub(super) fn string_literal(&mut self, text: &str) -> String {
        let text = if text.starts_with('"') {
            format!("{}({text})", self.runtime("wtf8"))
        } else {
            text.to_string()
        };
        if !super::shared_strings() {
            return format!("{}::from({text})", self.runtime("JString"));
        }
        let jstr = self.runtime("JStr");
        let interned = self.runtime("interned_literal");
        format!(
            "{{ static LITERAL: std::sync::OnceLock<{jstr}> = std::sync::OnceLock::new(); {interned}(&LITERAL, {text}) }}"
        )
    }

    /// The shared string that `e` makes, given `code`, the string the expression is written as:
    /// a new or interned `java::JStr` for what makes a string, `code` itself for what reads one.
    pub(super) fn shared_made(&mut self, e: &'a Expr, code: Code) -> Code {
        let wrap = match &e.kind {
            // A constant's name is its interned literal already (`enum_name`).
            ExprKind::Builtin(Builtin::EnumName, _) => return code,
            ExprKind::Builtin(builtin, _) => match made(*builtin) {
                Made::Interned => "interned_text",
                Made::New | Made::Unknown => "JStr::new",
                Made::NewOrEmpty => "new_or_empty",
                Made::ArrayText => "array_text",
                Made::PerKey => return self.per_key(e, code),
                Made::Text if *builtin == Builtin::ObjectText => "shared",
                Made::Kept | Made::Itself | Made::Text | Made::Element | Made::Never | Made::TypeName => {
                    return code;
                }
            },
            _ => return code,
        };
        let f = self.runtime(wrap);
        if self.nullable(e) {
            Code::atom(format!("{}.map({f})", code.at(ATOM)))
        } else {
            Code::atom(format!("{f}({})", code.text))
        }
    }

    /// A string that Java keeps one object of for each key: the same `java::JStr` for the same
    /// key and text. A class's simple and canonical names are kept by their text, which two
    /// classes share only where they have the same simple name; an environment variable by its
    /// value.
    fn per_key(&mut self, e: &'a Expr, code: Code) -> Code {
        let key = match &e.kind {
            ExprKind::Builtin(Builtin::Native(Native::SystemProperty(key)), _) => format!("property {key}"),
            ExprKind::Builtin(Builtin::Native(Native::LineSeparator), _) => "property line.separator".into(),
            ExprKind::Builtin(Builtin::Native(Native::Getenv), _) => "environment".into(),
            ExprKind::Builtin(Builtin::ClassMethod(ClassOp::GetCanonicalName), _) => "canonical name".into(),
            ExprKind::Builtin(
                Builtin::ClassName { simple: true }
                | Builtin::ClassLiteralName { simple: true, .. }
                | Builtin::ClassMethod(ClassOp::GetSimpleName),
                _,
            ) => "simple name".into(),
            _ => unreachable!("a string kept per key is a builtin's"),
        };
        let f = self.runtime("per_key");
        let key = rust_string(&key);
        if self.nullable(e) {
            Code::atom(format!("{}.map(|text_| {f}({key}, text_))", code.at(ATOM)))
        } else {
            Code::atom(format!("{f}({key}, {})", code.text))
        }
    }

    /// `Builtin::Kept`: the receiver itself where the JDK's `String` method `op` returns it, else
    /// the string of the call's text (see `java::kept`). The text is computed first, which
    /// throws for a null receiver.
    pub(super) fn kept(&mut self, op: StringOp, args: &'a [Expr]) -> Code {
        let (receiver, rest) = args.split_first().expect("a receiver");
        let (call, rest) = rest.split_last().expect("a call");
        let text = self.expr(call).text;
        let receiver = match self.nullable(receiver) {
            true => format!("{}.as_ref().unwrap()", self.local_name_of(receiver)),
            false => format!("&{}", self.local_name_of(receiver)),
        };
        let strings = |g: &mut Self| rest.iter().map(|a| g.text_ref(a)).collect::<Vec<_>>().join(", ");
        let kept = match op {
            StringOp::Replace if rest.iter().all(|a| !matches!(a.ty, Type::Char)) => {
                format!("{}({receiver}, text_, {})", self.runtime("kept_replace"), strings(self))
            }
            StringOp::Repeat => {
                let count = self.expr(&rest[0]).text;
                format!("{}({receiver}, text_, {count})", self.runtime("kept_repeat"))
            }
            StringOp::ReplaceAll | StringOp::ReplaceFirst => {
                let regex = self.text_ref(&rest[0]);
                format!("{}({receiver}, text_, {regex})", self.runtime("kept_regex"))
            }
            StringOp::StripIndent | StringOp::TranslateEscapes => {
                format!("{}({receiver}, text_)", self.runtime("empty_or_new"))
            }
            _ => format!("{}({receiver}, text_)", self.runtime("kept")),
        };
        Code::atom(format!("{{ let text_ = {text}; {kept} }}"))
    }

    /// `Builtin::ValueOf`: the text of its argument, or where strings are shared, the string
    /// itself, the interned `"null"`, or what the object's `toString` returns.
    pub(super) fn value_of(&mut self, args: &'a [Expr]) -> Code {
        if !super::shared_strings() {
            return self.expr(&args[0]);
        }
        let x = crate::analysis::value_of_operand(Builtin::ValueOf, args);
        let null = self.string_literal("\"null\"");
        let mut uncast = x;
        while let ExprKind::Cast(_, inner) = &uncast.kind {
            uncast = inner;
        }
        if matches!(uncast.kind, ExprKind::Null) {
            return Code::atom(null);
        }
        if x.ty == Type::String {
            let code = self.expr(x);
            if !self.nullable(x) {
                return code;
            }
            return Code::atom(format!("{}.unwrap_or_else(|| {null})", code.at(ATOM)));
        }
        let shared = self.runtime("shared");
        if !self.nullable(x) {
            let text = self.object_to_string(x);
            return Code::atom(format!("{shared}({})", text.text));
        }
        let option = format!("{}.as_ref()", self.object_place(x));
        let (text, raises) = self.shared_handle_text("o", x);
        if raises {
            let texts = self.passed_on(Code::atom(format!("{option}.map(|o| {text}).transpose()")));
            return Code::atom(format!("{texts}.unwrap_or_else(|| {null})"));
        }
        Code::atom(format!("{option}.map(|o| {text}).unwrap_or_else(|| {null})"))
    }

    /// The string that `toString` returns for a non-null object of `e`'s class, reached through
    /// `handle`, where strings are shared; and whether it is the `Result` of a `toString` that
    /// throws.
    fn shared_handle_text(&mut self, handle: &str, e: &'a Expr) -> (String, bool) {
        if let Some(t) = self.enum_of(e) {
            self.need(Need::Display(t));
            if self.enum_text_throws(t) {
                return (format!("{handle}.try_text()"), true);
            }
            self.need(Need::JavaString(t));
            return (format!("{handle}.java_string()"), false);
        }
        let shared = self.runtime("shared");
        match self.handle_text(handle, e) {
            (text, true) => (format!("{text}.map({shared})"), true),
            (text, false) => (format!("{shared}({text})"), false),
        }
    }

    /// A string as a `&java::JStr`, where strings are shared: null throws
    /// `NullPointerException`, as `deref` does.
    pub(super) fn shared_ref(&mut self, e: &'a Expr) -> String {
        let code = self.expr(e).at(ATOM);
        if self.nullable(e) {
            return self.dereference(&[e.id], &format!("{code}.as_ref()"));
        }
        format!("&{code}")
    }

    /// The Rust name of the local that `e`, a read of a local, reads.
    fn local_name_of(&mut self, e: &'a Expr) -> String {
        let ExprKind::Var(name) = &e.kind else { unreachable!("a kept receiver is a local") };
        self.local(name)
    }
}
