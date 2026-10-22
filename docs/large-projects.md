# Translating large multi-file projects (planned)

What has to change before the translator can take a real project of thousands of files, with
dependencies, resources and a build tool, and not only programs of a few files. It collects the
parts already planned elsewhere and adds what no other document covers yet.

## Where it stands

Whole-program mode already handles programs of several files and packages: `--main` names the
class that starts the program, `--source` the source roots, `--library` roots read only for what
is reached (the JDK's `java.base`). Packages become modules, written as a file per package
(`--crate`). The multi-file benchmarks of
Are We Fast Yet (DeltaBlue, Havlak, CD, Richards and others) translate this way and match Java.

What it does not do yet: scale far past a few thousand reached classes, write more than one
crate, read a build tool's project, carry resources, or make a library without a `main`.

## 1. The translator's own speed and memory

Planned in [translator-speed-and-memory.md](translator-speed-and-memory.md). In short:

- measure time and memory per phase in both processes;
- whole-program analyses on summaries of methods, not their bodies; bodies loaded one at a time
  for translation and dropped;
- javac releases what it has exported, a compact transfer format instead of JSON, interned
  names and compact trees in the translator;
- every core, and lowering redone only where a generics attempt changes something;
- reaching less: library dispatch of `toString`, `equals` and `hashCode` limited to the classes
  whose objects can reach the call;
- incremental translation: after a change, redo only what depends on it.

The risk to watch is the whole-program fixpoints (points-to, exceptions, reachability): their
cost must grow close to linearly with the program, or a project ten times larger takes far more
than ten times longer.

## 2. The shape of the output

Today a whole program is one crate with a file per package. For a large project rustc becomes
the bottleneck: one crate of millions of lines takes an hour or more and a lot of memory to
build, and the files do not change that (rustc builds a crate as a whole, whatever its files).
How far a split into crates can go is in [several-crates.md](several-crates.md): the enums of the
class hierarchies and Java's package cycles keep most of a program in one crate.

- **The translated JDK as its own crate** (also step 1 of [natural-rust.md](natural-rust.md)).
  It cannot simply be built once and reused: the translation sees the whole program, so the JDK's
  translated code depends on the program's classes (a program class that implements `Comparable`
  or extends `AbstractList` is one of the cases in the JDK's dispatch, and the analyses' facts
  about JDK methods depend on what the program passes them). It is reused between translations
  of the same program, and between programs only when what it depends on is the same, or where
  the dispatch at the boundary goes through a table of functions that the program fills in.
- **One crate per library and per group of packages**, so rustc builds them in parallel and a
  change rebuilds only the crates it touches. The cut follows the dependency graph between
  packages; cycles between packages are common in Java and must stay inside one crate.
- **One file per package inside each crate** (done for the one crate), so the output is
  readable and editors cope; a file per class for very large packages is a different layout.
- **Fewer copies of generic code**: real Rust generics are copied for every type they are used
  with, which multiplies compile time; the erased fallback is the tool to keep that in check
  where a generic class is used with many types.

## 3. Reading a real project

None of this exists yet.

- **Build tools.** Read a Maven or Gradle project (or a plain list of source roots and
  dependencies) to find the sources, the dependencies and the main classes, instead of passing
  each root by hand.
- **Dependencies need source.** Most libraries publish source jars next to their binaries
  (Maven Central has them), and those go in as `--library`. A dependency published only as a
  binary cannot be translated; decompiling it is a separate step the user chooses, because
  decompiled code is not always valid Java.
- **Generated sources.** Annotation processors (Lombok, AutoValue, Immutables, MapStruct,
  Dagger) generate or change code at compile time. Running javac with the project's processors
  and translating what it produces covers them, because the translator reads javac's own
  attributed trees. Processors that are loaded at run time are a different matter (see limits).
- **Resources.** Files on the class path (configuration, templates, data, images) read with
  `getResourceAsStream` end the program today. A project needs them embedded into the binary
  (`include_bytes!`) or installed next to it, chosen per project.
- **Several entry points.** A project with several `main` classes becomes one binary each,
  sharing the crates. A library without a `main` needs its public API as the starting points
  of reachability, and keeps its signatures as written (the "program, not a library" relaxation
  in [thorough-mode.md](thorough-mode.md) is the opposite case).
- **Tests.** JUnit finds tests by reflection at run time, which a closed world can do at
  translation time instead: a generated `main` that calls every `@Test` method, with the
  `@BeforeEach` and `@AfterEach` methods around each. Then a project's own tests check its
  translation.

## 4. Library coverage that large projects need

The 28 targets reach a small part of the JDK. Real projects lean on parts that are still on the
"later" list:

- **`java.util.concurrent`**: executors, futures, `CompletableFuture`, `ConcurrentHashMap`,
  locks and latches, `ThreadLocal`, and real atomics under them. Almost every large project uses
  an executor somewhere.
- **`ServiceLoader` as a closed world**: logging libraries (SLF4J, Log4j) and many others find
  their implementations this way. In a closed program the implementations are known at
  translation time.
- **Files, networking and processes**: `java.nio.file`, sockets, `ProcessBuilder`, each with
  hand-written natives.
- **Logging, JSON and other common libraries**: translated from their sources like the JDK, with
  their reflection resolved where the classes are known (Gson's field reflection over a known
  class, for example).

## 5. Working through a large project's blockers

A large project will not translate on the first try; the work is to burn its blockers down in
the order that unblocks the most.

- **Every blocker at once.** `--census` already runs past every rejection and counts what each
  blocks. It must become fast enough for a large project: today it reruns the analyses after
  each round of stubs, which took about an hour for the two `Scanner` programs.
- **A ranked list**: blockers by how many reached pieces each one blocks, which is the to-do list.
- **An opt-in preview build** for exploring: rejected methods become stubs that end the program
  with "not translated: <method>" if they are ever reached, so the rest of the project can be
  built and run while the blockers are being fixed. Never the default, and never shipped: it is
  still loud, but only when the code runs, so it must be asked for by name.

### javac end to end

javac is the first large target, and `scripts/javac-check.sh TRANSLATED_JAVAC [CASE...]` checks
a translated javac against the real one. Each case in `scripts/javac-check/` is a directory of
small Java files (hello world, generics, an enum with a switch, lambdas, a compile error), with
an optional `args` file of more javac arguments. Both compilers compile it with the same
arguments in their own copy of the directory, and the case passes when the class files are the
same byte for byte and so are standard output, standard error and the exit code. The real javac
is `javac` on `PATH` (or `JAVAC`), and must be the JDK whose `lib/src.zip` the translation was
made from; the translated one gets that JDK as `JAVA_HOME`, to read the platform's classes from.
`SLOT` names a command to run the translated javac under (the build-slot limiter), and
`JAVAC_CHECK_OUT` the directory that keeps each case's outputs for a look after a failure.

`scripts/javac-check.sh TRANSLATED_JAVAC --corpus [PATTERN]` runs the corpus instead, one case at a
time: each file of `corpus/features`, `corpus/threads` and `corpus/whole-program` on its own, each
file of `corpus/programs` on its own and all of them together, and each project with a `src/`
directory (`corpus/projects/*` and the multi-file `corpus/whole-program/*`) with all its sources.
A case is named by its directory and file names joined with a dash (`features-CodePoints`,
`projects-anonymous`), and `PATTERN` is a shell pattern that the names must match (`features-*`).
It prints one line for each case that differs, saying what differs (`exit`, `stdout`, `stderr`, or
the number of class files that differ, with the work directory of the case), and a summary:
`N cases, M identical, K known, J differ`. The exit status is 0 only when nothing differs.

`scripts/javac-check/known-differences` lists the cases expected to differ, one per line: the name,
then a one-line reason. They are reported as `KNOWN` and do not fail the run; a listed case that is
identical is reported as `NOTE`, to be removed from the list. The list holds the four cases whose
only difference is the class files of lone surrogate char literals (see docs/supported-java.md).

javac itself is never patched, as it is the program under test. Its census keeps the rejection
of `new DigestInputStream` in `Main.showClass` (Main.java:485), which is only reached with the
hidden `-XDshowClass` option and ends the program loudly there; translating `MessageDigest`
through the JDK's own SHA code would lift it.

## Limits

The hard limits from the project's earlier discussion apply to large projects in full: no code
loaded at run time (plugins, annotation processors loaded at run time, Java agents), no free
reflection or runtime proxies (Spring, Hibernate, Mockito and most dependency-injection
frameworks), no runtime bytecode changes (Minecraft mods' Mixin), and JVM-specific behaviour
(garbage collector timing, stack depth, identity hash codes) matched only as far as the rest of
this project already does.

## Order

1. Measure time and memory per phase.
2. Output in several crates, with the JDK as its own crate (several files: done).
3. Summaries instead of bodies, and the compact transfer format.
4. Every core; reaching less.
5. Reading a project: build tool, source jars, generated sources, resources, several entry points.
6. `java.util.concurrent` and `ServiceLoader`.
7. A real project of some ten thousand lines as a target, then larger ones, before anything the
   size of the Minecraft server.
