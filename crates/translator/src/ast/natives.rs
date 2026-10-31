//! The JDK methods that bottom out in the JVM, which the runtime implements by hand
//! (`java_runtime::natives`, `math`, `fdlibm`, `out`, `fd`, `vm`).

use super::{ChannelCall, channel_call, channel_nothing};

/// A call into the native layer; its arguments are those of the `Builtin` that holds it, with
/// the receiver first for instance methods.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// A `Math` or `StrictMath` function beyond `abs`, `max`, `min`, `sqrt` and `Math.pow`, by its
    /// Java name; `strict` for `StrictMath`.
    Math {
        #[serde(deserialize_with = "crate::ast::static_str")]
        name: super::StaticStr,
        strict: bool,
    },
    /// `System.arraycopy(src, srcPos, dest, destPos, length)`.
    ArrayCopy,
    /// A native of a whole program that the runtime does not implement, named `what` (its class,
    /// name and signature). Where it can run, it rejects the program (`analysis::live_methods`);
    /// elsewhere it is code that never runs, as in the branch of `Inflater.inflate` that only a
    /// direct buffer takes.
    Missing {
        #[serde(deserialize_with = "crate::ast::static_str")]
        what: super::StaticStr,
    },
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
    /// `System.err.flush()`.
    ErrFlush,
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
    SystemProperty(#[serde(deserialize_with = "crate::ast::static_str")] super::StaticStr),
    /// `System.getenv(name)`: the variable of the environment the program runs in, or null.
    Getenv,
    /// The patched `ProcessEnvironment.environ0()`: every entry of the environment the program
    /// runs in, each ending in a zero byte (`java_runtime::environ`).
    Environ,
    /// `Runtime.getRuntime().totalMemory()`, `freeMemory()` and `maxMemory()`.
    TotalMemory,
    FreeMemory,
    MaxMemory,
    /// `Runtime.getRuntime().gc()`, which does nothing.
    Gc,
    /// `Runtime.halt(status)`: ends the process at once, without running the shutdown hooks.
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
    /// `state0(tid)`: what the thread is doing, by `Thread.State`'s ordinal, or 0 without a record.
    ThreadState,
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
    /// Gives the runtime the text of the stack trace of an uncaught exception: the argument, a call
    /// of `Throwable.uncaughtTrace()` that lowering puts at the start of `main` (see
    /// `lower/natives.rs`), which is not evaluated where the native is, and which reads the
    /// exception with `Throwable.uncaught0()` (`UncaughtException`).
    InstallTrace,
    /// `installShutdown0()` of the translator's `Thread`, to which lowering adds the call
    /// `Thread.runShutdownHooks()` that the runtime makes, once, as the program exits.
    InstallShutdown,
    /// `Unsafe.park(absolute, time)` of `LockSupport`: waits for the calling thread's permit, until
    /// the deadline `time` (milliseconds since the epoch) when `absolute`, else for `time`
    /// nanoseconds, or without a limit for 0.
    Park,
    /// `Unsafe.unpark(thread)`: gives the thread its permit. Lowering passes the thread's id.
    Unpark,
    /// `o.wait()`, `o.wait(millis)` and `o.wait(millis, nanos)`: the object, the timeout in
    /// milliseconds (0 for none), and any nanoseconds.
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
    /// `RandomAccessFile.open0(name, mode)`: opens the file to read, or to read and write, and
    /// returns its descriptor.
    FileOpenRandom,
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
    /// `FileInputStream.isRegularFile0(fd)`, `length0()` and `position0()`.
    FileIsRegular,
    FileLength,
    FilePosition,
    /// `RandomAccessFile.seek0(pos)` and `setLength0(n)`; its `read0`, `readBytes0`, `write0`,
    /// `writeBytes0`, `length0` and `getFilePointer` are `FileRead` and the others.
    FileSeek,
    FileSetLength,
    /// A native of `java.io.UnixFileSystem` by its name (`getBooleanAttributes0`, `list0`, ...),
    /// whose `File` arguments are their paths (`java_runtime::fs`).
    FileSystem(#[serde(deserialize_with = "crate::ast::static_str")] super::StaticStr),
    /// A native of the translator's `sun.nio.fs.UnixNativeDispatcher`
    /// (`java/patches/sun/nio/fs/UnixNativeDispatcher.java`) by its name (`open0`, `stat0`, ...):
    /// a system call of `java.nio.file` on the bytes of a path, which records its `errno` rather
    /// than failing (`java_runtime::unix`, `UNIX_CALLS`).
    Unix(#[serde(deserialize_with = "crate::ast::static_str")] super::StaticStr),
    /// A native of the translator's `java.net.TranslatedNet`
    /// (`java/overlay/java/net/TranslatedNet.java`) by its name, or `InetAddress`'s tests of the
    /// address families: a call of the C library's name service on the bytes of a name or an
    /// address, which records its error rather than failing (`java_runtime::net`, `NET_CALLS`).
    Net(#[serde(deserialize_with = "crate::ast::static_str")] super::StaticStr),
    /// A native of the translator's `sun.nio.ch.TranslatedSockets`, by name: a system call on a
    /// socket, a pipe or a selector, which records its error rather than failing
    /// (`java_runtime::socket`, `SOCKET_CALLS`).
    Socket(#[serde(deserialize_with = "crate::ast::static_str")] super::StaticStr),
    /// The natives of the stream under the translator's `System.out` and `System.err` objects
    /// (`java/overlay/java/io/StandardOutput.java`), the runtime's buffered standard output and
    /// error (`java_runtime::out`): `write0(fd, b)`, `writeBytes0(fd, b, off, len)`,
    /// `flush0(fd)` and `close0(fd)`.
    StandardWrite,
    StandardWriteBytes,
    StandardFlush,
    StandardClose,
    /// A native that does nothing in a translated program, such as `registerNatives()`: its
    /// statement is dropped.
    Nothing,
    /// A native of `sun.nio.ch` that a file channel calls (`ast/channels.rs`).
    Channel(#[serde(deserialize_with = "crate::ast::leaked")] &'static ChannelCall),
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
    /// `java.lang.reflect.Array`'s `newArray(componentType, length)`, `getLength(array)` and
    /// `set(array, index, value)`, on the arrays that become `Object`s: the generator matches the
    /// component type's name, or the array's variant, against those of the program.
    ReflectNewArray,
    ReflectArrayLength,
    ReflectArraySet,
    /// `Unsafe.arrayIndexScale(c)` of a class literal of an array class, by the element type: a
    /// primitive type, or `java.lang.Object` for every reference type.
    ArrayIndexScale(#[serde(deserialize_with = "crate::ast::static_str")] super::StaticStr),
    /// `FileDescriptor.getHandle(fd)`.
    DescriptorHandle,
    /// `FileDescriptor.getAppend(fd)`.
    DescriptorAppend,
    /// `Unsafe.fullFence()`, and so its `loadFence()` and `storeFence()`: a fence that orders
    /// every memory access before it against every one after it.
    FullFence,
    /// `Unsafe.copyMemory0(srcBase, srcOffset, destBase, destOffset, bytes)` between two arrays of
    /// primitives, by their bytes (`java::copy_memory`); with `swap`, `copySwapMemory0`, whose last
    /// argument is the size of the units whose bytes it reverses.
    CopyMemory {
        swap: bool,
    },
    /// `Unsafe.allocateMemory0(bytes)`: a block of memory outside the heap, by its address
    /// (`java::allocate_native`, see the runtime's `native_memory`).
    AllocateNative,
    /// The system's page size, `UnsafeConstants.PAGE_SIZE` of the translator's
    /// (`java/patches/jdk/internal/misc/UnsafeConstants.java`).
    PageSize,
    /// `Unsafe.reallocateMemory0(address, bytes)`.
    ReallocateNative,
    /// `Unsafe.freeMemory0(address)`.
    FreeNative,
    /// `Unsafe.setMemory0(base, offset, bytes, value)`: in an array of primitives held as an
    /// `Object`, or with a null base at an address outside the heap.
    SetMemory,
    /// `CRC32.update(crc, b)`.
    Crc32Update,
    /// `CRC32.updateBytes0(crc, b, off, len)`.
    Crc32UpdateBytes,
    /// `CRC32.updateByteBuffer0(crc, address, off, len)`, of memory outside the heap.
    Crc32UpdateBuffer,
    /// `Adler32.update(adler, b)`, `updateBytes(adler, b, off, len)` and
    /// `updateByteBuffer(adler, address, off, len)`.
    Adler32Update,
    Adler32UpdateBytes,
    Adler32UpdateBuffer,
    /// `ProcessHandleImpl.getCurrentPid0()`.
    CurrentPid,
    /// `ProcessHandleImpl.isAlive0(pid)`: the process's start time, or -1 when it does not exist.
    ProcessStartTime,
    /// `ProcessImpl.spawn0(prog, argBlock, argc, envBlock, envc, dir, fds, redirectErrorStream)`
    /// of the translator's patch (`java/patches/java/lang/ProcessImpl.java`): starts a process.
    Spawn,
    /// `ProcessHandleImpl.waitForProcessExit0(pid, reap)`.
    WaitForProcess,
    /// `ProcessHandleImpl.destroy0(pid, startTime, force)`.
    DestroyProcess,
    /// `Reference.hasReferencePendingList()`: whether the collector cleared the referent of a
    /// reference with a queue that the program has not taken yet (`java::has_pending_references`).
    HasPendingReferences,
    /// `Reference.waitForReferencePendingList()`: waits until one is.
    WaitForPendingReferences,
    /// `TranslatedReferences.register(reference, kind, queued)` of the translator's overlay:
    /// registers a reference with the collector (`java::register_reference`).
    RegisterReference,
    /// `TranslatedReferences.next()`: a reference whose referent the collector cleared, or null
    /// (`java::take_pending_reference`).
    NextPendingReference,
    /// `Unsafe.getLoadAverage0(loadavg, nelems)`.
    LoadAverage,
    /// A native of `com.sun.management.internal.OperatingSystemImpl` or
    /// `sun.management.VMManagementImpl` without arguments, by the runtime's function that
    /// answers it (`os_total_memory`, `vm_startup_time`, ...).
    OperatingSystem(#[serde(deserialize_with = "crate::ast::static_str")] super::StaticStr),
    /// `MappedMemoryUtils.isLoaded0(address, length, pageCount)`: a mapping is a block of memory
    /// of the process holding the file's bytes, so it is loaded.
    MappingLoaded,
    /// `SecureRandom.getSeed(numBytes)`: seed bytes from the operating system (`random.rs`).
    SecureSeed,
    /// `Inflater`'s natives (`inflate.rs`): `init(nowrap)`, then `inflateBytesBytes` and its buffer
    /// forms (`inflateBytesBuffer`, `inflateBufferBytes`, `inflateBufferBuffer`), `getAdler`,
    /// `reset`, `end`, and `setDictionary` with its buffer form, on the handle that `init` returned.
    InflateInit,
    InflateBytes,
    InflateBytesBuffer,
    InflateBufferBytes,
    InflateBufferBuffer,
    InflateAdler,
    InflateReset,
    InflateEnd,
    InflateSetDictionary,
    InflateSetDictionaryBuffer,
    /// `Deflater`'s natives (`deflate.rs`): `init(level, strategy, nowrap)`, then
    /// `deflateBytesBytes` and its buffer forms (`deflateBytesBuffer`, `deflateBufferBytes`,
    /// `deflateBufferBuffer`), `getAdler`, `reset`, `end`, and `setDictionary` with its buffer
    /// form, on the handle that `init` returned.
    DeflateInit,
    DeflateBytes,
    DeflateBytesBuffer,
    DeflateBufferBytes,
    DeflateBufferBuffer,
    DeflateAdler,
    DeflateReset,
    DeflateEnd,
    DeflateSetDictionary,
    DeflateSetDictionaryBuffer,
    /// `TimeZone.getSystemTimeZoneID(javaHome)`: the zone's name, or `null` (`time_zone.rs`).
    SystemTimeZoneId,
    /// `TimeZone.getSystemGMTOffsetID()`: the local offset from UTC as `GMT+hh:mm`.
    SystemGmtOffsetId,
    /// `FileDispatcherImpl.transferTo0` and `transferFrom0`: the runtime answers
    /// `IOStatus.UNSUPPORTED_CASE`, so the JDK copies through buffers instead (`file_channels.rs`).
    TransferUnsupported,
    /// `UnixFileDispatcherImpl.map0(fd, prot, position, length, isSync)`: a block outside the heap
    /// with the file's bytes (`file_channels.rs`). Its descriptor is lowered to its number.
    MapFile,
    /// `UnixFileDispatcherImpl.unmap0(address, length)`: frees the block that `MapFile` made.
    UnmapFile,
    /// `UnixFileDispatcherImpl.allocationGranularity0()`: the page size.
    AllocationGranularity,
    /// `LinuxNativeDispatcher.posix_fadvise(fd, offset, len, advice)`: `file_copy.rs`.
    PosixFadvise,
    /// `LinuxNativeDispatcher.directCopy0(dst, src, cancel)`: answers `IOStatus.UNSUPPORTED`.
    DirectCopy,
    /// `TranslatedProxies.name0(c, name, module, dynamic)`: gives the proxy class `c` its run-time
    /// name, none that is known for an empty name, the module and loader that a
    /// `ClassCastException` names, and whether that module is a dynamic one
    /// (`java::name_proxy_class`, see `java/overlay/java/lang/reflect/TranslatedProxies.java`).
    NameProxyClass,
    /// `Module.dynamicProxy0(c)`: whether `c` is a proxy class in a dynamic module
    /// (`java::in_dynamic_module`).
    DynamicProxy,
    /// `TranslatedAnnotations.made0()`, which the patched `AnnotationInvocationHandler` calls as
    /// an annotation is made, and `TranslatedProxies.annotationsMade0()`, whether one was: Java
    /// makes an annotation as a proxy, of a proxy class defined then (`java::note_annotation`).
    NoteAnnotation,
    AnnotationsMade,
    /// `UnixFileSystem.bufferedCopy0Errno(dst, src, address, size, cancel)` (the patch's name for
    /// the JDK's `bufferedCopy0`): copies through the buffer at `address`, and answers the `errno`
    /// of a failed read or write (`file_copy.rs`).
    BufferedCopy,
}

impl Native {
    /// A native of `java.io`'s streams, `FileDescriptor` or `UnixFileSystem`.
    pub fn is_java_io(self) -> bool {
        matches!(
            self,
            Native::FileOpenRead
                | Native::FileOpenWrite
                | Native::FileOpenRandom
                | Native::FileRead
                | Native::FileReadBytes
                | Native::FileAvailable
                | Native::FileSkip
                | Native::FileClose
                | Native::FileWrite
                | Native::FileWriteBytes
                | Native::FileIsRegular
                | Native::FileLength
                | Native::FilePosition
                | Native::FileSeek
                | Native::FileSetLength
                | Native::FileSystem(_)
        )
    }
}

/// The native methods of translated JDK classes that the runtime implements and whose calls
/// become a `Native` of their arguments as they are, by class, name and javac's signature. (The
/// stream natives of `java.io`, which take their object's descriptor, are lowered on their own.)
pub fn declared_native(class: &str, name: &str, signature: &str) -> Option<Native> {
    Some(match (class, name, signature) {
        // Registering a class's natives with the JVM, and looking up the fields they use.
        (_, "registerNatives" | "initIDs", "()void") => Native::Nothing,
        // The lookups of the fields that the JVM's networking code sets.
        (
            "java.net.InetAddress"
            | "java.net.Inet4Address"
            | "java.net.Inet6Address"
            | "java.net.NetworkInterface",
            "init",
            "()void",
        ) => Native::Nothing,
        // Whether the system makes sockets of each address family (`java_runtime::net`).
        ("java.net.InetAddress", "isIPv4Available", "()boolean") => Native::Net("isIPv4Available"),
        ("java.net.InetAddress", "isIPv6Supported", "()boolean") => Native::Net("isIPv6Supported"),
        // Registers `getNanoTimeAdjustment`, and nothing else.
        ("jdk.internal.misc.VM", "initialize", "()void") => Native::Nothing,
        ("jdk.internal.misc.VM", "getNanoTimeAdjustment", "(long)long") => Native::NanoTimeAdjustment,
        // The javaHome argument is only the place of the JDK's `tzmappings`, which is not read.
        ("java.util.TimeZone", "getSystemTimeZoneID", "(java.lang.String)java.lang.String") => {
            Native::SystemTimeZoneId
        }
        ("java.util.TimeZone", "getSystemGMTOffsetID", "()java.lang.String") => Native::SystemGmtOffsetId,
        ("jdk.internal.misc.CDS", "getCDSConfigStatus", "()int") => Native::CdsConfigStatus,
        // The entries of the environment, which the patch of `ProcessEnvironment` splits as the
        // JVM's `environ()` does (`java/patches/java/lang/ProcessEnvironment.java`).
        ("java.lang.ProcessEnvironment", "environ0", "()byte[]") => Native::Environ,
        ("java.lang.reflect.Array", "newArray", "(java.lang.Class<?>,int)java.lang.Object") => {
            Native::ReflectNewArray
        }
        ("java.lang.reflect.Array", "getLength", "(java.lang.Object)int") => Native::ReflectArrayLength,
        ("java.lang.reflect.Array", "set", "(java.lang.Object,int,java.lang.Object)void") => {
            Native::ReflectArraySet
        }
        ("jdk.internal.misc.CDS", "getRandomSeedForDumping", "()long") => Native::CdsRandomSeed,
        // Without an archive there is nothing to restore. Its argument is a class literal, which
        // the lowering drops with the call (`class_literal_call`).
        ("jdk.internal.misc.CDS", "initializeFromArchive", "(java.lang.Class<?>)void") => Native::Nothing,
        ("jdk.internal.vm.ContinuationSupport", "isSupported0", "()boolean") => {
            Native::ContinuationsSupported
        }
        ("java.io.FileDescriptor", "getHandle", "(int)long") => Native::DescriptorHandle,
        ("java.io.FileDescriptor", "getAppend", "(int)boolean") => Native::DescriptorAppend,
        ("java.io.StandardOutput", "write0", "(int,int)void") => Native::StandardWrite,
        ("java.io.StandardOutput", "writeBytes0", "(int,byte[],int,int)void") => Native::StandardWriteBytes,
        ("java.io.StandardOutput", "flush0", "(int)void") => Native::StandardFlush,
        ("java.io.StandardOutput", "close0", "(int)void") => Native::StandardClose,
        ("jdk.internal.misc.Unsafe", "fullFence", "()void") => Native::FullFence,
        (
            "jdk.internal.misc.Unsafe",
            "copyMemory0",
            "(java.lang.Object,long,java.lang.Object,long,long)void",
        ) => Native::CopyMemory { swap: false },
        (
            "jdk.internal.misc.Unsafe",
            "copySwapMemory0",
            "(java.lang.Object,long,java.lang.Object,long,long,long)void",
        ) => Native::CopyMemory { swap: true },
        ("jdk.internal.misc.Unsafe", "allocateMemory0", "(long)long") => Native::AllocateNative,
        ("jdk.internal.misc.UnsafeConstants", "pageSize0", "()int") => Native::PageSize,
        (
            "java.lang.reflect.TranslatedProxies",
            "name0",
            "(java.lang.Class<?>,java.lang.String,java.lang.String,boolean)void",
        ) => Native::NameProxyClass,
        ("java.lang.Module", "dynamicProxy0", "(java.lang.Class<?>)boolean") => Native::DynamicProxy,
        ("java.lang.reflect.TranslatedProxies", "annotationsMade0", "()boolean") => Native::AnnotationsMade,
        ("sun.reflect.annotation.TranslatedAnnotations", "made0", "()void") => Native::NoteAnnotation,
        ("jdk.internal.misc.Unsafe", "reallocateMemory0", "(long,long)long") => Native::ReallocateNative,
        ("jdk.internal.misc.Unsafe", "freeMemory0", "(long)void") => Native::FreeNative,
        // A transfer is copied by the JDK's own fallback, and a mapping by `MapFile`, which
        // `lower/file_channels.rs` lowers with the descriptor's number.
        (
            "sun.nio.ch.FileDispatcherImpl",
            "transferTo0",
            "(java.io.FileDescriptor,long,long,java.io.FileDescriptor,boolean)long",
        ) => Native::TransferUnsupported,
        (
            "sun.nio.ch.FileDispatcherImpl",
            "transferFrom0",
            "(java.io.FileDescriptor,java.io.FileDescriptor,long,long,boolean)long",
        ) => Native::TransferUnsupported,
        ("sun.nio.ch.UnixFileDispatcherImpl", "allocationGranularity0", "()long") => {
            Native::AllocationGranularity
        }
        (
            "sun.nio.ch.UnixFileDispatcherImpl",
            "map0",
            "(java.io.FileDescriptor,int,long,long,boolean)long",
        ) => Native::MapFile,
        ("sun.nio.ch.UnixFileDispatcherImpl", "unmap0", "(long,long)int") => Native::UnmapFile,
        // Nothing is written back: no mapping is writable or synchronous (`file_channels.rs`).
        ("java.nio.MappedMemoryUtils", "force0", "(java.io.FileDescriptor,long,long)void") => Native::Nothing,
        // The copy of `Files.copy` (`file_copy.rs`): advice, the kernel's copy (not used), and
        // the copy through a buffer, which `UnixFileSystem.copyFile` falls back to.
        ("sun.nio.fs.LinuxNativeDispatcher", "init", "()void") => Native::Nothing,
        ("sun.nio.fs.LinuxNativeDispatcher", "posix_fadvise", "(int,long,long,int)int") => {
            Native::PosixFadvise
        }
        ("sun.nio.fs.LinuxNativeDispatcher", "directCopy0", "(int,int,long)int") => Native::DirectCopy,
        ("sun.nio.fs.UnixFileSystem", "bufferedCopy0Errno", "(int,int,long,int,long)int") => {
            Native::BufferedCopy
        }
        ("jdk.internal.misc.Unsafe", "writeback0", "(long)void") => Native::Nothing,
        ("jdk.internal.misc.Unsafe", "writebackPreSync0", "()void") => Native::Nothing,
        ("jdk.internal.misc.Unsafe", "writebackPostSync0", "()void") => Native::Nothing,
        ("jdk.internal.misc.Unsafe", "setMemory0", "(java.lang.Object,long,long,byte)void") => {
            Native::SetMemory
        }
        ("java.util.zip.CRC32", "update", "(int,int)int") => Native::Crc32Update,
        ("java.util.zip.CRC32", "updateBytes0", "(int,byte[],int,int)int") => Native::Crc32UpdateBytes,
        ("java.util.zip.CRC32", "updateByteBuffer0", "(int,long,int,int)int") => Native::Crc32UpdateBuffer,
        ("java.util.zip.Adler32", "update", "(int,int)int") => Native::Adler32Update,
        ("java.util.zip.Adler32", "updateBytes", "(int,byte[],int,int)int") => Native::Adler32UpdateBytes,
        ("java.util.zip.Adler32", "updateByteBuffer", "(int,long,int,int)int") => Native::Adler32UpdateBuffer,
        // On Linux, `initNative` reads the boot time and the clock's ticks, which `isAlive0`
        // reads again itself (`java_runtime::process_start_time`).
        ("java.lang.ProcessHandleImpl", "initNative", "()void") => Native::Nothing,
        ("java.lang.ProcessHandleImpl", "getCurrentPid0", "()long") => Native::CurrentPid,
        ("java.lang.ProcessHandleImpl", "isAlive0", "(long)long") => Native::ProcessStartTime,
        ("java.lang.ProcessHandleImpl", "waitForProcessExit0", "(long,boolean)int") => Native::WaitForProcess,
        ("java.lang.ProcessHandleImpl", "destroy0", "(long,long,boolean)boolean") => Native::DestroyProcess,
        // A process starts with `std::process` (`java_runtime::spawn_process`), which needs no
        // helper set up first.
        ("java.lang.ProcessImpl", "init", "()void") => Native::Nothing,
        ("java.lang.ProcessImpl", "spawn0", "(byte[],byte[],int,byte[],int,byte[],int[],boolean)int") => {
            Native::Spawn
        }
        ("java.util.zip.Inflater", "init", "(boolean)long") => Native::InflateInit,
        ("java.util.zip.Inflater", "inflateBytesBytes", "(long,byte[],int,int,byte[],int,int)long") => {
            Native::InflateBytes
        }
        ("java.util.zip.Inflater", "inflateBytesBuffer", "(long,byte[],int,int,long,int)long") => {
            Native::InflateBytesBuffer
        }
        ("java.util.zip.Inflater", "inflateBufferBytes", "(long,long,int,byte[],int,int)long") => {
            Native::InflateBufferBytes
        }
        ("java.util.zip.Inflater", "inflateBufferBuffer", "(long,long,int,long,int)long") => {
            Native::InflateBufferBuffer
        }
        ("java.util.zip.Inflater", "getAdler", "(long)int") => Native::InflateAdler,
        ("java.util.zip.Inflater", "reset", "(long)void") => Native::InflateReset,
        ("java.util.zip.Inflater", "end", "(long)void") => Native::InflateEnd,
        ("java.util.zip.Inflater", "setDictionary", "(long,byte[],int,int)void") => {
            Native::InflateSetDictionary
        }
        ("java.util.zip.Inflater", "setDictionaryBuffer", "(long,long,int)void") => {
            Native::InflateSetDictionaryBuffer
        }
        ("java.util.zip.Deflater", "init", "(int,int,boolean)long") => Native::DeflateInit,
        (
            "java.util.zip.Deflater",
            "deflateBytesBytes",
            "(long,byte[],int,int,byte[],int,int,int,int)long",
        ) => Native::DeflateBytes,
        ("java.util.zip.Deflater", "deflateBytesBuffer", "(long,byte[],int,int,long,int,int,int)long") => {
            Native::DeflateBytesBuffer
        }
        ("java.util.zip.Deflater", "deflateBufferBytes", "(long,long,int,byte[],int,int,int,int)long") => {
            Native::DeflateBufferBytes
        }
        ("java.util.zip.Deflater", "deflateBufferBuffer", "(long,long,int,long,int,int,int)long") => {
            Native::DeflateBufferBuffer
        }
        ("java.util.zip.Deflater", "getAdler", "(long)int") => Native::DeflateAdler,
        ("java.util.zip.Deflater", "reset", "(long)void") => Native::DeflateReset,
        ("java.util.zip.Deflater", "end", "(long)void") => Native::DeflateEnd,
        ("java.util.zip.Deflater", "setDictionary", "(long,byte[],int,int)void") => {
            Native::DeflateSetDictionary
        }
        ("java.util.zip.Deflater", "setDictionaryBuffer", "(long,long,int)void") => {
            Native::DeflateSetDictionaryBuffer
        }
        ("java.lang.ref.Reference", "hasReferencePendingList", "()boolean") => Native::HasPendingReferences,
        ("java.lang.ref.Reference", "waitForReferencePendingList", "()void") => {
            Native::WaitForPendingReferences
        }
        (
            "java.lang.ref.TranslatedReferences",
            "register",
            "(java.lang.ref.Reference<?>,int,boolean)void",
        ) => Native::RegisterReference,
        ("java.lang.ref.TranslatedReferences", "next", "()java.lang.ref.Reference<?>") => {
            Native::NextPendingReference
        }
        ("jdk.internal.misc.Unsafe", "getLoadAverage0", "(double[],int)int") => Native::LoadAverage,
        // The memory, swap, processor time and descriptors of the process and the machine, as the
        // JDK's `libmanagement_ext` reads them on Linux (`java_runtime::os_total_memory`, ...).
        // `initialize0` only reads the page size, which each answer reads itself.
        ("com.sun.management.internal.OperatingSystemImpl", "initialize0", "()void") => Native::Nothing,
        // The JVM's start and uptime are the process's (the JVM's other figures are the
        // translator's patch, `java/patches/sun/management/VMManagementImpl.java`).
        ("sun.management.VMManagementImpl", "getStartupTime", "()long") => {
            Native::OperatingSystem("vm_startup_time")
        }
        ("sun.management.VMManagementImpl", "getUptime0", "()long") => Native::OperatingSystem("vm_uptime"),
        ("com.sun.management.internal.OperatingSystemImpl", name, "()long" | "()int") => {
            Native::OperatingSystem(match name {
                "getTotalMemorySize0" => "os_total_memory",
                "getFreeMemorySize0" => "os_free_memory",
                "getTotalSwapSpaceSize0" => "os_total_swap",
                "getFreeSwapSpaceSize0" => "os_free_swap",
                "getCommittedVirtualMemorySize0" => "os_committed_virtual_memory",
                "getProcessCpuTime0" => "os_process_cpu_time",
                "getOpenFileDescriptorCount0" => "os_open_file_descriptors",
                "getMaxFileDescriptorCount0" => "os_max_file_descriptors",
                "getHostOnlineCpuCount0" => "os_online_processors",
                _ => return None,
            })
        }
        // A mapping is the file's bytes in memory of the process (`MapFile`): loading it has
        // nothing to read, and unloading it nothing to give back that the file holds, as dropping
        // the pages of a copy would lose them.
        ("java.nio.MappedMemoryUtils", "isLoaded0", "(long,long,long)boolean") => Native::MappingLoaded,
        ("java.nio.MappedMemoryUtils", "load0" | "unload0", "(long,long)void") => Native::Nothing,
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
        ("java.lang.Thread", "state0", "(long)int") => Native::ThreadState,
        ("java.lang.Thread", "join0", "(long,long)void") => Native::ThreadJoin,
        ("java.lang.Thread", "rename0", "(long,java.lang.String)void") => Native::ThreadRename,
        ("java.lang.Thread", "interrupt0", "(long)void") => Native::ThreadInterrupt,
        ("java.lang.Thread", "isInterrupted0", "(long)boolean") => Native::ThreadIsInterrupted,
        ("java.lang.Thread", "clearInterrupt0", "()boolean") => Native::ThreadClearInterrupt,
        ("java.lang.Thread", "installUncaught0", "()void") => Native::InstallUncaught,
        ("java.lang.Thread", "installShutdown0", "()void") => Native::InstallShutdown,
        ("java.lang.Thread", "uncaught0", "()java.lang.Throwable") => Native::UncaughtException,
        ("java.lang.Thread", "printUncaught0", "(java.lang.String,java.lang.Throwable)void") => {
            Native::PrintUncaught
        }
        ("java.lang.Throwable", "uncaught0", "()java.lang.Throwable") => Native::UncaughtException,
        _ => return None,
    })
}

/// Whether the translation implements a native method of a translated JDK class: one of
/// `declared_native`, a stream native of `java.io`, which `lower/natives.rs` lowers with the
/// descriptor of the stream's object, a native of `java.io.UnixFileSystem` (`FILE_SYSTEM`), or
/// one that lowering writes as code.
pub fn runtime_native(class: &str, name: &str, signature: &str) -> bool {
    declared_native(class, name, signature).is_some()
        || (class == "java.io.UnixFileSystem" && FILE_SYSTEM.contains(&name))
        || (class == UNIX_DISPATCHER && unix_call(name).is_some())
        || (class == NET_DISPATCHER && net_call(name).is_some())
        || (class == SOCKET_DISPATCHER && socket_call(name).is_some())
        || channel_call(class, name).is_some()
        || channel_nothing(class, name)
        || matches!(
            (class, name),
            (
                "java.io.FileInputStream",
                "open0" | "read0" | "readBytes" | "available0" | "skip0" | "close0" | "isRegularFile0"
                    | "length0" | "position0"
            )
                | ("java.io.FileOutputStream", "open0" | "write" | "writeBytes" | "close0")
                | ("java.io.FileDescriptor", "close0")
                | (
                    "java.io.RandomAccessFile",
                    "open0" | "read0" | "readBytes0" | "write0" | "writeBytes0" | "getFilePointer"
                        | "seek0" | "length0" | "setLength0"
                )
                // Lowered as code on the referent (`lower::references`).
                | ("java.lang.ref.Reference", "get0" | "refersTo0" | "clear0")
                | ("java.lang.ref.PhantomReference", "refersTo0" | "clear0")
                // `LockSupport`'s, with the thread's id for `unpark` (`lower/natives.rs`).
                | ("jdk.internal.misc.Unsafe", "park" | "unpark")
                // What the translator read from the program's roots: its modules and the
                // resources its class loaders find (`lower/modules.rs`), the providers of a
                // service (`lower/reflection.rs`) and a processor's annotations
                // (`lower/processors.rs`).
                | (
                    "java.lang.Module",
                    "modules0" | "moduleOf0" | "declares0" | "holds0" | "classPath0" | "manifest0" | "jdkResource0"
                )
                | ("java.lang.ClassLoader", "holder0")
                | ("java.util.ServiceLoader", "providers0" | "create0")
                | ("javax.annotation.processing.TranslatedProcessorAnnotations", "values0")
                // The members of an annotation type (`lower/annotation_objects.rs`).
                | ("sun.reflect.annotation.TranslatedAnnotations", "members0")
        )
}

/// The natives of `java.io.UnixFileSystem` that the runtime implements (`Native::FileSystem`).
pub const FILE_SYSTEM: &[&str] = &[
    "getBooleanAttributes0",
    "checkAccess0",
    "getLastModifiedTime0",
    "getLength0",
    "setPermission0",
    "createFileExclusively0",
    "delete0",
    "list0",
    "createDirectory0",
    "rename0",
    "setLastModifiedTime0",
    "setReadOnly0",
    "canonicalize0",
    "getNameMax0",
    "getSpace0",
];

/// The translator's class of the system calls of `java.nio.file`.
pub const UNIX_DISPATCHER: &str = "sun.nio.fs.UnixNativeDispatcher";

/// The natives of `UNIX_DISPATCHER` that the runtime implements (`Native::Unix`), with the
/// runtime's function of each. The dispatcher's other natives (the calls relative to an open
/// directory, extended attributes, the mount table) are not among them.
pub const UNIX_CALLS: &[(&str, &str)] = &[
    ("lastError", "unix_last_error"),
    ("getcwd0", "unix_getcwd"),
    ("dup0", "unix_dup"),
    ("open0", "unix_open"),
    ("close0", "unix_close"),
    ("link0", "unix_link"),
    ("unlink0", "unix_unlink"),
    ("rename0", "unix_rename"),
    ("mkdir0", "unix_mkdir"),
    ("rmdir0", "unix_rmdir"),
    ("readlink0", "unix_readlink"),
    ("realpath0", "unix_realpath"),
    ("symlink0", "unix_symlink"),
    ("stat0", "unix_stat"),
    ("fstat0", "unix_fstat"),
    ("statvfs0", "unix_statvfs"),
    ("chown0", "unix_chown"),
    ("fchown0", "unix_fchown"),
    ("chmod0", "unix_chmod"),
    ("fchmod0", "unix_fchmod"),
    ("futimens0", "unix_futimens"),
    ("utimensat0", "unix_utimensat"),
    ("opendir0", "unix_opendir"),
    ("closedir0", "unix_closedir"),
    ("readdir0", "unix_readdir"),
    ("access0", "unix_access"),
    ("getpwuid0", "unix_getpwuid"),
    ("getgrgid0", "unix_getgrgid"),
    ("strerror0", "unix_strerror"),
    ("fgetxattr0", "unix_fgetxattr"),
    ("fsetxattr0", "unix_fsetxattr"),
    ("fremovexattr0", "unix_fremovexattr"),
    ("flistxattr0", "unix_flistxattr"),
    ("mknod0", "unix_mknod"),
    ("readBytes", "unix_read"),
    ("writeBytes", "unix_write"),
    ("position", "unix_position"),
    ("seek", "unix_seek"),
    ("size", "unix_size"),
    ("truncate", "unix_truncate"),
    ("sync", "unix_sync"),
    ("lock", "unix_lock"),
    ("unlock", "unix_unlock"),
];

/// The translator's class of the name service calls of `java.net`.
pub const NET_DISPATCHER: &str = "java.net.TranslatedNet";

/// The natives of `NET_DISPATCHER` that the runtime implements (`Native::Net`), and the two of
/// `InetAddress` that it answers itself, with the runtime's function of each.
pub const NET_CALLS: &[(&str, &str)] = &[
    ("lookup0", "net_lookup"),
    ("lastError0", "net_last_error"),
    ("errorText0", "net_error_text"),
    ("hostname0", "net_hostname"),
    ("hostOf0", "net_host_of"),
    ("bound0", "net_bound"),
    ("interfaces0", "net_interfaces"),
    ("interfaceFlags0", "net_interface_flags"),
    ("interfaceMtu0", "net_mtu"),
    ("hardwareAddress0", "net_hardware_address"),
    ("interfaceError0", "net_interface_error"),
    ("systemErrorText0", "unix_strerror"),
    ("isIPv4Available", "net_ipv4_available"),
    ("isIPv6Supported", "net_ipv6_supported"),
];

/// The translator's class of the system calls of `sun.nio.ch`'s sockets, pipes and selectors
/// (`java/overlay/sun/nio/ch/TranslatedSockets.java`), which the JDK's natives of those are
/// redirected to (`program::SOCKET_NATIVES`).
pub const SOCKET_DISPATCHER: &str = "sun.nio.ch.TranslatedSockets";

/// The natives of `SOCKET_DISPATCHER` (`Native::Socket`), with the runtime's function of each
/// (`java_runtime::socket`).
pub const SOCKET_CALLS: &[(&str, &str)] = &[
    ("sysLastError", "socket_last_error"),
    ("sysErrorText", "unix_strerror"),
    ("sysIPv6Available", "socket_ipv6_available"),
    ("sysOpen", "socket_open"),
    ("sysBind", "socket_bind"),
    ("sysListen", "socket_listen"),
    ("sysConnect", "socket_connect"),
    ("sysAccept", "socket_accept"),
    ("sysAcceptedPeer", "socket_accepted_peer"),
    ("sysLocalAddress", "socket_local_address"),
    ("sysRemoteAddress", "socket_remote_address"),
    ("sysGetOption", "socket_get_option"),
    ("sysSetOption", "socket_set_option"),
    ("sysShutdown", "socket_shutdown"),
    ("sysPoll", "socket_poll"),
    ("sysPollConnect", "socket_poll_connect"),
    ("sysAvailable", "socket_available"),
    ("sysRead", "socket_read"),
    ("sysWrite", "socket_write"),
    ("sysReadv", "socket_readv"),
    ("sysWritev", "socket_writev"),
    ("sysClose", "socket_close"),
    ("sysPreClose", "socket_pre_close"),
    ("sysConfigureBlocking", "socket_configure_blocking"),
    ("sysFdLimit", "socket_fd_limit"),
    ("sysMakePipe", "socket_make_pipe"),
    ("sysDrain", "socket_drain"),
    ("sysWriteByte", "socket_write1"),
    ("sysEpollCreate", "epoll_open"),
    ("sysEpollControl", "epoll_control"),
    ("sysEpollWait", "epoll_wait_events"),
    ("sysEventFdOpen", "event_fd_open"),
    ("sysEventFdSet", "event_fd_set"),
];

/// The name of a native of `SOCKET_DISPATCHER` that the runtime implements, as `SOCKET_CALLS`
/// has it.
pub fn socket_call(name: &str) -> Option<&'static str> {
    SOCKET_CALLS.iter().find(|(n, _)| *n == name).map(|(n, _)| *n)
}

/// The name of a native of `NET_DISPATCHER` that the runtime implements, as `NET_CALLS` has it.
pub fn net_call(name: &str) -> Option<&'static str> {
    NET_CALLS.iter().find(|(n, _)| *n == name).map(|(n, _)| *n)
}

/// The name of a native of `UNIX_DISPATCHER` that the runtime implements, as `UNIX_CALLS` has it.
pub fn unix_call(name: &str) -> Option<&'static str> {
    UNIX_CALLS.iter().find(|(n, _)| *n == name).map(|(n, _)| *n)
}

/// The `Math` and `StrictMath` functions the native layer covers (beyond `abs`, `max`, `min`).
pub const MATH_FUNCTIONS: &[&str] = &[
    "random",
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
    "ceilDiv",
    "ceilMod",
    "ceilDivExact",
    "divideExact",
    "floorDivExact",
    "powExact",
    "signum",
    "ulp",
    "nextUp",
    "nextDown",
    "fma",
    "addExact",
    "subtractExact",
    "multiplyExact",
    "multiplyHigh",
    "unsignedMultiplyHigh",
    "multiplyFull",
    "incrementExact",
    "decrementExact",
    "negateExact",
    "absExact",
    "toIntExact",
    "copySign",
    "clamp",
    "toRadians",
    "toDegrees",
    "scalb",
    "getExponent",
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
        assert_eq!(
            found(
                "sun.nio.ch.FileDispatcherImpl",
                "transferTo0",
                "(java.io.FileDescriptor,long,long,java.io.FileDescriptor,boolean)long"
            ),
            Some(Native::TransferUnsupported)
        );
        assert_eq!(
            found(
                "sun.nio.ch.UnixFileDispatcherImpl",
                "map0",
                "(java.io.FileDescriptor,int,long,long,boolean)long"
            ),
            Some(Native::MapFile)
        );
        assert_eq!(
            found("sun.nio.ch.UnixFileDispatcherImpl", "unmap0", "(long,long)int"),
            Some(Native::UnmapFile)
        );
        assert_eq!(
            found("sun.nio.ch.UnixFileDispatcherImpl", "allocationGranularity0", "()long"),
            Some(Native::AllocationGranularity)
        );
        assert_eq!(
            found("java.nio.MappedMemoryUtils", "force0", "(java.io.FileDescriptor,long,long)void"),
            Some(Native::Nothing)
        );
        assert_eq!(found("jdk.internal.misc.Unsafe", "writeback0", "(long)void"), Some(Native::Nothing));
        assert_eq!(found("jdk.internal.misc.Unsafe", "writebackPreSync0", "()void"), Some(Native::Nothing));
        assert_eq!(found("jdk.internal.misc.Unsafe", "writebackPostSync0", "()void"), Some(Native::Nothing));
        assert_eq!(found("java.util.zip.CRC32", "update", "(int,int)int"), Some(Native::Crc32Update));
        assert_eq!(
            found("java.util.zip.CRC32", "updateBytes0", "(int,byte[],int,int)int"),
            Some(Native::Crc32UpdateBytes)
        );
        assert_eq!(found("java.util.zip.Inflater", "init", "(boolean)long"), Some(Native::InflateInit));
        assert_eq!(
            found("java.util.zip.Inflater", "inflateBytesBytes", "(long,byte[],int,int,byte[],int,int)long"),
            Some(Native::InflateBytes)
        );
        assert_eq!(
            found("java.util.zip.Inflater", "inflateBytesBuffer", "(long,byte[],int,int,long,int)long"),
            Some(Native::InflateBytesBuffer)
        );
        assert_eq!(
            found("java.util.zip.Inflater", "inflateBufferBytes", "(long,long,int,byte[],int,int)long"),
            Some(Native::InflateBufferBytes)
        );
        assert_eq!(
            found("java.util.zip.Inflater", "inflateBufferBuffer", "(long,long,int,long,int)long"),
            Some(Native::InflateBufferBuffer)
        );
        assert_eq!(
            found("java.util.zip.Inflater", "setDictionaryBuffer", "(long,long,int)void"),
            Some(Native::InflateSetDictionaryBuffer)
        );
        assert_eq!(
            found("java.util.zip.Deflater", "init", "(int,int,boolean)long"),
            Some(Native::DeflateInit)
        );
        assert_eq!(
            found(
                "java.util.zip.Deflater",
                "deflateBytesBytes",
                "(long,byte[],int,int,byte[],int,int,int,int)long"
            ),
            Some(Native::DeflateBytes)
        );
        assert_eq!(
            found("java.util.zip.Deflater", "deflateBufferBuffer", "(long,long,int,long,int,int,int)long"),
            Some(Native::DeflateBufferBuffer)
        );
        assert_eq!(found("java.util.zip.Deflater", "reset", "(long)void"), Some(Native::DeflateReset));
        assert_eq!(
            found("java.util.zip.CRC32", "updateByteBuffer0", "(int,long,int,int)int"),
            Some(Native::Crc32UpdateBuffer)
        );
        assert_eq!(found("java.util.zip.Adler32", "update", "(int,int)int"), Some(Native::Adler32Update));
        assert_eq!(
            found("java.util.zip.Adler32", "updateBytes", "(int,byte[],int,int)int"),
            Some(Native::Adler32UpdateBytes)
        );
        assert_eq!(
            found("java.util.zip.Adler32", "updateByteBuffer", "(int,long,int,int)int"),
            Some(Native::Adler32UpdateBuffer)
        );
        assert_eq!(
            found("java.lang.ProcessHandleImpl", "getCurrentPid0", "()long"),
            Some(Native::CurrentPid)
        );
        assert_eq!(
            found("java.lang.ProcessHandleImpl", "isAlive0", "(long)long"),
            Some(Native::ProcessStartTime)
        );
        // Another overload, or another class's method of the same name, is not the native.
        assert_eq!(found("java.util.zip.CRC32", "update", "(byte[],int,int)void"), None);
        assert_eq!(found("jdk.internal.misc.Unsafe", "initialize", "()void"), None);
        // `TimeZone`'s natives are private, so only `getDefault` calls them, and `java.util` cannot
        // be stood in for; the mapping is checked here.
        assert_eq!(
            found("java.util.TimeZone", "getSystemTimeZoneID", "(java.lang.String)java.lang.String"),
            Some(Native::SystemTimeZoneId)
        );
        assert_eq!(
            found("java.util.TimeZone", "getSystemGMTOffsetID", "()java.lang.String"),
            Some(Native::SystemGmtOffsetId)
        );
        assert!(runtime_native("java.io.FileInputStream", "readBytes", "(byte[],int,int)int"));
        assert!(!runtime_native("java.lang.Throwable", "fillInStackTrace", "(int)java.lang.Throwable"));
        // The name service calls that the runtime answers (`NET_CALLS`), as the census lists them.
        assert!(runtime_native(NET_DISPATCHER, "lookup0", "(byte[],int)byte[]"));
        assert!(runtime_native(
            "java.util.ServiceLoader",
            "providers0",
            "(java.lang.String)java.lang.Class<?>[]"
        ));
        assert!(!runtime_native(NET_DISPATCHER, "connect0", "()void"));
    }
}
