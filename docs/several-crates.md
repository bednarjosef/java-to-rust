# Several crates (planned)

A whole program is one Rust crate today, with a file per Java package. This note is about
splitting it into several crates: what stops it, how to find what can be split off, and what a
split would buy for javac. It is a design; nothing here is built except the data it needs
(`--modules-json`) and the script that reads it (`scripts/crate-groups.py`).

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

## What keeps code in one crate

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

javac does not translate yet, so this is measured on the code its census generates (2026-10-22),
with every rejected piece stubbed:

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

The census code is not a translation that builds yet, so its rustc time and memory are not
measured. If rustc's memory grows with the lines as in the table at the top (about 600 MB for
19,000 lines), one crate of 317,000 lines needs roughly 10 GB, and each of the two halves about
half of that.

## Breaking the loops

Only the first two sources of loops are the translator's own; the third is the program's, and a
split must take it as it is. Two changes would cut the first two, each at a cost:

- **Dispatch through tables at a crate's boundary.** A hierarchy whose classes are in several
  crates would be a trait (a table of functions) in the crate of its root class instead of an
  enum, with each subclass implementing it in its own crate. The parent then no longer names its
  subclasses. The cost is the enum's advantages there: calls through a pointer instead of a
  `match`, and no proof that the subclasses are all known. Inside a crate the enum stays.
- **`Object` with an open variant.** Classes in later crates would be held as
  `Object::Other(Rc<dyn Any>)`-like values that cast back through a table, instead of a variant of
  their own, so that `Object` does not name them.

These are the same boundary the JDK crate needs ([large-projects.md](large-projects.md),
[natural-rust.md](natural-rust.md)): the translated JDK names the program's classes only through
these enums (a program class in `Object`, a `Comparable` or `Iterable` that the program
implements, the dispatch of `toString`, `equals` and `hashCode`).

## Order

1. Measure javac's groups again once it translates, at the size it then has.
2. A crate per group of the program's own code against one crate for the rest, with dispatch
   through tables only across the boundary; the generator's layout gets a rule that puts each
   module in a crate, and the name resolver writes `other_crate::path` and `pub` across crates.
3. The JDK as its own crate, the boundary case above.
