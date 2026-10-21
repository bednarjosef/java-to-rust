//! The JDK methods that bottom out in the JVM, which the runtime implements by hand
//! (`java_runtime::natives`, `math`, `fdlibm`, `out`, `fd`, `vm`).

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
    /// An `o.hashCode()` that resolves to `Object`'s.
    IdentityHash,
    /// `System.identityHashCode(o)`, which is 0 for a null `o`.
    SystemIdentityHash,
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
    /// `System.err.print(char[])` and `println(char[])`.
    ErrPrintChars {
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
    /// The natives of the translator's `Thread` (`java/Threads.java`): the number of the next
    /// thread created without a name, and the id of the next thread.
    ThreadNumber,
    ThreadId,
    /// The id of the main thread.
    MainThreadId,
    /// Whether the calling thread has its `Thread` object, `setCurrent0(t)` that gives it one,
    /// and `current0()` that returns it.
    HasCurrentThread,
    SetCurrentThread,
    CurrentThread,
    /// `start0(t, tid, name, daemon)`, to which lowering adds the call `t.run()` that the new
    /// thread makes, as a fifth argument that is not evaluated where the native is.
    ThreadStart,
    /// `alive0(tid)`, `join0(tid, millis)` and `rename0(tid, name)`.
    ThreadAlive,
    ThreadJoin,
    ThreadRename,
    /// `interrupt0(tid)`, `isInterrupted0(tid)`, and `clearInterrupt0()` of `Thread.interrupted()`,
    /// which reads and clears the calling thread's status.
    ThreadInterrupt,
    ThreadIsInterrupted,
    ThreadClearInterrupt,
    /// `installUncaught0()`, to which lowering adds the call `Thread.dispatchUncaught()` that the
    /// runtime makes on a thread that an exception nothing catches ends; `uncaught0()`, that
    /// exception there; and `printUncaught0(name, e)`, how the runtime prints it without a handler.
    InstallUncaught,
    UncaughtException,
    PrintUncaught,
    /// `o.wait()` and `o.wait(millis)`: the object, and the timeout in milliseconds (0 for none).
    MonitorWait,
    /// `o.notify()`, or with `all` `o.notifyAll()`: the object.
    MonitorNotify {
        all: bool,
    },
    /// `Thread.holdsLock(o)`: the object.
    HoldsLock,
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
    /// A native that does nothing in a translated program, such as `registerNatives()`: its
    /// statement is dropped.
    Nothing,
    /// `CDS.getCDSConfigStatus()`.
    CdsConfigStatus,
    /// `CDS.getRandomSeedForDumping()`.
    CdsRandomSeed,
    /// `ContinuationSupport.isSupported0()`.
    ContinuationsSupported,
    /// `VM.getNanoTimeAdjustment(offsetInSeconds)`.
    NanoTimeAdjustment,
    /// `Unsafe.arrayBaseOffset(c)` of a class literal `c` of an array class.
    ArrayBaseOffset,
    /// `Unsafe.arrayIndexScale(c)` of a class literal of an array class, by the element type: a
    /// primitive type, or `java.lang.Object` for every reference type.
    ArrayIndexScale(&'static str),
    /// `FileDescriptor.getHandle(fd)`.
    DescriptorHandle,
    /// `FileDescriptor.getAppend(fd)`.
    DescriptorAppend,
    /// `CRC32.update(crc, b)`.
    Crc32Update,
    /// `CRC32.updateBytes0(crc, b, off, len)`.
    Crc32UpdateBytes,
}

/// The native methods of translated JDK classes that the runtime implements and whose calls
/// become a `Native` of their arguments as they are, by class, name and javac's signature. (The
/// stream natives of `java.io`, which take their object's descriptor, are lowered on their own.)
pub fn declared_native(class: &str, name: &str, signature: &str) -> Option<Native> {
    Some(match (class, name, signature) {
        // Registering a class's natives with the JVM, and looking up the fields they use.
        (_, "registerNatives" | "initIDs", "()void") => Native::Nothing,
        // Registers `getNanoTimeAdjustment`, and nothing else.
        ("jdk.internal.misc.VM", "initialize", "()void") => Native::Nothing,
        ("jdk.internal.misc.VM", "getNanoTimeAdjustment", "(long)long") => Native::NanoTimeAdjustment,
        ("jdk.internal.misc.CDS", "getCDSConfigStatus", "()int") => Native::CdsConfigStatus,
        ("jdk.internal.misc.CDS", "getRandomSeedForDumping", "()long") => Native::CdsRandomSeed,
        // Without an archive there is nothing to restore. Its argument is a class literal, which
        // the lowering drops with the call (`class_literal_call`).
        ("jdk.internal.misc.CDS", "initializeFromArchive", "(java.lang.Class<?>)void") => Native::Nothing,
        ("jdk.internal.vm.ContinuationSupport", "isSupported0", "()boolean") => {
            Native::ContinuationsSupported
        }
        ("java.io.FileDescriptor", "getHandle", "(int)long") => Native::DescriptorHandle,
        ("java.io.FileDescriptor", "getAppend", "(int)boolean") => Native::DescriptorAppend,
        ("java.util.zip.CRC32", "update", "(int,int)int") => Native::Crc32Update,
        ("java.util.zip.CRC32", "updateBytes0", "(int,byte[],int,int)int") => Native::Crc32UpdateBytes,
        ("java.lang.Thread", "nextThreadNum", "()int") => Native::ThreadNumber,
        ("java.lang.Thread", "nextThreadId", "()long") => Native::ThreadId,
        ("java.lang.Thread", "mainId0", "()long") => Native::MainThreadId,
        ("java.lang.Thread", "hasCurrent0", "()boolean") => Native::HasCurrentThread,
        ("java.lang.Thread", "setCurrent0", "(java.lang.Thread)void") => Native::SetCurrentThread,
        ("java.lang.Thread", "current0", "()java.lang.Thread") => Native::CurrentThread,
        ("java.lang.Thread", "start0", "(java.lang.Thread,long,java.lang.String,boolean)void") => {
            Native::ThreadStart
        }
        ("java.lang.Thread", "alive0", "(long)boolean") => Native::ThreadAlive,
        ("java.lang.Thread", "join0", "(long,long)void") => Native::ThreadJoin,
        ("java.lang.Thread", "rename0", "(long,java.lang.String)void") => Native::ThreadRename,
        ("java.lang.Thread", "interrupt0", "(long)void") => Native::ThreadInterrupt,
        ("java.lang.Thread", "isInterrupted0", "(long)boolean") => Native::ThreadIsInterrupted,
        ("java.lang.Thread", "clearInterrupt0", "()boolean") => Native::ThreadClearInterrupt,
        ("java.lang.Thread", "installUncaught0", "()void") => Native::InstallUncaught,
        ("java.lang.Thread", "uncaught0", "()java.lang.Throwable") => Native::UncaughtException,
        ("java.lang.Thread", "printUncaught0", "(java.lang.String,java.lang.Throwable)void") => {
            Native::PrintUncaught
        }
        _ => return None,
    })
}

/// Whether the runtime implements a native method of a translated JDK class: one of
/// `declared_native`, or a stream native of `java.io`, which `lower/natives.rs` lowers with the
/// descriptor of the stream's object.
pub fn runtime_native(class: &str, name: &str, signature: &str) -> bool {
    declared_native(class, name, signature).is_some()
        || matches!(
            (class, name),
            ("java.io.FileInputStream", "open0" | "read0" | "readBytes" | "available0" | "skip0" | "close0")
                | ("java.io.FileOutputStream", "open0" | "write" | "writeBytes" | "close0")
                | ("java.io.FileDescriptor", "close0")
                // Lowered as code on the referent (`lower::references`).
                | ("java.lang.ref.Reference", "get0" | "refersTo0" | "clear0")
                | ("java.lang.ref.PhantomReference", "refersTo0" | "clear0")
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natives_of_jdk_classes_are_found_by_class_name_and_signature() {
        // The classes of `java.io` and `java.util.zip` cannot be stood in for in a test (java.base
        // exports them, so javac reads the JDK's), so their mapping is checked here.
        let found = |class, name, signature| declared_native(class, name, signature);
        assert_eq!(
            found("java.io.FileDescriptor", "getAppend", "(int)boolean"),
            Some(Native::DescriptorAppend)
        );
        assert_eq!(found("java.io.FileDescriptor", "getHandle", "(int)long"), Some(Native::DescriptorHandle));
        assert_eq!(found("java.io.FileInputStream", "initIDs", "()void"), Some(Native::Nothing));
        assert_eq!(found("java.nio.MappedMemoryUtils", "registerNatives", "()void"), Some(Native::Nothing));
        assert_eq!(found("java.util.zip.CRC32", "update", "(int,int)int"), Some(Native::Crc32Update));
        assert_eq!(
            found("java.util.zip.CRC32", "updateBytes0", "(int,byte[],int,int)int"),
            Some(Native::Crc32UpdateBytes)
        );
        // Another overload, or another class's method of the same name, is not the native.
        assert_eq!(found("java.util.zip.CRC32", "update", "(byte[],int,int)void"), None);
        assert_eq!(found("jdk.internal.misc.Unsafe", "initialize", "()void"), None);
        assert!(runtime_native("java.io.FileInputStream", "readBytes", "(byte[],int,int)int"));
        assert!(!runtime_native("java.lang.Throwable", "fillInStackTrace", "(int)java.lang.Throwable"));
    }
}
