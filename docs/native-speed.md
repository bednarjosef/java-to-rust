# Native speed

The plan for making translated programs much faster and smaller than the same programs on the
JVM, for any Java program, with no input beyond its sources, and how far it has got: of step 1,
the collected heap (docs/collector.md), thin references and fields as cells are built, and
4-byte references in the fields of celled classes behind a switch (1.6 lists what is built and
what is planned); of steps 2 and 3, escape within a method and objects on the
stack are built, the rest is planned. The translated javac is the main
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

On the Minecraft benchmark (4,754 decompiled server files), the fat-LTO and PGO build with
reference counting took 60 s and 6.2 GB, against the real javac's 11.3 s and 4.1 GB; with the
collector it takes 13.1 s and 1.36 GB (see "Measuring"). With reference
counting, while it attributed types, it missed
the data cache five times as often as the JVM, the instruction cache 5.6 times and the
instruction TLB 17 times. The causes were 16-byte references (an enum tag and a pointer) against the
JVM's 4-byte compressed ones, 48 bytes of count, lock and borrow flag per object against the
JVM's 12-byte header, and a 90 MB binary of dispatch `match`es and drop code.

## Step 1: the object model

A heap managed by a tracing collector, references of 4 bytes, objects without headers.

### 1.1 Heap and references

- Built: the heap is one reserved range of address space, from 4 GB to 32 GB, committed as it is
  used (docs/collector.md, "The heap").
- Built behind a switch, `JAVA_TO_RUST_COMPACT_REFERENCES=1` (compact references): a reference
  held in a field of a celled class (docs/object-model.md 1.8) to an object of one class, an
  array or a string is 4 bytes, the object's address divided by 8 (`java::RefField`,
  `java::SharedRefField`, runtime `compact.rs`). With the heap below 32 GB there is no base to
  add: decoding is one shift, which an x86-64 load folds into its address (`[r*8]`), and null
  is 0, which no object's address is. `java::Arc` stays the form of a reference in locals,
  parameters and registers, 8 bytes used with no decoding, so only a field's reads and writes
  convert. The collector reads every 4 bytes it scans as a possible reference to the start of a
  slot (docs/collector.md, "Collection"). An object on a method's stack is never stored in such
  a field: its address is past 32 GB (writing it stops the program), and the escape analysis
  keeps those objects out of fields altogether. Which other references become 4 bytes, and in
  what order, is the plan in 1.6, item 2.
- Planned: the heap is cut into blocks. Each block holds objects of one class only ("big bag of
  pages"), and its header names the class. An object therefore needs no header of its own: its
  class, size and reference map are read from its block. An object is only its fields. (Built:
  blocks of one slot size, with no header in the object, docs/collector.md.)
- An array is one allocation, a 16-byte header (its length, a borrow flag and one word for its
  class or monitor) followed by its elements (docs/object-model.md 1.13, `runtime/src/array.rs`).
- Planned: blocks of a class start small (4 KB) and grow (up to 64 KB), so the thousands of
  classes that have few objects waste little.
- Allocation takes the next free slot of the current block of its class, kept per thread. That is
  a few instructions, like the JVM's pointer bump.

### 1.2 Fields as cells

Java lets any reference read or write any field at any time. Where objects are not celled, that
is a `RefCell` (or a lock) per object, with a borrow flag checked on each access, and copies
where a borrow cannot last. With `Copy` references every field value is plain data, so a field
is read and written by value, through the object's address, the way Java does it:

- No borrow flags, no `Ref` guards, no copies taken to end a borrow, no field-level borrow rules.
- Built for the classes whose objects one thread uses (docs/object-model.md 1.8, "Celled"): an
  object is an `Arc<C>` whose fields are `java::Field` cells, read with `get()` and written with
  `set()`.
- Built for the classes that threads share, which are nearly all of javac's: an object is an
  `Arc<C>` whose fields are `java::SharedField`s (an `Arc<java::Monitored<C>>` where a monitor
  site may lock it), acquire loads and release
  stores of a word (the same instructions as plain ones on x86-64), sequentially consistent for a
  `volatile` field, a striped lock for a wider value (docs/multi-threading.md, 2.1).
- Built for the arrays that threads share, which are nearly all of javac's: a
  `java::SharedArray` keeps each element in a `java::SharedField` cell, so `a[i]` is a bounds
  check and one load, with no lock taken (docs/multi-threading.md, 2.2). Arrays of one thread
  keep their `RefCell`. An array that only a constructor or a class's initialization writes, and
  that the program then only indexes and measures, is a `java::Frozen`, read with plain loads,
  with a constant length where its arrays all have one (docs/multi-threading.md, 2.3).
- Built for the classes whose objects no monitor site can lock: they carry no lock word
  (docs/multi-threading.md, 3.1). Planned: `synchronized` and `wait`/`notify` use monitors kept
  in a side table, only for the objects that are locked, so that no object carries one.
- Planned: identity hash codes are also kept in a side table, only for classes whose identity
  hash is used.

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
- **Generations** (built, opt-in with `JAVA_GC_GENERATIONS=1`; docs/collector.md,
  "Generations"). Most objects die young, and a full collection marks everything alive again:
  `BinaryTrees` marks its 89 MB tree at each of 72 collections, `SmallObjects` its list of 2
  million nodes at each of 22 (docs/performance.md, "Allocation"). The young generation uses
  sticky mark bits, in the runtime alone:
  - **Old and young.** An object that survives a collection keeps its mark bit, and is old. A
    minor collection marks from the roots and from the old objects on the pages written since
    the last collection, stops at any marked object, and frees the taken slots left unmarked:
    the young objects that died. A full collection clears every mark first. It runs once a
    quarter more (plus 1 MB) is alive than after the last full one, in the same pause as the
    minor collection that finds it, so that old objects that died do not decide when the next
    collection runs. Slots that threads hold free (spans, free lists), which a collection keeps
    marked, lose their marks after the sweep, so that what is made in them is young, and a slot
    freed by Rust (`untake`) loses its mark too. Sweeping still visits every block.
  - **Which old objects were written: the kernel's write protection, not a barrier in the
    code.** The heap is registered with `userfaultfd` in asynchronous write-protect mode
    (`UFFD_FEATURE_WP_ASYNC`, Linux 6.7, unprivileged with `UFFD_USER_MODE_ONLY`). A write to a
    protected page faults once, and the kernel unprotects the page and notes it, without waking
    any thread. Each minor collection, with the world stopped, asks `PAGEMAP_SCAN` for the pages
    not protected and scans the old objects on them; after the sweep, the pages among them that
    now hold old objects that may hold pointers are protected again (`UFFDIO_WRITEPROTECT`, in
    runs). Pages of young objects only stay writable, so allocation does not fault. No barrier is
    emitted, and every write counts: the generated code's, the runtime's, the standard
    library's (a `Vec` or a `RefCell` written in place), atomics, the kernel's (a `read` into a
    buffer), and every thread's, since the protection is the process's. Where the kernel
    refuses (an older kernel, a seccomp filter), or stops answering, every collection is a full
    one; this was tested with `systemd-run -p SystemCallFilter=~userfaultfd`.
  - **Memory outside the heap that can hold a reference.** A reference held where neither
    the roots nor the protected heap are would be missed by a minor collection. What exists:
    - Rust's own allocations: every `Vec` buffer (an array's elements), `RefCell` contents,
      `Box`, and the runtime's collections (the interned strings of `shared_string.rs`, the
      thread table of `thread.rs`) are taken from the collector, which is the global allocator,
      so they are in the protected heap.
    - Statics (the writable segments of every loaded object), thread-local storage, the
      threads' stacks and registers, and objects on the stack: roots, scanned whole at every
      collection, minor or full.
    - The C library's `malloc`: the runtime takes it only for a starting thread's closure
      (`Start`), and no collection runs until the thread has registered and taken it.
    - `native_memory.rs` blocks and mapped files: bytes, never references.
    - The collector's own memory (block metadata, mark stacks, `PAGEMAP_SCAN`'s buffer):
      reserved outside the heap, holding no references.
  - **Why not card marking.** A card barrier (`card[address >> 9] = 1` after a reference store)
    would have to see every reference written into the heap. `java::Field::set`,
    `java::SharedField::set` and an array's element store are the runtime's and could mark, but
    the translation also writes references through `RefCell` borrows (a `RefCell` object's fields,
    the bulk operations on an array's `Vec`: copies, fills, sorts), into local `Vec` arrays,
    through `&mut` methods of owned values held in objects, and the runtime keeps references in
    Rust collections (`shared_string.rs`, `thread.rs`). One store missed frees an object that is
    still reached. Measured on a loop of four reference stores per iteration (two fields of old
    objects, two elements of an old array), the barrier in the runtime's store paths cost 44% with
    a range check, 38% as HotSpot's unconditional byte store over a table for the whole address
    space, and 18% as a conditional store (read the card, write it if clean): the loop waits on
    cache misses, and the card stores queue behind them. On the benchmarks it was within noise
    (NodeList 3%), except that a barrier on every 8-byte store slowed `ParticleArray`'s `double`
    stores by 53%, by stopping their vectorization: a real barrier would have to know a reference
    type from a `long`.
  - **What it costs and gains.** The first write to a protected page in a cycle costs 0.26 to
    0.4 µs, and scanning and protecting 256 MB of heap 0.2 to 0.6 ms. A protected huge page is
    split into 4 KB pages (the protection is per page). Measured (median of alternating runs,
    off against on): `SmallObjects` 0.43 s and 1.2 s of processor time against 0.34 s and 0.49
    s, but 170 MB against 200 MB at its peak, since the dead old nodes wait for a full
    collection; `BinaryTrees` 8% less wall time and a third less processor time (5.37 s and
    13.2 s against 5.86 s and 20.4 s, on a machine whose cores ran at about 850 MHz);
    `ManyThreadsAllocating` 0.10 s against 0.13 s and 196 MB against 201 MB, from the
    full collections that follow minor ones; `SortMillion` 47 MB against 56 MB at the same
    speed. The others are unchanged. That mix is why it is opt-in: it is to be tried on the
    Minecraft server before it becomes the default.
  - **Threads and forms.** Nothing changes in the generated code or the object forms
    (`java::Monitored`, `java::SharedField`, `RefCell`): the protection sees writes, not
    types. A `LazyMonitor` that makes its lock writes the lock's pointer into the object, a
    write like any other. The collector already stops every thread, so the pages are read and
    protected again while nothing writes.

### 1.4 Types, dispatch and values

- Built: a reference to an interface, an abstract class or `Object` is eight bytes, not an enum
  of handles: the class's number in the top 16 bits and the handle below (`java::Thin`,
  docs/object-model.md 1.2). With compact references such a reference keeps its 8 bytes for
  now; how it becomes 4 is planned in 1.6, item 2.
- Calls on it dispatch through the class: a `match` on the class number (built; a jump table),
  then per-class function tables where that is smaller (planned).
- `Integer`, `Character`, `Short`, `Byte` and `Boolean` values held as `Object` or in generic
  collections are stored in the reference itself, with one tag bit, so boxing allocates nothing.
  Their identity is preserved where Java defines it: the cached range of `valueOf`.
- Strings are heap objects: Latin-1 bytes when every character fits, WTF-8 otherwise. Short
  strings are stored inside the string object.

### 1.5 Memory pages and layout

- Built: the heap uses huge pages (docs/collector.md, "Huge pages"). The program's code on huge
  pages (BOLT's `-hugify`) measured within noise on the Minecraft benchmark, and is not used.
- Hot fields of an object go first and cold ones last. Hot means read in loops or in many
  methods, by static count.

### 1.6 Order

Each part lands on its own and passes the gate (all checks and the 28 targets):

1. Built: uncounted `java::Arc` handles in a collected heap, replacing `Rc` and the counted
   handles for objects, with the enums and cells unchanged. This removed the counting and
   freeing code, which is where the measured third was. The collector scans conservatively.
2. Compact references (1.1): references of 4 bytes in the heap. Each stage below lands on its
   own behind `JAVA_TO_RUST_COMPACT_REFERENCES=1`, on the corpus and the small programs before
   the large targets, and the switch becomes the default once the gate passes with it. The
   order is by the bytes each stage saves in the live heap of the Minecraft server, measured by
   the census (`JAVA_GC_CENSUS=3`, docs/collector.md) after a 50,000-tick run, where ours was
   293 MB against Java's 186 MB. Of the 249 MB of slots that may hold references, references
   are 62 MB: 50 MB of `java::Thin`s and 12 MB of one-class handles; zero words, nulls among
   them, are another 99 MB. Planned stages, the most bytes first, except the first, which the
   others build on:

   1. Built: the placement, the decoding and the cells of 1.1, for the fields of celled
      classes. With the next stage it saves at most half the 12 MB of one-class handles.
   2. Planned: the plain fields of immutable and owned classes, read by copying the reference
      out instead of lending it in place.
   3. Planned: references to objects of several classes, in fields and in array elements: 4
      bytes instead of a `java::Thin`'s 8, about 25 MB of the 50, and up to 24 MB in reference
      arrays, whose 380,000 element buffers hold 44 MB, nulls included, against Java's 24 MB of
      reference arrays. The class number then has to live elsewhere. A 4-byte class word in
      each object that thin references reach (3.05 million) costs 16 MB, as it pushes 2 million
      objects of 38 classes into the next slot size: most of the gain. One class per block
      costs nothing per object but a partly filled block per class: the 60 largest of the
      10,537 classes hold 114 MB, the others 17 MB, about 1.6 KB each, so even a 4 KB page each
      would waste about 25 MB. The plan is pages of one class for the classes that fill them,
      and pages of mixed classes, with each slot's class number in the side table (2 bytes a
      slot), for the rest: dispatch reads the class from the side table by the object's address,
      a load that does not wait for the object's own. Small values held in the reference (1.4)
      keep a place: the heap starts at 4 GB, so a 4-byte value below 2^29 is never a reference,
      which leaves 29 bits for a value and its kind; larger values are boxed, which Java allows
      outside the cached range of `valueOf`.
   4. Built: arrays as one allocation, a 16-byte header in front of the elements, so reading an
      element takes one load after the handle's, as Java's does. In the census above the
      514,000 live arrays were each a 32-byte object and a separate buffer (16 MB of objects);
      the header saves about 8 MB of that, which a fresh census is to confirm. On the
      benchmarks (user instructions, `perf stat`): BlockReads' reads 4% fewer and the building
      of its chunks 11% fewer, TokenScanner 6.5% fewer, SortMillion and ParticleArray the
      same. An array whose elements take a power of two gets a slot of that size and 16 bytes
      (docs/collector.md); one whose elements fill whole 64 KB blocks takes a block more.
   5. Planned: an 8-byte slot size, for the objects that 4-byte references leave at 8 bytes
      (one reference and an `int`, two references): the 848,000 objects in 16-byte slots hold
      13 MB, of which 4-byte references save 0.6 to 5 MB.
   6. Planned if needed: reference maps, so that marking reads only the reference fields. A
      number that names a slot's start keeps that slot: 15,000 of the 20 million halves of the
      server's words that are not references do, which is to be measured as live bytes with the
      switch against without before deciding.
   7. Planned: objects on the stack in the fields of other objects of the same frame again
      (1.1, docs/collector.md, "Objects on the stack"), by keeping such a field at 8 bytes. With
      the switch, StackCaptures keeps 4 of its 8 objects on the stack and StackObjectsInLists 8
      of its 10.

   Of the 107 MB more than Java's, then, about 25 to 50 MB is references at 8 bytes and 8 MB was
   arrays in two parts (item 4). Most of the rest is objects that ours keeps and Java does not, outside
   this item: 108,000 `Class` objects against Java's 26,000 (8 MB more), NBT tags (57,000
   `StringTag`s against 5,400, 61,000 `ListTag`s against 218), 271,000 64-byte slots of numbers
   that no object owns (17 MB), `int[]` buffers of 10.9 MB against 2 MB, and 17% more chunk
   sections than Java at that point of the run.
3. Built: thin references for multi-class types, replacing the enums of handles, with dispatch
   on the class number.
4. Fields as cells, monitors and identity hashes in side tables, removing `RefCell` and
   `Locked`. Built: fields as cells, for one thread and for threads, and the elements of the
   arrays that threads share; the monitors still live in the `java::Monitored` of each object a
   monitor site may lock, and
   identity hashes are addresses.
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
   Built for methods (`analysis/escape.rs`): the `new`s whose objects never leave their method,
   and the methods that return a new object of their own, with a summary of each method's
   parameters over the calls that the classes of each receiver (`Class::receivers`, from
   `live_methods`) can run. A receiver escapes only through what its own class's methods do
   with `this`, and what an object holds in fields only its own methods read (a lambda's
   captures, an iterator's list) escapes only with it. On the Minecraft benchmark, 34% of
   javac's objects are made on the stack.
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
  too many to become locals, go in a region freed when the method returns. Built for single
  objects: an object that never leaves its method, or that a factory returns to a caller that
  keeps it, is made in a slot in the caller's frame (docs/collector.md, "Objects on the
  stack"). An object that leaves only on some paths, at a call or a store, starts in a slot too
  and is copied to the heap just before the statement where it leaves, as a JIT's partial escape
  analysis would. Regions for objects made in a loop and kept across its iterations are planned.
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
- **Profiled calls** (built): a call that the points-to facts leave on every class of its type
  can still meet one class almost always at run time, as `list.iterator()` on the Minecraft
  server's lists does, where a JIT's inline cache tests for that class and inlines its method.
  A translation made with `JAVA_TO_RUST_COUNT_DISPATCH=1` counts, at each such call, the class
  of each receiver (`java::count_dispatch`), and writes the counts to the file
  `JAVA_DISPATCH_COUNTS` names when the program exits: a line per call, keyed by the method it
  is in, its place in the source and the method called (`codegen::dispatch::profile_key`), with
  the counts by class number. A translation made with `JAVA_TO_RUST_DISPATCH_PROFILE` naming
  that file gives each call that saw one class in 90% of at least 1000 calls, or two in 95%, the
  inlined dispatcher among those classes that the points-to facts' small sets get (`_among_`):
  a test of the class number and a direct call the compiler can inline, with the whole
  dispatcher for any other class, so a profile that no longer fits the program costs speed,
  never correctness. Class numbers are the same in any translation of the same program
  (`dispatch::class_numbers`), so the counts name classes by them. Where the profiled class
  runs a factory (`iterator()`), its object can go on the caller's stack (docs/collector.md,
  "Objects on the stack"): the iterator of a loop over a list that is nearly always an
  `ArrayList` is then never allocated, as a JIT's escape analysis would do after inlining.
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
  escapes; for an object of a method's own frame this is built (see **Stack and method
  regions**).
- **Start-up snapshot**: static initializers that depend on nothing outside the program run at
  build time, and their objects are part of the binary.

## Measuring

Every landed part is measured on four workloads: hello world, the 47 corpus programs, the 540
corpus features, and the Minecraft benchmark. Each is measured for time and peak memory, and
always with identical output to the real javac's. The numbers to beat, and the translation's
with fat LTO, PGO and BOLT on the parts built so far:

| workload | real javac | translation now |
|---|---:|---:|
| hello | 0.19 s | 0.010 s |
| 47 programs | 0.63 s, 210 MB | 0.08 s, 99 MB |
| 540 features | 1.85 s, 430 MB | 0.54 s, 177 MB |
| Minecraft server | 11.3 s, 4.1 GB | 13.1 s, 1.36 GB |

## Risks

- Conservative stack scanning keeps an object alive when an integer happens to look like its
  address. That costs memory, never correctness.
- A Java reference stored where the collector does not look (a Rust collection in the runtime)
  would be freed while in use. The rule in 1.3 and the stress mode exist for that.
- Points-to analysis precise enough for these transformations, over a program and the JDK,
  costs translation time and memory. Precision lost costs speed, never correctness.
