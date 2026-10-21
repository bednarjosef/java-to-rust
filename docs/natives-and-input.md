# Natives and standard input: what the JDK step needs

Whole-program mode follows everything a program reaches in the JDK's own source and translates
it. This is a census of where that reach bottoms out, so that the JDK-support step has concrete
lists:

1. the native methods reached, with one call chain each and what each becomes in Rust;
2. what reading standard input pulls in, end to end;
3. the unsupported language features, ranked by how much reachable JDK code they block;
4. the natives to hand-write first, and changes to the reachability boundary.

**Status.** The natives of section 4.1 that need neither the object model nor exceptions are
implemented, with the others of the census that need no objects (4.4), and so are boundary
changes 2, 4 and 7 of section 4.2 (4.5). The tables of section 1 and the numbers of sections 2
and 3 are the census as it was made, before those changes; section 4.5 has the numbers after.

The hand-writing policy is unchanged: only (1) what bottoms out in the JVM, (2) the core types the
runtime maps to Rust types (`String`, `StringBuilder`, the boxes, arrays, the `Object`, `Enum` and
`Throwable` bases) and (3) calls with a direct Rust std equivalent are hand-written; everything
else is translated.

## How the numbers were made

Each program was surveyed alone, as a source directory of its own, with the java.base sources of
JDK 26 (`$JDK`) as the library root:

```
target/debug/java-to-rust --main Name --source DIR --library $JDK --survey-json Name.json
```

The programs are the 28 of `targets/`, the three projects of `corpus/projects/` and four small
standard-input programs written for this census (section 2). The survey now names, for every
native method, the shortest chain of calls from `main` that reaches it (`via ...` in the text
report, `chain` in the JSON), and the JSON also lists the methods of the runtime's classes that
are called (`external`) and every class whose objects are created (`instantiated`), each with its
chain. A chain is one example of why something is reached, found by a breadth-first search over
the call graph of the reachable code: calls, virtual dispatch to overriders in live classes, and
static initialization.

The numbers below are after one pruning fix made for this census (section 4.2): the code that Java
Flight Recorder's `jfrTracing` flags guard is no longer reached. Before it, every program that
creates an exception reached `jdk.internal.event.ThrowableTracer`, an `AtomicLong` and three more
`Unsafe` natives.

| Programs | Classes reached | Natives reached (before the fix) | Natives reached (after) |
|---|---|---|---|
| 25 typical targets (not the three below) | 40 to 108 | 8 to 11 | 1 to 8 |
| `StreamPipeline` | 390 | 31 | 31 |
| `ScannerGrades`, `TextAdventure` (read with `Scanner`) | 1,649, 1,654 | 191 | 191 |
| the three corpus projects | 2 to 6 | 0 | 0 |

The corpus projects use only the runtime's classes (strings, printing, `Math`), so they reach no
JDK code at all.

Over all 35 runs, 191 distinct natives are reached; 24 of them are threads and synchronization,
167 are not. But almost all of them come from `Scanner` alone: the 25 typical targets together
reach only 8 natives (section 4.1), and `StreamPipeline` adds 23 more (22 `Unsafe` operations and
`VM.initialize`), all behind `ConcurrentHashMap` and `ForkJoinPool` on the parallel paths of
streams.

## 1. Native methods reached

Two layers bottom out in the JVM:

- **Natives in translated code**: methods declared `native` in JDK classes that are translated.
  These are the census proper; they are listed by group below.
- **Natives of the runtime's classes**: methods of `RUNTIME_CLASSES` (`Object`, `System`, `Class`,
  `Thread`, the boxes, `String`) that are native in the JDK and called by reachable code. The
  runtime already provides most of them (`lower/natives.rs`, `runtime/src/natives.rs`):

| Native of a runtime class | Programs | Status |
|---|---|---|
| `Object.getClass()` | 28 | not yet: needs `Class` objects (object model), see class and reflection |
| `Object.hashCode()` | 28 | done (`java::identity_hash`) |
| `System.arraycopy` | 22 | done |
| `System.nanoTime()` | 21 | done; reached through `ImmutableCollections.<clinit>` (the `Set.of` seed) |
| `System.identityHashCode` | 13 | done |
| `Class.getSuperclass`, `isAssignableFrom`, `isInstance` | 2 to 4 | not yet: class model |
| `Object.clone()` | 3 | done for arrays (`Array.clone`); objects need the object model |
| `Runtime.availableProcessors`, `System.currentTimeMillis` | 3 | done |
| `String.intern()` | 3 | done: the string itself (`StringOp::Identity`); `==` on two strings stays rejected, see 4.1 |
| `Thread.currentThread`, `holdsLock`, `Object.notifyAll` and the scoped-value natives of `Thread` | 2 to 3 | threads design |
| `Double.doubleToRawLongBits`, `longBitsToDouble` | 2 | done |

`StrictMath` has no natives in JDK 26 (it is the Java port of fdlibm), and `Float` and `Double`
are done, so the numbers group has nothing left at this layer.

Below, each native in translated code is listed with the number of the 31 target and project
programs that reach it, one chain, whether the reach is real or an artifact of the boundary, and
the proposed Rust. "Scanner" means the two `Scanner` targets (`ScannerGrades`, `TextAdventure`).
Chains are shortened to their interesting part with `...`; the full ones are in the survey output.

### Memory and arrays

| Native | Programs | Chain | Real? | Rust |
|---|---|---|---|---|
| `Unsafe.registerNatives()` | 22 | `ArrayList.grow -> ArraysSupport.<clinit> -> Unsafe.<clinit> -> Unsafe.runtimeSetup -> registerNatives` | real (class initialization) | no-op |
| `Unsafe.arrayBaseOffset0(Class)` | 22 | `ArrayList.grow -> ArraysSupport.<clinit> -> Unsafe.<clinit> -> arrayBaseOffset -> arrayBaseOffset0` | real: `Unsafe.<clinit>` computes `ARRAY_*_BASE_OFFSET` | constant, see 4.1 |
| `Unsafe.arrayIndexScale0(Class)` | 22 | same, `ARRAY_*_INDEX_SCALE` | real | constant, see 4.1 |
| `Unsafe.getInt/putInt/getLong/putLong/getReference/putReference(Object, long)` | 3 | `StreamPipeline ... ConcurrentHashMap ... ThreadLocalRandom.getProbe -> Unsafe.getInt` | real, but only on `ConcurrentHashMap` and `ThreadLocalRandom` paths | field access through a symbolic offset (threads design, 4.3) |
| `Unsafe.getByte/putByte/getShort/getChar/...`, `copyMemory0`, `copySwapMemory0`, `setMemory0`, `allocateMemory0`, `freeMemory0` | 2 (Scanner) | `ByteBuffer.compact -> DirectByteBuffer.compact -> ScopedMemoryAccess.copyMemory -> Unsafe.copyMemory0` | artifact: direct buffers are never created on these paths; `ByteBuffer.compact` dispatches to `DirectByteBuffer` because something made it live | none: prune direct and mapped buffers (4.2) |
| `MappedMemoryUtils.registerNatives`, `isLoaded0`, `load0`, `unload0`, `force0` | 2 (Scanner) plus the stdin programs | `StreamDecoder.<init> -> ByteBuffer.<clinit> -> Buffer.<clinit> -> MappedMemoryUtils.<clinit> -> force -> force0` | artifact: `Buffer.<clinit>` registers an anonymous `JavaNioAccess` whose methods are all scanned with the initializer | none (4.2) |
| `ScopedMemoryAccess.registerNatives` | 2 plus stdin | `Buffer.<clinit> -> ScopedMemoryAccess.<clinit>` | real (static field of `Buffer`) | no-op |
| `Unsafe.writeback0`, `writebackPreSync0`, `writebackPostSync0` | 2 plus stdin | `MappedMemoryUtils.force -> Unsafe.writebackMemory` | artifact (mapped files) | none |

`System.arraycopy` and array `clone` are done in the runtime; `java.lang.reflect.Array.newInstance`
(the array creation behind `Arrays.copyOf(T[], int)`, `ArrayList.toArray(T[])` and
`Collection.toArray`) is cut off as reflection today and reached from 5 programs. It is
`Array.newArray`, a native, and belongs with the hand-written natives once arrays of objects exist:
`java::Array::filled(length, None)` for the element class given, with `NegativeArraySizeException`
for a negative length.

### Numbers

Nothing left: `Float` and `Double` bit conversions are done, `StrictMath` is translated Java in
JDK 26, and `CRC32.updateBytes0` (Scanner only, through time zone data) is listed under I/O.

### Identity

`Object.hashCode` and `System.identityHashCode` are done (derived from the `Rc`'s address, stable
for the object's life, different on every run as in Java). Nothing in translated code is native
here. `String.intern` (3 programs, through `ObjectStreamField.getSignature`, an artifact of
`Object.toString` dispatch) is the only identity-related native left.

### Time and environment

| Native | Programs | Chain | Real? | Rust |
|---|---|---|---|---|
| `CDS.getCDSConfigStatus()` | 21 | `List.of -> ImmutableCollections.<clinit> -> CDS.<clinit> -> getCDSConfigStatus` | real | `0` (no class data sharing archive) |
| `CDS.getRandomSeedForDumping()` | 21 | `List.of -> ImmutableCollections.<clinit> -> getRandomSeedForDumping` | real | `0` |
| `CDS.initializeFromArchive(Class)` | 21 | `List.of -> ImmutableCollections.<clinit> -> initializeFromArchive` | real | no-op |
| `VM.initialize()` | 3 | `ConcurrentHashMap.addCount -> ThreadLocalRandom.<clinit> -> VM.<clinit> -> initialize` | real | no-op |
| `VM.getNanoTimeAdjustment(long)` | 2 (Scanner) | `Object.equals -> DecimalFormatSymbols.equals -> getCurrencySymbol -> Currency.<clinit> -> ... LocalDateTime.parse -> ...` | artifact here (`Object.equals` dispatch); real for `Instant.now()` | `SystemTime::now()` relative to the offset, see 4.1 |
| `TimeZone.getSystemTimeZoneID(String)`, `getSystemGMTOffsetID()` | 2 (Scanner) | `... LazyConstantImpl.toString -> Object.toString -> Date.toString -> Date.normalize -> TimeZone.getDefaultRef -> setDefaultZone` | artifact (`Object.toString` dispatch) | `TZ` or `/etc/localtime`, later |
| `Perf.registerNatives`, `Perf.createLong` | 2 (Scanner) | `JarLoader.getJarFile -> ZipFile.<init> -> PerfCounter.getZipFileCount -> ... Perf.createLong` | artifact (class loading) | none |

### Threads and synchronization

All of these are part of the threads design; they are listed so that it covers them.

| Native | Programs | Chain |
|---|---|---|
| `Unsafe.compareAndSetInt/Long/Reference`, `compareAndExchangeLong/Reference` | 3 | `Collectors.groupingBy -> Map.computeIfAbsent -> ConcurrentHashMap.computeIfAbsent -> casTabAt -> compareAndSetReference` |
| `Unsafe.getIntVolatile/getLongVolatile/getReferenceVolatile`, `putIntVolatile/putLongVolatile/putReferenceVolatile`, `fullFence` | 3 | `ConcurrentHashMap.get -> ... TreeBin.find -> Unsafe.getAndAddInt -> getIntVolatile` |
| `Unsafe.park(boolean, long)`, `unpark(Object)` | 3 | `ConcurrentHashMap.TreeBin.contendedLock -> LockSupport.park -> Unsafe.park` |
| `VirtualThread.registerNatives`, `takeVirtualThreadListToUnblock` | 2 (Scanner) | the virtual-thread unblocker, through `Thread` paths |
| `Continuation.registerNatives`, `pin`, `unpin`, `ContinuationSupport.isSupported0` | 2 plus stdin | `Buffer.<clinit> -> MappedMemoryUtils.force -> Blocker.begin -> CarrierThread.beginBlocking -> Continuation.pin` (artifact, as above) |
| `NativeThread.current0`, `init`, `signal0`, `supportPendingSignals0` | 2 (Scanner) | socket and file channels (artifact) |

In `StreamPipeline` they are reached because `Stream.distinct` and `Stream.count` have parallel
branches (`DistinctOps.makeRef` creates a `ConcurrentHashMap`, `ReduceOps.evaluateParallel` a
`ForkJoinPool`), and then `Map.computeIfAbsent` dispatches to the live `ConcurrentHashMap`. A
sequential program runs none of them (4.2). In the Scanner programs they come with the class
loaders. `Object.wait`/`notify` and `Thread.start` are not reached by any program.

`Unsafe` itself is the main open question for the threads design: the JDK's atomics
(`AtomicInteger`, `AtomicLong`, `ConcurrentHashMap`, `LazyConstantImpl`) address fields by an
offset from `Unsafe.objectFieldOffset(C.class, "name")` and array elements by
`ARRAY_*_BASE_OFFSET + (i << shift)`. Both are computed from constants, so the translator can
resolve an offset to the field or the element it names and turn `U.compareAndSetInt(this, VALUE,
e, n)` into an atomic operation on that field. With that, `Unsafe` belongs in `RUNTIME_CLASSES`.

### Class and reflection

| Native | Programs | Chain | Real? | Rust |
|---|---|---|---|---|
| `Throwable.fillInStackTrace(int)` | 28 | `Exception.<init> -> Throwable.<init> -> fillInStackTrace() -> fillInStackTrace(int)` | real: every exception | return `this`, record nothing (4.1) |
| `NullPointerException.getExtendedNPEMessage()` | 25 | `Throwable.getMessage -> NullPointerException.getMessage -> getExtendedNPEMessage` | real when a program prints an NPE's message | `null` (4.1) |
| `Unsafe.knownObjectFieldOffset0`, `objectFieldOffset0`, `staticFieldOffset0`, `staticFieldBase0` | 3 | `AtomicLong.<clinit> -> Unsafe.objectFieldOffset -> knownObjectFieldOffset0` | real, constants | symbolic field offsets (threads design) |
| `Unsafe.ensureClassInitialized0`, `throwException` | 2 (Scanner) | class loading, `Lookup` | artifact | none |
| `StackTraceElement.initStackTraceElements`, `StackStreamFactory.*` (4) | 2 (Scanner) | `Throwable.printStackTrace -> getOurStackTrace -> StackTraceElement.of`, `StackWalker.walk` | artifact: needs a backtrace, which the runtime never records | none |
| `Module.defineModule0`, `addReads0`, `addExports0`, `addExportsToAll0` | 2 (Scanner) | `StackTraceElement.computeFormat -> isHashedInJavaBase -> ModuleLayer.<clinit> -> Module.defineModules` | artifact (stack traces, class loading) | none |
| `BootLoader.*` (3), `NativeLibraries.*` (3), `NativeImageBuffer.getNativeMap` | 2 (Scanner) | `ResourceBundle.loadBundle -> Module.getResourceAsStream -> BootLoader.packages`; `UnixNativeDispatcher.<clinit> -> BootLoader.loadLibrary` | real only for class and resource loading, which a translated program does not do | none (4.2) |

`Object.getClass` (28 programs, a runtime class native) is reached mostly as an exact-type test:
`o.getClass() == ArrayList.class` in `ArrayList.equals` and `ArrayList(Collection)`, and the same in
`ImmutableCollections.listFromTrustedArray` and `EnumMap.typeCheck`. The object model can lower
`x.getClass() == C.class` to a test of the dynamic class without any `Class` object. The other uses
(`Throwable.toString`'s `getClass().getName()`, `HashMap.TreeNode.tieBreakOrder`) need `Class`
objects: one static `Class` per translated class with its name, superclass and interfaces, which
also answers `isInstance`, `isAssignableFrom` and `getSuperclass`.

### I/O

| Native | Programs | Chain | Real? | Rust |
|---|---|---|---|---|
| `FileInputStream.readBytes`, `read0`, `available0`, `initIDs` | 2 (Scanner); `ReaderLinesFd` | `Scanner.hasNext -> readInput -> Reader.read -> StreamDecoder.implRead -> readBytes -> InputStream.read -> FileInputStream.read -> readBytes` | real: this is reading standard input | done in `fd.rs` (`fd_read_bytes`, `fd_read`, `fd_available`); `initIDs` no-op |
| `FileInputStream.open0`, `length0`, `position0`, `isRegularFile0` | 2 (Scanner) | `Currency.initStatic -> FileReader.<init>`; `JarLoader` | artifact (currency data file, jars) | `open0` done; the others with `std::fs` when a program reads files |
| `FileDescriptor.initIDs`, `getHandle`, `getAppend`, `close0` | 2 (Scanner); `ReaderLinesFd` | `FileDescriptor.<clinit> -> initIDs`, `getHandle(0)` | real: `FileDescriptor.in` is created in the initializer | `initIDs` no-op, `getHandle` `-1` (Windows only), `getAppend` `false` for 0, 1, 2, `close0` done |
| `FileCleanable.cleanupClose0` | 2 | `FileDescriptor.<clinit> -> (anonymous JavaIOFileDescriptorAccess) -> FileCleanable.register` | artifact | none |
| `RandomAccessFile.*` (7), `UnixFileSystem.*` (7), `sun.nio.fs.UnixNativeDispatcher.*` (15), `sun.nio.ch.UnixFileDispatcherImpl.*` (14), `FileDispatcherImpl.*` (3) | 2 (Scanner) | `Scanner.close -> Closeable.close -> URLClassPath.JarLoader.close -> ensureOpen -> getJarFile -> JarFile.<init> -> ZipFile ... RandomAccessFile.open0` | artifact: `Closeable.close` dispatches to every live `Closeable`, and class loading made jar loaders live | none now; `std::fs` once programs use files |
| `java.net.*` (12), `sun.nio.ch.Net`, `IOUtil`, `EPoll`, `SocketDispatcher`, `UnixDispatcher` (27) | 2 (Scanner) | `Hashtable.computeIfAbsent -> Object.hashCode -> URL.hashCode -> URLStreamHandler.getHostAddress -> InetAddress.<clinit>`; `InputStream.read -> SocketInputStream.read -> ... EPoll.create` | artifact (`Object.hashCode` and `InputStream.read` dispatch) | none |
| `Inflater.*` (6), `CRC32.updateBytes0` | 2 (Scanner) | `InputStream.read -> InflaterInputStream.read -> Inflater.inflate`; `ZoneInfoFile.Checksum.update` | artifact | none |

## 2. Standard input end to end

The two programs, for the corpus once they translate:

```java
import java.util.Scanner;

// Reads a count, that many integers, then the rest of the lines; prints the sum and each line.
public class ScannerSum {
    public static void main(String[] args) {
        Scanner in = new Scanner(System.in);
        int n = in.nextInt();
        long sum = 0;
        for (int i = 0; i < n; i++) {
            sum += in.nextInt();
        }
        in.nextLine();
        System.out.println("sum " + sum);
        int lines = 0;
        while (in.hasNext()) {
            String line = in.nextLine();
            lines++;
            System.out.println(lines + ": " + line.trim());
        }
        System.out.println("lines " + lines);
    }
}
```

```java
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;

// Reads standard input line by line and prints each line's length and the longest line.
public class ReaderLines {
    public static void main(String[] args) throws IOException {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String longest = "";
        int count = 0;
        String line;
        while ((line = in.readLine()) != null) {
            count++;
            System.out.println(count + " " + line.length());
            if (line.length() > longest.length()) {
                longest = line;
            }
        }
        System.out.println("longest: " + longest);
    }
}
```

With the input `3`, `10 20`, `30`, `first line`, `  second  ` (one per line), Java prints
`sum 60`, `1: first line`, `2: second`, `lines 2` and `1 1`, `2 5`, `3 2`, `4 10`, `5 8`,
`longest: first line`.

`ScannerSumFd` and `ReaderLinesFd` are the same programs with `System.in` written out as the JVM
builds it in `System.initPhase1`: `new BufferedInputStream(new FileInputStream(FileDescriptor.in))`.

| Program | Classes reached | Instantiated | Methods | Pieces of code | Natives |
|---|---|---|---|---|---|
| `ReaderLines` | 395 | 165 | 1,164 | 1,617 | 41 |
| `ReaderLinesFd` | 413 | 179 | 1,234 | 1,709 | 50 |
| `ScannerSum` | 1,647 | 1,101 | 10,692 | 13,532 | 191 |
| `ScannerSumFd` | 1,647 | 1,101 | 10,693 | 13,533 | 191 |

### `System.in` is not followed today

`System` is a runtime class, so reachability stops at the field `System.in`: its static type is
`InputStream`, and `InputStream.read()` is abstract. In `ReaderLines` no class that can read is
live, so the survey reaches no input native at all: `FileInputStream` is not even among the
classes. With `System.in` written out (`ReaderLinesFd`) the chain goes down to the descriptor:

```
ReaderLinesFd.main -> BufferedReader.readLine -> BufferedReader.fill -> Reader.read
  -> StreamDecoder.read -> StreamDecoder.implRead -> StreamDecoder.readBytes
  -> InputStream.read -> FileInputStream.read -> FileInputStream.readBytes
```

and reaches `FileInputStream.readBytes`, `read0`, `available0` (through `StreamDecoder.inReady`)
and `initIDs`, and `FileDescriptor.initIDs`, `getHandle`, `getAppend`, `close0`. All of these but
the three `initIDs`/`getHandle`/`getAppend` trivia are already in the runtime's descriptor table
(`fd.rs`), so the bottom of standard input is done; what is missing is the path to it.

### What `BufferedReader` over `InputStreamReader` pulls in

`ReaderLinesFd` reaches 259 classes with code. The main pieces, with the unsupported features of
their reachable code (pieces of code that use each feature):

| Class | Pieces | Blocked by |
|---|---|---|
| `InputStream`, `FilterInputStream`, `BufferedInputStream`, `FileInputStream` | 4, 4, 16, 10 | inheritance (all), interfaces, exceptions (6 in `BufferedInputStream`) |
| `FileDescriptor` | 17 (11 free) | generic types, one anonymous class (the `SharedSecrets` accessor) |
| `Reader`, `InputStreamReader`, `BufferedReader` | 3, 3, 16 | inheritance, interfaces, `Object` values, exceptions |
| `sun.nio.cs.StreamDecoder` | 13 | inheritance, `Object` values (its lock), exceptions, instanceof |
| `Charset`, `CharsetDecoder`, `UTF_8`, `UTF_8.Decoder`, `StandardCharsets` | 17, 21, 5, 17, 19 | inheritance, interfaces, generic types, anonymous classes (3 in `Charset`), exceptions |
| `CoderResult` | 20 (17 free) | exceptions, one lambda |
| `Buffer`, `ByteBuffer`, `HeapByteBuffer`, `CharBuffer`, `HeapCharBuffer` | 29, 21, 12, 24, 13 | inheritance, interfaces, super calls, exceptions, anonymous classes (2 in `Buffer`) |
| `StaticProperty`, `Properties`, `ConcurrentHashMap` | 4, 1, 56 | `ConcurrentHashMap`: generic classes, interfaces, casts, exceptions, and `Unsafe` |

Over all 1,708 library pieces of `ReaderLinesFd`, 337 use no unsupported feature; inheritance
(848 pieces), interfaces (687), generic classes (451), generic types (408), `Object` values (237)
and exceptions (167) block the rest, so the object model and exceptions free almost all of it.
The rest of the 413 classes are the exception classes, `java.util` collections reached through
`Charset` and `Properties`, `java.lang.foreign` and `jdk.internal.foreign` (the memory-segment
side of `ByteBuffer`), `java.lang.constant` and `jdk.internal.ref` (the `Cleaner`).

Of its 50 natives, eight are the stream and descriptor natives above (three of them, `readBytes`,
`read0` and `available0`, do the reading), eight are the shared ones of section 4.1, and the
remaining 34 are artifacts or threads:
`Buffer.<clinit>`'s anonymous accessor (`MappedMemoryUtils`, `Continuation`, the `writeback`
natives), `FileDescriptor.<clinit>`'s anonymous accessor (`FileCleanable`, which starts the
`Cleaner` thread), and `Properties`, which is a `ConcurrentHashMap` (the `Unsafe` operations).

### What `Scanner` pulls in

`Scanner` is a different size: 1,647 classes, 191 natives, 13,532 pieces of code. The scanner's own
classes are moderate (`Scanner` 68 pieces, `java.util.regex.Pattern` 97 and `Matcher` 48, both
blocked by interfaces, exceptions and, in `Pattern`, 17 lambdas). The growth comes from one call:

```
Scanner.<init> -> Scanner.useLocale -> NumberFormat.getNumberInstance(locale)
  -> LocaleResources.getNumberPatterns -> LocaleData.getNumberFormatData -> LocaleData.getBundle
  -> Bundles.<clinit> -> ResourceBundle.<clinit> -> ResourceBundle.getBundleImpl -> findBundle
  -> loadBundle -> ResourceBundleProviderHelper.loadPropertyResourceBundle
  -> Module.getResourceAsStream -> BootLoader ... -> Files.newInputStream -> ...
```

Locale data is found by class and resource loading: `ResourceBundle` and `ServiceLoader` look up
classes by name, through the module layer, the class loaders, jar and jimage files, URLs and file
channels. Once those classes are live, the dispatch of `Object.hashCode`, `Object.equals`,
`Object.toString`, `InputStream.read` and `Closeable.close` reaches every one of them:
`Scanner.close` reaches `URLClassPath.JarLoader.close` and from there `ZipFile` and `RandomAccessFile`;
`Hashtable.computeIfAbsent` reaches `URL.hashCode` and `InetAddress`.

Cutting class loading off as untranslated (an experiment, not committed: `java.lang.Module`,
`java.util.ResourceBundle`, `java.util.ServiceLoader` and `jdk.internal.loader.` added to
`UNTRANSLATED`) brings `ScannerSum` down to 1,199 classes and 77 natives, and does not change
`ReaderLinesFd` (401 classes, 50 natives). What remains of `Scanner` is `java.util.stream` (105
instantiated classes), `java.util.regex` (56), `java.util.concurrent` (43), `java.time.format` (35),
`java.text` (32) and the ICU normalizer (`jdk.internal.icu`, 52), mostly through `Object.toString`
dispatch again (`DecimalFormat.toString -> Locale.getDisplayName -> streams`). A second
experiment, without the rule that makes `toString`, `equals` and `hashCode` of every instantiated
class reachable, gave exactly the same counts: library code calls those methods itself, so it is
the class-hierarchy dispatch on `Object`'s methods, not the rule, that spreads the reach.

### The recommended boundary for standard input

- **Translate**: `InputStream`, `FilterInputStream`, `BufferedInputStream`, `FileInputStream`,
  `FileDescriptor`, `Reader`, `InputStreamReader`, `BufferedReader`, `StreamDecoder`, `Charset`,
  `CharsetDecoder` and the standard charsets, the heap buffers, `Scanner`, `Pattern` and
  `Matcher`, `DecimalFormat`, `DecimalFormatSymbols` and `Locale`. They are ordinary Java whose
  blockers are the object model and exceptions.
- **Hand-write at the bottom**: reading from descriptor 0 is done (`fd.rs`); add the trivial
  `initIDs`, `getHandle` and `getAppend`. Make `System.in` the object the JVM makes (below).
- **Make `System.in` real**: when reachable code reads `System.in`, reachability should also reach
  `FileDescriptor.in` and the constructors `FileInputStream(FileDescriptor)` and
  `BufferedInputStream(InputStream)`, and the runtime's `System.in` should be that translated
  object. Without it, the translated `Scanner` has nothing to read from. The same holds for
  `System.out` and `System.err` once a program passes them as a `PrintStream` value.
- **Prune or replace**: locale data (pin the default locale to the one the runtime formats in,
  `en-US`, and give `LocaleProviderAdapter` the bundles of that locale directly instead of loading
  them by name), class and resource loading (`ResourceBundle` loading, `ServiceLoader`,
  `jdk.internal.loader`, modules: untranslated, like reflection), `Charset`'s extended providers
  (none exist in java.base; `lookupViaProviders` returns null), direct and mapped buffers, and the
  `SharedSecrets` accessors (4.2).

## 3. Blockers by feature

Over the 28 targets and 3 projects, counting each distinct piece of code once (a JDK method reached
by many programs is one piece). Pieces are methods, field initializers and initializer blocks;
"only" is the number of pieces that the feature alone blocks.

The 26 targets other than the two `Scanner` programs reach 2,752 distinct library pieces, 175 of
them free:

| Feature | Library pieces | Only blocker | Classes | Programs |
|---|---|---|---|---|
| interfaces | 1,900 | 117 | 238 | 26 |
| inheritance | 1,712 | 99 | 230 | 26 |
| generic classes | 1,693 | 6 | 201 | 26 |
| generic types | 1,350 | 49 | 280 | 26 |
| `Object` values | 413 | 42 | 118 | 26 |
| exceptions | 315 | 7 | 130 | 26 |
| inner classes | 222 | 0 | 43 | 26 |
| reference casts | 218 | 0 | 89 | 26 |
| generic methods | 201 | 0 | 64 | 26 |
| instanceof | 91 | 0 | 57 | 26 |
| super calls | 54 | 0 | 29 | 23 |
| lambdas | 44 | 0 | 24 | 4 |
| anonymous classes | 32 | 0 | 17 | 21 |
| patterns | 28 | 0 | 20 | 26 |
| method references | 25 | 0 | 16 | 1 |
| for-each over collections | 22 | 0 | 16 | 26 |
| reflection | 21 | 0 | 15 | 13 |
| local classes | 12 | 0 | 7 | 1 |
| records, serialization, security, classes outside the sources | 1 to 2 each | 0 | | |

Their own code (428 distinct pieces, 99 free) is blocked mostly by generic types (134), inheritance
(119), interfaces (87), exceptions (53), generic classes (41), for-each over collections (32),
lambdas (16) and records (15).

With the two `Scanner` programs the library has 13,808 distinct pieces, 2,255 free, in the same
order at five times the size (interfaces 6,880, inheritance 6,821, generic types 3,517, generic
classes 2,995, exceptions 1,614, `Object` values 1,466), and reflection (101 pieces), security (20)
and lambdas (132) grow most.

The order in which supporting features frees the most code (over the 26, program and library
together): inheritance (+177 pieces, 14.2%), interfaces (24.2%), generic classes (39.1%), generic
types (60.1%), exceptions (67.0%), `Object` values (74.7%), inner classes (81.1%), generic methods
(85.2%), reference casts (89.2%), instanceof (90.9%), super calls (92.4%), lambdas (93.6%),
for-each over collections (94.9%), patterns (96.1%), method references, anonymous classes,
reflection, records, local classes.

**What the object model and exceptions remove.** Counting inheritance, interfaces, `Object` values,
reference casts, generic classes, types and methods, instanceof, patterns and super calls as the
object model, it and exceptions together free 2,743 of the 3,180 distinct pieces of the 26 (86.3%;
91.3% of 14,257 with `Scanner`). What then remains, by pieces it blocks (and blocks alone):

| Feature left | Pieces (26 targets) | Alone | Pieces (with Scanner) | Alone |
|---|---|---|---|---|
| inner classes | 230 | 228 | 587 | 571 |
| lambdas | 60 | 41 | 149 | 117 |
| for-each over collections | 54 | 39 | 141 | 108 |
| anonymous classes | 41 | 24 | 130 | 102 |
| method references | 34 | 17 | 82 | 57 |
| reflection | 21 | 18 | 101 | 91 |
| records | 17 | 17 | | |
| local classes, enum constants with bodies | 13, 10 | 4, 5 | | |

So the next language features by payoff are: inner classes (the iterators, entry sets and views
of every collection: `ArrayList.Itr`, `HashMap.KeySet`, `AbstractList.ListItr`), then lambdas and
for-each over collections together (most of `for-each` is `Iterable` dispatch, which the object
model gives, plus the iterator protocol), then anonymous classes (mostly `Comparator`s and the
`SharedSecrets` accessors, which should be pruned rather than translated), then method references.

## 4. Recommendations

### 4.1 Natives to hand-write first

Without `Scanner` and parallel streams, the 25 other targets (and the three projects, which reach
none) reach exactly eight natives. None of them depends on threads. Four are static, and two are
instance methods of the stateless `Unsafe` singleton that the lowering can call like static ones
(as it does for `Runtime.getRuntime()`): these six need nothing from the object model. The last two
are instance methods of exceptions, trivial once `Throwable` objects exist. In order:

1. **`jdk.internal.misc.Unsafe.registerNatives()`** (22 programs)
   - Java: `private static native void registerNatives();`, called from `Unsafe.<clinit>` through
     `runtimeSetup()`.
   - Behaviour: registers the JNI functions of `Unsafe`; no result, no exception, nothing visible.
   - Rust: nothing. The lowering drops the call (`Native::Nothing`, or no builtin at all).
   - **Done**: every `static native void registerNatives()` and `initIDs()` is `Native::Nothing`,
     whose statement the lowering drops.
2. **`Unsafe.arrayBaseOffset0(Class<?> arrayClass)`** (22 programs)
   - Java: `private native int arrayBaseOffset0(Class<?> arrayClass);`, called by
     `public long arrayBaseOffset(Class<?>)`, which throws `NullPointerException` for null first.
     `Unsafe.<clinit>` calls it for `boolean[]`, `byte[]`, `short[]`, `char[]`, `int[]`, `long[]`,
     `float[]`, `double[]` and `Object[]`.
   - Behaviour: the byte offset of element 0 from the start of an array object. HotSpot on 64-bit
     with compressed class pointers (the default) answers 16 for every array class. Not defined
     for a class that is not an array (HotSpot asserts); no reachable code passes one.
   - Rust: `java::unsafe_array_base_offset() -> i32 { 16 }`; the argument is a class literal the
     lowering resolves (`int[].class`), so no `Class` object is needed.
3. **`Unsafe.arrayIndexScale0(Class<?> arrayClass)`** (22 programs)
   - Java: `private native int arrayIndexScale0(Class<?> arrayClass);`, behind
     `public int arrayIndexScale(Class<?>)` (null check first).
   - Behaviour: the size of one element: 1 for `boolean[]` and `byte[]`, 2 for `short[]` and
     `char[]`, 4 for `int[]` and `float[]`, 8 for `long[]` and `double[]`, and 4 for arrays of
     references (compressed oops, HotSpot's default below 32 GB of heap). `ArraysSupport.<clinit>`
     takes `exactLog2` of each and throws `Error("Data type scale not a power of two")` otherwise,
     so the values must be powers of two.
   - Rust: `java::unsafe_array_index_scale(element: ElementKind) -> i32` with those values. Like
     the base offset, it only feeds `Unsafe` address arithmetic, which the runtime interprets
     itself (4.3), so these are the safest consistent values rather than anything observable.
   - **Done** for 2 and 3, one level up: the class literal is at the call of the public
     `arrayBaseOffset(Class)` and `arrayIndexScale(Class)` (`theUnsafe.arrayBaseOffset(int[].class)`),
     and those methods take a `Class`, which the translation has no type for. So reachability does
     not enter them (`REPLACED_METHODS` in `reach.rs`; the two natives are then not reached at all),
     and the lowering turns such a call into `java::array_base_offset()` (16, a `long`) or
     `java::array_index_scale("int")` (`runtime/src/vm.rs`) before it lowers the argument. The null
     check cannot fail for a literal, and `Unsafe` has no state, so a receiver that is a name or
     `Unsafe.getUnsafe()` is not evaluated. Any other argument, such as the local
     `ForkJoinPool.<clinit>` passes, and a class that is not an array are rejected by name.
4. **`jdk.internal.misc.CDS.getCDSConfigStatus()`** (21 programs)
   - Java: `private static native int getCDSConfigStatus();`, read once into
     `CDS.configStatus` by `CDS.<clinit>`.
   - Behaviour: bit flags of class data sharing (dumping an archive, using one, logging lambda
     form invokers). No archive exists for a translated program.
   - Rust: `0`. Then `isDumpingArchive()`, `isUsingArchive()` and the rest are false, as on a JVM
     run with `-Xshare:off`.
5. **`CDS.getRandomSeedForDumping()`** (21 programs)
   - Java: `public static native long getRandomSeedForDumping();`.
   - Behaviour: a seed derived from the JDK build while dumping an archive, and `0` otherwise.
   - Rust: `0`. `ImmutableCollections.<clinit>` then seeds `SALT32L` and `REVERSE` from
     `System.nanoTime()`, exactly as a JVM does: the iteration order of `Set.of(...)` and
     `Map.of(...)` differs from run to run, in Java and in the translation alike.
6. **`CDS.initializeFromArchive(Class<?> c)`** (21 programs)
   - Java: `public static native void initializeFromArchive(Class<?> c);`, called from the static
     initializers of `ImmutableCollections`, `BaseLocale`, `Integer.IntegerCache` and others.
   - Behaviour: with an archive, sets the class's archived static fields (such as
     `ImmutableCollections.archivedObjects`); without one, does nothing, and the caller builds the
     values itself (`if (archivedObjects == null) { ... }`).
   - Rust: nothing. The argument is a class literal, so the call can be dropped. Since nothing
     sets the archived fields, a null test of one right after the call is always true: the fields
     whose class tests them directly (`ImmutableCollections.archivedObjects`,
     `Attributes.Name.KNOWN_NAMES`, `Configuration.EMPTY_CONFIGURATION`) are listed in
     `program::reach::ARCHIVED_FIELDS`, and reachability, the survey and the lowering keep only
     the branch that builds the values (`live_branch`). Without it, `ImmutableCollections`'s
     sentinel `EMPTY` also took `archivedObjects[0]`, which could be any `Object`, so
     `e1 != EMPTY` in `List12` compared two values that might be strings and was rejected. The
     classes that copy the field into a local first (`FDBigInteger`, `BaseLocale`) are not
     covered; the runtime's boxes have no source to translate.
   - **Done** for 4 to 6: `java::cds_config_status()` and `java::cds_random_seed_for_dumping()`
     (both 0), and `initializeFromArchive(C.class)` is dropped before its argument is lowered;
     another argument than a class literal is rejected.
7. **`java.lang.Throwable.fillInStackTrace(int dummy)`** (28 programs)
   - Java: `private native Throwable fillInStackTrace(int dummy);`, called by the public
     `fillInStackTrace()` when the stack trace is writable (`stackTrace != null`), which every
     constructor calls (and overriders such as `NullPointerException.fillInStackTrace` call
     through `super`).
   - Behaviour: records the current thread's frames in the private fields `backtrace` and `depth`
     and returns `this`; never throws.
   - Rust: return the receiver and record nothing. `backtrace` stays null, so
     `getOurStackTrace()` returns the empty `UNASSIGNED_STACK`: `getStackTrace()` is an empty
     array and `printStackTrace()` prints only the `java.lang.X: message` line, without the
     `\tat ...` lines. That changes only what goes to standard error, and it keeps
     `StackTraceElement.initStackTraceElements` and the stack walker unreachable in practice.
8. **`java.lang.NullPointerException.getExtendedNPEMessage()`** (25 programs)
   - Java: `private native String getExtendedNPEMessage();`, called by `getMessage()` when no
     message was given, and by `fillInStackTrace()` on a second fill.
   - Behaviour: the helpful message the JVM derives from the bytecode that dereferenced null
     (`Cannot invoke "String.length()" because "<local1>" is null`), and `null` for an NPE that
     code created with `new` or that has no such bytecode.
   - Rust: `null`. Every NPE that translated JDK code creates is created with `new`, so `null` is
     what Java gives there. Where the translated program itself dereferences null
     (`java::non_null`), Java's text depends on the bytecode and on whether local variable names
     were compiled in; the translator could pass that text at the throw site later, but a program
     that prints the message of an implicit NPE is rare.

7 and 8 are not done, and since exceptions landed they are not reached either: the translator
provides `Throwable`, `NullPointerException` and the other standard exception classes from its own
source (`java/Standard.java`, without stack traces), and reachability stops at them as at the
runtime's classes. They are needed only if the translation takes the JDK's `Throwable` instead,
for stack traces. What each needs then:

- **`fillInStackTrace(int)`** is called on `this` inside `Throwable.fillInStackTrace()`, a
  `synchronized` instance method that every `Throwable` constructor calls when `stackTrace` is not
  null. The lowering needs the receiver as a `Throwable` object (the translated `Throwable` class,
  or the runtime's base if `Throwable` becomes a runtime class) and the call's type, `Throwable`,
  as a reference to it: the native is the receiver itself, unchanged, so it becomes the receiver
  expression (as `Identity` does for strings), with `backtrace` and `depth` left at null and 0.
  `synchronized` on the receiver must lower (as monitors do on one thread), and the constructors
  must run `fillInStackTrace()` through dynamic dispatch, since `NullPointerException` overrides
  it. Nothing in the runtime: no function is needed.
- **`getExtendedNPEMessage()`** is called on `this` in `NullPointerException.getMessage()` and
  `fillInStackTrace()`. It needs the receiver as a `NullPointerException` and a `String` result
  that may be null (the translation's `Option<String>`), and it is `None` for every receiver: the
  lowering can make it a null literal of type `String` once the receiver, which has no effects,
  need not be evaluated. If NPEs raised by the translation itself (`java::non_null`) should later
  carry Java's helpful message, the message would come from the throw site, not from this native.

Next, for the programs beyond the typical ones, still independent of threads:

9. `jdk.internal.misc.VM.initialize()` (`private static native void initialize();`, from
   `VM.<clinit>`): nothing.
10. `jdk.internal.misc.ScopedMemoryAccess.registerNatives()` and
    `java.nio.MappedMemoryUtils.registerNatives()`: nothing.
11. `jdk.internal.vm.ContinuationSupport.isSupported0()` (`private static native boolean`):
    `false`, so virtual threads are not supported and `Continuation` stays unused.
12. `java.io.FileInputStream.initIDs()`, `java.io.FileDescriptor.initIDs()`: nothing.
    `FileDescriptor.getHandle(int fd)` (`private static native long`): `-1` (a Windows handle;
    not used elsewhere). `FileDescriptor.getAppend(int fd)` (`private static native boolean`):
    whether the descriptor was opened in append mode, `false` for 0, 1 and 2; the runtime's
    descriptor table knows it for files.
13. `String.intern()` (`public native String intern();`, a runtime class native): the string
    itself. Strings are values in the runtime, so a canonical copy has nothing to add; `==` on
    strings is decided elsewhere.
14. `jdk.internal.misc.VM.getNanoTimeAdjustment(long offsetInSeconds)` (`public static native
    long`): the current time in nanoseconds relative to `offsetInSeconds` seconds after the epoch,
    that is `(now - offset) * 10^9 + nanos`, or `-1` when the offset is more than 2^32 seconds
    away from now. `std::time::SystemTime::now().duration_since(UNIX_EPOCH)`.
15. `java.lang.reflect.Array.newArray(Class<?> componentType, int length)` (behind
    `Array.newInstance`, 5 programs through `Arrays.copyOf` and `toArray(T[])`): an array of
    `length` nulls (or zeros) of that component type; `NegativeArraySizeException` for a negative
    length, `IllegalArgumentException` for `void`. Needs arrays of objects; reachable once
    `java.lang.reflect.Array` is let through the reflection boundary (4.2).

9 to 14 are **done** (4.4 has the details), and 15 is reached now that `java.lang.reflect.Array`
is translated, but stays a rejection until arrays of objects and `Class` values exist.

Everything else in section 1 is either the threads design (the `Unsafe` atomics, `park`,
`VirtualThread`, `Continuation`, `NativeThread`), the class model (`Object.getClass`, `Class`), or
an artifact that the boundary changes below remove.

### 4.2 Boundary changes in reach.rs

Made in this branch:

- **Code guarded by Java Flight Recorder's flags is not reached** (`FALSE_FLAGS`, `never_true`).
  `Throwable`, `FileInputStream`, `FileOutputStream`, `RandomAccessFile` and `FileChannelImpl`
  have a static `jfrTracing` flag that only `jdk.internal.event.JFRTracing.enable()` sets, by
  reflection when JFR starts. Reachability, the survey and the lowering skip the branch of an `if`
  whose condition reads one of these flags (alone or under `&&`), and lower such an `if` to its
  `else` branch. Every program that creates an exception reached `ThrowableTracer`, its
  `AtomicLong` and through it `Unsafe.compareAndSetLong`, `getLongVolatile` and
  `knownObjectFieldOffset0`; now the 25 typical targets reach 1 to 8 natives instead of 8 to 11,
  and `CheckedExceptions` 1 instead of 8. A unit test in `reach.rs` covers it.

Recommended, in order of payoff:

1. **Follow `System.in`** (and `System.out`/`System.err` as values): a read of the runtime's
   `System.in` reaches `FileDescriptor.in`, `FileInputStream(FileDescriptor)` and
   `BufferedInputStream(InputStream)`, the objects the JVM builds in `System.initPhase1`, and the
   runtime makes `System.in` that object. Without it no input path reaches the descriptor.
2. **Class and resource loading as untranslated** ("class loading": `java.lang.Module`,
   `java.lang.ModuleLayer`, `java.util.ServiceLoader`, the loading half of `ResourceBundle`,
   `jdk.internal.loader.`), with the locale data of the runtime's locale given directly. Measured:
   `Scanner` goes from 1,647 to 1,199 classes and from 191 to 77 natives. Class loading by name is
   reflection in all but name: a closed-world translation knows its classes. **Made** (4.5):
   1,649 to 1,193 classes and 191 to 76 natives.
3. **Anonymous and local classes are reached by dispatch, not scanned with the code that declares
   them.** Today an anonymous class's methods are reachable as soon as the method or initializer
   that creates it is. 27 files of java.base register a `SharedSecrets` accessor as an anonymous
   class in a static initializer (`Buffer`, `FileDescriptor`, `System`, ...), so initializing
   `Buffer` reaches every method of `JavaNioAccess`: mapped memory, `writeback`, continuations.
   This needs javac's trees to name anonymous classes (their qualified name is empty today, so
   their methods' keys collide), after which their methods are overriders like any other.
   **Made** (4.5).
4. **Sequential streams do not reach the parallel paths.** `AbstractPipeline.evaluate` and the
   `opEvaluateParallel*` methods run only when `isParallel()` is true, which needs a reachable
   call of `BaseStream.parallel()`, `Collection.parallelStream()` or a parallel
   `StreamSupport.stream`. When none is reachable, the parallel branches are dead, as with the JFR
   flags. This removes `ForkJoinPool`, the parallel `ConcurrentHashMap` of `distinct()` and with
   them up to 23 of `StreamPipeline`'s 31 natives (all but the eight of 4.1). **Made** (4.5),
   with a check that the assumption holds; the natives stay until 3 is made, since the parallel
   paths are also reached through anonymous classes.
5. **JVM-injected constants.** `jdk.internal.misc.UnsafeConstants` has static finals that its own
   initializer sets to 0 and false and the JVM then overwrites (`ADDRESS_SIZE0` 8, `PAGE_SIZE`,
   `BIG_ENDIAN`, `UNALIGNED_ACCESS`, `DATA_CACHE_LINE_FLUSH_SIZE`). Translated as written they
   silently stay 0. The lowering should substitute the platform's values (8, 4096,
   `cfg!(target_endian = "big")`, true on x86-64 and AArch64, 0).
6. **`Unsafe` into `RUNTIME_CLASSES`**, once the threads design gives it symbolic field offsets:
   its Java code (`getAndAddInt` loops over `weakCompareAndSetInt` and the like) is then the
   runtime's, and the 43 `Unsafe` natives are runtime functions.
7. **`java.lang.reflect.Array` through the reflection boundary**: `Array.newInstance` and
   `getLength` are what `Arrays.copyOf` and `toArray` need; they are array natives, not reflection.
   **Made** (4.5).

### 4.3 Surprises

- **`Scanner` is twenty times the size of `BufferedReader`** (1,647 against 413 classes), all
  because `useLocale` loads locale data through `ResourceBundle`, and class loading then makes
  jars, URLs, sockets and the module system live. For the translation, `BufferedReader` is the
  cheap path to standard input and `Scanner` waits for the class-loading boundary.
- **`System.in` reaches nothing.** The runtime's `System` hides what `System.in` is, so a program
  reading standard input reaches no input native at all until the boundary follows it.
- **Exceptions reached Flight Recorder.** Every `Throwable` constructor has
  `if (jfrTracing) ThrowableTracer.traceThrowable(...)`, which made 3 of the 8 to 11 natives of
  every program `Unsafe` atomics; fixed here.
- **The dispatch of `Object`'s methods spreads the reach**, not the rule that adds them: removing
  the rule changed no count, because library code itself calls `hashCode`, `equals` and
  `toString` on `Object`-typed values and the class-hierarchy dispatch then reaches every live
  class (`Hashtable.computeIfAbsent -> URL.hashCode -> InetAddress`). Only a type-flow or points-to
  analysis (thorough mode's first analysis) narrows that.
- **`Set.of` and `Map.of` iterate in a different order on every run**, in Java too: the seed is
  `System.nanoTime()` unless a CDS archive is being dumped. Their output cannot be matched against
  one recorded run.
- **`Properties` is a `ConcurrentHashMap`**, so reading a system property through
  `StaticProperty` (which `Charset.defaultCharset` does) reaches `Unsafe` compare-and-set and
  `LockSupport.park`.
- **The JDK sources are JDK 26**: `Charset` keeps its default in a `LazyConstant`
  (`jdk.internal.lang.LazyConstantImpl`), whose `get` is `Unsafe.getReferenceAcquire` and
  `putReferenceRelease`; standard input therefore needs the `Unsafe` reference operations even
  in a single-threaded program.

### 4.4 What is implemented

A native method's declaration is never lowered (it has no body). javac's symbols now say whether
a method is `native`, and a call of a native of a translated class becomes the runtime function
that `ast::declared_native` names for its class, name and signature, or is rejected with
`native method `Class.name(signature)` not supported yet`. The survey marks the natives the
runtime implements (`(in the runtime)`, and `runtime` in the JSON) and counts them in its first
line. Tests: the runtime functions in `runtime/src/vm.rs` and `fd.rs`; the mapping in
`ast/natives.rs`; whole programs over stand-ins for `CDS`, `Unsafe`, `VM` and
`ContinuationSupport` in `tests/translation.rs` (`natives_of_jdk_classes_become_runtime_calls`,
`natives_the_runtime_lacks_are_rejected_by_name`). The classes of `java.io` and `java.util.zip`
cannot be stood in for there, since javac reads an exported package of java.base from the JDK.

| Java | Rust | Behaviour |
|---|---|---|
| `static native void registerNatives()`, `initIDs()` of any class | nothing (statement dropped) | JNI registration and field lookups |
| `VM.initialize()` | nothing | registers `getNanoTimeAdjustment` |
| `CDS.getCDSConfigStatus()int` | `java::cds_config_status()` | 0: no archive, as with `-Xshare:off` |
| `CDS.getRandomSeedForDumping()long` | `java::cds_random_seed_for_dumping()` | 0: not dumping |
| `CDS.initializeFromArchive(Class)` | nothing, for a class literal | no archive to restore from |
| `Unsafe.arrayBaseOffset(Class)long` of `T[].class` | `java::array_base_offset()` | 16 |
| `Unsafe.arrayIndexScale(Class)int` of `T[].class` | `java::array_index_scale("T")` | 1, 2, 4, 8 by element; 4 for references |
| `ContinuationSupport.isSupported0()boolean` | `java::continuations_supported()` | `false`, as with `-XX:-VMContinuations` (HotSpot's default is `true`; without continuations, virtual threads are bound to a platform thread each) |
| `VM.getNanoTimeAdjustment(long)long` | `java::nano_time_adjustment(offset)` | `(now - offset) * 10^9 + nanos`; -1 when `now - offset` is 2^32 seconds or more either way (HotSpot's `JVM_GetNanoTimeAdjustment`) |
| `FileDescriptor.getHandle(int)long` | `java::fd_handle(fd)` | -1 (Windows handles only) |
| `FileDescriptor.getAppend(int)boolean` | `java::fd_append(fd)` | the descriptor's `O_APPEND`, as `fcntl(fd, F_GETFL)` tells (from `/proc/self/fdinfo`; `false` where that is missing); `true` for a descriptor that is not open, since Java reads `fcntl`'s -1 as all flags |
| `CRC32.update(int,int)int`, `updateBytes0(int,byte[],int,int)int` | `java::crc32_update`, `crc32_update_bytes` | zlib's CRC-32 |
| `String.intern()` | the string itself (`StringOp::Identity`, there before) | strings are values; `==` and `!=` on two strings stay rejected at translation (`== on strings compares references`), so no program can see that copies are not canonical |

Not done, because they need objects, exceptions or threads: everything else of section 1, in
particular `Throwable.fillInStackTrace(int)` and `NullPointerException.getExtendedNPEMessage()`
(above), the `Unsafe` memory and atomic operations, `Array.newArray` and the `FileInputStream`
natives of a stream's object (`length0`, `position0`, `isRegularFile0`). The `TimeZone` natives
need no objects but read the system's zone configuration (`TZ`, `/etc/localtime`), which is left
for when a program needs time zones.

### 4.5 Boundary changes made, and their numbers

- **Class and resource loading untranslated** (4.2, 2): `java.lang.Module` (and so
  `ModuleLayer`), `java.util.ResourceBundle`, `java.util.ServiceLoader` and `jdk.internal.loader.`
  are `UNTRANSLATED` as "class loading". All of `ResourceBundle` is cut, not only its loading
  half, so a translated `Scanner` still needs the locale data of the runtime's locale given
  directly before it can run.
- **`java.lang.reflect.Array` translated** (4.2, 7): `TRANSLATED_ANYWAY` exempts it from the
  reflection prefix; its natives (`newArray`) now show among the natives reached.
- **Sequential streams** (4.2, 4): reachability assumes first that no stream is parallel and skips
  what `AbstractPipeline.isParallel()` guards, in `if` statements and `?:` expressions, then checks
  the assumption on the code so reached (`program/reach/sequential.rs`): every assignment of
  `AbstractPipeline.parallel` must assign a value that cannot be true, following `boolean`
  parameters of constructors and static or private methods through every reachable call.
  `BaseStream.parallel()` and `Collection.parallelStream()` fail the check, and then the walk is
  made again without the assumption. On `StreamPipeline` the check holds, but the gain is small
  (392 to 388 classes, 1,990 to 1,969 methods, no native): the parallel paths are still reached
  through anonymous classes, such as the `StatefulOp` of `SliceOps.makeRef` and the `ReduceOp` of
  `ReduceOps.makeRefCounting`, whose `opEvaluateParallel` and `evaluateParallel` overrides are
  scanned with the method that creates them. The 23 natives come off once anonymous classes are
  reached by dispatch (4.2, 3).
- **Local and anonymous classes reached by dispatch** (4.2, 3): javac's trees name them by their
  binary names (`DistinctOps$1`), reachability declares them, and `Walk::scan` no longer scans
  them with the code that creates them: a constructor is reached by its `new`, an instance
  method by a call or by dispatch once its class is instantiated. The check of the sequential
  assumption indexes them the same way.
- **Nodes without children** (with 3): one more path reached `ForkJoinPool` from sequential
  code, `Nodes.flatten`, which copies a tree of nodes into an array with a `ForkJoinTask` when
  `node.getChildCount() > 0`. Only parallel evaluation concatenates nodes (`Nodes.conc`), so the
  sequential assumption covers that test too, checked by no class that overrides
  `Node.getChildCount()` having objects.
- **Flags of the streams' `Tripwire`** (`java.util.Tripwire`, `java.util.stream.Tripwire`): its
  `ENABLED` is `Boolean.getBoolean` of a property that only `-D` sets, so it joins the JFR flags in
  `FALSE_FLAGS`, and the condition of a skipped branch is no longer scanned (the lowering drops
  it), so the class is not reached for its flag.

With these, `StreamPipeline` reaches 264 classes (176 instantiated), 1,144 methods and 4 natives,
all in the runtime (the `CDS` ones and `Unsafe.registerNatives`), against the 372 classes and 28
natives above. `ForkJoinPool`, `ConcurrentHashMap`, `Thread` and the `Unsafe` atomics are gone.

Survey numbers on main before this work and after it (natives, then those in the runtime):

| Program | Classes | Methods | Natives before | Natives after (in the runtime) |
|---|---|---|---|---|
| `GenericContainers` | 40 to 40 | 136 to 136 | 1 | 1 (0) |
| `ListOperations` | 87 to 87 | 470 to 466 | 8 | 6 (4) |
| `ComparatorSorting` | 90 to 90 | 408 to 404 | 8 | 6 (4) |
| `StreamPipeline` | 390 to 388 | 1,992 to 1,969 | 31 | 30 (5) |
| `ScannerGrades` | 1,649 to 1,193 | 10,704 to 7,441 | 191 | 76 (22) |
| `TextAdventure` | 1,654 to 1,199 | 10,716 to 7,466 | 191 | 76 (22) |

The typical targets lose `arrayBaseOffset0` and `arrayIndexScale0`, and of the six natives left
four are in the runtime; the other two are the exception natives of 7 and 8. `StreamPipeline`
gains `Array.newArray` (through `toArray`). The `Scanner` programs lose 456 classes and 115
natives to the class loading boundary; of their 54 natives left that the runtime lacks, 35 are
`Unsafe` (atomics, memory, `writeback`), and the rest the stack walker and stack traces, mapped
memory, continuations and virtual threads, `TimeZone`, `FileCleanable`, `UnixFileSystem` and
`Array.newArray`.

Merged with the standard exception classes, which take the exception natives away: `ListOperations`
and `ComparatorSorting` reach 4 natives, all in the runtime, and `GenericContainers` none;
`StreamPipeline` reaches 372 classes and 28 natives (5 in the runtime); `ScannerGrades` and
`TextAdventure` reach 1,163 and 1,169 classes and 73 natives each (22 in the runtime), having lost
`StackTraceElement.initStackTraceElements` too.

## Reproducing

Build the translator, put each program in a directory of its own, and run the survey with the
java.base source root as the library (`--survey` for the text report, `--survey-json FILE` for the
JSON). The runs take one to five minutes each and about 1.7 GB of memory, most of it javac's tree
as JSON. The scripts used for the tables (aggregation over the JSON files) are not part of the
repository; the JSON fields they read are `natives[].chain`, `external[].chain`,
`instantiated[].chain`, `runtime_calls` and `pieces`.
