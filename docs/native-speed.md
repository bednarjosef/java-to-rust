# Native speed

The plan for making translated programs much faster and smaller than the same programs on the
JVM, for any Java program, with no input beyond its sources, and how far it has got: the
collected heap of step 1 is built (docs/collector.md); the rest is planned. The translated javac is the main
benchmark. The object model it replaces is described in docs/object-model.md. The idiom side of
the whole-program mode (designs a person would write, relaxations) stays in docs/thorough-mode.md.

Rules that hold for every step:

- **The output stays identical**: what the program prints and writes, its exit code and its
  exceptions. Internals (where an object lives, when its memory is reused, how a list is stored)
  may change freely, except where Java makes them visible: identity hash codes, `HashMap`
  iteration order, weak references and finalization, and `OutOfMemoryError`.
- **No training data.** Every decision comes from analysing the program's code, or from the
  program adapting while it runs (the collector, representation switches). Nothing depends on
  inputs that someone has to provide.
- **Every decision is proven or guarded.** What the analysis proves is used directly. What it
  cannot prove is either left general or made true by a cheap check with a general fallback, so
  a wrong guess costs speed, never correctness.

## Why: what reference counting cost

Measured on 2026-10-24 with quick builds of the translated javac (the same code with two
experimental runtimes), on the 540 corpus features:

| runtime | time | binary | peak memory |
|---|---:|---:|---:|
| reference counting (before the collector) | 3.93 s | 160 MB | 257 MB |
| counts kept, nothing freed | 3.86 s | 81 MB | 1,875 MB |
| no counting, nothing freed | 2.57 s | 52 MB | 1,855 MB |

- Changing reference counts costs about a third of the time; the code that frees objects when a
  count reaches zero is two thirds of the binary.
- Freeing itself costs about 2%.
- Freeing is still needed: javac makes about seven times more short-lived objects than it keeps.
  Without freeing, the Minecraft benchmark passes 10 GB within 20 seconds.

On the Minecraft benchmark (4,754 decompiled server files), today's fat-LTO and PGO build takes
60 s and 6.2 GB, against the real javac's 11.3 s and 4.1 GB. While it attributes types, it misses
the data cache five times as often as the JVM, the instruction cache 5.6 times and the
instruction TLB 17 times. Causes are 16-byte references (an enum tag and a pointer) against the
JVM's 4-byte compressed ones, 48 bytes of count, lock and borrow flag per object against the
JVM's 12-byte header, and a 90 MB binary of dispatch `match`es and drop code.

## Step 1: the object model

A heap managed by a tracing collector, references of 4 bytes, objects without headers.

### 1.1 Heap and references

- The heap is one reserved range of address space (32 GB, committed as it is used). A reference
  in a field or an array is a `u32`: the object's offset from the heap's base divided by 8.
  `java::Gc<T>` is that reference with its static type. It is `Copy`: copying a reference is
  copying four bytes, with no count.
- The heap is cut into blocks. Each block holds objects of one class only ("big bag of pages"),
  and its header names the class. An object therefore needs no header of its own: its class,
  size and reference map are read from its block. An object is only its fields.
- Arrays and other objects of variable size live in blocks of their own kind, with a small header
  of length and element type.
- Blocks of a class start small (4 KB) and grow (up to 64 KB), so the thousands of classes that
  have few objects waste little.
- Allocation takes the next free slot of the current block of its class, kept per thread. That is
  a few instructions, like the JVM's pointer bump.

### 1.2 Fields as cells

Java lets any reference read or write any field at any time. Today that is a `RefCell` (or a
lock) per object, with a borrow flag checked on each access, and copies where a borrow cannot
last. With `Copy` references every field value is plain data, so a field is read and written by
value, through the object's address, the way Java does it:

- No borrow flags, no `Ref` guards, no copies taken to end a borrow, no field-level borrow rules.
- In a class whose objects can reach a second thread, field accesses are relaxed atomic loads and
  stores. On x86-64 these are the same instructions as plain ones. Java's memory model allows
  exactly the races this permits.
- `synchronized` and `wait`/`notify` use monitors kept in a side table, only for the objects that
  are locked. No object carries a lock word.
- Identity hash codes are also kept in a side table, only for classes whose identity hash is
  used.

### 1.3 The collector

Built so far (docs/collector.md): a collector that is the global allocator, with uncounted
handles, conservative scanning of the roots and of every slot that may hold pointers, sweeping
at each collection, and marking in parallel. Planned beyond it:

- **Marking** follows each class's reference map (the offsets of its reference fields), one
  table per class, read by a single marking loop with an explicit stack. No per-class tracing
  code is generated.
- **Sweeping** is per block and lazy: a block's mark bits become its free slots when the
  allocator next reaches it. A block with no live objects goes back to the pool.
- **Roots**:
  - Thread stacks and registers are scanned conservatively. Any word that points into an object,
    as a full pointer or as a 4-byte reference, keeps that object alive. The block header turns
    an address into its object with one division, so interior pointers work too. Objects found
    this way are never moved.
  - Static fields are precise roots, from a table the translator writes.
  - The runtime keeps Java references only in heap objects, on the stack, or in registered root
    slots, never inside a Rust `Vec` or `Box`. A stress mode, which collects at every allocation,
    runs in the tests to catch any that escape.
- **When**: a collection runs when the heap has grown by a factor since the last one, so the
  marking cost per allocated byte stays bounded.
- **Threads**: a collection stops the other threads with a signal. Each records its registers
  and stack bounds and waits. Each thread allocates from its own blocks.
- **Values that own Rust memory** (a buffer of a runtime type) are dropped when their object is
  swept. Such classes are marked for it. Strings and arrays live in the heap, so there are few.
- **Generations**: most objects die young. Once the basic collector is measured, a young
  generation (sticky mark bits: young objects are collected often without tracing the old ones)
  is added, with card marking on reference stores into old objects. Analysis removes the barrier
  where the stored-into object is known to be young (constructors).

### 1.4 Types, dispatch and values

- A reference to an interface, an abstract class or `Object` is one `u32`, not an enum of
  handles. Its class is read from its block.
- Calls on it dispatch through the class: a `match` on the class number at first (a jump table),
  then per-class function tables where that is smaller.
- `Integer`, `Character`, `Short`, `Byte` and `Boolean` values held as `Object` or in generic
  collections are stored in the reference itself, with one tag bit, so boxing allocates nothing.
  Their identity is preserved where Java defines it: the cached range of `valueOf`.
- Strings are heap objects: Latin-1 bytes when every character fits, WTF-8 otherwise. Short
  strings are stored inside the string object.

### 1.5 Memory pages and layout

- The heap and the program's code use huge pages, which attacks the TLB misses.
- Hot fields of an object go first and cold ones last. Hot means read in loops or in many
  methods, by static count.

### 1.6 Order

Each part lands on its own and passes the gate (all checks and the 28 targets):

1. Built: uncounted `java::Arc` handles in a collected heap, replacing `Rc` and the counted
   handles for objects, with the enums and cells unchanged. This removed the counting and
   freeing code, which is where the measured third was. The collector scans conservatively.
2. Header-free class blocks and `u32` references, with tracing from generated reference maps.
3. Thin references for multi-class types, replacing the enums of handles, with dispatch on the
   class number.
4. Fields as cells, monitors and identity hashes in side tables, removing `RefCell` and
   `Locked`.
5. Strings and arrays in the heap; boxed values in references.
6. Young generation, huge pages, field order.

## Step 2: whole-program analysis

One analysis engine: facts as bitsets over allocation sites, classes, fields and methods, solved
to a fixed point over the whole program and the JDK code it reaches, within the memory budget of
the translation.

1. **Points-to, separated by receiver object.** For each variable, field and array, the
   allocation sites whose objects can be there. Each method is analysed once per receiving
   allocation site, the standard precision for Java. Everything below depends on it.
2. **Call graph** from points-to: the methods each call can reach.
3. **Escape**: whether an object can leave its method, its parent object, or its thread.
4. **Uniqueness and ownership**: fields that hold the only reference to their object, and the
   parent each object is reachable through.
5. **Immutability**: objects, and whole classes, never written after their constructor ends,
   with fixed fields as the per-field version.
6. **Identity use**: whether an object's identity (`==`, identity hash, `synchronized`) is ever
   used. Objects whose identity is never used can be copied, merged or stored inline.
7. **Nullness**: references that are never null.
8. **Integer ranges**: the values each integer field and local can hold, for bounds checks and
   field widths.
9. **Exceptions**: what each call can throw, and which handlers can run.
10. **Effects**: what each method reads and writes, to reorder, hoist and drop calls whose
    results are unused.
11. **Reachability**: classes, methods and fields that are never used, which are removed.

## Step 3: using the facts

Each transformation is separate, with its own corpus programs. Each uses a fact proven in step
2, or a check with a fallback.

- **Scalar replacement**: an object that never leaves its method becomes local variables. No
  allocation at all.
- **Stack and method regions**: objects that do not outlive their method, but are too big or
  too many to become locals, go in a region freed when the method returns.
- **Inline objects**: a child that is created with its parent, never re-pointed and never
  compared by identity is stored inside the parent. A list that owns objects of one class becomes
  a `Vec` of structs side by side.
- **Specialized collections**: `ArrayList<Integer>` becomes `Vec<i32>`, an `ArrayList` of
  objects becomes a `Vec` of references with no casts, and `HashMap`s and `HashSet`s whose
  iteration order never reaches the output become faster tables. Others keep Java's order.
- **Specialized methods**: copies of a method for different receivers or argument classes, so
  each knows its exact types and Rust can inline it.
- **Direct calls**: a call with one possible target is a direct call; one with two or three is
  a test and direct calls.
- **Checks removed**: null checks on never-null references, bounds checks inside proven ranges,
  write barriers on young objects, monitors on objects that never reach another thread.
- **Smaller objects**: unused fields removed, integers proven small stored as `u8` or `u16`,
  booleans packed into bits.
- **Shared immutable values**: equal immutable objects whose identity is never used become one
  object (hash-consing), so comparing them is comparing references.
- **Exceptions as values**: exceptions caught by a nearby caller, used for control flow, become
  return values instead of unwinding.
- **Check once, then run fast**: a loop or method is duplicated into a fast version and the
  general one, chosen by one check at its entry. For example: two arrays are different, or a
  receiver is of one class.
- **Representation switches**: a list starts as `Vec<i32>`, and converts itself to the general
  form if anything else is ever stored. An object starts inline and moves to the heap if it
  escapes.
- **Start-up snapshot**: static initializers that depend on nothing outside the program run at
  build time, and their objects are part of the binary.

## Measuring

Every landed part is measured on four workloads: hello world, the 47 corpus programs, the 540
corpus features, and the Minecraft benchmark. Each is measured for time and peak memory, and
always with identical output to the real javac's. The numbers to beat:

| workload | real javac | translation now (fat LTO, PGO) |
|---|---:|---:|
| hello | 0.19 s | 0.02 s |
| 47 programs | 0.61 s | 0.36 s |
| 540 features | 1.90 s | 2.43 s |
| Minecraft server | 11.3 s, 4.1 GB | 60 s, 6.2 GB |

## Risks

- Conservative stack scanning keeps an object alive when an integer happens to look like its
  address. That costs memory, never correctness.
- A Java reference stored where the collector does not look (a Rust collection in the runtime)
  would be freed while in use. The rule in 1.3 and the stress mode exist for that.
- Points-to analysis precise enough for these transformations, over a program and the JDK,
  costs translation time and memory. Precision lost costs speed, never correctness.
