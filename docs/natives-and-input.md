# Natives, the reachability boundary and standard streams

A whole program (`--main` with `--library` naming the `java.base` sources) translates the JDK's
own code that the program reaches. That code bottoms out in `native` methods, which the JVM
implements. This document covers:

1. what is hand-written and what is translated;
2. how a call of a native becomes Rust, and which natives the runtime implements;
3. the rules of reachability that keep JDK code out of a translation where only the JVM's own
   state could make it run;
4. `System.out`, `System.err`, `System.in` and the `java.io` streams in a translation;
5. the survey that finds what a program reaches.

## What is hand-written

Only three kinds of code are written by hand in the runtime (`crates/runtime`); everything else
is translated from the JDK's sources:

- what bottoms out in the JVM (natives);
- the core types that the runtime maps to Rust types: `String`, `StringBuilder`, the boxes,
  arrays, and the `Object`, `Enum` and `Throwable` bases;
- calls with a direct equivalent in Rust's standard library.

The classes that the runtime provides are `RUNTIME_CLASSES` in `program/reach.rs`; reachability
stops at them. The translator also brings classes of its own, written in Java
(`crates/translator/java`: `Standard.java` for the standard exceptions, `Threads.java` for
`Thread`, and the overlays and patches of [jdk-overlays.md](jdk-overlays.md)).

## How a native call is translated

A native method's declaration has no body and is never lowered. javac's symbols say whether a
method is `native`. A call of a native of a translated class becomes the runtime function that
`ast::declared_native` names for its class, name and javac signature (`crates/translator/src/ast/natives.rs`,
lowered in `lower/natives.rs`). `ast::runtime_native` adds the stream natives of `java.io`, which
take the descriptor of the stream's object, and the tables `FILE_SYSTEM`, `UNIX_CALLS` and
`NET_CALLS`.

A native that the runtime lacks becomes `Native::Missing`. Where it can run, the program is
rejected with ``native method `Class.name(signature)` not supported yet``
(`analysis/live_methods.rs`); where it cannot (a branch that only a direct buffer takes, say), it
is code that never runs.

Some methods are replaced at the call, before their argument is lowered, so that reachability
never enters them (`REPLACED_METHODS` in `program/reach.rs`). They take a `Class`, which the
translation has no general value for at that point: `Unsafe.arrayBaseOffset(int[].class)` and
`arrayIndexScale(..)` with a class literal of an array class, and `Unsafe.objectFieldOffset` of
a field named by a class literal and a string, which becomes a field handle (see
[multi-threading.md](multi-threading.md)). Any other argument is
rejected by name.

## The natives the runtime implements

Each answers as HotSpot does, or as HotSpot does with the flag named where the default depends on
something a translated program does not have.

| Java | Rust | Behaviour |
|---|---|---|
| `registerNatives()`, `initIDs()` of any class; `VM.initialize()`; `InetAddress.init()` and its relatives | nothing (statement dropped) | JNI registration and field lookups |
| `CDS.getCDSConfigStatus()` | `java::cds_config_status()` | 0: no archive, as with `-Xshare:off` |
| `CDS.getRandomSeedForDumping()` | `java::cds_random_seed_for_dumping()` | 0: not dumping |
| `CDS.initializeFromArchive(C.class)` | nothing | no archive to restore from (see "Archived fields" below) |
| `Unsafe.arrayBaseOffset(T[].class)` | `java::array_base_offset()` | 16 |
| `Unsafe.arrayIndexScale(T[].class)` | `java::array_index_scale("T")` | 1, 2, 4 or 8 by element; 4 for references (compressed pointers) |
| `ContinuationSupport.isSupported0()` | `java::continuations_supported()` | `false`, as with `-XX:-VMContinuations`: no virtual threads |
| `VM.getNanoTimeAdjustment(offset)` | `java::nano_time_adjustment(offset)` | `(now - offset) * 10^9 + nanos`; -1 when `now - offset` is 2^32 seconds or more either way |
| `TimeZone.getSystemTimeZoneID`, `getSystemGMTOffsetID` | `time_zone.rs` | the local zone from `TZ` and the system's files, as the JDK finds it on Linux |
| `FileDescriptor.getHandle(fd)` | `java::fd_handle(fd)` | -1 (Windows handles only) |
| `FileDescriptor.getAppend(fd)` | `java::fd_append(fd)` | the descriptor's `O_APPEND` (from `/proc/self/fdinfo`, `false` where missing); `true` for a descriptor that is not open, as Java reads `fcntl`'s -1 |
| `FileInputStream`, `FileOutputStream`, `RandomAccessFile`, `FileDescriptor` natives (`open0`, `read0`, `readBytes`, `available0`, `skip0`, `write`, `seek0`, `close0`, ...) | `fd.rs` | a descriptor table of the process; 0, 1 and 2 are the standard streams |
| `sun.nio.ch.UnixFileDispatcherImpl` natives (`read0`, `pread0`, `write0`, `pwrite0`, `seek0`, `size0`, `truncate0`, `available0`, `isOther0`, `setDirect0`, `release0`); `FileDispatcherImpl.init0`, `IOUtil.iovMax`, `IOUtil.writevMax`, `NativeThread.init`, `current0`, `signal0` | `file_channel.rs` | the channel natives of `FileChannelImpl`, on the descriptor table: `read0` and `pread0` give -1 at the end of the file; `seek0` with -1 gives the position; `setDirect0` always fails with an `IOException` ("Operation not supported": the standard library cannot set `O_DIRECT` on an open file); `release0` does nothing (no lock is taken, as `lock0` is not implemented); `signal0` does nothing (no translated thread is blocked in a native call to wake); `current0` is never 0 |
| `UnixFileSystem` natives (`FILE_SYSTEM`) | `fs.rs` | `File`'s attributes, listing, creating, renaming, deleting; not `getSpace0` |
| the translator's `sun.nio.fs.UnixNativeDispatcher` (`UNIX_CALLS`) | `unix.rs` | the system calls of `java.nio.file`, recording `errno` rather than failing; the path it gives them, `UnixPath.getByteArrayForSysCalls()`, is never null, as the JDK's code shows (`analysis/nullness.rs`: `never_null_result`), so a call has no message of a null path to know |
| `UnixNativeDispatcher.statvfs0` (the translator's patch, `long[]` fields) | `unix.rs`: `unix_statvfs` | the four fields `UnixFileStoreAttributes.fill` copies (block size, blocks, free and available blocks) |
| `UnixNativeDispatcher.fgetxattr0`, `fsetxattr0`, `fremovexattr0`, `flistxattr0` and `mknod0` (the translator's patch) | `unix.rs` | the extended attributes of an open file, read from and written to the native memory of the JDK's `NativeBuffer`s, as the DOS and user-defined attribute views and `Files.copy` use them, and `mknod` for the copy of a special file |
| `LinuxNativeDispatcher.init`, `posix_fadvise`; `UnixFileSystem.directCopy0`, `bufferedCopy0` | `file_copy.rs`, `codegen/file_copy.rs` | `init` does nothing; `posix_fadvise` checks the descriptor is open and gives 0, as its advice has no effect a program can see. `directCopy0` gives `UNSUPPORTED` (-4), so the JDK copies through `bufferedCopy0`, which moves the same bytes between the two descriptors (`copy_file_range` is not used). The JDK's native is `bufferedCopy0Errno` in the translator's patch of `UnixFileSystem`: it answers the `errno` of a failed read or write, and `bufferedCopy` throws `UnixException(errno)` as HotSpot does, so `copyFile` raises the same `IOException` subclass and message (`NoSuchFileException`, `AccessDeniedException`, ...). `posix_fadvise` still raises `java.io.IOException` for a descriptor that is not open; `Files.copy` opens both descriptors first, so it is not reached from there |
| the translator's `java.net.TranslatedNet` (`NET_CALLS`) | `net.rs` | the C library's name service |
| `Unsafe.allocateMemory0`, `reallocateMemory0`, `freeMemory0`, `setMemory0`, `copyMemory0`, `copySwapMemory0`; `UnsafeConstants.pageSize0` | `native_memory.rs`, `java::copy_memory` | memory outside the heap by address, for direct buffers |
| `FileDispatcherImpl.transferTo0`, `transferFrom0` | `file_channels.rs`: `UNSUPPORTED_CASE` | the JDK's `transferToArbitraryChannel` (or its `transferFrom` twin) copies through buffers at the same positions and counts, so `Files.copy` and `FileChannel.transferTo`/`transferFrom` give Java's bytes and positions; `copy_file_range` is not used |
| `UnixFileDispatcherImpl.map0(fd, prot, position, length, isSync)` | `file_channels.rs`, lowered with the descriptor's number (`lower/file_channels.rs`) | a read-only (`MAP_RO`) or private (`MAP_PV`) mapping is a block of memory outside the heap holding the file's bytes from `position` (zeros past the end of the file), taken when the map is made. Later writes to the file are not seen, and writes to a private mapping do not reach the file, as in Java. A read-write or synchronous mapping ends the program with a message |
| `UnixFileDispatcherImpl.unmap0(address, length)` | `file_channels.rs` | frees the block; 0 |
| `UnixFileDispatcherImpl.allocationGranularity0()` | `file_channels.rs` | the page size (4096), which `FileChannelImpl` aligns mapping positions to |
| `MappedMemoryUtils.force0`, `Unsafe.writeback0`, `writebackPreSync0`, `writebackPostSync0` | nothing (statement dropped) | exact because no mapping is writable or synchronous, so there is nothing to write back |
| `Unsafe.fullFence()` (and `loadFence`, `storeFence`) | a full fence | |
| `Unsafe.park`, `unpark` | `thread.rs` | `LockSupport`'s permits |
| `CRC32.update`, `updateBytes0` | `java::crc32_update`, `crc32_update_bytes` | zlib's CRC-32 |
| `Inflater` natives (`inflateBytesBytes`, `inflateBytesBuffer`, `inflateBufferBytes`, `inflateBufferBuffer`, `setDictionary`, `setDictionaryBuffer`, ...) | `inflate.rs` | zlib inflation, from arrays or from memory outside the heap by address (direct buffers) |
| `Deflater` natives (`init`, `deflateBytesBytes`, `deflateBytesBuffer`, `deflateBufferBytes`, `deflateBufferBuffer`, `setDictionary`, `setDictionaryBuffer`, `getAdler`, `reset`, `end`) | `deflate.rs` | a compressor of the runtime's own, see "Compression" below |
| `Reference.get0`, `refersTo0`, `clear0`; `hasReferencePendingList()` | code on the field `referent`; `false` | see "References" below |
| `SecureRandom.getSeed` | `random.rs` | seed bytes from the operating system |
| the natives of the translator's `Thread` | `thread.rs` | see [multi-threading.md](multi-threading.md) |
| `StandardOutput.write0`, `writeBytes0`, `flush0`, `close0` | `out.rs` | see "System.out, System.err and System.in as values" |
| `java.lang.reflect.Array.newArray`, `getLength`, `set` | code (`codegen/dispatch.rs`) | a `match` over the arrays that become `Object`s, see below |

`java.lang.reflect.Array`'s natives work on the arrays that the program makes `Object`s, each a
variant of `Object` (`int[]`, `String[]`): `newArray` matches the component type's name
(`Class.getName()`) against their element types, `getLength` and `set` the array's variant, and
`set` widens a box to a primitive element as the JVM does, failing in its order (a null array, one
that is not an array, a null for a primitive, the index, then the value). A component type of which
the program makes no array as an `Object` ends the program with `not_translated`. The analyses take
`newArray` to make an array of each of those types, with null references, and an `Object[]` that
stands for several array types ([object-model.md](object-model.md) 1.11, covariant arrays) takes any
reference, as a store into it does. The failures without a message (`void`, a null array) are built
with the class's constructor without arguments, so `getMessage()` is null. The JDK's `(T[])
Array.newInstance(a.getClass().getComponentType(), n)` (`toArray(T[])`, `TimSort`'s work arrays)
does not reach the native: it is lowered to a new array of `n` nulls of `a`'s type
(`lower::arrays::component_array`, `program::component_array`).

The natives of the runtime's own classes (`System.arraycopy`, `Object.hashCode`,
`System.identityHashCode`, the bit conversions of `Double` and `Float`, the clocks,
`availableProcessors`, `String.intern`) are in `runtime/src/natives.rs`; the checked copy between
arrays of two classes, when one side is a plain `Vec`, is `try_copy_refs` in
`runtime/src/copy_refs.rs`. The identity hash is derived from the object's address, which the collector never changes: stable for the object's life and
different on every run, as in Java. `String.intern()` is the string itself where strings are
values, and the interned object of its text where the program compares strings by reference
(see [object-model.md](object-model.md), section 1.6).

`Set.of` and `Map.of` iterate in a different order on every run, in Java too: with the CDS seed
0, `ImmutableCollections` seeds its order from `System.nanoTime()`. Their output cannot be
compared with one recorded run.

### Compression

`Deflater` has no zlib to call, so the runtime compresses with code of its own (`deflate.rs`,
without external crates), and its output is not zlib's byte for byte: it is valid DEFLATE data
that zlib, the JDK's `Inflater` and the runtime's own read back to the same bytes. A program that
prints or compares compressed bytes, or their number (`getBytesWritten`, `getTotalOut`), sees
other values than Java; everything else is zlib's: the bytes read by each call (and so
`needsInput`, `getBytesRead`, the input buffer's position), `finished`, the Adler-32 of the input
(`getAdler`), the zlib header with the level's flags and a preset dictionary's id, the empty
stored block that ends a `SYNC_FLUSH` or `FULL_FLUSH`, the input that a change of level or
strategy compresses first, and the `IllegalArgumentException` of a level outside -1 to 9 (in the
constructor) and of a dictionary zlib refuses.

It is a real compressor rather than stored or fixed-code blocks only, since the compressed size
is what a program uses `Deflater` for (javac's zip file system writes jars with it): input is
gathered into blocks of 64 KiB, matches are found along hash chains of three-byte prefixes within
the last 32 KiB, with the chain lengths and lazy matching of zlib's levels, and each block is
written as stored bytes, with the fixed codes or with dynamic Huffman codes made for it, whichever
is smallest. Level 0 writes stored blocks, and `HUFFMAN_ONLY` literals only. The output is within
a few percent of zlib's size at each level.

### Not implemented

- `Throwable.fillInStackTrace(int)` and `NullPointerException.getExtendedNPEMessage()` are not
  reached: the translator gives the program its own `Throwable` and standard exceptions
  (`java/Standard.java`). Stack traces and helpful null-pointer messages are in
  [exceptions.md](exceptions.md).
- `UnixFileDispatcherImpl.force0`, which `FileChannel.force` of `FileChannelImpl` (the channel of
  `RandomAccessFile.getChannel()` and `FileInputStream.getChannel()`) reaches. Its reads, writes,
  positions and sizes go through the channel natives above. `AbstractInterruptibleChannel` calls
  `JavaLangAccess.blockedOn` to record the operation a thread is in, which does nothing in a
  translation (`TranslatedLangAccess`): a file's operations do not wait indefinitely, so an
  interrupt during one is seen just after it, when the next operation's `begin` closes the
  channel. `FileChannel.open` and `Files.newByteChannel` do not use `FileChannelImpl` at all: they
  use the overlay `UnixFileChannel` (`java/overlay/sun/nio/fs`).
- Stack walking (`StackTraceElement.initStackTraceElements`, `StackStreamFactory`), class
  definition and module natives, sockets, `MappedMemoryUtils.isLoaded0`, `load0` and
  `unload0` (`MappedByteBuffer.isLoaded`, `load`, `unload`), and every native not in the table.

## The reachability boundary

Reachability (`program/reach.rs` and `program/reach/`) follows calls, virtual dispatch to
overriders in live classes, and static initialization. Without the rules below it reaches large
parts of the JDK that only the JVM's own state could make run. Each rule rests on a fact about
every translated program; a change that breaks the fact must drop the rule.

### Flags that are never true

`never_true` skips the branch of an `if` (or `?:`) whose condition reads one of these, alone or as
an operand of `&&`, and the lowering lowers only the other branch:

- `FALSE_FLAGS`: static flags that only the JVM or a `-D` option sets, which a translated program
  never has. Java Flight Recorder's `jfrTracing` of `Throwable`, `FileInputStream`,
  `FileOutputStream`, `RandomAccessFile` and `FileChannelImpl` (only `JFRTracing.enable()` sets it,
  by reflection); `ThreadLocal.TRACE_VTHREAD_LOCALS`; `ResourceBundle.TRACE_ON`; and `ENABLED` of
  `java.util.Tripwire` and `java.util.stream.Tripwire`. Without the JFR flag, every exception
  reached `ThrowableTracer` and `Unsafe` atomics.
- `FALSE_CALLS`: `ContinuationSupport.isSupported()`, since the runtime answers `isSupported0`
  false. It guards the pinning of virtual threads (`ReferenceQueue.poll` and other code under a
  lock).
- `FALSE_INSTANCE_CALLS`: `Thread.isVirtual()`; there are no virtual threads.
- A constant variable that is `false`, named by its simple name (`private final boolean
  crtDebug = false;` in javac's `CRTable`): javac writes no code for the branch either, so the
  messages of its null dereferences, which `BytecodeNames` finds no code for, are not needed.
- `Boolean.getBoolean(key)` is `false` for every key (`lower/system.rs`): the JVM sets none of its
  own properties to `"true"`, only `-D` does, and a translation has no `System.setProperty` for
  it. A key whose evaluation has effects is still evaluated.

### Archived fields

Without a class data sharing archive, `CDS.initializeFromArchive` sets nothing, so the null test
right after it of the fields in `ARCHIVED_FIELDS` (`ImmutableCollections.archivedObjects`,
`Attributes.Name.KNOWN_NAMES`, `Configuration.EMPTY_CONFIGURATION`) is always true.
Reachability, the survey and the lowering keep only the branch that builds the values
(`live_branch`). Without it, `ImmutableCollections.EMPTY` could be any `Object`, and `List12`'s
comparisons with it were rejected. Classes that copy the field into a local first are not covered.

### Sequential streams

Reachability first assumes that no stream is parallel and skips what
`AbstractPipeline.isParallel()` guards, and the `getChildCount() > 0` test of `Nodes.flatten`
(only parallel evaluation concatenates nodes). `program/reach/sequential.rs` then checks the
assumption on the code reached: every assignment of `AbstractPipeline.parallel` must assign a
value that cannot be true, following `boolean` parameters of constructors and static or private
methods through every reachable call. `BaseStream.parallel()` and `Collection.parallelStream()`
fail the check, and then reachability runs again without the assumption. This keeps
`ForkJoinPool` and the parallel `ConcurrentHashMap` out of sequential programs.

### Local and anonymous classes are reached by dispatch

javac's trees name local and anonymous classes by their binary names (`DistinctOps$1`), and they
are classes like any other: a constructor is reached by its `new`, an instance method by a call
or by dispatch once its class has objects. They are not scanned with the code that declares
them. This matters because 27 files of `java.base` register a `SharedSecrets` accessor as an
anonymous class in a static initializer; scanning them with the initializer reached every method
of the accessor (mapped memory, continuations).

### Classes without objects

- **Dormant methods** (`Reach::dormant`): an instance method of a class that no reachable
  constructor creates objects of, nor of a subclass, can only be called on null. It is declared
  with a body that never runs (`Builtin::NoObject`), and what its body uses is not reached. It
  wakes up when its class becomes live. Interfaces are left out, as the runtime's strings and
  boxes are `CharSequence`s and `Comparable`s that no constructor shows.
- **Stubs for untranslated types** (`program/reach/stubs.rs`, `lower/stubs.rs`): a variable whose
  type is a class that is not translated (`UNTRANSLATED`) is only ever null, since no translated
  code can create one. The class is declared as a stub without constructors or fields, with the
  instance methods called on it as dormant methods; a call throws `NullPointerException` and
  `instanceof` is false, as in Java. Its static members, constructors, generic and array types
  stay rejected. Example: `TreeMap.buildFromSorted` takes an `ObjectInputStream` that every copy
  of a sorted map passes as null.
- **`serialPersistentFields`** is initialized only if code reads it: serialization reads it by
  reflection, and its initializer needs `Class` values.

### Class loading, resources and classes by name

A translated program is closed: it knows all its classes. So:

- `jdk.internal.loader.` is untranslated ("class loading"), as are reflection
  (`java.lang.reflect.`, `java.lang.invoke.`, with the exceptions in `TRANSLATED_ANYWAY`),
  serialization streams and security (`UNTRANSLATED`). Of security, message digests are
  translated: `MessageDigest`, `DigestInputStream` and the `SUN` provider's digests, which
  `MessageDigest.getInstance(String)` makes by name (`java/patches/java/security/MessageDigest.java`,
  `TranslatedDigests`); a `Provider` and the methods that name one are not. Neither are keys
  and specs, except their plain interfaces, which the program's own classes implement (see
  [object-model.md](object-model.md) 1.9). `ResourceBundle`, `ServiceLoader` and
  `Module` are translated, the last two (and `ModuleLayer`) patched (see [reflection-and-plugins.md](reflection-and-plugins.md)).
- `Class.forName` of a name spelled out as a literal is that class's literal, after its static
  initialization when the call initializes (`program::class_for_name`, `Builtin::InitializeClass`).
  The loader argument is evaluated for its effects only.
- `(T) Class.forName(name).getDeclaredConstructor().newInstance()` with a computed name is a
  `switch` over the classes whose binary names reachable code writes as string literals
  (`program/reach/by_name.rs`, `lower/by_name.rs`); any other name throws a
  `ClassNotFoundException` that says so (`Builtin::UnknownClass`). A computed
  `Class.forName(module, name)` finds the locale data classes of the root locale, `en` and
  `en_US` (`Reach::found_by_name`), and `null` for any other name, as for a bundle the JDK lacks.
- `MethodHandles.lookup().ensureInitialized(c)` initializes `c`'s class. Reachability takes as
  initialized the classes whose literals reachable code passes to it, directly or through one
  method's parameter (`program/reach/initialize.rs`, `Reach::ensured`).
- `Class.getResourceAsStream` and `getResource` end the program with an `InternalError` naming
  the resource (`Builtin::MissingResource`): data files such as `uniName.dat` (`Character.getName`),
  `currency.data` and the ICU normalizer data are generated when the JDK is built and are not in
  its sources. A class file, a name that certainly ends in `.class` (a literal, or a
  concatenation whose last operand is one), is `null` instead, a resource that is not there: the
  translation has no class files, since its classes were translated from their source.

### Strings and characters in the JDK's code

- `String` is the runtime's, so the JDK's `String` methods are not translated. `chars()` is the
  JDK's `Arrays.stream(int[])` over the string's UTF-16 units, and `lines()` streams an
  `Object[]` of the lines through an iterator of unknown size, as the JDK's `LinesSpliterator`
  has no size, so `count()` still runs the pipeline's lambdas (`STRING_STREAMS`).
- `JavaLangAccess.join`, which `StringJoiner` calls, is the runtime's `join_range`; the
  `JavaLangAccess` methods in `TRANSLATED_LANG_ACCESS` are the translator's own static methods,
  since `SharedSecrets.getJavaLangAccess()` is null in a translation.
- `==` on strings in library code is value equality (`Lowering::string_comparison`). The
  JDK compares by reference only strings it interned, locale parts and canonical charset names,
  or as a shortcut before comparing characters, where value equality gives the JVM's answer. It
  would be wrong only where the library keeps a `new String(..)` as a marker to tell apart; none
  of the reached code does. The program's own `==` on strings is decided by where its strings
  come from ([object-model.md](object-model.md), section 1.6).
- `toUpperCase(locale)` and `toLowerCase(locale)` are the root locale's mapping, which is every
  language's but Turkish, Azeri and Lithuanian; for those three the program ends with a message
  (`java_runtime::string::uppercase_in`).
- `Character` is translated from the JDK, with its nested classes and `CharacterData` tables,
  except the box, its instance methods and the `char` and code point methods that the runtime
  answers (`lower::runtime_character_method`).
- `String.format` given an `Object[]` of arguments formats each element by its `toString`
  (`java::format_objects`), so only `%s`, `%S`, `%n` and `%%` work; any other conversion fails
  loudly.
- `System.getLogger(name).log(level, message)` prints `LEVEL: message` on standard error from
  `INFO` up, as the JDK's default logger does, without its line of time and caller.
- The JDK's internal `sun.util.logging.PlatformLogger` logs the same way (`lower/platform_logger.rs`):
  `info`, `warning` and `severe` print `LEVEL: message`, with the stack trace of a `Throwable`
  logged with one, the lower levels only evaluate their arguments, and `isLoggable(level)` is
  whether the level is `INFO` or above. A logger has no value: it is `getLogger(name)` itself, a
  local that `getLogger(name)` sets, or a static field that its declaration sets so, with a
  constant name, that nothing reassigns and that is set before the rest of its class's static
  initializer runs. A read of such a field from another class (a nested class's read of its
  outer class's) initializes the field's class first, as in Java. A message with parameters at
  a level that is printed, and any other use of a logger, is rejected.

### References

`java.lang.ref` is translated from the JDK (`lower/references.rs`). A translated program has no
garbage collector that clears a reference behind its back: an object is freed when its last strong
reference goes, and a reference holds one. So a reference behaves as on a JVM whose collector
never needs the memory: `get` gives the referent until `clear`, and no reference is enqueued but
by `enqueue`.

## Standard streams and `java.io` in a translation

With the JDK's sources, a program can use `System.out`, `System.err` and `System.in` as objects:
keep one in a variable, field or parameter, wrap it (`new BufferedReader(new
InputStreamReader(System.in))`), and subclass `OutputStream`, `InputStream`,
`FilterOutputStream`, `ByteArrayOutputStream` or `PrintStream`. The byte streams of `java.io` are
the JDK's, translated; the runtime writes only the descriptors, and three classes are patched.
Without the JDK's sources, `System.out` as a value is rejected with "`System.out` as a value
needs the JDK's sources".

### System.out, System.err and System.in as values

A call written on `System.out` or `System.err` itself (`System.out.println(x)`, `print`,
`printf`, `format`, `flush`, `write(int)`; `System.err.print`, `println`, `flush`) stays the
runtime's direct call, and reachability does not enter `PrintStream` for it. Any other use of the
field reads a static field of a holder class that the translator adds to the JDK's sources
(`crates/translator/java/overlay/java/io`):

- `System.out` and `System.err` are `StandardOutput.out` and `StandardOutput.err`: a
  `PrintStream` with automatic flushing over a `StandardOutput`, an `OutputStream` whose natives
  are the runtime's standard streams (`out.rs`: `standard_write`, `standard_write_bytes`,
  `standard_flush`, `standard_close`).
- `System.in` is `StandardInput.in`: a `BufferedInputStream` over a `FileInputStream` of
  `FileDescriptor.in`, the object the JVM makes, all translated from the JDK over the runtime's
  descriptor table (`fd.rs`). `available` on a pipe or terminal answers the bytes waiting
  (`FIONREAD`). Descriptor 0 is read without a buffer in between, so each read returns what the
  JVM's would, and standard output is flushed before a read so that a prompt shows.

The JVM's `System.out` is a `PrintStream` over a 128-byte `BufferedOutputStream` over a
`FileOutputStream`. The translation does not build that chain, because the direct calls and the
value must write through one buffer to stay in order: `System.out.write('x')` through the value
followed by a direct `System.out.println()` must give `x` and then the newline, and an unflushed
`write` must be lost at exit either way. So the runtime keeps the 128 bytes itself (`out.rs`) with
the same rules: a byte waits until a newline (automatic flushing), a flush, a print or a full
buffer; a range of 128 bytes or more bypasses it; what is left at exit is lost. The bytes, their
order between standard output and standard error, the flushes and the exit code are Java's. An
out-of-range `write(byte[], off, len)` throws what the JVM's buffer throws.

`PrintStream` is the JDK's with a patch (`java/patches/java/io/PrintStream.java`): the JDK's
encodes through a `StreamEncoder`, which the translation cannot take yet. The patched class
encodes each piece of text with `String.getBytes(charset)` and writes the bytes through its own
`write(byte[], int, int)`, as the JDK's encoder does, so a subclass that overrides `write` sees
the same bytes. `println` of a subclass goes through its `print` and then the line separator;
errors set `checkError` and are not thrown. `printf` and `format` on a `PrintStream` value are
formatted at the call and printed through the package-private `formattedText`.

`Charset` and `StandardCharsets` are runtime classes, so reachability does not enter the JDK's
charset lookup (whose provider search reaches `GB18030`, which reads a JVM property). The JDK's
decoders and encoders of the six standard charsets are translated all the same and made by
`java/overlay/sun/nio/cs/TranslatedCoders.java` for `newDecoder()` and `newEncoder()`.
`Charset.decode(ByteBuffer)`, `encode(CharBuffer)` and `encode(String)` are routed to the same coders
(`program::CHARSET_CODINGS`), with `REPLACE` for malformed input, so they never throw, as in Java.
`corpus/whole-program/CharsetCodings.java` runs them on valid and malformed UTF-8, on ISO-8859-1
text outside the charset, and on `Charset.encode(String)`, and matches Java.

### Readers and writers over byte streams

`Reader`, `Writer`, `BufferedReader`, `BufferedWriter`, `PrintWriter` and the byte streams
(`BufferedInputStream`, `BufferedOutputStream`, `FilterOutputStream`, `ByteArrayInputStream`,
`ByteArrayOutputStream`, `FileInputStream`, `FileOutputStream`) are translated from the JDK.
`InputStreamReader` and `OutputStreamWriter` are patched (`java/patches/java/io`) for the same
reason as `PrintStream`: the JDK's decode and encode through `StreamDecoder` and `StreamEncoder`.

- The patched `InputStreamReader` reads into a buffer of 8192 bytes, reading again in one call
  only while the stream has bytes available, and decodes with `new String(bytes, off, len,
  charset)`, which replaces malformed input as the JDK's decoder does. Bytes at the end of a read
  that may begin a character wait for the next read.
- The patched `OutputStreamWriter` buffers as `StreamEncoder` does: the buffer starts at 512
  bytes, grows up to 8192 for a long write, is passed on when the next character does not fit, on
  `flush` and on `close`, and is lost if the program ends without either. A lone surrogate and an
  unmappable character become the replacement (`?`, U+FFFD in UTF-16).

### What differs from Java

- `InputStreamReader` and `OutputStreamWriter` take UTF-8, US-ASCII, ISO-8859-1, UTF-16BE and
  UTF-16LE; any other charset (UTF-16 with a byte-order mark among them) throws
  `UnsupportedOperationException` when the reader or writer is created.
- `InputStreamReader` may ask its stream for a different number of bytes per read than the JDK's,
  which only an `InputStream` subclass that counts its calls can see.
- `printf` and `format` on a `PrintStream` subclass print the formatted text with one `print`
  call, where the JDK's `Formatter` calls `append` once per piece, and an override of `format`
  is not called by `printf`.
- `PrintStream.append(csq, start, end)` takes the substring of `String.valueOf(csq)`, which for a
  program's own `CharSequence` is not its `subSequence`.
- Standard input is descriptor 0 on Unix only; elsewhere reading it panics.
- A `NullPointerException` raised inside the three patched classes has the message of the patch's
  code, not the JDK's.
- `Scanner` reaches far more of the JDK than `BufferedReader` (locale data, regular expressions,
  `java.text`); no corpus program reads standard input with it yet.

## Plans

Not done:

- Hand-writing the remaining natives as programs need them, keeping to the policy above (for
  example `UnixFileSystem.getSpace0`, which needs `statvfs`).
- The `CharSequence` enum of the object model has no variants when a program's only conversion of a
  `CharBuffer` to `CharSequence` is the translator's own call (`char_sequence_call`). The flow
  analysis (`analysis/hierarchy.rs` `flows`) records no edge for it, so the interface's methods have
  no impl, and `FileChannelReadWrite` (which decodes into a `CharBuffer` from the JDK code) does not
  compile. Fixing it changes how every interface's variants are chosen, so it is not done here.
- Narrowing the dispatch of `Object`'s methods: library code calls `hashCode`, `equals` and
  `toString` on `Object`-typed values, and class-hierarchy dispatch then reaches every live class
  (`Hashtable.computeIfAbsent -> URL.hashCode -> InetAddress`). The rule that makes those methods
  of every instantiated class reachable is not the cause; only a points-to analysis of the
  library code would narrow it.

## The survey

`--survey` (text) and `--survey-json FILE` (JSON) report what a program reaches beyond the
translated sources: untranslated classes, classes outside the sources, the methods of the
runtime's classes it calls, and the natives it reaches, each native marked when the runtime
implements it (`program/survey.rs`). Every native and every method called outside the sources
comes with the shortest chain of calls from `main` that reaches it (`chain` in the JSON). `--census`
lists what the translator rejects, past every rejection (`program/census.rs`).

```
target/debug/java-to-rust --main Name --source DIR --library "$JAVA_BASE_SOURCES" --survey-json Name.json
```

Put each program in a directory of its own (javac rejects a listed file next to a module's source
root). A run over a program that reaches much of the JDK takes minutes and over a gigabyte of
memory, most of it javac's tree as JSON.
