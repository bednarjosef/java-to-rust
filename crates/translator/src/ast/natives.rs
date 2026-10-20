//! The JDK methods that bottom out in the JVM, which the runtime implements by hand
//! (`java_runtime::natives`, `math`, `fdlibm`, `out`).

/// A call into the native layer; its arguments are those of the `Builtin` that holds it, with
/// the receiver first for instance methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// A `Math` or `StrictMath` function beyond `abs`, `max`, `min`, `sqrt` and `Math.pow`, by its
    /// Java name; `strict` for `StrictMath`.
    Math {
        name: &'static str,
        strict: bool,
    },
    /// `System.arraycopy(src, srcPos, dest, destPos, length)`.
    ArrayCopy,
    DoubleToLongBits,
    DoubleToRawLongBits,
    LongBitsToDouble,
    FloatToIntBits,
    FloatToRawIntBits,
    IntBitsToFloat,
    /// `System.identityHashCode(o)` or an `o.hashCode()` that resolves to `Object`'s.
    IdentityHash,
    CurrentTimeMillis,
    NanoTime,
    /// `Runtime.getRuntime().availableProcessors()`.
    AvailableProcessors,
    /// `System.lineSeparator()` and `System.getProperty("line.separator")`.
    LineSeparator,
    /// `System.err.print(x)` and `System.err.println(x)`.
    ErrPrint {
        newline: bool,
    },
    /// `System.out.write(b)`.
    OutWrite,
    /// `System.out.flush()`.
    OutFlush,
    /// `System.out.print(char[])` and `println(char[])`.
    PrintChars {
        newline: bool,
    },
}

/// The `Math` and `StrictMath` functions the native layer covers (beyond `abs`, `max`, `min`).
pub const MATH_FUNCTIONS: &[&str] = &[
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "exp",
    "log",
    "log10",
    "pow",
    "sqrt",
    "cbrt",
    "hypot",
    "expm1",
    "log1p",
    "sinh",
    "cosh",
    "tanh",
    "IEEEremainder",
    "floor",
    "ceil",
    "rint",
    "round",
    "floorDiv",
    "floorMod",
    "signum",
    "ulp",
    "nextUp",
    "fma",
    "toRadians",
    "toDegrees",
];
