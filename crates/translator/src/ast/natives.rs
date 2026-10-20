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
    /// `System.getProperty(key)` for a key whose value does not depend on the JVM (see
    /// `lower/system.rs`), by its literal key.
    SystemProperty(&'static str),
    /// `Runtime.getRuntime().totalMemory()`, `freeMemory()` and `maxMemory()`.
    TotalMemory,
    FreeMemory,
    MaxMemory,
    /// `Runtime.getRuntime().gc()`, which does nothing.
    Gc,
    /// `Runtime.halt(status)`: exits without running shutdown hooks, which translated programs
    /// do not have.
    Halt,
    /// `Thread.sleep(millis)`, or `Thread.sleep(millis, nanos)` when `nanos`.
    Sleep {
        nanos: bool,
    },
    /// `Thread.onSpinWait()`.
    OnSpinWait,
    /// `Thread.yield()`.
    ThreadYield,
    /// `FileInputStream.open0(name)`: opens the file and returns its descriptor.
    FileOpenRead,
    /// `FileOutputStream.open0(name, append)`: creates or opens the file and returns its descriptor.
    FileOpenWrite,
    /// The natives of a stream that act on its descriptor, which is their first argument: the
    /// stream's `fd` as an `int` (the object model supplies it); `read0()`.
    FileRead,
    /// `readBytes(b, off, len)`.
    FileReadBytes,
    /// `available0()`.
    FileAvailable,
    /// `skip0(n)`.
    FileSkip,
    /// `close0()`, of a `FileInputStream`, `FileOutputStream` or `FileDescriptor`.
    FileClose,
    /// `write(b, append)`; `append` is decided when the file is opened, so it is not an argument.
    FileWrite,
    /// `writeBytes(b, off, len, append)`.
    FileWriteBytes,
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
    "nextDown",
    "fma",
    "addExact",
    "subtractExact",
    "multiplyExact",
    "incrementExact",
    "decrementExact",
    "negateExact",
    "absExact",
    "toIntExact",
    "copySign",
    "clamp",
    "toRadians",
    "toDegrees",
];
