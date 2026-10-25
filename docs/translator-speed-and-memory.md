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
- **The translator runs mostly on one core.** Only some steps of the analyses run alongside each
  other (`performance.md`, steps of the analyses alongside each other); when measured, before
  they did, CPU time was about wall time (10.4 s against 9.3 s for StreamPipeline).
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

## Scaling to javac

Measured on 2026-10-22 with release builds, on the javac setup described under "The goal" below
and on TextAdventure, before (main at 1c62e39) and after the changes of branch
`translator-memory`. The machine had a load of about 6 on 16 cores. "Translator peak" is the
translator process's peak resident memory, "javac peak" the java process's.

### What was slow, and how it was fixed

Every whole-program analysis computes the smallest set of facts that is closed under its rules
(which methods a method reaches, which objects a variable can hold, which exceptions a call can
throw). The old code got there by going over the whole program again and again until a round
changed nothing, and some of the rules looked things up by going through every class or method.
On small programs that is quick; on javac, with 26,000 methods, the number of rounds and the cost
of each both grow with the program, so the cost grows with its square or worse.

The fixes give the same results, because the smallest closed set does not depend on the order in
which it is reached. They only change how the work is ordered:

- **Groups of methods that call each other, done once each.** What a method reaches through its
  calls is computed once per group of methods that call each other in a cycle, callees first, as
  sets of bits (`analysis/closure.rs`). This replaced rounds over every call with sets of names,
  in borrowing, codegen's call chains and the exception analysis.
- **A list of what is left to do.** The points-to analysis (`object_values`) records which facts
  each method's visit read, and when a fact grows it visits again only the methods that read it,
  instead of every method in the next round. The exception analysis does the same. A method's
  variables in one context are read only by its own visit there, so they need no record: a
  change to them runs that visit again.
- **One scan instead of one per class.** Several analyses (objects that are owned, eager statics,
  ranges, the static fields of the exception guards, enum field writes in lowering) scanned the
  whole program once for each class or field; they now scan it once and keep an index.
- **Lookups that were searches.** Class hierarchy questions (the members of a class, its
  subclasses, who overrides a method) went through every class each time; they are now tables
  built once. The generator did the same for names and enum variants, and lowering for the class
  or method that a name or symbol names (`type_index`, `method_index`, which searched javac's
  3,141 classes and 27,000 methods on every lookup and were most of lowering's time). The
  same went for a method of a class by name and signature, and for the methods of a class
  (`method_named`, `methods_of`): each call redirected to the translator's own code, each
  reflection table (once for every supertype of every class), each overridden method and each
  for-each loop's iterator method searched the Minecraft server's 180,000 methods, about 70% of a
  lowering attempt. The
  implementation that a call runs on a class is kept once found (`Hierarchy::implementation`),
  as the analyses and the generator ask for it once for every concrete class under a type.
- **The reflection table, made once per lowering attempt.** Each of the 400 or so natives of
  `java/Reflection.java` (`tableInts`, `invokePart`, `getPart` and the rest, several for each
  of the 32 parts) asked for the whole table again, reading the types of every member of every
  class that reflection can reach: on javac, about half of each lowering attempt. Lowering now
  keeps each table it makes (`Lowering::table`), keyed by what the table reads that can change
  during an attempt: whether a census's stub is being lowered, and for fields the type
  variables in scope where the table is asked for. What making a table records (covariant
  arrays, Rust generics that fail, rejected classes) its first making records, so making it
  again would add nothing. javac's five lowering attempts went from 21.1 s to 13.8 s.
- **javac once for a census.** A census is taken again when the reflection table misses classes
  that its lookups can get, and with the writes of `String` arrays once one of them can run
  (`program/census.rs`); each time it loaded the program again, and javac attributed the same
  units again: three times on javac's census. A load now takes the units that javac gave the
  census's earlier load, round by round, for as long as its walk asks for what that load's
  rounds asked for, and starts javac only when a round asks for something else, asking it first
  for what the rounds before asked (`program::Earlier`). On javac's census both later loads take
  all 18 rounds: javac runs once, and the census goes from 317 s to 231 s, 488 to 295 s of CPU
  and 1,862 G to 1,103 G instructions (the JVM's with the translator's), with the same crate,
  tree and report. The Minecraft server's census loads once, so it gains nothing.

`whole_objects`, the analysis the old census never got past on javac, was slow for the first
three of these reasons: the borrowing rounds over name sets, a scan of the program per class for
the objects each class owns, and hierarchy searches inside both. It went from 193 s to 0.6 s on
TextAdventure, and from more than 55 minutes of CPU (never finished) to about 2 s on javac.

### Memory

- **javac streams the trees and lets go of each unit once sent.** `JavaTree` used to build the
  whole output as one string and keep every tree; it now writes each unit as it is made and then
  drops its method bodies. Its young generation is capped (`-XX:MaxNewSize=128m`), so the garbage
  collector runs before the heap grows. javac's peak on javac went from 2.31 GB to 0.66 GB.
- **The translator drops the trees once lowered**, and the census drops them and the
  reachability results before the analyses. This doesn't lower the peak, which is reached when
  the trees arrive (318 MB of JSON become about 3 GB of `serde_json` values, 9 to 10 times the
  text), but it frees that memory for the rest of the run. (The process's resident size doesn't
  shrink afterwards, because the allocator keeps freed memory, so "now" in `--timings` stays at
  the peak.)
- **The trees are held as compact values** (`value.rs`), not `serde_json` values. A `serde_json`
  object is a `BTreeMap` whose smallest node has room for eleven fields, each name a `String` of
  its own, so even a node of a few fields takes over half a kilobyte. Here an object is a sorted
  list of its fields and every string, name or value, is interned once for the whole run. On
  javac's census the peak after loading went from 3.15 GB to 1.07 GB (328 MB of JSON), and the
  peak of the whole run from 3.22 GB to 1.43 GB; the census and the corpus's translations are
  the same as before. A value is 16 bytes and a field 24: an interned string is one pointer, to
  the place that holds its text's address and length, and an array's elements and an object's
  fields are behind one pointer to their length and items (`thin_vec::ThinVec`), read into a
  list of their exact length. With a `&str` and `Vec`s, a value took 32 bytes and a field 48:
  javac's census (435 MB of JSON, 3.7 million objects with 21 million fields) held 1.73 GB
  after loading and peaked at 2.25 GB; it now holds 0.87 GB and peaks at 1.40 GB, with the same
  translation. The names of an object's fields are held once for every object with the same
  names, and an object whose values are all scalars once for all objects equal to it; with them
  and the shared copies below, javac's whole census peaks at 1.18 GB (2.22 GB before all four,
  the same translation), at about 5% more instructions (478 G against 455 G).
- **The analyses hold one copy of what many methods share.** On the Minecraft server three of
  them grew the census by gigabytes after the trees were gone. The arrays analysis copies each call
  to every method of the callee's override group; the copies now share the call's arguments
  (`Rc<[Arg]>`), where each held its own sets of names (a call of `toString` runs thousands of
  methods). The exceptions analysis keeps what escapes each method as bits over the numbered kinds
  alone, not also as a set of kinds per method, and numbers the types that some `catch` names
  densely, so that what a method's callers observe is a few words rather than one for every 64
  types below the highest caught one. Borrowing's reach (`closure::reached_union`) holds the union
  of each strongly connected component once, not a copy for each of its methods.
- **Not done: lowering the generics attempts only where they change.** Each attempt lowers the
  whole program again from the same trees, which are already held, so no tree is read twice. The
  scans of the trees whose answers don't depend on the attempt are made once and handed on to the
  next attempt (`lower/scans.rs`): the standard classes and functional interfaces that the code
  uses, what of `Object` it needs, the names of each class's variables, and in each member its
  local and anonymous classes, its lambdas and what it asks about its caller. That took an
  attempt after the first on javac from 6.3 s to 3.5 s. What costs is the lowering of the methods
  itself. Redoing only the methods an attempt affects means tracking which methods depend on which
  generics decision, which is not a small change.
- **Not done: a compact transfer format.** A unit's JSON repeats few strings (239 distinct ones
  among 3,075 in a typical unit), and node kinds, types, owners and names are about 65% of the
  bytes, so a table of strings would make the text several times smaller. But the peak comes from
  the `serde_json` values the text becomes, not from the text, so a smaller wire format alone would
  not move it. The first step that pays is reading typed nodes straight from the stream (step 5 of
  "Memory for large projects"), and that is a rewrite of how the translator reads trees.

### Before and after

javac, translated (`-o`, not a census). Both end the same way: 2,800 of 25,945 methods can run,
then a rejection (`Runtime.Version`, at JarFile.java:142).

| phase | before | after |
|---|---:|---:|
| javac | 31.0 s | 32.4 s |
| reachability | 24.5 s | 25.3 s |
| lowering | 147.4 s (5 attempts of 29.5 s) | 107.7 s (5 of about 21 s) |
| methods that can run | 34.7 s | 20.5 s |
| javac peak | 1.38 GB | 0.66 GB |
| translator peak | 3.01 GB | 3.01 GB |

javac, census (`--census`):

Getting the code and lowering it cost the same as above (31.1 s for javac, 22.1 s for
reachability and 97.9 s for lowering, after).

| phase | before | after |
|---|---:|---:|
| whole objects | never finished (more than 55 min of CPU) | 2.3 s a run |
| analyses and generation | — | 2,164 s: 118 rounds of the analyses (1,403 s) and 97 of the generator (654 s) |
| result | none | 95.8% of 26,607 pieces translate (below) |
| javac peak | 1.38 to 2.31 GB | 0.66 GB |
| translator peak | 3.01 GB | 3.01 GB |

The census takes about 39 minutes in all. It is long because the generator stops at its first
panic, and so runs once for each piece of code that makes it panic (97 rounds of 6 to 7 s; each
was 115 s before), and the analyses likewise run again after each rejection they cannot go past
within a round (118 rounds of about 12 s).

TextAdventure, translated. Both end at the same rejection (a `Charset` subclass, at
ISO_8859_1.java:42); 135 MB of trees.

| phase | before | after |
|---|---:|---:|
| javac | 16.3 to 17.4 s | 16.4 to 16.5 s |
| reachability | 2.8 s | 2.9 s |
| lowering | 21.7 s (4 attempts of 5.4 s) | 13.5 s (4 of 3.35 s) |
| methods that can run | 4.7 to 5.0 s | 2.3 s |
| javac peak | 0.59 to 2.53 GB | 0.52 GB |
| translator peak | 1.25 GB | 1.25 GB |

(Without the cap on the young generation, javac's peak varies a lot from run to run.)

TextAdventure, census:

| phase | before | after |
|---|---:|---:|
| lowering | 22.0 s | 13.5 s |
| whole objects | 380 s | 0.65 s |
| analyses and generation | 9,320 s (23 rounds of the analyses, about 390 s each), then a rejection it could not go past | 326 s (70 rounds of the analyses, about 3 s each, and 61 of the generator) |
| result | stopped at Pattern.java:1670, with 98.4% of the pieces counted so far | complete: 96.2% of 10,044 pieces translate |
| javac peak | 2.41 GB | 0.52 GB |
| translator peak | 1.34 GB | 1.25 GB |

The outputs are the same before and after: every target program's translation and census gives
the same text.

### Round two

- **Summaries instead of bodies** (step 2 of "Memory for large projects"). The translator's peak
  is now all trees. Keeping a summary of each method and its lowered form, and dropping the
  `serde_json` values unit by unit as they are lowered, would bring it down to what the lowered
  program needs; reading typed nodes from the stream would do it without the values at all.
- **javac freeing more.** javac keeps its symbols and types to the end, and the attributed trees
  of what it has not yet sent. Attributing and sending one unit at a time, rather than all of them
  first, would keep only the symbols.
- **Narrowing by points-to facts during reachability** (step 7 of "Memory for large projects").
  Of javac's 25,945 lowered methods, 2,800 can run. Using the points-to facts while deciding what
  to read would mean javac never attributes most of the rest, which shrinks every copy of the
  program and every phase after it.
- **A census that doesn't rerun the generator per panic.** Catching a panic per piece of code,
  instead of per round, would cut the 97 generator rounds on javac to one.

## Steps for the translator

In order of payoff for the effort:

1. **Use every core.** Lowering, codegen and the per-method parts of the analyses are independent
   per method or class, so they can run in parallel. The generator can already write a round's
   modules on several threads, but does so only when asked, until the allocator lets its threads
   gain (see "Generator threads" in [performance.md](performance.md)). The whole-program
   fixpoints (points-to, exceptions, reachability) are harder and come later.
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

1. **Measure first.** Done: `--timings` prints the time of each phase and the peak memory so far,
   the translator's and javac's.
2. **Summaries instead of bodies.** Most of the translator needs to know *about* a method (what
   it calls, which objects it can return, which exceptions it can raise), not its code. Read a
   method, record its summary, drop its body; run the whole-program analyses on the summaries;
   then load each method's body again, translate it, write it out and drop it. Compilers that
   optimize whole programs at scale work this way (ThinLTO's summaries).
3. **javac forgets what it has handed over.** Once a method's tree is exported, javac needs only
   its symbol and signature; the body can be released, as javac's own compile policy does after
   generating a class's bytecode. Partly done: `JavaTree` streams the trees and releases each
   unit's method bodies once sent (see "Scaling to javac").
4. **A compact transfer format.** A table of names and types written once and referred to by
   number, nodes as small binary records, streamed through the pipe instead of built as one
   string. Likely five to ten times smaller than JSON.
5. **Compact data in the translator.** Typed structures read straight from that format instead of
   `serde_json` values; names and types interned as 32-bit ids; tree nodes in large blocks
   instead of one allocation each. Done for the points-to analysis for dispatch, which follows
   each instance method once for each concrete class that runs it, and whose facts took the
   Minecraft server's census from 8.5 GB past 16 GB: a set of classes keeps only the words from
   its lowest class to its highest, a method's variables in a context are a vector with a slot
   for each, not a map from names, and its maps keyed by indices use a cheap hash.
6. **Output written as it is made**, straight to each crate's files, instead of one string of
   millions of lines.
7. **Reaching less.** Dispatch of `toString`, `equals` and `hashCode` from library code reaches
   every live class's override, which pulls in large parts of the JDK that the program never runs
   (the thread pool, currencies, the file system, stack walking). After lowering, the points-to
   facts now cut this down to the methods that can run (see "Methods that can run" above). Using
   the same facts during reachability, so that javac never reads those units, would shrink all
   three copies.

Expected effect: several times less peak memory. The Minecraft server's census peaks at 11.6 GB
(below); these steps aim at a few GB.

## At scale: the Minecraft server

The census of the dedicated server and its libraries ([minecraft-server.md](minecraft-server.md)),
measured on 2026-11-21 on main at ccd8091c with a release build of the translator: javac gives
15,523 units (2,306 MB of trees), reachability finds 27,041 classes and 208,131 methods in 18
rounds, and lowering makes 288,097 methods, of which 259,612 can run. The translator's peak is
11.6 GB, javac's 2.6 GB.

| phase | time |
|---|---:|
| javac | 74 s |
| reachability | 47 s |
| lowering | 115 s, 75 s of it finding the methods that can run |
| the analyses and the generator (`analyze`) | 186 s |

In `analyze`, the analyses take 93 s (several run alongside each other) and the generator 66 s.
The escape analysis (`analysis/escape.rs`) was the longest of the analyses, 7,967,485 solves of
a method in 237 s, while it solved the methods of each component of the calls smallest first.
Solved in rounds, it makes 531,725 solves in 20 s, and `analyze` went from 329 s to 186 s.

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

Translating javac as the translator works today does not use less memory: every object often
carries a `RefCell`, and every reference to a class tree is an enum around an 8-byte handle,
16 bytes against the JVM's compressed 4. docs/native-speed.md plans the object model that
removes both.

The rule is that every improvement is a general analysis in the translator, never a special case
for javac, so that every program gains from it and javac is the benchmark that shows it. The
analyses that get there are mostly the ones planned in [thorough-mode.md](thorough-mode.md) and
[natural-rust.md](natural-rust.md):

- **Lifetimes, and arenas.** Most of javac's symbols, types and trees live until the compilation
  ends. Objects proven to live as long as a phase or the whole run go into an arena that is freed
  in one step, with nothing for the collector to trace.
- **Freeing early.** A tree that is provably not used after a phase is dropped when the phase
  ends, instead of staying reachable as it does in javac.
- **Compact layout.** No per-object header, as the JVM has. Small enums stored inline, and
  references into an arena as 32-bit indices instead of 64-bit pointers.
- **Ownership.** An object with a single owner is stored inline in it, without a handle or `RefCell`.

### Measuring

A table that is kept up to date: javac on the JVM against the translated javac, compiling the same
inputs (the corpus, then a large real project), with peak memory and time for each. The first row
is "does not translate yet".

#### javac

Measured on 2026-10-21 with a release build, on the JDK 26 sources (`jdk.compiler` as the program,
`java.compiler`, `jdk.internal.opt` and `java.base` as libraries, all with their `module-info.java`),
running `--census`:

- **Getting the code**: javac attributes 1599 units in 14 to 17 s; reachability takes 14 to 19 s
  over 15 rounds and reaches 3405 classes and 21,539 methods (the quicker `--survey` counts
  26,102 pieces of code: 11,358 of javac's own and 14,744 of the libraries').
- **Lowering** (2026-10-23, 3,141 classes reached): five attempts (the Rust generics are narrowed
  down between them) of 8 to 31 s, 108 s in all on a quiet machine; then 22 s to find the methods
  that can run. The whole census takes about 6 minutes: 4 rounds of the analyses of about 20 s
  each (one more for each round in which the analyses or the generator rejected something that
  only they find), and the generator's last, complete run of about 80 s.
- **The census, complete** (2026-10-22, see "Scaling to javac"): of javac's own 11,358 pieces of
  code, 10,755 (94.7%) translate; of the libraries' 15,249, 14,742 (96.7%); together 25,497 of
  26,607 (95.8%). It reaches 143 native methods, 43 of them in the runtime. What holds javac's own
  code up, by the stage that rejects it:
  - the analyses: catching an exception that a `toString` may throw (198 pieces), catching the
    `NullPointerException` of a dereference (88), catching an exception that a library call may
    throw (71), constructors of a class whose subclasses use `this` or call overridable methods
    (40), a `StringBuilder` that threads may share (25), a static initializer that throws inside
    a `try` (21), a `toString` that throws a caught exception (18), an exception's constructor
    that throws (16), `==` on `Object` values that may be strings or boxes (9), catching the
    exception of an operation (9);
  - lowering: `Throwable.printStackTrace` and its relatives (13), `ClassLoader` and other types
    (13), reflection (9);
  - the generator: a call that lowering left unresolved (`acquireFence`, 8) and 4 pieces where it
    panicked.

  The libraries' code: native methods (87), catching around `toString` (59) and library calls
  (42), types (53) and methods (44) of the runtime that are not there yet, `Unsafe` accesses
  whose offset is not a field's (36), reflection (23), implementing (21) or extending (13) a
  class that is not supported, values set by the JVM (15), a `StringBuilder` that threads may
  share (10).
- **What lowering alone rejects** (a census that stopped after lowering, before the census could
  go past rejected classes): 99.1% of javac's own 11,358 pieces of code and 97.3% of the libraries'
  15,253 lower. javac's own code is held up most by method references on a computed receiver
  (29 pieces), reflection (16), `Throwable.printStackTrace` and its relatives (13) and
  `ClassLoader` as a type (12); the libraries' by methods of the runtime's classes that are not
  there yet (102, such as `CharSequence.subSequence` and `String.codePointAt`), native methods
  (95, `Unsafe.park` and `unpark` first), `Unsafe` accesses whose offset is not a field's (60) and
  `ClassLoader` as a type (28).

## Choosing what comes next

After the target programs pass, the two candidate tracks are this one and
[natural-rust.md](natural-rust.md). They overlap: the ownership, lifetime and arena analyses make
the output both look more like Rust and use less memory. Steps 1 to 4 above stand alone and are
safe; the shared ownership work serves both tracks.
