# Translating large multi-file projects (planned)

What has to change before the translator can take a real project of thousands of files, with
dependencies, resources and a build tool, and not only programs of a few files. It collects the
parts already planned elsewhere and adds what no other document covers yet.

## Where it stands

Whole-program mode already handles programs of several files and packages: `--main` names the
class that starts the program, `--source` the source roots, `--library` roots read only for what
is reached (the JDK's `java.base`). Packages become nested modules. The multi-file benchmarks of
Are We Fast Yet (DeltaBlue, Havlak, CD, Richards and others) translate this way and match Java.

What it does not do yet: scale far past a few thousand reached classes, write more than one file,
read a build tool's project, carry resources, or make a library without a `main`.

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

Today everything goes into one `main.rs`. For a large project rustc becomes the bottleneck: one
crate of millions of lines takes an hour or more and a lot of memory to build.

- **The translated JDK as its own crate**, built once and reused (also step 1 of
  [natural-rust.md](natural-rust.md)).
- **One crate per library and per group of packages**, so rustc builds them in parallel and a
  change rebuilds only the crates it touches. The cut follows the dependency graph between
  packages; cycles between packages are common in Java and must stay inside one crate.
- **One file per package or class inside each crate**, so the output is readable and editors cope.
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

## Limits

The hard limits from the project's earlier discussion apply to large projects in full: no code
loaded at run time (plugins, annotation processors loaded at run time, Java agents), no free
reflection or runtime proxies (Spring, Hibernate, Mockito and most dependency-injection
frameworks), no runtime bytecode changes (Minecraft mods' Mixin), and JVM-specific behaviour
(garbage collector timing, stack depth, identity hash codes) matched only as far as the rest of
this project already does.

## Order

1. Measure time and memory per phase.
2. Output in several files and crates, with the JDK as its own crate.
3. Summaries instead of bodies, and the compact transfer format.
4. Every core; reaching less.
5. Reading a project: build tool, source jars, generated sources, resources, several entry points.
6. `java.util.concurrent` and `ServiceLoader`.
7. A real project of some ten thousand lines as a target, then larger ones, before anything the
   size of the Minecraft server.
