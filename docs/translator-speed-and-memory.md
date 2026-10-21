# Translator speed and memory, and javac as the goal (planned)

How fast the translator runs and how much memory it needs, how far that can be pushed, and a
goal that ties it together: translating javac itself, and making the translation use less memory
than javac does on the JVM. How fast the *translated* programs run is in
[performance.md](performance.md); that document's "Translating whole programs" section has the
history of the translator's own speed so far.

## Where it stands

Measured on 2026-10-21 on main, with a release build of the translator, in whole-program mode
against the JDK's `java.base` sources. The machine was busy (load 11 to 20 on 16 cores), so the
times are on the high side. Memory is the peak of the largest process (the java process that runs
javac, in every case).

| program | Java lines | JDK units read | reached | time | memory | Rust lines |
|---|---:|---:|---|---:|---:|---:|
| CheckedExceptions | 156 | 1 | 4 classes, 12 methods | 2.8 s | 1.0 GB | 506 |
| WordFrequency | 84 | 25 | 51 classes, 245 methods | 3.9 s | 1.15 GB | 4,316 |
| StreamPipeline | 100 | 102 | 271 classes, 1,239 methods | 9.3 s | 1.8 GB | (rejected late) |
| TextAdventure | 148 | 592 | 1,188 classes, 7,619 methods | 14.6 s | 2.5 GB | (rejected late) |

Phases, from `--timings`:

| program | javac | reachability | lowering | whole objects | analyses | codegen |
|---|---:|---:|---:|---:|---:|---:|
| CheckedExceptions | 2.76 s | 0.01 s | 0.01 s | 0.00 s | 0.00 s | 0.00 s |
| WordFrequency | 3.42 s | 0.08 s | 0.21 s (2 attempts) | 0.03 s | 0.05 s | 0.07 s |
| StreamPipeline | 4.71 s | 0.33 s | 1.37 s (4 attempts) | 1.36 s | 1.34 s | — |
| TextAdventure | 8.85 s | 2.35 s | 2.85 s (2 attempts) | — | — | — |

rustc then builds WordFrequency's 4,316 lines in about 2.7 s (release, incremental, runtime crate
already built).

What this shows:

- **javac is most of the time and all of the peak memory.** Starting the JVM and attributing the
  program's own file costs about 2.5 s and 1 GB even for a program that reaches no JDK unit.
- **The size of a translation is set by how much of the JDK the program reaches**, not by the
  program: all four are 100 to 160 lines. `Scanner` and text formatting reach locales, calendars
  and regular expressions, 592 units.
- **The translator runs on one core.** CPU time is about wall time (10.4 s against 9.3 s for
  StreamPipeline).
- **Lowering is repeated whole.** Each attempt of the Rust generics lowers the whole program again:
  four times for StreamPipeline.

### Methods that can run

Reachability over javac's trees has no points-to facts: a call of `toString`, `equals` or
`hashCode` on an `Object` in library code reaches the override of every class, so a rejection
anywhere in that code rejected the program. Lowering now goes on past rejections, and a walk over
the lowered program (`analysis/live_methods.rs`) keeps only the methods that can run. It uses the
points-to facts of `object_values`: a call goes to the classes whose objects can reach its
receiver, and an instance method runs only once some running code creates an object of its class.
A rejection counts only where it can run. The others get a body that panics, `no object of X is
created`, and the instance field initializers of classes that no running code creates are dropped.
Where the facts don't cover a call, the walk gives up and every rejection counts as before.

Measured on 2026-10-21 against main at 0710e6e, release builds, load 4 to 11 on 16 cores:

| program | JDK units read | reached | methods that can run | rejections, all reached code | rejections that can run | time | memory |
|---|---:|---|---:|---|---|---:|---:|
| ScannerGrades, before | 624 | 1,222 classes, 7,872 methods | — | first one only | `Charset` subclass (ISO_8859_1.java:42) | 15.2 s | 2.34 GB |
| ScannerGrades, after | 624 | 1,222 classes, 7,872 methods | 3,721 of 9,072 | 220 (110 distinct) | 69 (49 distinct) | 24.0 s | 2.34 GB |
| TextAdventure, before | 625 | 1,225 classes, 7,868 methods | — | first one only | `Charset` subclass (ISO_8859_1.java:42) | 14.1 s | 2.41 GB |
| TextAdventure, after | 625 | 1,225 classes, 7,868 methods | 3,703 of 9,068 | 220 (110 distinct) | 69 (49 distinct) | 28.8 s | 2.41 GB |
| StreamPipeline, before | 102 | 271 classes, 1,239 methods | — | 0 | 0 | 10.2 s | 1.74 GB |
| StreamPipeline, after | 102 | 271 classes, 1,239 methods | 1,116 of 1,583 | 0 | 0 | 9.2 s | 1.70 GB |

(Lowered methods outnumber reached ones: lowering adds bridges, lambdas' functions and the
builders of enum tables.)

- **Two thirds of the rejections in the scanner programs are in code that can't run.** The first
  that can is now `MethodHandles.arrayElementVarHandle` in `AtomicReferenceArray`, not a `Charset`
  subclass.
- **Generated code shrinks where the program translates.** StreamPipeline's Rust goes from 22,173
  to 18,516 lines, and whole objects, analyses and codegen run 30 to 40 % faster (4.7 s to 3.1 s
  together). Its output is the same.
- **Units read and reached classes don't change.** The narrowing happens after lowering. Reaching
  less in javac, and so cutting its memory, needs the same facts during reachability (step 7
  below).
- **Lowering costs more on programs that are rejected.** It used to stop at the first rejection,
  and now lowers everything (3 to 5 s against 11 to 16 s). The walk itself takes 2 to 3 s.

## Steps for the translator

In order of payoff for the effort:

1. **Use every core.** Lowering, codegen and the per-method parts of the analyses are independent
   per method or class, so they can run in parallel. The whole-program fixpoints (points-to,
   exceptions, reachability) are harder and come later.
2. **Lower again only what an attempt changes.** A failed generics attempt should redo the methods
   it affects, not the program. Up to 4x on lowering for StreamPipeline.
3. **Keep javac running.** `JavaTree --serve` already exists; a server kept between translations
   removes the 2.5 s and 1 GB start-up per program. This is what makes test and target runs fast.
4. **A compact tree format.** The trees go from javac to the translator as JSON and are held whole.
   A binary format, read as needed, and trees dropped once their methods are lowered, cut the
   translator's peak several times over.
5. **The JDK translated once.** Together with step 1 of [natural-rust.md](natural-rust.md) (the
   translated JDK as its own crate): most of a program's JDK part is the same from one program to
   the next. Because the analyses look at the whole program, the JDK's translation depends a little
   on the program (which overrides exist, which classes are created), so this is a cache with a
   check of what it depends on, not a free win.
6. **Incremental translation.** After a change to one file, redo only what depends on it. This is
   what large projects need for a usable edit-translate-build loop.
7. **Output split into crates.** For large programs rustc is the bigger cost: one crate of millions
   of lines takes an hour or more for a release build. One crate per library or group of packages
   builds in parallel and rebuilds only what changed.

## Memory for large projects

Today a translation holds the whole reached program in memory three times over:

1. **In javac**, every attributed unit with its trees and symbols. This is the largest part today
   (2.5 GB for TextAdventure).
2. **In transit**, as JSON text that spells out every name and node kind in full (356 MB for the
   2,500 JDK units javac used to attribute for every program).
3. **In the translator**, as `serde_json` values (about ten times the text), then the lowered
   program, the analyses' results and the output text.

Peak memory grows with the code of everything reached. The goal is that it grows with summaries
of that code, plus the few methods being worked on at a time. In order:

1. **Measure first.** `--timings` prints the time of each phase; it should also print the peak
   memory of each, in both processes, so the largest consumer is fixed first.
2. **Summaries instead of bodies.** Most of the translator needs to know *about* a method (what
   it calls, which objects it can return, which exceptions it can raise), not its code. Read a
   method, record its summary, drop its body; run the whole-program analyses on the summaries;
   then load each method's body again, translate it, write it out and drop it. Compilers that
   optimize whole programs at scale work this way (ThinLTO's summaries).
3. **javac forgets what it has handed over.** Once a method's tree is exported, javac needs only
   its symbol and signature; the body can be released, as javac's own compile policy does after
   generating a class's bytecode.
4. **A compact transfer format.** A table of names and types written once and referred to by
   number, nodes as small binary records, streamed through the pipe instead of built as one
   string. Likely five to ten times smaller than JSON.
5. **Compact data in the translator.** Typed structures read straight from that format instead of
   `serde_json` values; names and types interned as 32-bit ids; tree nodes in large blocks
   instead of one allocation each.
6. **Output written as it is made**, straight to each crate's files, instead of one string of
   millions of lines.
7. **Reaching less.** Dispatch of `toString`, `equals` and `hashCode` from library code reaches
   every live class's override, which pulls in large parts of the JDK that the program never runs
   (the thread pool, currencies, the file system, stack walking). After lowering, the points-to
   facts now cut this down to the methods that can run (see "Methods that can run" above). Using
   the same facts during reachability, so that javac never reads those units, would shrink all
   three copies.

Expected effect: several times less peak memory; for a project the size of the Minecraft server,
the difference between the 20 to 40 GB estimated below and a few GB. That is an estimate until
step 1 measures it.

## At scale: the Minecraft server, roughly

Assuming every feature it uses were supported, which is not yet the case. The dedicated server is
several thousand classes, plus its libraries (Netty, Guava, fastutil, DataFixerUpper and more) and
a large part of the JDK: likely 10,000 or more units and 100,000 to 200,000 reached methods, 15 to
25 times TextAdventure.

- **Today:** probably tens of minutes, and 20 to 40 GB of memory, which is the first wall.
- **After steps 1 to 4:** a few minutes and under about 8 GB is a realistic target.
- **rustc:** millions of lines of output; with step 7, roughly 10 to 30 minutes for a release build.

These are estimates from four data points, not measurements.

## The goal: javac, translated, in less memory

javac is the `jdk.compiler` module: 369 files, about 7 MB of source, with its source in the JDK the
translator already reads. As a target it is better than most large programs: no graphics, no
network, mostly one thread, pure computation, and its output (class files, or the trees that
`JavaTree` writes) can be compared byte for byte with the real javac.

### Milestones

1. **javac translates and agrees with itself.** The translated javac compiles a fixed set of
   sources (the corpus) and its output matches the JVM javac's byte for byte. Annotation processors
   and compiler plugins are left out: they are loaded at run time, which a closed-world translation
   cannot do. javac reads the platform's classes from the JDK's `lib/modules` image through the
   `jrt` file system, which is Java code (`jdk.internal.jimage`) and translates like the rest.
2. **It uses less memory than javac on the JVM**, on the same inputs, measured as peak memory.
3. **The translator uses it.** `JavaTree` and javac, translated, replace the JVM in the
   translator: one native binary, no JVM to start, no JSON in between.

### Why less memory is a matter of a smarter translator

Translating javac as the translator works today would not use less memory, and would likely use
more:

- **javac's data is full of cycles.** A class symbol points to its members and each member back
  to its owner; types point to symbols and symbols to types. The JVM's garbage collector frees
  cycles; the translation's reference counting (`Rc`) does not, so cycles leak.
- **Every object carries a counter and often a `RefCell`**, and every reference to a class tree
  is an enum around an `Rc`.

The rule is that every improvement is a general analysis in the translator, never a special case
for javac, so that every program gains from it and javac is the benchmark that shows it. The
analyses that get there are mostly the ones planned in [thorough-mode.md](thorough-mode.md) and
[natural-rust.md](natural-rust.md):

- **Lifetimes, and arenas.** Most of javac's symbols, types and trees live until the compilation
  ends. Objects proven to live as long as a phase or the whole run go into an arena that is freed
  in one step: no counter per object, no cycle problem, no garbage collector.
- **Freeing early.** A tree that is provably not used after a phase is dropped when the phase
  ends, instead of staying reachable as it does in javac.
- **Compact layout.** No per-object header, as the JVM has. Small enums stored inline, and
  references into an arena as 32-bit indices instead of 64-bit pointers.
- **Ownership.** An object with a single owner is stored inline in it, without `Rc` or `RefCell`.

### Measuring

A table that is kept up to date: javac on the JVM against the translated javac, compiling the same
inputs (the corpus, then a large real project), with peak memory and time for each. The first row
is "does not translate yet".

## Choosing what comes next

After the target programs pass, the two candidate tracks are this one and
[natural-rust.md](natural-rust.md). They overlap: the ownership, lifetime and arena analyses make
the output both look more like Rust and use less memory. Steps 1 to 4 above stand alone and are
safe; the shared ownership work serves both tracks.
