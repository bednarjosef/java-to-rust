# Architecture

How the translator is built today, what holds up as it grows, and the four changes that come
before the larger plans ([natural-rust.md](natural-rust.md),
[translator-speed-and-memory.md](translator-speed-and-memory.md),
[large-projects.md](large-projects.md)) and before a second, "port" mode next to today's exact
translation.

## The pipeline

```
Java sources
  │  javac (JavaTree.java, run as a JVM process; JavaTree --serve in whole-program mode)
  ▼
javac's attributed trees, as JSON                       crates/translator/src/javac*, program/
  │  reachability (program/reach*) asks javac for the library units the program reaches
  ▼
lowering: one typed tree for the whole program          crates/translator/src/lower*, ast*
  │  (ast::Class: every type, method and expression; expressions have ExprIds)
  ▼
analyses: facts proven about the program                crates/translator/src/analysis*
  │  (analysis::Facts: about 57 sets and maps keyed by expression, method or type)
  ▼
code generation: Rust source text                       crates/translator/src/codegen*
  │
  ▼
main.rs, built against the hand-written runtime         crates/runtime
```

`lib.rs::translate` shows the single-file path in five lines; `program::translate_program` is the
whole-program path with reachability, the census and the survey around it.

### The JDK's code, and the translator's changes to it

With `--library`, the JDK's classes are translated from the JDK's own sources, which stay outside
the repository. Where that code cannot be translated as it is, the translator changes it in one
of three ways, all in `crates/translator/java` and all holding only our own code (see
[jdk-overlays.md](jdk-overlays.md)):

- **Redirects** (`REDIRECTED_CALLS` in `program/reach.rs`): every call of a JDK method becomes a
  call of a static method of ours.
- **Patches** (`java/patches/`): a file at the path of a JDK class that holds only the members it
  replaces, adds, removes or edits. javac reads the JDK's file with the patch applied
  (`java/JdkPatch.java`, inside the javac process), and refuses a patch that names a member the
  JDK's file does not have.
- **Overlay classes** (`java/overlay/`): whole classes of ours that the JDK does not have, put on
  javac's source path before the JDK's sources.

`scripts/check.sh` fails, with `JAVA_BASE_SOURCES` set, when one of these files shares more than
a few lines with the JDK's file of the same path.

## Safe defaults, plainer forms

Every value starts at a safe default: an object is `Arc<RefCell<C>>`, a reference that may be
null an `Option`, an array a shared `java::Array<T>`, arithmetic wrapping. Analyses move values
to plainer forms (`Arc<C>`, plain structs, `Vec`, plain `+`) where they prove it changes nothing.
The generator writes the plain form of a construct only where a fact allows it, and the faithful
form otherwise, so a better analysis improves the output without touching the generator. The
runtime crate (`crates/runtime`) supplies what Rust lacks: Java's number semantics and printing,
strings and builders, `String.format`, regular expressions, charsets, threads and monitors, the
native methods of the JDK, and the garbage collector that frees objects.

## Size, on 2026-10-21

| part | files | lines |
|---|---:|---:|
| `lower`: javac's tree to the typed tree | 48 | 12,800 |
| `analysis`: ranges, nullness, ownership, moves, exceptions, threads, monitors, points-to, … | 28 | 10,300 |
| `codegen`: Rust text | 43 | 14,200 |
| `program`: reachability, survey, census | 7 | 4,000 |
| `ast`, `javac`, crate root | 20 | 6,900 |
| `crates/runtime`: strings, formatting, threads, math, Unicode tables, allocator, … | 50 | 15,600 |
| `crates/translator/java`: JavaTree, BytecodeNames, Standard.java and the overlay classes (since 2026-10-22 also the patches of JDK classes) | 10 | 3,200 |
| tests (`crates/translator/tests`) | 11 | 7,100 |

Plus 542 Java programs in `corpus/` (each compared with real Java's output), 28 target programs,
benchmarks and the fuzzer. Files average about 330 lines, the largest is `codegen/objects.rs` at
about 2,000; about 11% of the lines are doc comments; clippy runs with warnings as errors.

## What holds up

- **The stages are separate.** Only lowering and `program` read javac's JSON (through
  `lower::Node`); the analyses and the generator see only the typed tree. Replacing JSON with a
  compact format touches the first stage only.
- **Facts are separate from the generator.** The analyses prove; the generator reads what they
  proved. That is the split a second mode needs.
- **Behaviour is tested against Java itself**, not against expectations someone wrote: the corpus,
  the whole-program test, the targets, the fuzzer, and the staging gate that runs them all before
  anything reaches main.

## Four changes before the larger plans

### 1. A representation plan between the analyses and the generator

Today the generator decides on the spot. About 150 places in `codegen` read a fact and choose:
proven, so the simple form; not proven, so the general form (`Option` or not, a handle or owned,
`wrapping_add` or `+`, `Result` or not). Every new mode or relaxation would have to be added at
each of those places.

The plan is one table, filled after the analyses and before the generator: for every value,
field, parameter, method and class, the decision (owned, shared or locked; nullable or not;
wrapping or plain arithmetic; raising or not; `Vec` or the JDK's `ArrayList`; …) and the reason
for it, which is a proof ("the nullness analysis shows it is never null") or, in port mode, a
relaxation ("port mode: Rust's `HashMap`"). The generator prints the plan and decides nothing.

What this gives:

- **Modes are policies.** Exact mode fills the plan from proofs only; port mode also from
  relaxations; thorough mode with stronger analyses. One generator for all of them.
- **Port mode's report is free.** Every plan entry whose reason is a relaxation is a place where
  the program may behave differently from Java; listing them is the report that
  [natural-rust.md](natural-rust.md) asks for.
- **The natural-Rust and ownership work becomes "improve the plan"**, and decisions can be tested
  as data, without reading generated text.

[thorough-mode.md](thorough-mode.md) already describes this table ("analyses fill the
representation table and the code generator follows it"); it was never built as one structure.

### 2. The generator produces items with a module path, not one string (done)

The generator still appends text (about 1,150 `format!`, `push_str` and `write!` calls), but what
it makes is now a tree of modules, one per Java package, and a separate writer lays the tree out.
The pieces, smallest first:

- **`output::Item`**: one item of a module, such as a class's struct with its `impl`s, a static
  method, or the package's statics: its text (at no indentation), what it was made from
  (`Origin`: the class, its enum, the statics, `fn main`), and the packages and classes it names.
- **`output::Module`**: a package's module: its path (`["com", "example", "util"]`), its `use`
  lines and its items.
- **`output::ModuleTree`**: every module of the program, the crate root first.
- **The name resolver** (`Resolver` in `codegen/modules.rs`): every name of a class, a static
  field or a static method in another module goes through it (`modules::split`). It chooses the
  path from the module being written (`util::Counter` through `use crate::com::example::util;`,
  or `crate::java_::util::Optional` when two modules share a name) and records what each item
  names. `codegen/imports.rs` names the crate root's items through `use crate::Object;`.
- **`output::Layout`**: `OneFile` (inline modules in one `main.rs`: a translated file, `-o`, the
  translation tests) or `FilePerModule` (`--crate`: `src/com/example/util.rs` for
  `com.example.util`). An item's text is the same in both; the one-file layout indents it.

What a resolver knows of the program (the package of each class, each package's path, the
aliases, and the names of nested classes that would clash) is found once per translation
(`modules::names`, kept in the generator's `Setup`) and shared by every round and thread; each
thread enters it with a part of its own for the module and item being written
(`modules::enter_names`). That entered resolver is held in a thread-local for the duration of
`generate`, because the functions that make names (`struct_name`, `cell_name`, `init_name`) are
called from about 40 places in 10 files; passing the resolver to them is mechanical and was left
for a time when fewer branches edit the generator. Natural names plug in at the resolver (it decides every
path and could decide the names). The split into crates (`--crates`, `split.rs`) works on the
finished module tree, separating declarations from code, so it needs nothing of the resolver (see
[several-crates.md](several-crates.md)).

### 3. Snapshot tests instead of strings in one file

`tests/translation.rs` has 418 tests in 4,700 lines that compare generated Rust character for
character. Every change to the output's shape (natural names, the plan, files) changes hundreds of
them by hand. Snapshot files, one per feature, with one command to review and accept a change,
keep the shape visible without making every output change expensive. Behaviour stays guarded by
the corpus, which compares what programs print, not how they are written.

### 4. A crash in one method is a rejection, not the end of the run

The translator has about 220 `unwrap`, `expect`, `panic!` and `unreachable!` calls. Many guard
real invariants, but a panic in an analysis ends the whole translation (it ended two censuses
early). Each stage should catch a failure per method and turn it into a rejection with a position
and a message, so that a large project gets a list of problems instead of a crash.

## Smaller items

- **Names.** `ast::Class` is the whole program (all its types and methods), not a class; renaming
  it to `Program` and the like makes the code easier to read for anyone new.
- **Per-method entry points for the whole-program analyses**, so that they can later run on
  summaries of methods instead of their bodies ([translator-speed-and-memory.md](translator-speed-and-memory.md)).
- **The stand-ins for JDK classes** (`Standard.java`, `java/overlay`, `java/patches`) on one
  written list, each with the reason it exists and what would let it be replaced by the JDK's own
  class (the patches have theirs in [jdk-overlays.md](jdk-overlays.md)).
- **Global state in the generator** (the thread-local name resolver) goes once the name
  functions take the resolver as a parameter (see change 2), which also matters once the
  generator runs on every core.

## Order and effort

Calibrated on how long work took agents in this repository (Java's `NullPointerException` messages
took under an hour from assignment to a green branch; the census tool about five hours):

1. Changes 2, 3 and 4 in parallel, one agent each: a few hours.
2. Change 1: a short design note first, then the move of the generator's decisions into the plan,
   one or two agents: about half a day.
3. Then the larger plans: speed and memory, natural Rust and ownership, large projects.

With three to four agents at once; more mostly produces merge conflicts, since all four changes
touch the generator.
