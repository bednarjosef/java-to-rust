# The Minecraft server as a target (survey)

The dedicated server of Minecraft: Java Edition 26.3 is a future target. This document records how
it is prepared for the translator and how far the translator gets on it today, with the numbers
of the first survey (2026-10-22) and the blockers they show.

Mojang's code, and everything made from it, stays outside the repository: the jar, the
decompiled sources and the census output are kept in `~/.cache/java-to-rust-targets/minecraft-26.3/`
(`MINECRAFT_CACHE`). Only the scripts and this document are in the tree.

## The steps

`scripts/minecraft-server-survey.sh` runs them all, or the ones named. Every heavy step runs
under `SLOT` (such as the build-slot limiter), and the census under a memory cap (`MEMORY_CAP`,
6 GB by default, through `systemd-run`). If the cap is reached, the census is stopped instead of
the machine running out of memory.

| step | what it does | time |
|---|---|---|
| `extract` | unpacks the bundler jar: the real server jar is `META-INF/versions/26.3/server-26.3.jar`, with 39 library jars under `META-INF/libraries`; `META-INF/main-class` names `net.minecraft.server.Main`. Java 25 is required. Then unpacks the jars' resources, every file but their classes: the server jar's without its `META-INF` (the data packs under `data/`, `assets/`, `version.json`: 9,889 files, 47 MB) into `resources/server`, and each library jar's without its manifest and signatures (`META-INF/services`, through which slf4j finds log4j, log4j's index of its plugins, native libraries: 229 files) into `resources/libraries/<n>-<jar>`, numbered in the order of the class path | seconds |
| `decompile` | Vineflower 1.12.0 (`-dgs=1 -rsy=1`, `-Xmx3g`) on `net/minecraft` and `com/mojang`, with every library jar given to it as a library (`-e`, [below](#decompiling-with-the-libraries)): 5,037 files, 546,535 lines. Also Vineflower without the libraries, and CFR and Procyon on the files that have errors. Since 26.1 the jar is not obfuscated, so the names are Mojang's | 3 min 12 s for Vineflower with the libraries on a busy machine (34 s without them on a quiet one) |
| `best-of` | `src/server` from the four decompiles, taking each file from Vineflower with the libraries except where another setup's file gives the whole server fewer javac errors ([below](#2-decompiled-sources-that-do-not-compile)) | 5 compiles of the server, about 1.5 min each |
| `libraries` | each library jar decompiled the same way, with the other library jars as libraries, one at a time, except the three that only load a native library: 7,375 files, 2.03 million lines (fastutil alone is 2,411 generated files) | 8 min 40 s without the libraries; they add about 4 s per jar |
| `library-sources` | downloads the sources jar that Maven Central publishes for each library, checks its SHA-1, and unpacks its Java files into `src/library-sources`: 30 of the 35 libraries, every class that their decompiled code has. Mojang's authlib, brigadier, datafixerupper, jtracy and logging are not published there and stay decompiled | under a minute |
| `library-best-of` | each library without published sources decompiled the four ways of `decompile` into `decompiled/libraries/<artifact>`, and put together into `src/libraries/<artifact>` as `best-of` does for the server, compiled against the other library jars | 3 min |
| `multi-release` | the classes that a multi-release jar gives for the Java that runs it (`META-INF/versions/<n>`, the highest version up to Java's for each class), which Java loads instead of the jar's own, decompiled with Vineflower over that class's source in the library's source roots: log4j-api's `StackLocator`, `ProcessIdUtil`, `Base64Util` and `DefaultObjectInputFilter`, and five classes of log4j-core (the other multi-release jars version only their `module-info`). Not applied yet: log4j's Java 9 `StackLocator` walks the stack (`StackWalker.walk`), which the translation rejects, while its base one answers without a caller class | a minute |
| `repair` | edits `src/server`, and each library in `src/libraries` that has no published sources, where javac rejects it, with edits that keep the bytecode's behaviour ([below](#repairing-what-the-decompiler-left-out)), up to `REPAIR_ROUNDS` (10) rounds; every edit is listed in `repair-log.txt` for the server and `repair-log-<artifact>.txt` for a library; then `hand-fixes/server.patch` in the cache, if there is one, is applied to `src/server` ([below](#what-is-fixed-by-hand)) | about 5 min, and 1 min for the libraries |
| `jdk-sources` | unpacks the JDK modules the server and its libraries use from the JDK's `src.zip`, [below](#the-jdk-modules): `java.base` plus 30 others, including `java.desktop` (the server's Swing console), `jdk.jfr` (its profiling events), `jdk.unsupported` (`sun.misc.Unsafe` for Netty and others) and `jdk.httpserver` (the HTTP server of an authentication library) | under a minute |
| `parse` | javac on the decompiled server against the original library jars, then on each decompiled library, counting errors by kind | 1 min 37 s |
| `census` | first puts each jar's manifest in the source root of its classes (`META-INF/MANIFEST.MF`, as the jar has it), from which the translation takes the versioning attributes of the root's packages; then the translator's census from `main`, past every rejection and past every compile error (`--keep-going`, below), with the messages of a `NullPointerException` naming locals as the server's class files do (`--named-locals`: the server and its libraries were compiled with `-g`, [exceptions.md](exceptions.md#the-messages-of-nullpointerexception)) and approximating those whose wording is not certain (`--approximate-messages`), into `census/`; the census before it, if it wrote its `census.json`, is moved to `census-previous/` first | stopped by the memory cap, see below |
| `summary` | splits the census by origin, into the server, each library and the JDK (`scripts/census-by-origin.py`), and compares it with the previous one (`scripts/census-compare.py`, below) | seconds |
| `trial-write` | not run by default. Writes the trial translation again from the `trial-tree.bin` that the census saved, with the translator as it is now (see below) | |
| `trial` | not run by default. Builds the trial translation that the census writes when `TRIAL_CRATES` is set: the whole server with each rejected piece of code ending the program with its rejection, as a workspace split into that many body crates ([several-crates.md](several-crates.md)), in `trial/`. Then runs it in a new directory `trial-run/` with the EULA accepted, as `--nogui`, with `stop` on standard input once it logs that it is done starting (the server fails a `stop` that comes earlier, and keeps running), so that it makes a world, loads, and stops; the run ends when the server does, or after `SERVER_SECONDS` (600). The build is capped at `TRIAL_MEMORY_CAP` (16 GB), with the `dev` profile unless `TRIAL_PROFILE` says otherwise, the runtime crate optimized (`opt-level` 2) all the same, and goes on past a crate that fails (`--keep-going`), so that `trial-build.txt` has the errors of every crate | |
| `java-run` | not run by default. The same run of the server on Java, from the jars, in `java-run/`: what the trial's run is compared with | |

The census gives the translator only `Main.java` as a source file. The rest of the server is a
`--library` root like the libraries and the JDK modules, and so are the resource roots, from
which the translated server reads its resources as Java reads them from the jars, and whose
service declarations the translation's `ServiceLoader` finds
([reflection-and-plugins.md](reflection-and-plugins.md)). Each library's root is its published
sources where `library-sources` has them, and its decompiled code otherwise. This way javac reads and
attributes only the classes that the code reachable from `main` uses, not all 5,037 server files
and 2,411 fastutil files. fastutil is then only the few dozen classes the server reaches.

### Census checkpoints

javac, reach, lowering and the walk of the code that can run take about a quarter of an hour of
a census of the server. With `JAVA_TO_RUST_CENSUS_CHECKPOINT=FILE`, the translator saves the
lowered program to `FILE` (`program/checkpoint.rs`) when the file does not exist, and when it
does, it reads the program from it and starts the census at the analyses. A census taken from a
checkpoint is the same as the full one (checked on javac's census and translation). A checkpoint
belongs to one program and to a translator whose javac, reach and lowering give the same
program. After a change to those, delete the file so that the next census writes a new one.

A census that writes the trial translation (`TRIAL_CRATES`) also saves it, before it is written,
to `trial-tree.bin` (`--save-tree`). After a change to how a translation is written or split,
the `trial-write` step writes the trial again from that file (`--from-tree`) in seconds, rather
than through a census (checked on javac: the workspace written from the saved translation is the
census's, byte for byte).

### Comparing censuses

`scripts/census-compare.py OLD NEW [NAME=ROOT...]` reads two censuses (each a `census.json` or a
directory with one) and prints, in plain text:

- whether each is complete, or where it stopped. A census that stopped has only the rejections
  found before it stopped, so its pieces look better than they are;
- for each origin, the pieces and the share that translate, old and new side by side, with the
  change in points and in pieces that translate. With no roots given, the origins are the
  census's own, program and library;
- the rejection messages whose pieces went down the most and up the most, and the new ones, each
  with its old and new count (a message's pieces as the census counts them, `--top N` of each,
  20 by default).
- the handle calls that the translation does not follow and makes throw, and the charset names
  known only at run time (both below), each by pieces, old and new, and the messages whose
  pieces changed.

The `summary` step runs it with the roots of `census-by-origin.py`, against `census-previous/`
(or `PREVIOUS_CENSUS`, such as a census kept by hand under another name), and writes the result
to `census/compare.txt` as well. Only one previous census is kept: each `census` step deletes
the one before last.

### The JDK modules

javac compiles the program with every root as part of one module (`program/modules.rs`), so a
package of a JDK module whose sources are not a root is "not visible", and the code that uses it
is cut off as a compile error. `jdk-sources` therefore unpacks every module that the code may use,
which `scripts/jdk-modules.py` finds from the server's and the libraries' sources and the
`module-info.java` files in `src.zip`:

- each module with a package that the code names, in an import, a qualified name or a string
  (`Class.forName`), but not in a comment;
- the modules that those require, transitively;
- the modules that provide a service that one of them uses, as the JVM binds services when it
  starts: `ManagementFactory` (`java.management`) finds the platform beans of `jdk.management`,
  and `java.compiler`'s `ToolProvider` finds javac in `jdk.compiler`. `java.base`'s own services
  are left out, as their providers are in most of the JDK (its tools, the locale data, charsets,
  security providers); a module that provides one is there only when the code names it or a
  module requires it, and `ServiceLoader` does not find the others.

It writes the list to `modules.txt` in `JDK_SOURCES`, and `census` gives each of them as a
`--library`. For 26.3 that is 31 modules, among them `jdk.httpserver` (msal4j's sign-in
listener) and the compiler's modules (log4j's annotation processor names `java.compiler`, whose
`JavaCompiler` service `jdk.compiler` provides).

### Decompiling with the libraries

A class file keeps the generic signatures of the classes, fields and methods it declares, but not
the type arguments of the calls in its code, nor the types of its local variables beyond what
the local variable table records. Vineflower puts them back from the signatures of the methods
called. Without the library jars it does not have those signatures for library classes such as
DataFixerUpper's `RecordCodecBuilder` or Guava's `ImmutableList`. It then writes
`RecordCodecBuilder.mapCodec(i -> ...).validate(...)` where the original had
`RecordCodecBuilder.<RangeInfo>mapCodec(...)`, and javac, with nothing to infer `RangeInfo` from,
infers `Object`: one error for the call and one "cannot find symbol" for each field the lambdas
read. With every library jar given as `-e`, Vineflower writes the type arguments where javac
needs them, and types local variables (`Pair<RecipeCategory, String>`, `Tag`) instead of leaving
them `Object`.

Measured by the `parse` step (javac on all of the server against the library jars), this takes
the server from 579 errors in 282 files to 185 errors in 117 files. 172 files compile that did
not; 7 that compiled have an error now (a lambda parameter or a method reference that javac
cannot infer with the type arguments written out). The other Vineflower settings tried do not
help. Compared on the files that had errors: `--explicit-generics`, which writes the type
arguments of every generic call, gives more errors, with the libraries (296 against 181) or
without them (642 against 585); `--verify-merges`, `--verify-pre-post-merges` and
`--simplify-stack=false` change the count by at most one; `--old-try-dedup` writes `switch`
code that does not parse. 1.12.0 is the latest Vineflower release.

### Repairing what the decompiler left out

The `repair` step (`scripts/RepairDecompile.java`) compiles the decompiled server, and each
decompiled library on its own, with javac's tree API, so that it sees each error's kind, the
expression it is at and the types involved, not just javac's message. It makes only edits whose
bytecode the class file already has:

- **A cast to the type javac expects**, where the type found cannot be converted but its erasure
  is a subtype of the expected type's erasure: `return (Brain<Hoglin>) (Brain<?>)
  super.getBrain();`, `(List<Component>) (List<?>) stream.toList()`. Such a cast is unchecked:
  javac emits no `checkcast` for it, which is what the original's cast compiled to. The cast
  through the wildcard type is what makes it legal when the two parameterizations are provably
  distinct. When the expected type is a capture, which source cannot write, the cast is to its
  erasure, a raw type (`(Codec) codec`), which javac accepts with an unchecked warning.
- **A downcast to the type javac expects**, where the expected type's erasure is a subtype of
  the found type's (`Object` to `EntityHitResult`). That cast is checked, so it is made only
  when the class file of the enclosing class has a `checkcast` to that class: the jar is read with
  `java.lang.classfile`, and a class without one gets no such edit.
- **A loop over a raw collection** (`for (EntityHitResult hit : (Collection) hits)`), where the
  class file casts each element to the loop variable's class: the collection is cast to
  `Iterable` of that class, through the raw type, so that javac emits that cast on each element.
- **A lambda for an interface that is not functional** (`return () -> codec;` for an
  `EntityDataSerializer<T>`, which has two abstract methods): cast to the one interface that
  extends it and that the class file's lambdas are made for (`(EntityDataSerializer.ForValueType<T>)
  () -> codec`), read from the `invokedynamic` instructions of the class. That is what the
  original's lambda was for; its default methods leave one abstract method.
- **A type argument for a generic call** whose result a call on it unwraps
  (`.map(key -> Component.literal(..)).orElse(INVALID_TEST_NAME)`, where javac infers
  `MutableComponent` from the lambda): the type that the whole is returned as or initializes,
  written out (`.<Component>map(..)`), as the original's source had it.
- **Type arguments worked out around an inference error**, or around a call that no method
  fits: those of the calls in the whole expression around the error, through calls, lambdas,
  conditionals and casts, as the original's source must have given them. They come from the type
  the expression is for (the variable it initializes, the result it returns, the parameter it is
  passed to, the result of the lambda it is returned from), the declared types of the
  arguments, what a lambda or a method reference returns given its parameter types, and the
  bounds of the method's type variables (`M extends Map<K, V>`), through chains of calls
  (`a.b(..).c(..)`, where the type `c` is for tells the type `a.b(..)` must have). Each type
  variable is the type it must be, else the most specific of those it must be below, else the
  most general of those it must be above: `MethodInfo.<Params>paramsTypedCodec()`,
  `RecordCodecBuilder.<Pair<P, P>>create(..)`, `new RunOne<Allay>(..)`. Type arguments the decompiler
  wrote that disagree are rewritten (`<PathfinderMob>` to `<Piglin>`), and a raw `new C(..)`
  gets them. They are written first only for the calls whose type variables javac names as not
  inferred, then, if that does not do, for every call. An argument whose declared type the
  parameter then does not take is cast to it, unchecked as the original's conversion was
  (`Optional.<U>of((U) this.value)`), and so is a lambda's body that the result of its function
  type then does not take (`e -> (Optic<? super P, ?, ?, ?, ?>) (Optic<?, ?, ?, ?, ?>)
  e.optic()`). A diamond that cannot be inferred is written out, with its arguments cast the
  same way (`new Typed<Either<A, B>>(.., (Either<A, B>) (Either<?, ?>) inj1().build(value))`). A
  receiver that javac infers on its own, whose result does not fit what the call on it is for,
  gets its type arguments from that instead (`.<Single<?>>map(Single::fromEntry).toList()`
  returned as a `List<Single<?>>`): the original wrote them. As a third try, every receiver
  that needs inference gets them from what the call is for, even where its own result fits:
  `elements.stream().map(e -> e.optic()).collect(toList())` assigned to a `List<Optic<? super
  P, ?, ?, ?, ?>>` maps to that type, the lambda's own being narrower. Type arguments are
  inference only; the calls and their descriptors stay.
- **Other options at such an error**, each tried when the ones before it did not lower the
  file's errors: the decompiler's cast to a raw class made a cast to a type variable in scope of
  that erasure (`(Comparable) v` to `(T) v`, the same `checkcast`); an argument whose type holds a
  captured wildcard cast to its raw class; a lambda or method reference passed to the call cast to
  the one interface that fits the parameter and that the class file makes for it (for a method
  reference, the interfaces made for that method's name: `(EntitySpawnerItemVariantComponentFix.Fixer)
  EntitySpawnerItemVariantComponentFix::fixSalmonBucket`); the receiver cast to its raw class
  where the argument's expected type is a capture of the receiver's wildcard
  (`((Registry) BuiltInRegistries.REGISTRY).getId(..)`); the call cast through the wildcard
  type to the declared type of the whole (`(Codec<X>) (Codec<?>) typedCodec()`); a diamond for
  every raw `new C(..)` of a generic class passed anywhere in the statement (`new
  Pair.Instance<>()`); the method's one type variable that none of its code names, written for
  each `?` in the statement's casts (Vineflower writes `?` for a type variable that the method
  declares but never names); and the statement's casts that compile to nothing left out. Those
  casts are the decompiler's guesses at types, often with the wrong type variables
  (`(Either<Either<B, C>, A>) prism.match(a)`, where the original infers `A2`), and they keep
  javac from inferring what the original inferred. A cast between two parameterizations of one
  class has no `checkcast`, and neither has a cast to a wider class; the casts between
  parameterizations of one class are left out first, before any other option. A cast to a wider
  class stays where its type can choose what the code means: when it is not to a parameterized
  type (`(Object) s`), when a member of it is selected (a hidden field), and when it is passed to
  a method or constructor that has another one of the name with as many parameters.
- **A diamond in an argument** `new C(..)` of a generic class C, written raw, of a call whose
  type variables javac then cannot infer (`register(registry, "resource", ..., new
  ResourceArgument.Info())`): `new C<>(..)`, tried before a cast at the same error.
- **Type arguments for a diamond** that javac cannot infer, copied from the declared type of
  the variable it initializes or the method result it is returned as, when that is the same
  class: `return new SortedArraySet<T>(...)`. Type arguments are inference only, with no
  bytecode of their own.
- **A type argument the decompiler left empty** (`(Function<Object, >) a1 -> input` in
  DataFixerUpper's `RecordCodecBuilder`, a type Vineflower could not name), found at the `>` where
  javac's parser stops: the cast to the raw type (`(Function) a1 -> input`), which erases to the
  same cast.
- **A class's type variable in a static method**, written in a cast or in a call's type
  arguments (`Lens.<S, T, A, B>unbox(l)`, `(S) g.apply(c)`), where the decompiler used the class's
  names for the method's own type variables: first every cast in the statement that compiles to
  nothing left out (the other casts there are the decompiler's guesses too), then the one cast or
  the type arguments. A cast to a type variable has no `checkcast`, and type arguments are
  inference only.
- **A method that the class file declares static** but the source does not, before the first
  round: declared `static`. Procyon writes a static interface method as `default` (`unbox` of
  DataFixerUpper's `CartesianLike`), which javac then rejects at every call from a static
  method.
- **A decompiler's cast to exactly the type javac expects** that javac does not allow, as the
  two parameterizations are provably distinct: a cast through its class's wildcard type before
  it (`(PointFree<Function<A, C>>) (PointFree<?>) f`). The original's cast is still made, with its
  `checkcast` where it had one.
- **A bound that names the type variable itself** (`CartesianLike<..., Mu extends Mu>`), where
  the original's bound is a member class of the same name: written as the member
  (`Mu extends CartesianLike.Mu`).
- **A misnamed catch variable:** Vineflower sometimes declares `catch (Exception e)` and uses
  `ex` in the block. Every use of the unresolved name is renamed to the parameter of a catch
  that encloses all of them, if that parameter is not used in its block, no other catch around
  a use has an unused parameter, and nothing inside the block declares the parameter's name
  again. A use inside a nested catch whose own parameter is unused could read either
  parameter; the class file tells which. A string constant just before a use
  (`LOGGER.warn("Failed to delete ..", ex)`) is loaded right before the variable the use reads,
  and a string constant of a catch's own block comes after the start of its handler, which
  stores the exception in that catch's variable. The name becomes the parameter of the one catch
  whose variable is the one read, if each constant is in the class file once.
- **A temporary the decompiler did not declare** (`var24 = var10000.steps();` in a `try`, a
  record pattern's component that Vineflower reads into a variable it declares nowhere): declared
  with the type of the value first assigned to it (`List<PositionStep> var24;`), just before the
  statement of the innermost block that holds every use. The first use must be that assignment.
- **A local variable declared twice** in one scope (Vineflower giving two variables the same
  name): the second, and every use of the name after it in its block, renamed to a name the file
  does not use (`totalInterpolationTicks2`).
- **CFR's `SwitchBootstraps.typeSwitch(..)` call**, its way of writing the `invokedynamic` of a
  pattern `switch`, which javac rejects (the bootstrap method takes a lookup, a name and a method
  type): replaced with the index the call site returns, `x == null ? -1 : n <= 0 && x instanceof
  A ? 0 : n <= 1 && x instanceof B ? 1 : 2` for the labels `A.class, B.class` and the restart
  index `n`. This one is not the original's bytecode, but it computes the same index: -1 for
  `null`, else the first label from `n` on that `x` is an instance of, else the number of labels.
  It is made only where `x` and `n` are variables or constants, which can be read again, and
  every label is a class.
- **A parameter type the decompiler erased** in a method that then overrides nothing, so javac
  says its class does not implement an abstract method (`encode(Object input, ..)` in a
  `MapEncoder.Implementation<R>` of DataFixerUpper's `RecordCodecBuilder`, whose method is
  `encode(R input, ..)`): written with the parameter type of the supertype's method that has the
  same erased parameters, as the class inherits it (`R`), with the method's own type variables.
  The erasure, and with it the method's descriptor, stays. Two such methods that disagree get no
  edit.
- **Hints that inference has to meet, left out,** in an expression whose type variables javac
  cannot infer, or whose bounds disagree: type arguments that name `Object`, which the
  decompiler writes for a type it could not name (`Either.<Object, R>unbox(a)`; also in an
  argument, before a cast is tried), and a cast to a type variable erased to `Object` that is a
  lambda's whole body (`e -> (T) f.apply(e)` in `MapCodec` and `MapDecoder`, where the decompiler
  named the erased result with a type variable of the enclosing method). Type arguments are
  inference only, and a cast to such a type variable has no `checkcast`.
- **The raw qualifier of an inner class** in `new Outer.Inner<>(..)`, where `Inner` is an inner
  class of a generic class `Outer` that the enclosing class is or extends (`new
  ListCodec.DecoderState<>(ops)` inside `ListCodec<E>`): javac rejects a diamond on an inner
  class of a raw type, and `new Inner<>(..)` is the same creation with the enclosing `this` as
  the outer instance.

Classes are named as the file can name them: by simple name when the file imports them or they
are in its package or in `java.lang`, otherwise qualified. A type that source cannot write (a
captured wildcard outside a type argument, an intersection, a local class) gets no edit.

An error can have several options, tried in order: in each round, every error gets its first
option not tried before. After the round, a file whose errors (outside imports and annotations)
did not go down gets its text back. A single option that did not help is not tried again; where
the file had several errors' options at once, its options are tried one per round from then on.

On the server taken file by file from the decompile setups that compile best (151 errors in 100
files, 109 of them outside imports and annotations), it leaves 42 errors in 18 files, all of them
imports and annotations of annotation packages that no library jar has (`javax.annotation`,
`org.jetbrains.annotations`), in three rounds. DataFixerUpper needs nine.

### What is fixed by hand

The repair leaves no error in the server's own code outside imports and annotations, so nothing
there needs a fix by hand. Where one is needed, it would be Minecraft's code, so it lives in the
cache only, as `hand-fixes/server.patch` (with `hand-fixes/orig/`, the repaired text of each file
before its fix, and `make-patch.sh`, which diffs the two), and the `repair` step applies the
patch after its own edits. A hunk that the repair already made is skipped, and
`hand-fixes/applied.txt` lists it.

### Going on past compile errors

Decompiled code does not always compile. The first census attempt stopped at the first error. The decompiler had written
`(Function<Object, >)` in DataFixerUpper's `RecordCodecBuilder`.

So a census now takes `--keep-going`. With it, javac prints each compile error instead of
stopping, and each error is handled in one of two ways:

- **An error that names a class missing from the roots** (an optional dependency that is not
  shipped) does not reject anything. The code that would use the class throws
  `NoClassDefFoundError` with its binary name, as the JVM does when the class is missing at run
  time ([exceptions.md](exceptions.md#classes-missing-from-the-roots)). The census counts such
  pieces as translating and lists them in their own section, per missing class:
  `throws NoClassDefFoundError like Java: missing class io/netty/internal/tcnative/SSL, N pieces`.
- **Any other error** marks the piece of code that it is in with javac's error code: the
  innermost method, lambda, field initializer, initializer block, or the class when the error is
  in its declaration. The census rejects each marked piece with the message
  `javac reports a compile error (compiler.err.…)`, the same way it rejects any other piece. The
  rest is still counted.

Two kinds of error mark nothing:

- an error in an import, since the code that uses the missing name has errors of its own;
- an error in an annotation, since annotations are left out of the translation anyway.

An error that names a missing class in an annotation, or in an import that only annotations use,
is not even printed: the JVM ignores an annotation whose class is missing, so it is no error,
with or without `--keep-going`.

The census lists one more kind of code that translates but knowingly behaves otherwise than
Java: a call of a lookup's or a `MethodHandle`'s method that the translation does not follow,
which throws `UnsupportedOperationException` where Java runs it
([multi-threading.md](multi-threading.md), handles found at run time). netty looks up its
virtual-thread check and its cleaners of direct buffers this way and catches the failure. Such
calls are no rejections. Their own section, per part, groups them by what was not followed, with
the number of pieces and the place of every call:
``throws UnsupportedOperationException: `Lookup.findVirtual` of a handle or arguments that the
translation does not follow, or that may fail, N pieces``. `census.json` has them under
`unfollowedHandles`, and each piece's under its own `unfollowedHandles`.

So are the lookups of a charset by a name known only at run time (`Charset.forName`,
`isSupported`, `getBytes` and `new String` with a name): the name may be that of a charset of
the JDK that the translation has neither in the runtime nor as a table (Shift_JIS, GBK, UTF-32),
for which the program throws `UnsupportedOperationException` where Java finds the charset
([supported-java.md](supported-java.md), charsets). Their section reads ``may throw
UnsupportedOperationException: `Charset.forName` of a charset name known only at run time, N
pieces``, and `census.json` has them under `charsetNames`, the whole program's and each
piece's.

So is a security manager's `getClassContext()`, the classes on the call stack, which log4j,
slf4j and JNA read through a subclass of `SecurityManager`: a translated program has no stack to
walk, so it throws `UnsupportedOperationException`, which log4j and slf4j catch to fall back
([supported-java.md](supported-java.md), the caller's class). Its section reads ``throws
UnsupportedOperationException: SecurityManager.getClassContext: the classes on the call stack,
which a translated program does not have, N pieces``, and `census.json` has it under
`classContexts`.

A translation with `--keep-going` does the same, so a program whose library has optional
dependencies can be built: it stops only at a rejected piece, as any translation does.

## Numbers

### Parsing (javac on all of the decompiled code)

- **The server:** 185 errors in 117 of its 5,037 files with Vineflower and the libraries alone,
  compiled against the original library jars (579 in 282 when decompiled without the libraries;
  151 in 100 with the files chosen from the four setups by the `best-of` step, and 42 in 18,
  all in imports and annotations, after the `repair` step, see
  [below](#2-decompiled-sources-that-do-not-compile)). The largest kinds, with Vineflower alone:

  | errors | kind |
  |---:|---|
  | 30 | a type cannot be converted to another (`List<MutableComponent>` to `List<Component>`) |
  | 27 | a captured wildcard cannot be converted (`return super.getBrain()` without the original's cast to `Brain<Hoglin>`) |
  | 29 | inference failing (`cannot infer type-variable(s)`) |
  | 29 | other incompatible types: lambdas for interfaces that are not functional, method references, conditional expressions, incompatible bounds |
  | 34 | cannot find symbol: 24 annotations (`@Contract`, `@Immutable`, `@CheckReturnValue`); the rest are mostly a catch variable that the decompiler declares as `e` and uses as `ex`, and temporaries of record patterns used outside the `try` that declares them |
  | 18 | annotation packages missing (`org.jetbrains.annotations`, `javax.annotation`) |

  Most are type arguments or casts that the original source had and the class file does not
  keep, where Vineflower does not see that javac needs them. The annotations are compile-only
  libraries that the bundler does not ship.

- **The libraries,** each one compiled on its own:

  | library | errors in its files |
  |---|---|
  | Guava | 2,023 errors in 320 of its 608 files |
  | netty-handler | 526 errors in 28 files |
  | netty-common | 140 errors in 28 files |
  | DataFixerUpper | 172 errors in 57 of its 164 files (73 after the `library-best-of` and `repair` steps, all of them imports and uses of `javax.annotation`) |
  | authlib, Gson | 107 errors each |
  | netty-codec-compression | 101 errors |
  | most others | a few |
  | Brigadier, msal4j, slf4j | none |

  Guava's errors are mostly its compile-only annotations (Error Prone, J2ObjC, `javax.annotation`),
  which are missing.

### The census

Run from `net.minecraft.server.Main`, under an 8 GB cap:

| round | units held | units asked for next | JSON trees received | translator memory | javac peak |
|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 79 | 21 MB | 0.17 GB | 1.32 GB |
| 2 | 80 | 1,168 | 230 MB | 1.73 GB | 1.44 GB |
| 3 | 1,248 | 1,976 | 636 MB | 5.47 GB | 1.57 GB |
| 4 | 3,224 | (still growing) | | over the cap | |

The run was stopped in round 4, about a minute in. It never got past loading. So there is no
count yet of pieces that translate, nor of the translator's own rejections.

Each unit's tree is about 200 KB of JSON (about 130 bytes per tree node, and 1.1 KB per source
line). The translator holds it as `serde_json` values that take about 8.6 times the size of the
text. An earlier run without a cap passed 8.7 GB, still growing, before it was stopped by hand.

**Compile errors in the code the census reached:** the 3,224 units of the first three rounds have
2,445 compile errors, in 588 files. These are the ones `--keep-going` goes past:

| errors | origin | what they are |
|---:|---|---|
| 1,268 | Guava (173 files) | mostly missing compile-only annotations |
| 372 | the server (195 files) | 238 incompatible types (generics lost by the decompiler), 69 cannot find symbol, the rest inference |
| 305 | fastutil (96 files) | `wrong number of type arguments` and `Long2FloatMap.Entry does not take parameters`: the decompiler mixes up fastutil's own `Entry` types with `Map.Entry` |
| 104 | authlib | |
| 99 | DataFixerUpper | |
| 93 | Gson | |
| 67 | netty-handler | mostly `io.netty.internal.tcnative`, an optional native library |
| 60 | log4j-core | mostly `com.lmax.disruptor`, an optional library |
| 77 | other libraries | |

Grouped by kind, about 340 of them are "package does not exist" for annotation packages: Error
Prone, `javax.annotation`, J2ObjC, JetBrains. Many of the 1,300 "cannot find symbol" are the same
annotations used by name. Their uses in annotations are now ignored. None is "package is not
visible": `jdk-sources` unpacks every JDK module that the code names ([above](#the-jdk-modules)).

## Blockers, by kind

### 1. The translator's memory (the first wall)

The census cannot reach the end of loading on this machine. After three of perhaps six to ten
rounds, the translator holds 5.5 GB for 3,224 units, and it needs more for each further round.
The whole server's reach is likely several thousand more units: at the same rate, 12 to 20 GB
just to hold the trees, before lowering.

[translator-speed-and-memory.md](translator-speed-and-memory.md) predicted this wall: "20 to 40 GB
of memory, which is the first wall". The survey confirms it with measurements.

**What it would take:** the compact transfer format and the compact trees in the translator
(steps 4 and 5 there), or holding only one unit's tree at a time between reach and lowering.

**Shared with javac?** Yes, the same problem, larger. javac's census receives 318 MB of trees and
holds them in about 3 GB. The server's census already holds twice that after three rounds. Any
progress on javac's memory carries over directly.

**Until then:** on this machine the census runs with a cap of at most 12 GB, on a quiet machine,
so that it cannot run the machine out of memory; a larger cap needs a rented machine with more
memory. The script only needs `MEMORY_CAP` raised.

### 2. Decompiled sources that do not compile

These do not stop a census or a translation with `--keep-going` any more. A piece whose errors
all name missing classes throws as Java would; any other marked piece is rejected.

- **Libraries with published sources use their sources, not decompiled code.** Guava, Gson,
  Netty, log4j, fastutil, commons-io, commons-lang3, JNA, OSHI, JOML, jopt-simple, lz4-java, slf4j
  and msal4j are open source, and their source jars on Maven Central are the code that was
  compiled. By the decompiled-library counts this removes about 1,900 of the 2,445 errors in the
  reached code: all of Guava's, fastutil's and Gson's.

  Brigadier and DataFixerUpper are published by Mojang on GitHub under MIT, but not as source
  jars; they, authlib, jtracy and logging stay decompiled. Their GitHub sources cannot be matched
  to the bundled versions reliably. The tags stop at 1.0.18 and 1.0.20. A version is the
  `majorMinor` of `gradle.properties` and a build number, which the published history does not
  record: DataFixerUpper's 10.0 line is a single commit before the bump to 11.0, and Brigadier's
  1.3 line has three. Compiled with javac and compared instruction by instruction with
  `brigadier-1.3.11.jar`, even the closest commit differs in 7 of its 54 classes.

- **Compile-only annotation libraries** (Error Prone, JSR 305, J2ObjC, JetBrains, Checker
  Framework) are not in the bundle. This does not matter: as on the JVM, an annotation whose
  class is missing is ignored, and javac's errors for it are dropped, in a census and in a
  translation, with or without `--keep-going`.

- **Optional dependencies that are not shipped** (Netty's `tcnative` and Brotli, log4j's LMAX
  Disruptor, Bouncy Castle, Conscrypt) are only in code that the server does not run. Their code
  translates into code that throws `NoClassDefFoundError` where it would use the missing class,
  as on the JVM, so the libraries' own checks (a static initializer that catches the error and
  records the dependency as unavailable) work as they do in Java. The census lists these pieces
  per missing class. A class whose superclass is missing, or that catches a missing exception
  class, cannot be loaded at all, as on the JVM: log4j's `AsyncLoggerDisruptor` is one.

- **The server's own decompiled code:** 185 errors in 117 files (about 2% of them) when
  Vineflower decompiles with the library jars ([above](#decompiling-with-the-libraries)). Most
  are type arguments and casts that the class files do not record at all, so no pass over the
  class files can bring them back.

  So the server is decompiled four ways and put together file by file. The `decompile` step
  writes four trees into `decompiled/`: Vineflower with the libraries, Vineflower without them,
  CFR 0.152 and Procyon 0.6.0. CFR and Procyon's two jars are fetched from Maven Central like
  Vineflower, and run only on the classes of the files that have errors (CFR takes about 25
  minutes on the whole server). Procyon has no runnable jar there, so `scripts/DecompileProcyon.java`
  calls its decompiler with the server jar, the library jars and the JDK as the classes it reads;
  it also gives a public top-level class back the `public` that Procyon leaves off records. The
  `best-of` step (`scripts/decompile-best-of.py`)
  then builds `src/server` from the Vineflower-with-libraries tree, taking a file from another
  tree only where the whole tree then has fewer errors. A file's own errors are not enough to
  judge: a file from CFR or Procyon often declares its classes differently enough that the files using
  them stop compiling. It works in whole-server compiles, at most 8:
  1. for each other setup, the Vineflower tree with that setup's version of every file that has
     errors; a file whose own errors drop becomes a candidate, from the setup where they drop
     most;
  2. the Vineflower tree with all candidates, again each round: a round drops the candidates
     that no longer have fewer errors, and those whose class is named in a file that is not
     swapped and has gained errors. The round with the fewest errors in the whole tree is kept,
     if it has fewer than Vineflower alone.

  Measured with the `parse` step (which attributes every file even when one does not parse):
  from 186 errors in 118 files to 151 in 100, with 10 files from Vineflower without the
  libraries, 8 from CFR and 5 from Procyon (among them `TypedDataComponent`, whose static
  methods Vineflower writes with the record's type variable, and `DataFixers`), in 6 compiles.
  The `repair` step then puts back the type arguments, unchecked casts and catch variable names
  that javac's errors and the class files show ([above](#repairing-what-the-decompiler-left-out)),
  which leaves 42 errors in 18 files, all imports and annotations of annotation packages that no
  library jar has.

  The libraries without published sources go through the same two steps: `library-best-of`
  decompiles each four ways into `decompiled/libraries/<artifact>` and puts it together into
  `src/libraries/<artifact>`, compiled against the other library jars, and `repair` then edits it.
  Brigadier, jtracy and logging compile as Vineflower writes them. DataFixerUpper goes from 169
  errors in 57 files to 165 in 59 with 2 files from CFR and 2 from Procyon (Vineflower without the
  libraries, and CFR's and Procyon's whole sets, are all worse), and to 73 errors in 24 files
  after the repair, all imports and uses of `javax.annotation`. All 107 of authlib's are
  `javax.annotation`, so neither step changes them.

### 3. What the translator itself rejects

This is not measured yet, because the census does not get past loading. Expected from what the
server is:

- **Threads everywhere:** Netty event loops, a worker pool for chunk generation, and the server
  thread. The shared-object rules (locks, `StringBuilder`s that threads may share) apply widely.
- **Netty's and fastutil's low-level code:** `Unsafe`, `VarHandle`, direct buffers.
- **DataFixerUpper's heavy generics:** `App<Mu, ...>` encodings of higher-kinded types, with
  lambdas everywhere. This is the most likely new kind of rejection.
- **Reflection and service loading:** log4j's plugins, Gson, Brigadier's argument types.
- **The Swing console** (`MinecraftServerGui`), reached from `main` unless `nogui` is known.

Which of these javac already shares is in its census (`null` enums, `toString` that throws,
`StringBuilder` shared between threads, `==` on objects that may be strings). The comparison
needs the server's census first.

## Next

These are plans. Work on several of them is under way but not merged yet.

1. Run the census again with the fixes since the last one: the survey's `jdk-sources` step once
   (it writes `jdk-src/modules.txt`), then the decompile, best-of, library-best-of and repair
   steps, then the census. Then compare its rejections with the last census's and with javac's,
   and list the new kinds.
2. The census's speed. A failed try with Rust generics starts lowering over (about 50 s an
   attempt); the walk for the methods that can run (about 10 s) stops at the first rejection
   that can run, which is all a retry needs to know. A `String[]` written as an `Object[]` that
   can run starts the whole census over (javac, reach and lowering, several minutes before its
   walk); that walk stops at the first such write. Planned: cheaper retries, and knowing which
   `String[]` writes can run before the first reach.
3. The translator's refusals still open from the last census, by kind:
   - reflection: generic types of methods; a method's type variables; the rest of `Class`'s
     interfaces (`AnnotatedElement`, ...); member and parameter
     annotations; `Method.toGenericString`;
   - method handles: constant handles (`lookup`, `findVirtual`, `insertArguments`, ...), the
     handles Netty builds at run time, byte array and byte buffer view handles, and field
     updaters' `getAndUpdate`/`accumulateAndGet` loops;
   - logging: JDK logging at the levels it prints (the console handler's header line has the date
     and the caller);
   - natives: calls of library natives (Netty's epoll, JNA, jtracy) should throw
     `UnsatisfiedLinkError` as on a JVM where the library is not loaded, as loading the library
     already does, so Netty falls back to NIO; and `NetworkInterface`'s natives;
   - JDK APIs: NIO networking (channels and selectors, which Netty's server uses),
     `java.lang.management` and JFR, and service providers
     that live in JDK modules the code does not name;
   - small ones, one to four pieces each: `Runtime` held as a value, the whole `System.getenv()`
     map, `Integer.getInteger` with a key that is not a constant, `getProperty` defaults that are
     not literals, `asSubclass` of a class value, `Arrays.copyOf` into another array type, some
     `String` constructors, switches and patterns on some types, a few lambda forms,
     `JavaLangAccess`'s thread and module calls, `isAnnotationPresent(Deprecated.class)`, and
     DataFixerUpper's generic method types as values.
4. Two behaviours waiting for a decision: the objects `getAnnotation` and `getGenericSuperclass`
   return are equal but not the same object across calls, and `Thread.dumpStack` and
   `getStackTrace` print and return no frames.
