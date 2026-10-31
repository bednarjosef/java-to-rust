# Several crates

A whole program is one Rust crate by default, with a file per Java package. With
`--crate DIR --crates N` it is a Cargo workspace instead: a header crate, `N` body crates and the
program's binary (`crates/translator/src/split.rs`). This note says why a large program needs that,
how the split works, and what splitting along classes would give instead, which is what ruled that
out.

## Why split at all

rustc compiles a crate as one unit. Several crates build in parallel, and after a change only the
crates that the change touches, and those that depend on them, are built again. Files do not do
this: rustc reads the files of a crate into one tree before it does anything else, so a crate in
one file and the same crate in a file per package cost the same. Measured on the three largest
whole programs that translate today (built with `cargo build`, the runtime crate already built,
best of three runs, peak memory of the rustc process):

| program | Rust lines | layout | debug | release | peak memory |
|---|---:|---|---:|---:|---:|
| StreamPipeline | 18,700 | one file | 2.9 s | 6.1 s | 606 MB |
| | | 8 files | 3.3 s | 6.1 s | 615 MB |
| PathsAndDirectories | 15,700 | one file | 2.4 s | 4.3 s | 498 MB |
| | | 17 files | 2.5 s | 4.7 s | 488 MB |
| FileContentsThroughFiles | 15,400 | one file | 2.5 s | 4.3 s | 508 MB |
| | | 25 files | 2.3 s | 4.2 s | 506 MB |

The differences are within the noise of a machine that other builds share. For a program the size
of javac, which will be hundreds of thousands of lines, the build time and memory of one crate are
what a split would cut.

## What keeps code in one crate, split along classes

Crates cannot depend on each other in a loop: if crate A names something in crate B, B cannot name
anything in A. So any code whose parts name each other in a loop, directly or through others,
must be in one crate. A translation has three sources of such loops.

1. **The enums of class hierarchies.** A class with subclasses becomes an enum with a variant per
   subclass, and a virtual call is a `match` over it ([object-model.md](object-model.md)). The
   parent's enum names every subclass; every subclass names its parent (its fields, `super`
   calls, the enum it is a variant of). So a hierarchy is one loop, whatever packages its classes
   are in.
2. **`Object`, the largest hierarchy.** The crate root's `Object` enum has a variant for every class
   whose objects are ever held as an `Object`, and erased generics hold every element of a list
   and every key and value of a map as an `Object`. So nearly every class that is put in a
   collection is in `Object`'s loop, together with the code that names `Object`, which is nearly
   all code. `Throwable` and its exceptions are a second, smaller loop of the same kind.
3. **Java's own loops.** Java lets classes and packages name each other in loops, and real code
   does: `Stream` creates a `ReferencePipeline`, which implements `Stream`; `java.util` and
   `java.util.stream` use each other; javac's `code`, `comp`, `tree` and `util` packages all use
   each other. Inside a crate Rust allows this, between crates it does not.

## Finding the groups that can be split off

The dependency graph of the generated code is known exactly: the name resolver
(`codegen/modules.rs`) sees every name of another class that the generator writes, and records
it with the item it is written in. `--modules-json FILE` writes it, for a translation or, with
`--census`, for a program that does not translate yet (each rejected method is then a stub that
names nothing, which leaves out a few edges).

- **Nodes**: top-level classes (a nested or inner class stays with its top-level class), or
  packages. Classes find more and smaller groups; packages are what a reader expects to find in a
  crate. A package can be split off only together with its whole group.
- **Edges**: class A to class B when an item of A names B.
- **Groups**: the strongly connected components of that graph (Tarjan's algorithm, linear in the
  size of the graph). Everything in a group must be in one crate. The groups form a graph without
  loops, which gives the order of the crates: a group's crate depends on the crates of the groups
  it names.
- **Crates**: one per group would be thousands of tiny crates, which costs more than it saves. The
  groups are merged into crates along that order (the small groups that only the large group uses
  join it; a group that many others use, such as a library's leaves, is a crate of its own), up to
  a size where a crate is worth its own build.

`scripts/crate-groups.py MODULES_JSON` prints the groups over classes and over packages, as the
generator wrote the code and without the edges of the hierarchies' enums (what dispatch through
tables of functions would leave; see below).

## What a split would buy for javac

Measured on the code javac's census generated before the translation built, with every rejected
piece stubbed. The translation that builds is larger (below), but its shape is the same:

317,000 lines of Rust in 140 packages: 140,000 lines of javac's own code (`com.sun.*`,
`javax.*`, 27 packages) and 177,000 of the JDK and the crate root (113 packages).

| | largest group | share of the lines | the rest |
|---|---|---:|---|
| top-level classes, as generated | 1,853 of 2,631 classes | 91% | 773 groups, the largest 1,557 lines, 770 of a single class |
| packages, as generated | 107 of 140 packages | 100% | 33 packages of a few hundred lines |
| top-level classes, without the hierarchies' enums | 1,190 classes | 72% | `Object`'s 16,000 lines alone, then groups under 3,300 lines |
| packages, without the hierarchies' enums | 101 packages | 99% | |

And if the JDK were a crate of its own, with every name of a javac class in JDK code going
through tables (the boundary below), each side by itself:

| | classes, as generated | classes, without enums | packages |
|---|---:|---:|---:|
| javac's own code (140,000 lines) | 89% in one group | 82% | 22 of 27 packages, 100% |
| the JDK and the root (177,000 lines) | 89% | 61% | 83 of 113 packages, 99% |

What this says:

- **As the code is generated, a split buys nothing**: nine tenths of javac's translation is one
  loop, through `Object`, `Throwable` and the other hierarchies' enums, which name javac's
  classes from the root and the JDK's code. The groups outside it are single classes at the
  edges (a pretty-printer, an attribute table), too small to be worth a crate.
- **Without the enums it is still one large crate**: Java's own loops keep 72% together over
  classes and 99% over packages. javac's packages use each other in one loop (`code`, `comp`,
  `tree`, `util`, `jvm`, `main` and 16 more), and so do the JDK's (`java.util`,
  `java.util.stream`, `java.util.concurrent`, `java.util.regex`, `java.text`, ...). What does fall
  out without the enums are the packages of interfaces only, the compiler API
  (`com.sun.source.tree`, `javax.lang.model.element`, `javax.lang.model.type`, `javax.tools`):
  about 5,000 lines that could be a crate below javac's.
- **The one split worth making is the JDK from the program**: two crates of about 177,000 and
  140,000 lines that build in parallel, and a change to the program's code that does not rebuild
  the JDK's. It needs the boundary to go through tables: today JDK code names javac classes in
  the hierarchies' enums (`Object`, `Throwable`, `Comparable`, `Enum`, `java.io.Closeable` and
  others: about 2,900 names) and in about 140 other places (dispatch inside `EnumSet` and `Files`,
  `Thread` running the program's `Runnable`s). Each half stays one crate.
- Splitting further means cutting Java's own loops, which is the program's design, not the
  translator's. Inside a crate, rustc's parallel code generation is what remains.

javac's translation builds as one crate of about 516,000 lines (43 MB, beside the runtime's
crate). `cargo check` takes about 100 s with rustc at 6.2 GB at its peak; a debug build
(`debug = "line-tables-only"`, one job) takes about 210 s with rustc at 7.3 GB. That is far below
the table's 600 MB per 19,000 lines carried on linearly (about 17 GB), so rustc's memory grows
slower than the lines; the split is about build time and rebuilds after a change, not about
whether one crate fits.

## Header and body crates

Crates cannot name each other in a loop, and translated classes do, so `split.rs` does not split
along classes. It splits declarations from code, as C splits headers from sources:

- **The header crate** has every module and item of the program, made `pub`: the structs, the
  enums of the hierarchies, the statics, the traits, and every function and method. A function
  whose code can move is only declared there, in an `unsafe extern "Rust"` block of its module,
  by its own name and with the name of its code: `#[link_name = "j2r__<module>__<name>"] pub safe
  fn <name>(..)`. Callers name it as they did, and the header has no function of its own for it.
  Constructors and the methods that take their object as a parameter are such functions too,
  named after their struct (`Circle__new`, docs/object-model.md), and so are the dispatchers
  and bodies of a type's enum, which take its value (`Shape__area`). A method that takes `self`
  keeps only its signature, as Rust has no declaration of a method without its code. Its body
  calls a function declared in that block (`safe fn j2r__<module>__<type>__<name>(..)`). Both have their parameters renamed `p0`, `p1`,
  .. so that every pattern is a plain name. A function of the header that only calls a
  declaration costs rustc about three times what the declaration does: about 21 KB of memory
  against 8 KB, measured on synthetic header crates of 40,000 moved functions in a release
  build.
- **Each body crate** depends on the header alone and defines those functions for the items
  given to it: a function with `#[unsafe(no_mangle)]`, a method as below. A module of a body crate starts with
  `pub use header::<its path>::*` and has the module's own `use` lines, so moved code names
  everything as it did, `crate::..` paths included: the body crate has the modules with code of
  its own and the modules around them, and any other module is the header's, which the glob
  re-export of its parent names. It has the header's `recursion_limit`, which a
  threaded program raises for the proofs that its statics are `Send`. A method's code moves into
  a private trait that the body crate implements for the header's type, named after it (`impl
  Bodies_Circle for Circle { fn body_area(&self) .. }`), so that `self`, `Self` and calls of the type's other methods mean
  what they did; the trait is `Sized`, as the type is, for a signature that names `Option<Self>`.
  The method itself is the function the header declares, `#[unsafe(export_name =
  "j2r__..")]`: its `self` is passed as the declaration's first parameter is. (A function of
  its own that called the method cost every method a second function to check and compile, about
  3% of a body crate's work on javac.) The trait's methods are named `body_<name>`, so that a
  call of the original name finds the header's method, never the trait's.
- **The binary** depends on the header and every body crate (`use body_0 as _;` makes rustc
  link each) and its `main` calls the header's.

What cannot move stays in the header with its code: generic functions and the methods of generic
`impl`s (Rust needs their code in the crate that instantiates them), `const` functions (statics
call them), functions with an `impl Trait` in their signature, methods of a trait `impl` whose
signature names `Self::..`, and the initializers of statics. Small functions stay too: one whose
code is at most 48 tokens (`split.rs`, `small`; a `match` that passes a failure on counts as the
`?` it stands for, and a path as one name however deep its package) and that is neither `#[cold]`
nor `#[inline(never)]`, such as a getter, a buffer's `get`, `Mth.lerp` or a cast of an `Object`
to a class (`Object::as_x`), keeps its code in the header, and so does one that the generator
marked `#[inline]` itself: the check that a class is initialized (`init_x`, whose `init_x_slow`
moves), a variant's constructor, the view of a type's enum. Both those the generator did not mark
and the header's wrapper of each moved method are `#[cfg_attr(not(debug_assertions), inline)]`, so
that an optimized build inlines them into the crates that call them: a wrapper becomes a direct
call of the body crate's function, and a small function no call at all. A build at
`opt-level = 0` inlines nothing, and there each crate would compile a copy of every `#[inline]`
function it calls, so a debug build leaves them called. In the Minecraft server about 60% of the
functions are small, about a tenth of the code. The header calls into the body
crates, which depend on it; rustc's linker (lld, the default on x86-64 Linux) resolves symbols
whatever the order of the libraries.

The moved code goes to the body crates item by item (a function, or an `impl` with its trait), in
the order of the module tree and of each module's items, cut into runs of about the same size,
about `N` of them (`split.rs`, `chunks`). Where a run ends is chosen by the items' names, not
their positions: a run ends before an item whose first function's name hashes to a multiple of a
power of two, once the run has half a share of the code, or anyway at twice a share (the share
and the power of two rounded to powers of two, so that a program that grows a little keeps them).
Each body crate is named after the hash of its first item's name (`body-1f3a..`). So the same
program is split the same way every time, neighbouring packages share a crate, and a change to
one item, or an item added or removed, moves only the cuts around it: the crates beyond keep
their names and their code. A module with more code than a share is spread over several crates,
each with the module's `use` lines: the Minecraft server's crate root, whose `Object` enum's
methods and reflection table are 290 MB of code, would otherwise be one crate of its own.
Within a body crate, a module's code past about 512 KB (`CODEGEN_UNIT`) is cut into modules of
its own of about that size (`mod j2r_code_0 { use super::*; .. }`, with the module's `use` lines
again), and an `impl` with more moved code than that into several traits (`Bodies_Object`,
`Bodies_Object_2`, ..), each an item of its own: rustc compiles each module's code in one
codegen unit, on one thread. The Minecraft server's crate root, one module with 30 MB of code in
one body crate, 18 MB of it one `impl` of `Object`, kept one thread busy for an hour of a release
build (`opt-level = 1`) while the others had finished; its body crate of the reflection table's
reads and writes (11 MB) built in 57 s alone, and in 29 s cut into modules so.

Writing the workspace leaves alone each file whose text is already there and removes the body
crates of an earlier split that this one does not have, so Cargo, which goes by the files' times
of change, builds again only the crates whose code changed. A change that touches the header
(any declaration: a class, a field, a function's signature, a function added) still builds every
body crate again, as each depends on the header.

`--crates N` with `--crate DIR` writes the split workspace, for a translation or for a census's
trial translation (`--census --crate DIR --crates N`, each rejected piece ending the program with
its rejection), which is how the Minecraft server's trial is built ([minecraft-server.md](minecraft-server.md),
the `trial` step). Every program of `targets/` builds and prints what Java prints split into
three body crates, and the whole-program test `a_program_split_into_crates_matches_java` builds
one so. javac's census translation split into 16 body crates builds (a debug build, one job: about
150 s, rustc at 5.2 GB at its peak in a body crate of 11 MB, 2.3 GB for the 18 MB header) and
compiles a program.

The split parses each item of a module (a class with its `impl`s) by itself with `syn` and drops
its text once split: a syntax tree is many times its text, and the Minecraft server's crate root
alone is 2.3 million lines (its `Object` enum 650,000). It parses the items and the signatures of their
functions, not the functions' code: before `syn` parses an item, each function's braces are
emptied with their place kept (`hollow`), and the split takes the code's text from the module's
text. A function's code that is not Rust is then rustc's to report, not the split's.

What it costs: a call of a moved function from another crate cannot be inlined without
link-time optimization. The split workspace's release profile has thin LTO, which inlines across
crates again without the memory of a whole-program build: the Minecraft server, with its header
alone built at 15 GB, links in about ten minutes at 7 GB, and its 5000-tick sprint runs at about
11,000 ticks per second against about 4,000 without LTO (Java about 6,000), at 0.9 GB of peak memory
against 1.2 GB. Fat LTO of a program of this size is a whole-program build again. Debug builds and
`cargo check` gain the most from the split. Without
LTO, the functions kept in the header for their size are what keeps the cost small: before they
were, `benchmarks/NoiseTree.java` split into three body crates at `opt-level = 1` ran its kernel
in about 1240 ms against 470 ms in one crate with fat LTO; with them it runs in about 430 ms.
The Minecraft server's world generation, whose density functions call buffer accessors, static
helpers and field getters for every cell, spent about a quarter of its time inside functions of
at most 64 bytes of machine code, nearly all of them calls into body crates.

## Not done yet

- The translated JDK as a crate reused between programs ([large-projects.md](large-projects.md)):
  its code depends on the program's classes through the hierarchies' enums.
