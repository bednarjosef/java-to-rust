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
| `decompile` | Vineflower 1.12.0 (`-dgs=1 -rsy=1`, `-Xmx3g`) on `net/minecraft` and `com/mojang`: 5,037 files, 547,614 lines. Since 26.1 the jar is not obfuscated, so the names are Mojang's | 34 s |
| `libraries` | each library jar decompiled the same way, one at a time, except the three that only load a native library: 7,375 files, 2.03 million lines (fastutil alone is 2,411 generated files) | 8 min 40 s |
| `library-sources` | downloads the sources jar that Maven Central publishes for each library, checks its SHA-1, and unpacks its Java files into `src/library-sources`: 30 of the 35 libraries, every class that their decompiled code has. Mojang's authlib, brigadier, datafixerupper, jtracy and logging are not published there and stay decompiled | under a minute |
| `jdk-sources` | unpacks the JDK modules the server and its libraries use from the JDK's `src.zip`: `java.base` plus 14 others, including `java.desktop` (the server's Swing console), `jdk.jfr` (its profiling events) and `jdk.unsupported` (`sun.misc.Unsafe` for Netty and others) | seconds |
| `parse` | javac on the decompiled server against the original library jars, then on each decompiled library, counting errors by kind | 1 min 37 s |
| `census` | the translator's census from `main`, past every rejection and past every compile error (`--keep-going`, below) | stopped by the memory cap, see below |
| `summary` | splits the census by origin, into the server, each library and the JDK (`scripts/census-by-origin.py`) | seconds |

The census gives the translator only `Main.java` as a source file. The rest of the server is a
`--library` root like the libraries and the JDK modules. Each library's root is its published
sources where `library-sources` has them, and its decompiled code otherwise. This way javac reads and
attributes only the classes that the code reachable from `main` uses, not all 5,037 server files
and 2,411 fastutil files. fastutil is then only the few dozen classes the server reaches.

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

- **The server:** 579 errors in 282 of its 5,037 files, compiled against the original library
  jars. The largest kinds:

  | errors | kind |
  |---:|---|
  | 153 | cannot find symbol |
  | 156 | incompatible types (a type cannot be converted to another) |
  | 24 | invalid method reference |
  | 15 | a type variable used in a static context |
  | about 50 | inference failing (`cannot infer type-variable(s)`, incompatible bounds) |
  | 13 | `org.jetbrains.annotations` missing |

  Almost all are the decompiler losing generic type arguments (`ImmutableMap<Object, Object>`
  where the original had `ImmutableMap<String, String>`). The rest are compile-only annotation
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
  jars; they, authlib, jtracy and logging stay decompiled.

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

- **The server's own decompiled code:** 372 errors in 195 files, mostly generics that the
  decompiler lost. They cannot be fixed by hand under this target's rules. The options are:
  - another decompiler or other Vineflower settings, compared by error count;
  - a pass that copies the generic signatures from the class files back into the source, which
    is what javac would need;
  - leaving those pieces rejected, which is about 4% of the server's files.

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
