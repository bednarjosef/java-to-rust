# Changing JDK classes: patches, overlay classes and redirects

A whole-program translation reads the JDK's own sources (`--library` with the sources of
`java.base`) and translates the parts of them that the program reaches. Some of that code cannot
be translated as it is: it maps files into memory, asks the JVM for things a translated program
does not have, or uses a class the translator does not support yet. The translator replaces those
pieces with code of its own. This document says how, and what may be in the repository.

## The rule: only our own code

The repository holds only code its authors wrote. The JDK's sources are GPLv2 with the Classpath
exception and stay outside the repository: they are given to the translator when it runs
(`--library $JAVA_BASE_SOURCES`), never copied into it. So when the translator needs a JDK class
to behave differently, the repository holds only the difference, written by us, and the
translator puts it together with the JDK's file at translation time. A file generated from the
JDK's sources is a copy too: a generator is fine only if it runs at translation or build time and
writes outside the repository.

`scripts/check-jdk-copies.py` checks this, and `scripts/check.sh` runs it when
`JAVA_BASE_SOURCES` is set (see [Checks](#checks)).

## Three ways to change what the JDK does

From the lightest to the heaviest:

1. **A redirect.** Every call of one JDK method becomes a call of a static method of ours with the
   same parameters, and the JDK's method is not translated at all. The list is
   `REDIRECTED_CALLS` in `program/reach.rs` (and `TRANSLATED_LANG_ACCESS` for the methods of
   `JavaLangAccess`); the methods are in classes of our own, such as
   `java/overlay/sun/nio/fs/TranslatedFiles.java`. Use it when one JDK method should behave
   differently for every caller: `Files.readAllBytes`, `AccessController.doPrivileged`,
   `Runtime.version`.
2. **A patch.** The JDK's file of a class, with some of its members replaced, added, removed or
   edited. The patch is a file in `crates/translator/java/patches/`, at the path of the class it
   patches (`java/io/PrintStream.java`), that holds only what differs. Use it when the change is
   inside a class: one call in one method, a field that should not be initialized, a method
   whose body must be different.
3. **An overlay class.** A whole class of ours that the JDK does not have, in
   `crates/translator/java/overlay/`: the targets of redirects (`TranslatedFiles`,
   `TranslatedNet`, `TranslatedCoders`, `TranslatedClasses`, `TranslatedNumbers`), the objects
   behind `System.out` and `System.in` (`StandardOutput`), the object behind
   `String.CASE_INSENSITIVE_ORDER` and the stream of `String.codePoints()` (`TranslatedStrings`),
   and classes that patches use (`sun/nio/fs/UnixFileChannel.java`). The overlay directory is on
   javac's source path before the JDK's sources. It holds no file at a path the JDK has: a
   change to a JDK class is a patch.

## Patches

### How a patch is applied

The translator runs javac in a JVM of its own (`java/JavaTree.java`). With libraries, it passes
`--patches crates/translator/java/patches`, and `java/JdkPatch.java` sits between javac and the
files: when javac reads a source file of the source path, such as the JDK's
`java/io/PrintStream.java`, and a patch of that path exists, javac gets the JDK's text with the
patch applied. Nothing is written to disk, and the JDK's sources are only read. The patched unit
carries the JDK file's name, so messages about it name the JDK's file.

`JdkPatch` reads both files with javac's parser, finds the members the patch names in the JDK's
class, and changes the JDK's text in those places only. Everything else, comments and
formatting included, stays the JDK's.

### What a patch looks like

```java
package jdk.internal.misc;

import java.util.ArrayList;

/**
 * Why the class is patched (this comment is ours, for the reader of the patch).
 */
class TerminatingThreadLocal {
    @Edit(find = "REGISTRY.get().add(tl);", replace = """
            Collection<TerminatingThreadLocal<?>> registered = REGISTRY.get();
            if (!registered.contains(tl)) {
                registered.add(tl);
            }
            """)
    public static void register(TerminatingThreadLocal<?> tl);

    @Edit(find = "Collections.newSetFromMap(new IdentityHashMap<>(4))", replace = "new ArrayList<>(4)")
    public static final TerminatingThreadLocal<Collection<TerminatingThreadLocal<?>>> REGISTRY;
}
```

- The **package** must be the JDK file's. The **imports** are added to the JDK file's.
- The **class** is named as in the JDK's file; its modifiers, type parameters and supertypes are
  not read (write it as `class Name {`). To change the class's own line, use `@Edit` on the
  class (below).
- Each **member** of the patch says what it does with one marker annotation. A member is matched
  with the JDK's by its name and, for a method or constructor, its parameter types as written
  (`List<String>`, not `List`; `String...` and `String[]` are the same). A field is matched by its
  name, a nested class by its name.

| marker | what it does | the patch's member |
|---|---|---|
| `@Replace` | the JDK's member becomes this one | written in full |
| `@Add` | this member is added at the end of the class; the JDK's class must not have it | written in full |
| `@Remove` | the JDK's member is left out | declared without a body: `@Remove private void readObject(ObjectInputStream s);` |
| `@Edit(find = "...", replace = "...")` | inside the JDK's member, the text `find` becomes `replace`; `find` must be there exactly once | declared without a body |
| `@Edit(from = "...", to = "...", replace = "...")` | inside the JDK's member, the text from `from` (there exactly once) through the first `to` after it becomes `replace` | declared without a body |
| `@Patch` | patches the members of a nested class the same way | a nested class with marked members |

- `@Edit` may be given several times on one member. On the patch's **class**, it edits the whole
  class: its own line (`@Edit(find = "extends SecureClassLoader", replace = "extends
  ClassLoader")`), a field's modifiers, a static initializer. In `find`, `from` and `to`, a run of
  spaces or line breaks matches any run of whitespace. A multi-line `replace` is best written as
  a text block (`"""`); its lines are indented like the line it goes into.
- An **initializer block** has no name: a `static { ... }` block in the patch replaces the JDK
  class's only static initializer, and an empty one (`static {}`) removes it. The same goes for
  an instance initializer. A class with several of that kind is refused.
- Comments in the patch are for its reader; say for each change why it is there. `@Override`,
  `@SuppressWarnings` and other real annotations of a `@Replace`d or `@Add`ed member are kept.

### Refused loudly

A patch is checked against the JDK file it is applied to, every time. It is refused, and the
translation stops with the reason, when:

- it names a member the JDK's class does not have (`@Replace`, `@Remove`, `@Edit`, `@Patch`), or
  adds one it has (`@Add`), or patches one twice;
- the text of an `@Edit` is not in the member, or more than once;
- a member has no marker, or two edits overlap.

So when the JDK changes, a patch that no longer fits stops the translation instead of being
skipped. The error names the patch's file and line, and the JDK's file:

```
.../java/patches/sun/nio/fs/Util.java:20: the method toString(byte[]) (@Edit) is not in the class; it has toString(byte[],int) (patching .../sun/nio/fs/Util.java)
```

### Writing and debugging a patch

- Prefer the smallest change: an `@Edit` of the few lines that differ over a `@Replace` of a long
  method, and `@Remove` only for what must not be translated (a field whose initializer runs
  what a translated program does not have, a member that uses a removed one). A JDK method that
  nothing reaches costs nothing.
- To see what javac gets, set `JAVA_TO_RUST_PATCHED_SOURCES=DIR` for a translation: every patched
  file is written under `DIR` as javac read it. Line numbers in javac's messages about a patched
  class are those of that text.
- `cargo test --test jdk_patches` applies every patch to the JDK's sources when
  `JAVA_BASE_SOURCES` is set (and the JDK's other modules next to it), and checks the patch mechanism itself either way. By hand:
  `java -cp <build dir>/out JdkPatch crates/translator/java/patches $JAVA_BASE_SOURCES --out DIR`.
- A patch can patch any library's classes, not only the JDK's: it applies to the file of its path
  wherever javac's source path has it.

### The patches

| patch | what it changes |
|---|---|
| `java/io/PrintStream.java` | encodes text itself with `String.getBytes` instead of through a `StreamEncoder` |
| `java/io/InputStreamReader.java`, `OutputStreamWriter.java` | decode and encode the common charsets themselves instead of through `StreamDecoder` and `StreamEncoder` |
| `java/lang/Module.java`, `ModuleLayer.java` | the translation's modules, from its roots' declarations; no layers but the boot layer, and no dynamic modules: the module of a proxy class in one throws |
| `java/lang/Package.java` | a package of the translation, made by `Class.getPackage()`; the manifest attributes of an unnamed module's package and every package's annotations throw; not an `AnnotatedElement`; `getPackage(name)` and `getPackages()` throw, as the lowering answers or rejects their calls |
| `java/lang/reflect/Proxy.java` | defines and finds the proxy classes that the translator wrote through `TranslatedProxies` (`program/reach/proxies.rs`), without generating or caching classes; `InvocationHandler.invokeDefault` throws |
| `java/net/InetAddress.java`, `Inet4AddressImpl.java`, `Inet6AddressImpl.java` | names looked up by the system through `TranslatedNet`; no hosts file and no resolver of the program's own |
| `java/security/MessageDigest.java` | `getInstance(String)` makes the `SUN` provider's digests (MD2, MD5, SHA-1, SHA-2, SHA-3) by name through `TranslatedDigests`, without `Provider`, `Security` or `GetInstance`; `toString` names `SUN`, `getProvider()` is null |
| `java/security/SecureRandom.java` | `new SecureRandom()` and `getInstance(String)` make the `SUN` provider's generators (`NativePRNG` and its blocking and non-blocking variants, `SHA1PRNG`) by name through `TranslatedRandoms`, the default one as `SUN` chooses it; `getProvider()` is null. `DRBG`, which reads the security properties, throws a `ProviderException` that says it is not supported yet |
| `java/net/URI.java` | decodes escaped octets with `new String(bytes, UTF_8)` instead of a `CharsetDecoder` |
| `java/net/URLClassLoader.java` | finds only what its parent finds; code from its URLs fails loudly |
| `java/nio/file/DirectoryIteratorException.java`, `InvalidPathException.java` | constructors that do not throw |
| `java/nio/file/TempFileHelper.java` | names from `java.util.Random` instead of `SecureRandom` |
| `java/util/PropertyResourceBundle.java` | reads a stream through `TranslatedPropertyFiles` |
| `java/util/ServiceLoader.java` | the providers the translator read from the sources |
| `java/util/WeakHashMap.java` | matches a key by reading the referent, `k == key \|\| key.equals(k)`, instead of `refersTo` |
| `jdk/internal/foreign/Utils.java` | no method handles of address layouts; `makeRawSegmentViewVarHandle` throws |
| `jdk/internal/jimage/BasicImageReader.java` | reads the image's index and resources into arrays instead of mapping the file |
| `jdk/internal/jrtfs/JrtFileAttributes.java`, `JrtFileSystem.java`, `SystemImage.java` | the `jrt:` file system without `%tc`, `CodeSource` and exploded images |
| `jdk/internal/misc/UnsafeConstants.java` | the values the JVM injects: 8-byte little-endian addresses, unaligned access, 64-byte cache lines, the system's page size |
| `jdk/internal/misc/CarrierThreadLocal.java`, `TerminatingThreadLocal.java` | thread-locals without the JVM's carrier threads and identity maps |
| `jdk/internal/misc/VM.java` | `getRuntimeArguments` answers an empty array: a translated program has no JVM arguments |
| `jdk/internal/perf/Perf.java` | `createLong` returns the heap buffer that HotSpot returns with `-XX:-UsePerfData`: 8 bytes in native order holding the value |
| `jdk/internal/misc/InnocuousThread.java` | the thread group's parent read with `getParent`, not through an `Unsafe` offset held in a local; the `Thread` offsets stay, so `eraseThreadLocals` and the constructor write the thread's fields as the JDK does |
| `jdk/internal/util/ByteArray.java` | shifts bytes instead of using array views |
| `jdk/internal/util/Exceptions.java` | `%s` formatting of its own, and no wait for `VM.isBooted` |
| `jdk/internal/vm/ScopedValueContainer.java` | runs the operation directly |
| `javax/annotation/processing/AbstractProcessor.java` (module `java.compiler`) | reads its processor's annotations through `TranslatedProcessorAnnotations` |
| `sun/reflect/annotation/AnnotationParser.java`, `AnnotationInvocationHandler.java` | an annotation object is the handler itself, not a `Proxy`: `annotationType`, `toString`, `hashCode` and `equals` its own (`lower/annotation_objects.rs`) |
| `sun/reflect/generics/reflectiveObjects/ParameterizedTypeImpl.java`, `WildcardTypeImpl.java` | made from the types javac gives (`TranslatedTypes`, `lower/generic_types.rs`): the owner type as given, no check of the arguments, a wildcard's bounds as types rather than signatures |
| `sun/nio/cs/UTF_8.java` and the other standard charsets | their nested `Decoder` and `Encoder` can be made by `TranslatedCoders`, the coders of a charset that is the runtime's value |
| `sun/nio/fs/*.java` | system calls through the runtime (`UnixNativeDispatcher`), channels and streams of our own |
| `sun/security/provider/SecureRandom.java` | `SHA1PRNG` asks for the SHA-1 digest by its name alone, not by the `SUN` provider's name |
| `sun/security/provider/SeedGenerator.java` | the system's entropy leaves out the network adapters (`NetworkInterface`'s natives are not translated) and reads the heap's figures from `Runtime.getRuntime()` directly, not through a local; a seed source that is not a `file:` URL throws instead of opening a connection |
| `sun/security/provider/SunEntries.java` | the seed source is `file:/dev/random`, the value of the JDK's `java.security` file, unless `java.security.egd` sets it |
| `sun/security/util/Debug.java` | translated although `sun.security` is not; writing a message, which names the caller's frame, fails loudly |
| `sun/util/logging/PlatformLogger.java` | every logger's backend is `TranslatedConsoleLogger`, Java's default one; levels without their `System.Logger.Level`; loggers cached strongly; `setLevel` a native the translation does not have |

## Checks

`scripts/check-jdk-copies.py JAVA_BASE_SOURCES` compares each file of `java/patches` and
`java/overlay` with the JDK's file of the same path, in `java.base` or in another of the JDK's
modules next to it (as in the JDK's `src.zip`). It fails when:

- a file of `java/overlay` has the path of a JDK file (a change to a JDK class is a patch);
- a patch names a path the JDK does not have;
- a file shares more than 12 distinct lines with the JDK's file of the same path, or, for a
  class of our own, with any JDK file of its package.

Lines that say nothing on their own are not counted: blank lines, package and import lines,
lines of only brackets, the bare lines of comments, lines under 16 characters (`return true;`,
`} else {`), and in a patch the class's line and the declarations after the markers, which name
the JDK's members. With those rules the patches share at most 11 lines each (short idioms such as
`throw new NoSuchElementException();`), while the whole-file copies they replaced shared 14 to
987 (`InetAddress`), and the generated `TranslatedCoders` 241 with `UTF_8.java` alone.
`scripts/check.sh` runs the check when `JAVA_BASE_SOURCES` is set; `--verbose` lists the shared
lines of every file.

A class of our own (a file of `java/overlay` at a path the JDK does not have) is compared with
each JDK file of its package instead. Code copied from a JDK class of another package would not be
caught, so a review still asks where new code came from.
