//! The failures of the runtime and of the library that the runtime implements, which evaluating
//! one expression may raise: a null dereference, an index out of bounds, a division by zero, a
//! malformed number, a failed cast, and what each method of the library throws.

use super::*;

/// Whether the generator has the checked form of dereferencing `object`: the object of a call or
/// a field access is written by `Gen::object` and its relatives, which check it with `non_null`.
fn object_checked(object: &Expr) -> bool {
    matches!(object.ty, Type::Class(..))
}

/// Whether `builtin` is a regular expression call whose pattern (and replacement) are literals
/// that compile, so that it never throws.
fn constant_pattern(builtin: &Builtin, args: &[Expr]) -> bool {
    let text = |i: usize| match args.get(i).map(|a| &a.kind) {
        Some(ExprKind::Str(s)) => Some(s.as_str()),
        _ => None,
    };
    match builtin {
        Builtin::String(StringOp::Split | StringOp::Matches) => {
            text(1).is_some_and(|p| java_runtime::regex::never_fails(p, None))
        }
        Builtin::String(StringOp::ReplaceAll | StringOp::ReplaceFirst) => match (text(1), text(2)) {
            (Some(p), Some(r)) => java_runtime::regex::never_fails(p, Some(r)),
            _ => false,
        },
        _ => false,
    }
}

/// What a method of the library that the runtime implements may throw besides a
/// `NullPointerException` for a null argument: `None` when nothing, `"?"` when it is not known
/// exactly.
fn library_failure(builtin: &Builtin, args: &[Expr]) -> Option<&'static str> {
    match builtin {
        Builtin::String(op) => match op {
            StringOp::Substring => Some("java.lang.StringIndexOutOfBoundsException"),
            // `valueOf(chars)` copies the whole array; only an offset and a count can be out of range.
            StringOp::FromChars if args.len() == 3 => Some("java.lang.StringIndexOutOfBoundsException"),
            StringOp::Repeat => Some("java.lang.IllegalArgumentException"),
            // A malformed pattern, a bad group in a replacement, an unknown charset.
            StringOp::ReplaceAll
            | StringOp::ReplaceFirst
            | StringOp::Matches
            | StringOp::Split
            | StringOp::GetBytes
            | StringOp::FromBytes => Some("?"),
            _ => None,
        },
        Builtin::Builder(
            BuilderOp::New
            | BuilderOp::Append
            | BuilderOp::Reverse
            | BuilderOp::IndexOf
            | BuilderOp::LastIndexOf
            | BuilderOp::Length
            | BuilderOp::ToString
            | BuilderOp::IsEmpty
            | BuilderOp::CompareTo,
        ) => None,
        // An index out of the builder's bounds.
        Builtin::Builder(_) => Some("?"),
        Builtin::ArraysSort | Builtin::ArraysToString => None,
        Builtin::Arrays(
            ArraysOp::Equals | ArraysOp::HashCode | ArraysOp::SortAll | ArraysOp::BinarySearch,
        ) => None,
        Builtin::Arrays(ArraysOp::DeepToString | ArraysOp::Clone) => None,
        Builtin::Bits(BitsOp::ParseRadix | BitsOp::ParseRange, _) => Some("java.lang.NumberFormatException"),
        Builtin::Unsigned(UnsignedOp::Parse | UnsignedOp::ParseRange, _) => {
            Some("java.lang.NumberFormatException")
        }
        Builtin::EnumValueOf => Some("java.lang.IllegalArgumentException"),
        Builtin::Bits(..) => None,
        Builtin::Boxed(BoxedOp::Unbox | BoxedOp::Equals, _) => None,
        Builtin::Unsigned(UnsignedOp::Compare | UnsignedOp::ToString | UnsignedOp::Widen, _) => None,
        // Overflow, and bounds the wrong way round.
        Builtin::Native(Native::Math { name, .. }) if name.ends_with("Exact") => {
            Some("java.lang.ArithmeticException")
        }
        Builtin::Native(Native::Math { name: "clamp", .. }) => Some("java.lang.IllegalArgumentException"),
        // The others are functions on numbers, which never throw.
        Builtin::Native(Native::Math { .. }) => None,
        // A negative time. Nothing interrupts a thread of a translated program, so `sleep` never
        // throws `InterruptedException`.
        Builtin::Native(Native::Sleep { .. }) => Some("java.lang.IllegalArgumentException"),
        // `Thread`'s checks are in Java (`java/Threads.java`); its natives cannot fail.
        Builtin::Native(
            Native::OnSpinWait
            | Native::ThreadYield
            | Native::ThreadNumber
            | Native::ThreadId
            | Native::MainThreadId
            | Native::HasCurrentThread
            | Native::SetCurrentThread
            | Native::CurrentThread
            | Native::ThreadStart
            | Native::ThreadAlive
            | Native::ThreadJoin
            | Native::ThreadRename,
        ) => None,
        Builtin::CodePoint(CodePointOp::ToCharsInto) => Some("java.lang.IllegalArgumentException"),
        Builtin::CodePoint(
            CodePointOp::CharCount
            | CodePointOp::ToCodePoint
            | CodePointOp::HighSurrogate
            | CodePointOp::LowSurrogate
            | CodePointOp::IsValid
            | CodePointOp::IsBmp
            | CodePointOp::IsSupplementary
            | CodePointOp::IsLetter
            | CodePointOp::IsDigit
            | CodePointOp::IsLetterOrDigit
            | CodePointOp::IsWhitespace
            | CodePointOp::IsSpaceChar
            | CodePointOp::IsUpperCase
            | CodePointOp::IsLowerCase
            | CodePointOp::IsAlphabetic
            | CodePointOp::ToUpperCase
            | CodePointOp::ToLowerCase
            | CodePointOp::NumericValue
            | CodePointOp::Digit,
        ) => None,
        // A name that is not a charset: `UnsupportedCharsetException`, `IllegalCharsetNameException`
        // or, for null, `IllegalArgumentException`. Not checked, so a catch of it is rejected.
        Builtin::Charset(CharsetOp::ForName | CharsetOp::IsSupported) => Some("?"),
        Builtin::Charset(_) => None,
        _ => Some("?"),
    }
}

/// The failures of the runtime that evaluating `x` itself (not its operands) may raise, by
/// standard class name, with what raises them in words, whether the generator has their checked
/// form, and the expression that the checked form is written for: `x`, or for a
/// `NullPointerException` of a call or a field access, the object that is dereferenced.
pub(super) fn runtime_failures(
    class: &Class,
    facts: &Facts,
    m: usize,
    x: &Expr,
    safe: &[String],
) -> Vec<(&'static str, &'static str, bool, ExprId)> {
    const NPE: &str = "java.lang.NullPointerException";
    const INDEX: &str = "java.lang.ArrayIndexOutOfBoundsException";
    let tested = |e: &Expr| is_tested(e, safe);
    let null = |e: &Expr| {
        (m != usize::MAX && !tested(e) && maybe_null(facts, m, e)) || matches!(e.kind, ExprKind::Null)
    };
    let mut out = Vec::new();
    match &x.kind {
        ExprKind::Binary(BinOp::Div | BinOp::Rem, l, r)
            if l.ty.unboxed().is_integral() && r.ty.unboxed().is_integral() =>
        {
            let constant =
                matches!(r.kind, ExprKind::Int(v) if v != 0) || matches!(r.kind, ExprKind::Long(v) if v != 0);
            if !constant && !facts.safe_division.contains(&x.id) {
                out.push(("java.lang.ArithmeticException", "this division", true, x.id));
            }
        }
        ExprKind::Assign { op: Some(BinOp::Div | BinOp::Rem), value, .. }
            if x.ty.unboxed().is_integral() && value.ty.unboxed().is_integral() =>
        {
            let constant = matches!(value.kind, ExprKind::Int(v) if v != 0)
                || matches!(value.kind, ExprKind::Long(v) if v != 0);
            if !constant && !facts.safe_division.contains(&x.id) {
                out.push(("java.lang.ArithmeticException", "this division", true, x.id));
            }
        }
        ExprKind::Index(array, _) => {
            if null(array) {
                out.push((NPE, "this array access", false, x.id));
            }
            if !facts.in_bounds.contains(&x.id) {
                out.push((INDEX, "this array access", true, x.id));
            }
        }
        ExprKind::Length(array) if null(array) => out.push((NPE, "this length", false, x.id)),
        ExprKind::Field { object, .. } if null(object) => {
            out.push((NPE, "this field access", object_checked(object), object.id))
        }
        ExprKind::MethodCall { receiver, resolved: Some(r), is_super: false, .. }
            if !class.methods[*r].is_static && null(receiver) =>
        {
            out.push((NPE, "this call", object_checked(receiver), receiver.id))
        }
        ExprKind::NewArray(sizes) => {
            // A size the range analysis keeps non-negative cannot throw.
            let nonnegative = |s: &Expr| facts.index_ranges.get(&s.id).is_some_and(|&(lo, _)| lo >= 0);
            if sizes.iter().any(|s| !matches!(s.kind, ExprKind::Int(v) if v >= 0) && !nonnegative(s)) {
                out.push(("java.lang.NegativeArraySizeException", "this array creation", false, x.id));
            }
        }
        ExprKind::Cast(to, value)
            if !to.is_reference() && matches!(value.ty, Type::Boxed(_)) && null(value) =>
        {
            out.push((NPE, "this unboxing", false, x.id))
        }
        ExprKind::Builtin(builtin, args) => match builtin {
            Builtin::Println
            | Builtin::Print
            | Builtin::MathAbs
            | Builtin::MathMax
            | Builtin::MathMin
            | Builtin::MathSqrt
            | Builtin::MathPow
            | Builtin::CharIsDigit
            | Builtin::CharIsLetter
            | Builtin::CharIsUpperCase
            | Builtin::CharIsLowerCase
            | Builtin::CharToUpperCase
            | Builtin::CharToLowerCase
            | Builtin::CharIsWhitespace
            | Builtin::CharIsSpaceChar
            | Builtin::CharIsLetterOrDigit
            | Builtin::CharIsAlphabetic
            | Builtin::CharIsSurrogate
            | Builtin::CharIsHighSurrogate
            | Builtin::CharIsLowSurrogate
            | Builtin::CharDigit
            | Builtin::CharForDigit
            | Builtin::CharNumericValue
            | Builtin::EnumOrdinal
            | Builtin::EnumName
            | Builtin::EnumCompareTo
            | Builtin::Exit
            | Builtin::NoObject { .. }
            | Builtin::UnknownClass
            | Builtin::ClassName { .. }
            | Builtin::ClassLiteralName { .. }
            | Builtin::SameClass { .. }
            | Builtin::GetClass
            | Builtin::ClassLiteral(_)
            | Builtin::ClassMethod(_)
            | Builtin::ComparableClassFor
            // A null is a value to them, and an object's own method is a call of its own.
            | Builtin::ObjectsEquals
            | Builtin::ObjectsHashCode => {}
            Builtin::StringLength | Builtin::StringEquals => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", false, x.id));
                }
            }
            Builtin::StringCharAt => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", false, x.id));
                }
                if !facts.index_in_bounds.contains(&x.id) {
                    out.push(("java.lang.StringIndexOutOfBoundsException", "this charAt", true, x.id));
                }
            }
            Builtin::Boxed(BoxedOp::Parse | BoxedOp::ParseRadix, _) => {
                out.push(("java.lang.NumberFormatException", "this parse", true, x.id));
            }
            Builtin::Boxed(
                BoxedOp::Box
                | BoxedOp::ToString
                | BoxedOp::Compare
                | BoxedOp::HashCode
                | BoxedOp::Sum
                | BoxedOp::FloatTest(_)
                | BoxedOp::Logical(_),
                _,
            ) => {}
            Builtin::NullCheck => out.push((NPE, "this synchronized", false, x.id)),
            Builtin::NullChecked | Builtin::Unmatched => {
                if null(&args[0]) {
                    out.push((NPE, "this null check", false, x.id));
                }
            }
            // A zero divisor throws, as in a division.
            Builtin::Native(Native::Math { name: "floorDiv" | "floorMod", .. }) => {
                let constant = matches!(args[1].kind, ExprKind::Int(v) if v != 0)
                    || matches!(args[1].kind, ExprKind::Long(v) if v != 0);
                if !constant {
                    out.push(("java.lang.ArithmeticException", "this division", true, x.id));
                }
            }
            // The string is dereferenced first (a null one is the checked `NullPointerException`),
            // then the indices, the radix and the digits are read.
            Builtin::Bits(BitsOp::ParseRange, _) => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this parse", true, x.id));
                }
                out.push(("java.lang.IndexOutOfBoundsException", "this parse", true, x.id));
                out.push(("java.lang.NumberFormatException", "this parse", true, x.id));
            }
            _ => {
                // Writing past the end of the array throws, as `Character.toChars` does.
                if matches!(builtin, Builtin::CodePoint(CodePointOp::ToCharsInto)) {
                    out.push(("java.lang.ArrayIndexOutOfBoundsException", "this call of the library", true, x.id));
                }
                // A begin or end outside the string throws, as `parseUnsignedInt(s, begin, end, radix)`
                // does (`parseInt` has its own arm above).
                if matches!(builtin, Builtin::Unsigned(UnsignedOp::ParseRange, _)) {
                    out.push(("java.lang.IndexOutOfBoundsException", "this call of the library", true, x.id));
                }
                // A null argument throws, except where the method takes null (an `equals`, the
                // elements of `join`, the text of `Arrays.toString`).
                let takes_null = matches!(
                    builtin,
                    Builtin::String(StringOp::EqualsIgnoreCase | StringOp::EqualsOther)
                        | Builtin::ArraysToString
                        | Builtin::Arrays(ArraysOp::Equals | ArraysOp::HashCode | ArraysOp::DeepToString)
                        | Builtin::Boxed(BoxedOp::Equals, _)
                        // A null string is the `NumberFormatException` of `parse_radix`.
                        | Builtin::Bits(BitsOp::ParseRadix, _)
                        | Builtin::Charset(CharsetOp::ForName | CharsetOp::IsSupported)
                );
                let checked = if takes_null || matches!(builtin, Builtin::String(StringOp::Join)) {
                    &args[..args.len().min(1)]
                } else {
                    &args[..]
                };
                if !takes_null && checked.iter().any(null) {
                    out.push((NPE, "this call", false, x.id));
                }
                if let Some(failure) = library_failure(builtin, args)
                    && !constant_pattern(builtin, args)
                {
                    let checked = matches!(
                        builtin,
                        Builtin::String(StringOp::Substring | StringOp::FromChars)
                            | Builtin::Bits(BitsOp::ParseRadix | BitsOp::ParseRange, _)
                            | Builtin::Unsigned(UnsignedOp::Parse | UnsignedOp::ParseRange, _)
                            | Builtin::CodePoint(CodePointOp::ToCharsInto)
                            | Builtin::EnumValueOf
                    );
                    out.push((failure, "this call of the library", checked, x.id));
                }
            }
        },
        _ => {}
    }
    if let ExprKind::Assign { target: Place::Index(array, _), .. }
    | ExprKind::IncDec { target: Place::Index(array, _), .. } = &x.kind
    {
        if null(array) {
            out.push((NPE, "this array access", false, x.id));
        }
        if !facts.in_bounds.contains(&x.id) {
            out.push((INDEX, "this array access", true, x.id));
        }
    }
    if let ExprKind::Assign { target: Place::Field { object, .. }, .. }
    | ExprKind::IncDec { target: Place::Field { object, .. }, .. } = &x.kind
        && null(object)
    {
        out.push((NPE, "this field access", false, x.id));
    }
    // A cast to a class that the value's type is not under may fail, unless an `instanceof`
    // tested the variable.
    // The variable was tested to be an instance of a class under `t`.
    let instance = |t: usize, value: &Expr| {
        let ExprKind::Var(v) = &value.kind else { return false };
        let prefix = instance_fact(v, "");
        safe.iter()
            .filter_map(|fact| fact.strip_prefix(&prefix))
            .any(|name| class.type_index(name).is_some_and(|tested| class.hierarchy.is_subtype(tested, t)))
    };
    // A cast that lowering inserted (at its value's position) is javac's conversion of an erased
    // type variable's value, as `Entry<K, V> e = i.next()`, which does not fail.
    if let ExprKind::Cast(to, value) = &x.kind
        && x.pos != value.pos
        && let (Some(t), Some(s)) = (class.class_index(*to), class.class_index(value.ty))
        && !class.hierarchy.is_subtype(s, t)
        && !instance(t, value)
    {
        out.push(("java.lang.ClassCastException", "this cast", false, x.id));
    }
    // A box that may be null is unboxed by arithmetic and comparisons.
    if let ExprKind::Binary(_, l, r) = &x.kind {
        for operand in [l, r] {
            if matches!(operand.ty, Type::Boxed(_)) && null(operand) && !matches!(x.ty, Type::String) {
                out.push((NPE, "this unboxing", false, x.id));
            }
        }
    }
    out
}
