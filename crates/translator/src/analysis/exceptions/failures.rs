//! The failures of the runtime and of the library that the runtime implements, which evaluating
//! one expression may raise: a null dereference, an index out of bounds, a division by zero, a
//! malformed number, a failed cast, and what each method of the library throws.

use super::*;

/// Whether the generator has the checked form of dereferencing `object`: the object of a call or
/// a field access is written by `Gen::object` and its relatives, which check it with `non_null`,
/// and so is an enum (`Gen::enum_value`).
fn object_checked(object: &Expr) -> bool {
    matches!(object.ty, Type::Class(..) | Type::Enum(_))
}

/// Whether a `System.arraycopy` has an array of objects as its source, which may be an array of
/// another type than the destination (the `Object`-typed copy of `codegen/natives.rs`).
fn object_copy(args: &[Expr]) -> bool {
    matches!(args.first().map(|a| &a.ty), Some(Type::Class(name, _)) if *name == OBJECT)
}

/// Whether every object that a value of type `s` may be (`Facts::flows`, which decides the
/// variants of its enum) is of a class under `t`, so that a cast of it to `t` cannot fail: the
/// JDK's `((TreeMap<E,?>) m).keyIterator()` in `TreeMap.KeySet`, whose `m` is only ever a `TreeMap`
/// where nothing makes a submap.
fn only_under(class: &Class, facts: &Facts, s: usize, t: usize) -> bool {
    facts.flows.get(s).is_some_and(|objects| {
        class
            .hierarchy
            .concretes(s)
            .into_iter()
            .all(|c| !objects.contains(&c) || class.hierarchy.is_subtype(c, t))
    })
}

/// Whether `cond` being true (`when` true) or false (`when` false) means that local `v` is not
/// null: `v != null`, or `v == null` for false, alone or in a chain of `&&` (`||`).
fn checks_null(cond: &Expr, v: &str, when: bool) -> bool {
    let is_v = |e: &Expr| matches!(&e.kind, ExprKind::Var(n) if n == v);
    match &cond.kind {
        ExprKind::Binary(op @ (BinOp::Ne | BinOp::Eq), a, b)
            if (*op == BinOp::Ne) == when
                && (is_v(a) && matches!(b.kind, ExprKind::Null)
                    || is_v(b) && matches!(a.kind, ExprKind::Null)) =>
        {
            true
        }
        ExprKind::Binary(BinOp::And, a, b) if when => checks_null(a, v, when) || checks_null(b, v, when),
        ExprKind::Binary(BinOp::Or, a, b) if !when => checks_null(a, v, when) || checks_null(b, v, when),
        _ => false,
    }
}

/// Whether `builtin` is a regular expression call whose pattern (and replacement) are constants
/// that compile, or whose pattern is `Pattern.quote`'s, which matches its text and has no groups,
/// so that it never throws.
fn constant_pattern(class: &Class, m: usize, builtin: &Builtin, args: &[Expr]) -> bool {
    let arg = |i: usize| args.get(i).map(|a| crate::analysis::held_value(class, m, a));
    let text = |i: usize| arg(i).and_then(|a| crate::analysis::constants::text(class, a));
    let quoted = arg(1).is_some_and(|a| {
        matches!(a.kind, ExprKind::Call { resolved: Some(m), .. }
            if class.methods[m].name == "quote"
                && class.types[class.methods[m].class].name == "java.util.regex.Pattern")
    });
    match builtin {
        Builtin::String(StringOp::Split | StringOp::Matches) => {
            quoted || text(1).is_some_and(|p| java_runtime::regex::never_fails(p, None))
        }
        Builtin::String(StringOp::ReplaceAll | StringOp::ReplaceFirst) => match (text(1), text(2)) {
            (Some(p), Some(r)) => java_runtime::regex::never_fails(p, Some(r)),
            (None, Some(r)) if quoted => java_runtime::regex::never_fails("", Some(r)),
            _ => false,
        },
        _ => false,
    }
}

/// Whether `String.format` with `args` (the format first) may throw: unless its format is a
/// literal, or a `static final` constant, that fits its arguments' types, which then never throws;
/// the arguments in an `Object[]` are as many as the array has.
pub(in crate::analysis) fn format_may_fail(class: &Class, args: &[Expr]) -> bool {
    !crate::analysis::constants::text(class, &args[0]).is_some_and(|format| {
        !args[1..].iter().any(|a| matches!(a.ty, Type::Array(_)))
            && java_runtime::format::never_fails(format, &format_samples(&args[1..]))
    })
}

/// An argument of the runtime's `format` of each argument's kind, as the code generator passes
/// it (`format_arg`), with any value.
fn format_samples(args: &[Expr]) -> Vec<java_runtime::format::Arg<'static>> {
    use java_runtime::format::Arg;
    args.iter()
        .map(|a| match a.ty.unboxed() {
            Type::Null => Arg::Null,
            Type::Boolean => Arg::Bool(false),
            Type::Char => Arg::Char(0),
            Type::Byte => Arg::Byte(0),
            Type::Short => Arg::Short(0),
            Type::Int => Arg::Int(0),
            Type::Long => Arg::Long(0),
            Type::Float => Arg::Float(0.0),
            Type::Double => Arg::Double(0.0),
            Type::String => Arg::Str(""),
            _ => Arg::Text(""),
        })
        .collect()
}

/// What the strings and builders that a `CharSequence` `receiver` may hold throw in the
/// interface's method `r`, when it is one of `CharSequence`'s and the receiver may hold one.
fn sequence_failures(
    class: &Class,
    facts: &Facts,
    r: usize,
    receiver: &Expr,
) -> Option<&'static [&'static str]> {
    let decl = &class.types[class.methods[r].class];
    if decl.kind != TypeKind::Interface || decl.name != CHAR_SEQUENCE {
        return None;
    }
    // The value converted to a `CharSequence` holds what its own type does.
    let receiver = match &receiver.kind {
        ExprKind::Cast(_, value) => value,
        _ => receiver,
    };
    if matches!(receiver.ty, Type::String | Type::StringBuilder) {
        return sequence_failures_of(&class.methods[r].name, receiver.ty == Type::StringBuilder);
    }
    let t = class.class_index(receiver.ty)?;
    let held = |kind: Type| {
        class.hierarchy.concretes(t).into_iter().any(|c| {
            class.types[c].kind == TypeKind::Runtime
                && class.type_of(kind) == Some(c)
                && facts.flows.get(t).is_none_or(|objects| objects.contains(&c))
        })
    };
    let builder = held(Type::StringBuilder);
    if !builder && !held(Type::String) {
        return None;
    }
    sequence_failures_of(&class.methods[r].name, builder)
}

/// What a string's or (with `builder`) a builder's `CharSequence` method `name` throws.
fn sequence_failures_of(name: &str, builder: bool) -> Option<&'static [&'static str]> {
    const SIOOBE: &str = "java.lang.StringIndexOutOfBoundsException";
    match name {
        "length" => Some(&[]),
        "charAt" | "subSequence" => Some(&[SIOOBE]),
        "getChars" if builder => Some(&[SIOOBE, "java.lang.IndexOutOfBoundsException"]),
        "getChars" => Some(&[SIOOBE]),
        _ => None,
    }
}

/// What a method of the library that the runtime implements may throw besides a
/// `NullPointerException` for a null argument, by standard class: none when nothing, `"?"` when
/// it is not known exactly.
fn library_failure(class: &Class, m: usize, builtin: &Builtin, args: &[Expr]) -> &'static [&'static str] {
    const SIOOBE: &str = "java.lang.StringIndexOutOfBoundsException";
    let held = |i: usize| crate::analysis::held_value(class, m, &args[i]);
    match builtin {
        Builtin::String(op) => match op {
            StringOp::Substring => &["java.lang.StringIndexOutOfBoundsException"],
            // `valueOf(chars)` copies the whole array; only an offset and a count can be out of range.
            StringOp::FromChars if args.len() == 3 => &["java.lang.StringIndexOutOfBoundsException"],
            StringOp::Repeat => &["java.lang.IllegalArgumentException"],
            StringOp::TranslateEscapes => &["java.lang.IllegalArgumentException"],
            StringOp::GetChars => &["java.lang.StringIndexOutOfBoundsException"],
            StringOp::CodePointAt | StringOp::CodePointBefore | StringOp::GetLowBytes => {
                &["java.lang.StringIndexOutOfBoundsException"]
            }
            StringOp::CodePointCount | StringOp::OffsetByCodePoints => {
                &["java.lang.IndexOutOfBoundsException"]
            }
            // Encoding or decoding the whole text with a `Charset` (or the default one) never
            // fails; a charset's name may be unknown, and a range out of bounds.
            StringOp::GetBytes | StringOp::FromBytes
                if args.len() == 1 || args.len() == 2 && args[1].ty == Type::Charset =>
            {
                &[]
            }
            // A charset's name that is not one, and a range outside the text or the array.
            StringOp::GetBytes | StringOp::FromBytes => {
                let named = args.last().is_some_and(|a| a.ty == Type::String);
                match (named, args.len() > 2) {
                    (true, true) => &["java.io.UnsupportedEncodingException", SIOOBE],
                    (true, false) => &["java.io.UnsupportedEncodingException"],
                    (false, _) => &[SIOOBE],
                }
            }
            // A range outside the array, an invalid code point.
            StringOp::FromCodePoints => &[SIOOBE, "java.lang.IllegalArgumentException"],
            // `new String(ascii, hibyte)` copies the whole array.
            StringOp::FromHighByte if args.len() == 2 => &[],
            StringOp::FromHighByte => &[SIOOBE],
            // A malformed pattern, and a bad replacement, unless it is a constant without
            // references to groups and escapes: a dangling escape or a malformed reference
            // (`IllegalArgumentException`), or a group the pattern lacks
            // (`IndexOutOfBoundsException`), as `java::regex`'s `parse_replacement` raises them.
            StringOp::ReplaceAll | StringOp::ReplaceFirst
                if !crate::analysis::constants::text(class, held(2))
                    .is_some_and(|r| !r.contains(['$', '\\'])) =>
            {
                let pattern = crate::analysis::constants::text(class, held(1));
                if pattern.is_some_and(|p| java_runtime::regex::never_fails(p, None)) {
                    &["java.lang.IllegalArgumentException", "java.lang.IndexOutOfBoundsException"]
                } else {
                    &[
                        "java.util.regex.PatternSyntaxException",
                        "java.lang.IllegalArgumentException",
                        "java.lang.IndexOutOfBoundsException",
                    ]
                }
            }
            StringOp::ReplaceAll | StringOp::ReplaceFirst | StringOp::Matches | StringOp::Split => {
                &["java.util.regex.PatternSyntaxException"]
            }
            _ => &[],
        },
        // A range of the characters or the text that `append` appends outside them.
        Builtin::Builder(BuilderOp::Append) if args.len() == 4 => &["java.lang.IndexOutOfBoundsException"],
        Builtin::Builder(
            BuilderOp::New
            | BuilderOp::Append
            | BuilderOp::Reverse
            | BuilderOp::IndexOf
            | BuilderOp::LastIndexOf
            | BuilderOp::Length
            | BuilderOp::ToString
            | BuilderOp::IsEmpty
            | BuilderOp::CompareTo
            | BuilderOp::EnsureCapacity,
        ) => &[],
        // A negative count, an invalid code point.
        Builtin::Builder(BuilderOp::AppendCodePoint | BuilderOp::Repeat) => {
            &["java.lang.IllegalArgumentException"]
        }
        Builtin::Builder(BuilderOp::CodePointAt | BuilderOp::CodePointBefore) => {
            &["java.lang.StringIndexOutOfBoundsException"]
        }
        Builtin::Builder(BuilderOp::CodePointCount | BuilderOp::OffsetByCodePoints) => {
            &["java.lang.IndexOutOfBoundsException"]
        }
        // Inserting at the start is always in bounds.
        Builtin::Builder(BuilderOp::Insert) if matches!(args[1].kind, ExprKind::Int(0)) => &[],
        // The destination range of `getChars` outside the array is an `IndexOutOfBoundsException`.
        Builtin::Builder(BuilderOp::GetChars) => &[SIOOBE, "java.lang.IndexOutOfBoundsException"],
        // An index out of the builder's bounds, which `AbstractStringBuilder` checks with
        // `StringIndexOutOfBoundsException`.
        Builtin::Builder(_) => &[SIOOBE],
        Builtin::ArraysSort | Builtin::ArraysToString => &[],
        Builtin::Arrays(
            ArraysOp::Equals | ArraysOp::HashCode | ArraysOp::SortAll | ArraysOp::BinarySearch,
        ) => &[],
        Builtin::Arrays(ArraysOp::DeepToString | ArraysOp::Clone) => &[],
        Builtin::Bits(BitsOp::ParseRadix | BitsOp::ParseRange, _) => &["java.lang.NumberFormatException"],
        Builtin::Unsigned(UnsignedOp::Parse | UnsignedOp::ParseRange, _) => {
            &["java.lang.NumberFormatException"]
        }
        Builtin::EnumValueOf | Builtin::NotEnumClass => &["java.lang.IllegalArgumentException"],
        Builtin::CheckFromToIndex | Builtin::CheckIndex => &["java.lang.IndexOutOfBoundsException"],
        Builtin::Bits(..) => &[],
        Builtin::Boxed(BoxedOp::Unbox | BoxedOp::Equals, _) => &[],
        Builtin::Unsigned(UnsignedOp::Compare | UnsignedOp::ToString | UnsignedOp::Widen, _) => &[],
        // Overflow, and bounds the wrong way round. `ceilDivExact` also throws for a zero divisor,
        // which `runtime_failures` declares as the checked form.
        Builtin::Native(Native::Math { name: "ceilDivExact", .. }) => &[],
        Builtin::Native(Native::Math { name, .. }) if name.ends_with("Exact") => {
            &["java.lang.ArithmeticException"]
        }
        Builtin::Native(Native::Math { name: "clamp", .. }) => &["java.lang.IllegalArgumentException"],
        // The others are functions on numbers, which never throw.
        Builtin::Native(Native::Math { .. }) => &[],
        // A negative time (and an interrupt, which `runtime_failures` adds in a program that
        // interrupts).
        Builtin::Native(Native::Sleep { .. }) => &["java.lang.IllegalArgumentException"],
        // A negative count.
        Builtin::Native(Native::SecureSeed) => &["java.lang.IllegalArgumentException"],
        // Bad compressed data (`inflate.rs`), and a dictionary that the stream did not ask for.
        Builtin::Native(
            Native::InflateBytes
            | Native::InflateBytesBuffer
            | Native::InflateBufferBytes
            | Native::InflateBufferBuffer,
        ) => &["java.util.zip.DataFormatException"],
        Builtin::Native(Native::InflateSetDictionary | Native::InflateSetDictionaryBuffer) => {
            &["java.lang.IllegalArgumentException"]
        }
        // `Thread`'s checks are in Java (`java/Threads.java`); its natives cannot fail, but for
        // the interrupt of `join0`, which `runtime_failures` adds.
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
            | Native::ThreadState
            | Native::ThreadJoin
            | Native::ThreadRename
            | Native::ThreadInterrupt
            | Native::ThreadIsInterrupted
            | Native::ThreadClearInterrupt
            | Native::Park
            | Native::Unpark
            | Native::InstallUncaught
            | Native::UncaughtException
            | Native::PrintUncaught
            | Native::InstallTrace,
        ) => &[],
        // A monitor that the thread does not hold (and for `wait` a negative timeout, and an
        // interrupt, which `runtime_failures` adds).
        Builtin::Native(Native::MonitorWait | Native::MonitorNotify { .. }) => {
            &["java.lang.IllegalMonitorStateException"]
        }
        Builtin::Native(Native::HoldsLock) => &[],
        // What the JVM answers about itself, the clock and the bits of numbers (`vm.rs`, `natives`).
        Builtin::Native(
            Native::DoubleToLongBits
            | Native::DoubleToRawLongBits
            | Native::LongBitsToDouble
            | Native::FloatToIntBits
            | Native::FloatToRawIntBits
            | Native::IntBitsToFloat
            | Native::IdentityHash
            | Native::SystemIdentityHash
            | Native::CurrentTimeMillis
            | Native::NanoTime
            | Native::AvailableProcessors
            | Native::LineSeparator
            | Native::SystemProperty(_)
            | Native::Getenv
            | Native::TotalMemory
            | Native::FreeMemory
            | Native::MaxMemory
            | Native::Gc
            | Native::Nothing
            | Native::Missing { .. }
            | Native::CdsConfigStatus
            | Native::CdsRandomSeed
            | Native::ContinuationsSupported
            | Native::NanoTimeAdjustment
            | Native::SystemTimeZoneId
            | Native::SystemGmtOffsetId
            | Native::ArrayBaseOffset
            | Native::ArrayIndexScale(_)
            | Native::DescriptorHandle
            | Native::DescriptorAppend
            | Native::FullFence
            // A copy outside the arrays, or of memory outside the heap, ends the program
            // (`java::copy_memory`), as the translation cannot go on with it.
            | Native::CopyMemory { .. }
            // So does an access of memory outside every block (`java::native_memory`).
            | Native::AllocateNative
            | Native::PageSize
            | Native::ReallocateNative
            | Native::FreeNative
            | Native::SetMemory
            | Native::Crc32Update
            | Native::InflateInit
            | Native::InflateAdler
            | Native::InflateReset
            | Native::InflateEnd
            | Native::NoPendingReferences
            | Native::TransferUnsupported
            | Native::DirectCopy
            | Native::BufferedCopy
            | Native::UnmapFile
            | Native::AllocationGranularity,
        ) => &[],
        // A file that cannot be read for the mapping fails with an `IOException` (`file_channels.rs`).
        Builtin::Native(Native::MapFile) => &["java.io.IOException"],
        // A descriptor that is not open (`file_copy.rs`), which `Files.copy` never passes.
        Builtin::Native(Native::PosixFadvise) => &["java.io.IOException"],
        // A read or write outside the array, or of memory outside the heap, ends the program.
        Builtin::RawMemory { .. } => &[],
        // The system calls of `java.nio.file` record their `errno` (`java_runtime::unix`).
        Builtin::Native(Native::Unix(_)) => &[],
        // So do the name service calls of `java.net` (`java_runtime::net`).
        Builtin::Native(Native::Net(_)) => &[],
        // The natives of a file channel fail with an `IOException` where the JVM's do (`file_channel.rs`).
        Builtin::Native(Native::Channel(call)) => {
            if call.fails { &["java.io.IOException"] } else { &[] }
        }
        // The descriptor natives fail as the JVM's do, with an `IOException` (`fd.rs`); `close0`
        // of a closed descriptor does nothing.
        Builtin::Native(Native::FileOpenRead | Native::FileOpenWrite | Native::FileOpenRandom) => {
            &["java.io.FileNotFoundException"]
        }
        Builtin::Native(Native::FileSeek | Native::FileSetLength) => &["java.io.IOException"],
        // `java.io.File`'s questions answer `false` or 0 where they fail.
        Builtin::Native(Native::FileSystem(name)) => {
            if matches!(*name, "createFileExclusively0" | "canonicalize0") {
                &["java.io.IOException"]
            } else {
                &[]
            }
        }
        Builtin::Native(
            Native::FileRead
            | Native::FileReadBytes
            | Native::FileAvailable
            | Native::FileSkip
            | Native::FileWrite
            | Native::FileWriteBytes,
        ) => &["java.io.IOException"],
        Builtin::Native(Native::FileLength | Native::FilePosition) => &["java.io.IOException"],
        Builtin::Native(Native::FileClose | Native::FileIsRegular) => &[],
        // The standard streams drop what they cannot write, as `PrintStream` does.
        Builtin::Native(
            Native::StandardWrite
            | Native::StandardWriteBytes
            | Native::StandardFlush
            | Native::StandardClose
            | Native::OutWrite
            | Native::OutFlush
            | Native::ErrFlush,
        ) => &[],
        // A code point that is not valid (`java::to_chars`, `code_point_string`).
        Builtin::CodePoint(CodePointOp::ToCharsInto | CodePointOp::ToChars | CodePointOp::ToString) => {
            &["java.lang.IllegalArgumentException"]
        }
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
        ) => &[],
        // A name that is not a charset's, or a malformed one, or for null an
        // `IllegalArgumentException` (`charset.rs`).
        Builtin::Charset(CharsetOp::ForName) => &[
            "java.nio.charset.UnsupportedCharsetException",
            "java.nio.charset.IllegalCharsetNameException",
            "java.lang.IllegalArgumentException",
        ],
        Builtin::Charset(CharsetOp::IsSupported) => {
            &["java.nio.charset.IllegalCharsetNameException", "java.lang.IllegalArgumentException"]
        }
        Builtin::Charset(_) => &[],
        // What an object's `toString` throws is a point of its own (`text_objects`).
        Builtin::ObjectText => &[],
        Builtin::StringSame | Builtin::Kept(_) | Builtin::ValueOf => &[],
        // An atomic update of an element, and a view of a byte array's bytes, fail as their
        // operand, the access of the element, does (`lower::handles`).
        Builtin::ElementUpdate(_) | Builtin::ByteView { .. } => &[],
        Builtin::ArraysCopyOf => &["java.lang.NegativeArraySizeException"],
        Builtin::Native(Native::ArrayCopy) if object_copy(args) => {
            &["java.lang.ArrayIndexOutOfBoundsException", "java.lang.ArrayStoreException"]
        }
        Builtin::Native(Native::ArrayCopy) => &["java.lang.ArrayIndexOutOfBoundsException"],
        // A range outside the array, or (`runtime_failures` adds it) the wrong way round.
        Builtin::Arrays(op) if op.ranged() => &["java.lang.ArrayIndexOutOfBoundsException"],
        Builtin::Format if !format_may_fail(class, args) => &[],
        // What the runtime's `format` raises (`format.rs`); a format it does not support ends the
        // program.
        Builtin::Format => &[
            "java.util.DuplicateFormatFlagsException",
            "java.util.FormatFlagsConversionMismatchException",
            "java.util.IllegalFormatArgumentIndexException",
            "java.util.IllegalFormatCodePointException",
            "java.util.IllegalFormatConversionException",
            "java.util.IllegalFormatFlagsException",
            "java.util.IllegalFormatPrecisionException",
            "java.util.IllegalFormatWidthException",
            "java.util.MissingFormatArgumentException",
            "java.util.MissingFormatWidthException",
            "java.util.UnknownFormatConversionException",
        ],
        // A zero divisor.
        Builtin::Unsigned(UnsignedOp::Divide | UnsignedOp::Remainder, _) => {
            let nonzero = matches!(args[1].kind, ExprKind::Int(v) if v != 0)
                || matches!(args[1].kind, ExprKind::Long(v) if v != 0);
            if nonzero { &[] } else { &["java.lang.ArithmeticException"] }
        }
        // Copies of a whole array, an array filled whole (an `Object[]` of the translation takes
        // any object, see `lower::covariant`), text written to the standard error, which drops
        // what it cannot write, as `PrintStream` does.
        Builtin::StringArrayOf | Builtin::ObjectArrayOf | Builtin::ArraysFill => &[],
        Builtin::Native(Native::ErrPrint { .. }) => &[],
        // `CRC32.updateBytes0` runs on what the JDK's code has checked already, as the JVM's does.
        Builtin::Native(Native::Crc32UpdateBytes) => &[],
        _ => &["?"],
    }
}

/// Whether `array[index]` is a row of a table of an enum in table form (`lower/enum_tables.rs`):
/// `fOfConstants[e.ordinal()]` of a constant `e` of that enum, or `fOfConstants[constantOrdinal]`
/// in one of its constant builders. The enum's initialization makes its tables before it builds
/// its constants, so a table that a constant's row is read from is never null, even while the
/// initialization runs; and a table has a row for each constant, so neither index is ever out
/// of its bounds.
fn table_row(class: &Class, m: usize, array: &Expr, index: &Expr) -> bool {
    let ExprKind::Static { class: t, name } = &array.kind else { return false };
    let decl = &class.types[*t];
    if decl.kind != TypeKind::Enum || crate::lower::tabled_field(name).is_none() {
        return false;
    }
    match &index.kind {
        ExprKind::Builtin(Builtin::EnumOrdinal, args) => {
            matches!(args.first().map(|a| a.ty), Some(Type::Enum(e)) if e == decl.name)
        }
        ExprKind::Var(name) => {
            name == crate::lower::TABLE_ORDINAL
                && class.methods.get(m).is_some_and(|method| method.class == *t)
                && class.builds_constants(m)
        }
        _ => false,
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
    // A static that is never null where it is read, as it is or as the value of an assignment.
    fn initialized(facts: &Facts, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Static { class, name } => facts.initialized_statics.contains(&(*class, name.clone())),
            ExprKind::Assign { op: None, target: Place::Var(_), value } => initialized(facts, value),
            ExprKind::Cast(to, value) if to.is_reference() => initialized(facts, value),
            _ => false,
        }
    }
    let initialized = |e: &Expr| initialized(facts, e);
    let null = |e: &Expr| {
        (m != usize::MAX && !tested(e) && !initialized(e) && maybe_null(facts, m, e))
            || matches!(e.kind, ExprKind::Null)
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
        ExprKind::Index(array, index) => {
            let row = table_row(class, m, array, index);
            if null(array) && !row {
                out.push((NPE, "this array access", true, array.id));
            }
            if !facts.in_bounds.contains(&x.id) && !row {
                out.push((INDEX, "this array access", true, x.id));
            }
        }
        ExprKind::Length(array) if null(array) => out.push((NPE, "this length", true, array.id)),
        ExprKind::Switch { selector, .. }
            if matches!(selector.ty, Type::String | Type::Enum(_)) && null(selector) =>
        {
            out.push((NPE, "this switch", true, selector.id))
        }
        ExprKind::Field { object, .. } if null(object) => {
            out.push((NPE, "this field access", object_checked(object), object.id))
        }
        // A string's or a builder's own `charAt`, `subSequence` or `getChars`, as a
        // `CharSequence`, which the dispatcher runs (`codegen::dispatch`) inside `java::catching`.
        // A null `dst` of `getChars` too, once the range is checked.
        ExprKind::MethodCall { receiver, resolved: Some(r), is_super: false, args, .. }
            if sequence_failures(class, facts, *r, receiver).is_some() =>
        {
            if null(receiver) {
                out.push((NPE, "this call", object_checked(receiver), receiver.id));
            }
            for &failure in sequence_failures(class, facts, *r, receiver).unwrap_or_default() {
                out.push((failure, "this call of the library", false, x.id));
            }
            if class.methods[*r].name == "getChars" && null(&args[2]) {
                out.push((NPE, "this call of the library", false, x.id));
            }
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
            | Builtin::Exit
            | Builtin::MainArgs
            | Builtin::NoObject { .. }
            | Builtin::MissingResource
            | Builtin::InitializeClass { .. }
            | Builtin::ClassLiteralName { .. }
            | Builtin::SameClass { .. }
            | Builtin::ClassLiteral(_)
            | Builtin::ClassMethod(_)
            | Builtin::ClassIsInstance
            | Builtin::ComparableClassFor
            // A null is a value to them, and an object's own method is a call of its own.
            | Builtin::ObjectsEquals
            | Builtin::ObjectsHashCode => {}
            // The class of an object that may be null, which is dereferenced with the checked form
            // (`Gen::after_object`).
            Builtin::GetClass | Builtin::ClassName { .. } => {
                if null(&args[0]) {
                    out.push((NPE, "this call", true, args[0].id));
                }
            }
            // An enum that may be null is dereferenced with the checked form (`Gen::enum_value`),
            // the argument of `compareTo` too.
            Builtin::EnumOrdinal | Builtin::EnumName | Builtin::EnumCompareTo => {
                for arg in args.iter().filter(|a| matches!(a.ty, Type::Enum(_)) && null(a)) {
                    out.push((NPE, "this call", true, arg.id));
                }
            }
            // The receiver, a string, is dereferenced with the checked form (`Facts::null_checked`).
            Builtin::StringLength | Builtin::StringEquals => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", true, args[0].id));
                }
            }
            Builtin::StringCharAt => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this call", true, args[0].id));
                }
                if !facts.index_in_bounds.contains(&x.id) {
                    out.push(("java.lang.StringIndexOutOfBoundsException", "this charAt", true, x.id));
                }
            }
            Builtin::Boxed(BoxedOp::Parse | BoxedOp::ParseRadix, _) => {
                out.push(("java.lang.NumberFormatException", "this parse", true, x.id));
            }
            // `decode` calls `isEmpty` on its string. The runtime gives the failure of a null one
            // with Java's message, but only where nothing catches it, as a `NullPointerException`
            // that a `catch` sees is a dereference's (`Facts::null_checked`).
            Builtin::Boxed(BoxedOp::Decode, _) => {
                if args.first().is_some_and(null) {
                    out.push((NPE, "this decode", false, x.id));
                }
                out.push(("java.lang.NumberFormatException", "this decode", true, x.id));
            }
            Builtin::Boxed(
                BoxedOp::Box
                | BoxedOp::ToString
                | BoxedOp::HexString
                | BoxedOp::Compare
                | BoxedOp::HashCode
                | BoxedOp::Sum
                | BoxedOp::FloatTest(_)
                | BoxedOp::Logical(_),
                _,
            ) => {}
            // Only a lock that may be null throws, as a dereference does: the code generator
            // checks no other (`codegen::sync`).
            Builtin::MonitorEnter if null(&args[0]) => out.push((NPE, "this synchronized", true, args[0].id)),
            Builtin::MonitorEnter => {}
            // A class name that is not in the translation (`lower::by_name`).
            Builtin::UnknownClass => out.push(("java.lang.ClassNotFoundException", "this class name", false, x.id)),
            // A class that is not the object's; a `catch` sees it through `java::catching`.
            Builtin::ClassCheck => out.push(("java.lang.ClassCastException", "this cast", false, x.id)),
            // An object of a class that is not `Cloneable` refuses to be copied.
            Builtin::ObjectClone => {
                let refused = class
                    .class_index(args[0].ty)
                    .is_some_and(|t| class.hierarchy.concretes(t).iter().any(|&c| !class.types[c].cloneable));
                if refused {
                    out.push(("java.lang.CloneNotSupportedException", "this clone", true, x.id));
                }
            }
            Builtin::NullChecked | Builtin::Unmatched => {
                if null(&args[0]) {
                    out.push((NPE, "this null check", true, args[0].id));
                }
            }
            // A zero divisor throws, as in a division.
            Builtin::Native(Native::Math { name: "floorDiv" | "floorMod" | "ceilDiv" | "ceilMod", .. }) => {
                let constant = matches!(args[1].kind, ExprKind::Int(v) if v != 0)
                    || matches!(args[1].kind, ExprKind::Long(v) if v != 0);
                if !constant {
                    out.push(("java.lang.ArithmeticException", "this division", true, x.id));
                }
            }
            // `MIN_VALUE / -1` overflows too, so only another constant divisor is known to pass.
            Builtin::Native(Native::Math { name: "ceilDivExact", .. }) => {
                let constant = matches!(args[1].kind, ExprKind::Int(v) if v != 0 && v != -1)
                    || matches!(args[1].kind, ExprKind::Long(v) if v != 0 && v != -1);
                if !constant {
                    out.push(("java.lang.ArithmeticException", "this division", true, x.id));
                }
            }
            // An atomic update of an element fails as its store does, which is its operand.
            Builtin::ElementUpdate(_) => {}
            // Comparing strings, and the strings that the call or the text in them makes, fail as
            // those operands do (`analysis::string_identity`).
            Builtin::StringSame | Builtin::Kept(_) | Builtin::ValueOf => {}
            // A view of a byte array's bytes fails as its element access does, and also where
            // the index is in bounds but the bytes from it on are too few for the value.
            Builtin::ByteView { .. } => out.push((INDEX, "this array access", true, args[0].id)),
            // Each array that may be null is dereferenced with the checked form, then the
            // positions are checked, with the checked form of the copy.
            Builtin::Native(Native::ArrayCopy) => {
                for array in [&args[0], &args[2]] {
                    if null(array) {
                        out.push((NPE, "this call", true, array.id));
                    }
                }
                out.push(("java.lang.ArrayIndexOutOfBoundsException", "this call of the library", true, x.id));
                // An array of objects may be of another type than the destination's: the copy's
                // checked form returns the mismatch (`codegen/dispatch.rs`).
                if object_copy(args) {
                    out.push(("java.lang.ArrayStoreException", "this call of the library", true, x.id));
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
                        // A null string is the `NumberFormatException` of `parse_radix` and
                        // `parse_unsigned`.
                        | Builtin::Bits(BitsOp::ParseRadix, _)
                        | Builtin::Unsigned(UnsignedOp::Parse, _)
                        | Builtin::Charset(CharsetOp::ForName | CharsetOp::IsSupported | CharsetOp::Equals)
                        | Builtin::Native(Native::SystemIdentityHash)
                        // The JVM's `getSystemTimeZoneID` gives null for a null Java home.
                        | Builtin::Native(Native::SystemTimeZoneId)
                        // The expected and the new element of an atomic update may be null.
                        | Builtin::ElementUpdate(_)
                );
                // Appending or inserting a null string, builder or boxed value writes "null" (the
                // generator's `appended`); a null `char[]` throws.
                let text = |e: &Expr| {
                    matches!(e.ty, Type::Null | Type::String | Type::StringBuilder | Type::Boxed(_))
                };
                let writes_null = match builtin {
                    Builtin::Builder(BuilderOp::Append) => {
                        args.len() == 2 && text(&args[1])
                            || args.len() == 4 && (text(&args[1]) || matches!(args[1].ty, Type::Class(c, _) if c == CHAR_SEQUENCE))
                    }
                    Builtin::Builder(BuilderOp::Insert) => args.len() == 3 && text(&args[2]),
                    _ => false,
                };
                let checked = if takes_null
                    || writes_null
                    || matches!(builtin, Builtin::String(StringOp::Join) | Builtin::Format)
                {
                    &args[..args.len().min(1)]
                } else {
                    &args[..]
                };
                // Only the first may be the receiver, which is dereferenced with the checked
                // form; the code generator rejects a call that does not (`check_dereferences`).
                // The buffer of a descriptor native, after its descriptor, is dereferenced with the
                // checked form (`Gen::handle`).
                if let Builtin::Native(Native::FileReadBytes | Native::FileWriteBytes) = builtin {
                    if null(&args[1]) {
                        out.push((NPE, "this call", true, args[1].id));
                    }
                } else if let Builtin::Native(Native::CopyMemory { .. } | Native::SetMemory)
                | Builtin::RawMemory { .. } = builtin
                {
                    // A null base is an address outside the heap, which ends the program.
                } else if let Builtin::Arrays(ArraysOp::BinarySearch | ArraysOp::BinarySearchRange) = builtin {
                    // The array is dereferenced with the checked form, and so is a key that may
                    // be null, which fails where it is compared with an element; where elements
                    // may be null, comparing either fails in the search (`Gen::arrays_op`).
                    let key = &args[args.len() - 1];
                    if null(&args[0]) {
                        out.push((NPE, "this call", true, args[0].id));
                    }
                    if facts.nullable_elements.contains(&args[0].ty.element()) {
                        out.push((NPE, "this call", true, x.id));
                    } else if null(key) {
                        out.push((NPE, "this call", true, key.id));
                    }
                } else if !takes_null && checked.iter().any(null) {
                    let receiver = !checked[1..].iter().any(null);
                    out.push((NPE, "this call", receiver, if receiver { args[0].id } else { x.id }));
                }
                let failures = library_failure(class, m, builtin, args);
                if !failures.is_empty() && !constant_pattern(class, m, builtin, args) {
                    let checked = matches!(
                        builtin,
                        Builtin::String(
                            StringOp::Substring | StringOp::FromChars | StringOp::GetChars | StringOp::FromBytes
                                | StringOp::TranslateEscapes
                        )
                            | Builtin::ArraysCopyOf
                            | Builtin::Bits(BitsOp::ParseRadix | BitsOp::ParseRange, _)
                            | Builtin::Unsigned(UnsignedOp::Parse | UnsignedOp::ParseRange, _)
                            | Builtin::CodePoint(CodePointOp::ToCharsInto)
                            | Builtin::EnumValueOf
                            | Builtin::NotEnumClass
                            | Builtin::CheckFromToIndex
                            | Builtin::CheckIndex
                            | Builtin::Native(Native::ArrayCopy)
                            | Builtin::Native(Native::MonitorWait | Native::MonitorNotify { .. })
                            | Builtin::Native(
                                Native::FileOpenRead
                                    | Native::FileOpenWrite
                                    | Native::FileRead
                                    | Native::FileReadBytes
                                    | Native::FileAvailable
                                    | Native::FileSkip
                                    | Native::FileWrite
                                    | Native::FileWriteBytes
                                    | Native::FileLength
                                    | Native::FilePosition
                                    | Native::FileOpenRandom
                                    | Native::FileSeek
                                    | Native::FileSetLength
                                    | Native::FileSystem(_)
                                    | Native::SecureSeed
                            )
                    ) || (facts.interrupts && matches!(builtin, Builtin::Native(Native::Sleep { .. })))
                        // A range of a text, not of a `char[]`, has the checked form.
                        || (matches!(builtin, Builtin::Builder(BuilderOp::Append))
                            && args.len() == 4
                            && !matches!(args[1].ty, Type::Array(_)));
                    for &failure in failures {
                        out.push((failure, "this call of the library", checked, x.id));
                    }
                }
                if let Builtin::Arrays(op) = builtin
                    && op.ranged()
                {
                    out.push(("java.lang.IllegalArgumentException", "this call of the library", false, x.id));
                }
                if let Builtin::Native(Native::MonitorWait) = builtin
                    && !matches!(args[1].kind, ExprKind::Long(n) if n >= 0)
                {
                    out.push(("java.lang.IllegalArgumentException", "this call of the library", true, x.id));
                }
                // In a program that interrupts a thread, what waits ends with an interrupt.
                if facts.interrupts
                    && matches!(builtin, Builtin::Native(Native::Sleep { .. } | Native::ThreadJoin | Native::MonitorWait))
                {
                    out.push(("java.lang.InterruptedException", "this call of the library", true, x.id));
                }
            }
        },
        _ => {}
    }
    if let ExprKind::Assign { target: Place::Index(array, index), .. }
    | ExprKind::IncDec { target: Place::Index(array, index), .. } = &x.kind
    {
        let row = table_row(class, m, array, index);
        if null(array) && !row {
            out.push((NPE, "this array access", true, array.id));
        }
        if !facts.in_bounds.contains(&x.id) && !row {
            out.push((INDEX, "this array access", true, x.id));
        }
    }
    if let ExprKind::Assign { target: Place::Field { object, .. }, .. }
    | ExprKind::IncDec { target: Place::Field { object, .. }, .. } = &x.kind
        && null(object)
    {
        out.push((NPE, "this field access", object_checked(object), object.id));
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
        && !only_under(class, facts, s, t)
    {
        out.push(("java.lang.ClassCastException", "this cast", true, x.id));
    }
    // So may a cast of an object to a string, a box, an enum or a class, unless an `instanceof`
    // tested it.
    if let ExprKind::Cast(to, value) = &x.kind
        && x.pos != value.pos
        && matches!(value.ty, Type::Class(..))
        && matches!(to, Type::String | Type::Boxed(_) | Type::Enum(_) | Type::JavaClass)
        && let Some(name) = instance_name(*to)
        && !matches!(&value.kind, ExprKind::Var(v) if safe.contains(&instance_fact(v, name)))
    {
        out.push(("java.lang.ClassCastException", "this cast", true, x.id));
    }
    // A box that may be null is unboxed by arithmetic and comparisons, but not by `==` or `!=`
    // with an operand that is no primitive (`v == null`), which compares references.
    // The right operand of `&&` (`||`) runs once the left one is true (false): `v != null && v`
    // unboxes a `v` that is not null.
    if let ExprKind::Binary(op, l, r) = &x.kind {
        for (operand, other) in [(l, r), (r, l)] {
            let reference =
                matches!(op, BinOp::Eq | BinOp::Ne) && !(other.ty.is_numeric() || other.ty == Type::Boolean);
            let checked = std::ptr::eq(operand, r)
                && matches!(&r.kind, ExprKind::Var(v) if checks_null(l, v, *op == BinOp::And))
                && matches!(op, BinOp::And | BinOp::Or);
            if matches!(operand.ty, Type::Boxed(_))
                && null(operand)
                && !matches!(x.ty, Type::String)
                && !reference
                && !checked
            {
                out.push((NPE, "this unboxing", false, x.id));
            }
        }
    }
    out
}
