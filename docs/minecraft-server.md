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
| `extract` | unpacks the bundler jar: the real server jar is `META-INF/versions/26.3/server-26.3.jar`, with 39 library jars under `META-INF/libraries`; `META-INF/main-class` names `net.minecraft.server.Main`. Java 25 is required | seconds |
| `decompile` | Vineflower 1.12.0 (`-dgs=1 -rsy=1`, `-Xmx3g`) on `net/minecraft` and `com/mojang`, with every library jar given to it as a library (`-e`, [below](#decompiling-with-the-libraries)): 5,037 files, 546,535 lines. Also Vineflower without the libraries, and CFR on the files that have errors. Since 26.1 the jar is not obfuscated, so the names are Mojang's |
| `best-of` | `src/server` from the three decompiles, taking each file from Vineflower with the libraries except where another setup's file gives the whole server fewer javac errors ([below](#2-decompiled-sources-that-do-not-compile)) | 5 compiles of the server, about 1.5 min each | 3 min 12 s on a busy machine (34 s without the libraries on a quiet one) |
| `libraries` | each library jar decompiled the same way, with the other library jars as libraries, one at a time, except the three that only load a native library: 7,375 files, 2.03 million lines (fastutil alone is 2,411 generated files) | 8 min 40 s without the libraries; they add about 4 s per jar |
| `library-sources` | downloads the sources jar that Maven Central publishes for each library, checks its SHA-1, and unpacks its Java files into `src/library-sources`: 30 of the 35 libraries, every class that their decompiled code has. Mojang's authlib, brigadier, datafixerupper, jtracy and logging are not published there and stay decompiled | under a minute |
| `jdk-sources` | unpacks the JDK modules the server and its libraries use from the JDK's `src.zip`: `java.base` plus 14 others, including `java.desktop` (the server's Swing console), `jdk.jfr` (its profiling events) and `jdk.unsupported` (`sun.misc.Unsafe` for Netty and others) | seconds |
| `parse` | javac on the decompiled server against the original library jars, then on each decompiled library, counting errors by kind | 1 min 37 s |
| `census` | the translator's census from `main`, past every rejection and past every compile error (`--keep-going`, below), into `census/`; the census before it, if it wrote its `census.json`, is moved to `census-previous/` first | stopped by the memory cap, see below |
| `summary` | splits the census by origin, into the server, each library and the JDK (`scripts/census-by-origin.py`), and compares it with the previous one (`scripts/census-compare.py`, below) | seconds |

The census gives the translator only `Main.java` as a source file. The rest of the server is a
`--library` root like the libraries and the JDK modules. Each library's root is its published
sources where `library-sources` has them, and its decompiled code otherwise. This way javac reads and
attributes only the classes that the code reachable from `main` uses, not all 5,037 server files
and 2,411 fastutil files. fastutil is then only the few dozen classes the server reaches.

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

The `summary` step runs it with the roots of `census-by-origin.py`, against `census-previous/`
(or `PREVIOUS_CENSUS`, such as a census kept by hand under another name), and writes the result
to `census/compare.txt` as well. Only one previous census is kept: each `census` step deletes
the one before last.

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

### Going on past compile errors

Decompiled code does not always compile, and the rules for this target say not to fix it by hand.
The first census attempt stopped at the first error. The decompiler had written
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

A translation with `--keep-going` does the same, so a program whose library has optional
dependencies can be built: it stops only at a rejected piece, as any translation does.

## Numbers

### Parsing (javac on all of the decompiled code)

- **The server:** 185 errors in 117 of its 5,037 files with Vineflower and the libraries alone,
  compiled against the original library jars (579 in 282 when decompiled without the libraries;
  159 in 104 with the files chosen from the three setups by the `best-of` step, see
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
annotations used by name. Their uses in annotations are now ignored. Before `jdk.jfr`,
`java.datatransfer` and `java.prefs` were added, 147 more errors were "package is not visible".

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
  Vineflower decompiles with the library jars ([above](#decompiling-with-the-libraries)). They
  cannot be fixed by hand under this target's rules. Most are type arguments and casts that the
  class files do not record at all, so no pass over the class files can bring them back.

  So the server is decompiled three ways and put together file by file. The `decompile` step
  writes three trees into `decompiled/`: Vineflower with the libraries, Vineflower without them,
  and CFR 0.152. CFR is fetched from Maven Central like Vineflower, and runs only on the classes
  of the files that have errors, since it takes about 25 minutes on the whole server. Procyon is
  not on Maven Central as a runnable jar. The `best-of` step (`scripts/decompile-best-of.py`)
  then builds `src/server` from the Vineflower-with-libraries tree, taking a file from another
  tree only where the whole tree then has fewer errors. A file's own errors are not enough to
  judge: a file from CFR often declares its classes differently enough that the files using
  them stop compiling. It works in whole-server compiles, at most 8:
  1. for each other setup, the Vineflower tree with that setup's version of every file that has
     errors; a file whose own errors drop becomes a candidate, from the setup where they drop
     most;
  2. the Vineflower tree with all candidates, again each round: a round drops the candidates
     that no longer have fewer errors, and those whose class is named in a file that is not
     swapped and has gained errors. The round with the fewest errors in the whole tree is kept,
     if it has fewer than Vineflower alone.

  Measured with the `parse` step (which attributes every file even when one does not parse):
  from 186 errors in 118 files to 159 in 104, with 10 files from Vineflower without the
  libraries and 8 from CFR, in 5 compiles. What is left is mostly type arguments and casts that
  none of the decompilers writes; those pieces stay rejected.

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

1. Get the census through loading: a run with the library sources under a 12 GB cap on a quiet
   machine, then the memory steps if that is not enough.
2. Compare the server's rejections with javac's, and list the new kinds.
