# The collector

Translated programs free their objects with a tracing garbage collector, as the JVM does, not
by reference counting. It is the global memory allocator of every translated program
(`runtime/src/gc.rs`, installed with `#[global_allocator]`), so everything Rust allocates comes
from the heap it manages.

## Handles

An object is held by a `java::Arc<T>` (`runtime/src/shared.rs`): a pointer to the value and
nothing more. Copying a handle copies the pointer; dropping one does nothing, and the value is
never dropped, as a Java object is never destroyed. There is no count to change and no code that
frees a value when its last handle goes. Measured on the translated javac, counting took about a
third of the time and the code that freed objects two thirds of the binary.

Everything else Rust allocates (a `Vec`'s buffer, the bytes of a string) comes from the same
heap and is freed by Rust as usual, the moment it is dropped. Memory that an unreachable object
owned, and that Rust therefore never freed, is found unreachable too.

## The heap

- **Address space.** One range of 28 GB is reserved at start and used as needed, cut into
  blocks of 64 KB. It is placed from 4 GB to 32 GB of the address space when that range is free
  (`MAP_FIXED_NOREPLACE`), as it is in a Linux process, so that an object's address divided by 8
  fits in 4 bytes (4-byte references, docs/native-speed.md 1.1); else it goes where the system
  puts it, and a program with 4-byte references stops. A side table holds each block's kind,
  size and bitmaps, so objects carry no header.
- **Huge pages.** The range is aligned to 2 MB and asks the system for huge pages
  (`MADV_HUGEPAGE`; `JAVA_GC_HUGE_PAGES=0` leaves the system's default). Memory goes back to the
  system only as whole huge pages, when all 32 blocks of one are free. Giving back each empty
  64 KB block at every collection had the system fault that memory in and zero it again when the
  block was reused, and split the huge page around it: the translated javac kept 84 MB of its
  1.16 GB on huge pages, against 1.36 GB of 1.40 GB now, and runs the Minecraft benchmark in
  12.7 s instead of 13.6 to 14.3 s. The gain is the memory kept: asking for huge pages makes no
  measured difference on a system that gives them to every aligned range anyway. A free block
  whose memory stays keeps what it held, so a large allocation zeroes such blocks when it takes
  them.
- **Giving memory back.** A block that empties, in a sweep or when Rust frees a large
  allocation, keeps its memory. At the end of each collection, the whole free huge pages beyond
  the room the program takes before the next collection go back to the system, the highest
  first: that room is what the next collection waits for, less the untaken slots of small
  blocks and the free blocks on pages that stay. So the heap keeps what the next cycle fills
  again, and gives back what a program whose live objects shrank no longer needs. Giving every
  whole free page back as the sweep found it had the system fault that memory in and zero it
  again moments later: BinaryTrees, whose trees die between collections, ran 1.53 s instead of
  1.17 s (median of 7 alternating runs), and spent 0.14 s to 0.43 s in the system, the more
  where the system was short of huge pages.
- **Lowest first.** A new block is taken from the free run lowest in the heap, and each slot
  size hands out its partly free blocks lowest first, so the objects that live gather at the
  bottom and whole huge pages above them empty and go back to the system. On the Minecraft
  server's bench this lowered the peak from 1,030–1,089 MB to 1,013–1,020 MB, at the same tick
  speed (`JAVA_GC_LOW_FIRST=0` turns it off). The free runs are kept from the highest to the
  lowest, so the lowest one long enough is found from the end, at once for a single block, where
  a search of every run took 8% of ManyThreadsAllocating's instructions.
- **Code pages.** A second after the heap starts, then after waits that double up to 10 seconds
  (`JAVA_GC_CODE_PAGES=<seconds>` sets that most, 0 never), a collection gives the pages of the
  executable's code and constants back to the system once the program's threads run again. They
  are the file's own pages, which the loader does not write, so a page that is used again comes
  back from the system's cached copy of the file. A large program runs much of its code once, at
  startup: the Minecraft server held 181 MB of its 286 MB of code and 47 MB of its 85 MB of
  constants after its tick sprint, and its peak went from 1,030–1,089 MB to 953–957 MB, at the
  same tick speed. With every give-back 10 seconds apart, it held 172 MB of code and constants
  until the first, at 10 seconds, which was its peak (702 MB) once the heap was kept smaller
  (`JAVA_GC_GROWTH=50`). Given back every second (`JAVA_GC_CODE_PAGES=1`), it held 35–42 MB of
  them through startup and peaked at 646–671 MB, at its end; the doubling waits give startup's
  code back as soon, and the rest of the run as seldom as before.
- **Stack pages.** Each collection, while the threads are stopped, gives back the pages of each
  thread's stack below where it stopped (less 64 KB that the stop's handler still uses):
  nothing below a stack pointer is alive, and a page the thread reaches again comes back zeroed.
  Otherwise a thread keeps every page of stack it ever touched, so one deep call (world
  generation's recursion, a parser at startup) holds them for the rest of the run. The process's
  first thread, whose stack the system grows on demand, and the collecting thread keep theirs
  (`JAVA_GC_STACK_PAGES=0` turns it off).
- **Side tables on small pages.** The tables beside the heap that are reserved large and
  touched only at their start (the mark stacks, the list of free runs, the references) ask for
  pages of 4 KB (`MADV_NOHUGEPAGE`): on a system that gives huge pages to every aligned range,
  each took a whole 2 MB page at its first write, and the Minecraft server's 17 mark stacks
  (the collecting thread's, the shared pool's, 15 helpers') held 34 MB. The blocks' table stays
  on the system's pages: it is as dense as the heap, and marking reads it all over.
- **Small blocks** hold slots of one size: one of 54 sizes from 16 bytes to 32 KB, in steps of
  8 bytes up to 128, where most objects are, so that an object takes at most 7 bytes more than
  it needs, and then of a quarter of a power of two, with a power of two and 16 bytes beside
  each up to 8 KB, for an array whose elements take a power of two and its 16-byte header. A slot is aligned to 8 bytes, and to 16 when
  its size is a multiple of 16, which an allocation aligned to 16 takes. Three kinds of slots
  use blocks of their own: those that may hold pointers anywhere, which the collector scans
  whole; those that cannot (an alignment below 8, as in the bytes of a string), which it does
  not scan; and arrays of numbers (`gc::alloc_array`), whose 16-byte header may hold a pointer
  and whose elements cannot, of which it scans the header alone. So the numbers of a `long[]` or
  a `double[]` never keep an object alive by looking like its address.
- **Large allocations** take the blocks their size needs, and share the last one when they end
  1 KB (`TAIL_ROOM`) or more short of its end. The rest of that block, a tail, goes to the next
  large allocation, which starts there when the blocks after it are free (of the 8 latest tails),
  or to a new small block of slots under 1 KB, whose first slot is then past the tail's start.
  An array whose elements fill whole blocks ends 16 bytes into one more block, for its header,
  and shares that block, which the heap's huge pages make 64 KB of memory: LargeArrays, 700
  arrays of 64, 128 and 256 KB of elements, peaks at 87 MB, about what their elements take
  (88 MB; 134 MB with a block of its own for each array's end). A shared block records in the side table the allocation that ends in it
  (`tail_of`), below its own start: a word that points there keeps that allocation. Marking
  scans a large allocation's own bytes, not its blocks. When an allocation dies, its own blocks
  are freed, its first block becomes a tail again if another allocation ends in it, and its last
  block is freed if nothing else took the tail. A large allocation that `realloc` grows in its
  blocks does not grow into a shared last block. With generations, large allocations share no
  blocks: a minor collection finds the old ones by the blocks of the pages written.
- **Colors.** A small block's first slot is at one of eight cache lines past the block's start,
  chosen by the block's number, and each large allocation starts at one of the 64 cache lines of
  a page, in turn (`gc.rs`, `color`). With every block aligned to 64 KB, the first objects of
  each size would share their offset in a page, and the objects a program makes first are often
  the ones it uses most: two addresses that agree in their low 12 bits make the processor hold a
  load until an earlier store to the other is done ("4K aliasing"). In a loop that increments an
  element of an `int[]` and reads the array's length and buffer from its handle, all made first,
  that held each iteration's loads behind the last one's store: 2.05 s for 10^9 iterations, and
  1.21 s with colors, Java's time. A power-of-two slot stays aligned to its size, which
  allocations aligned to more than 16 bytes need, and blocks of 1 KB slots and larger are not
  moved. A large allocation's color stays within the room that the blocks its size needs leave,
  so a size of whole blocks starts at its first block: a `Vec` that doubles grows to sizes of
  whole blocks (128 KB), and colored it took one block more. The Minecraft server held 361 such
  allocations, 23 MB of blocks that only their colors took.
- **Bitmaps.** Each small block has a bit per slot saying that the slot is taken (an object, or
  a free slot that a thread holds) and a bit per slot for marking, in the side table: 1 KB per
  64 KB block, room for slots of 16 bytes. The two bitmaps of ages that generations need (below)
  are in a table of their own, which only a program with generations reserves; in the side
  table they took another 1 KB of every block, 1.6% of the heap.

## Allocation

A thread takes a slot of its size from its own free slots, a few instructions: from its span, a
run of neighbouring untaken slots of one block, handed out in order by moving a pointer to the
next one until it meets the span's end, or from the list of slots Rust freed (a freed slot goes
on the list of the thread that frees it). An object tries the span first and the list once the
span is empty (below); the global allocator tries the list first. When both are empty, the
thread takes the next run of untaken slots under the heap's lock: of its span's block, from where
the last run ended, or of a block from its class's list, or of a new block. That is one lock per
span, and touches no slot. A span takes no more slots than fit in 8 KB (`SPAN_BYTES`), one at least:
each thread keeps a span in every class it allocates in, and spans of whole 64 KB blocks held
the Minecraft server's 41 threads 28 MB of slots taken and never handed out. In a block of
young objects that died, the run is the whole 8 KB, 512 slots of 16 bytes, where a span of one
64-slot word of the bitmap, as before, took 64 and the heap's lock eight times as often:
BinaryTrees runs 30.5 G instructions instead of 31.9 G. The slots of a new span are zeroed right
away if they may hold pointers (below), which also brings them into the cache, and are fetched
into the cache (up to 4 KB) if they hold none, before the program writes them. Linking every
untaken slot of a block into a list, as the allocator did before, cost 5% of the translated
javac's time on the Minecraft benchmark, but it also brought the block into the cache: spans
without the prefetch were no faster, with it they are 3% faster (29.8 s to 28.9 s).

Where the heap's holes are short, a span is a few slots, and each costs a turn of the lock. So a
thread that takes a run under the lock also takes the untaken slots after it in the same 64-slot
word of the bitmap, as many as fit in the 8 KB with the run, and keeps them as bits of that word
(`Lists::span_rest`). When its span is empty, the next run of those bits becomes the span without
the lock (`span_from_rest`), about 40 instructions where a run under the lock takes about 300. In
ChunkSections, where loading a section makes short-lived objects of the sizes it keeps, the
sections that stay leave holes of about three slots, and at 24000 chunks the threads took 6.1
million runs of 3.3 slots on average, which was a quarter of the program's instructions; with the
rest of the word it runs 5.81 G instructions instead of 6.52 G (6.20 G instead of 7.13 G at 24000
chunks), while BinaryTrees, SmallObjects and ManyThreadsAllocating run as many as before.

An object (`java::Arc::new`) takes its slot inline (`take_slot`): its class and size are
constants of the object's type there, and a span's slots are zeroed already, so taking the span's
next slot is a load, a compare and a store. Everything else is one function, not inlined, that
is given the class and never returns null (`take_slot_cold`): the free list, which an object
takes from only once its span is empty, a new span (`refill`), and the census of types. In
`examples/allocation.rs` (pairs of 24- and 48-byte objects) an object takes 22.7 of the allocating
thread's instructions where all die young, against 34.2 when the inline take also tried the free
list, tested the slot for null, zeroed it and tested for the census; where one in eight is kept
and the spans are short, 58.9 against 75.9 with the collector's markers, and 120.1 against 138.0
with one marker, which then marks on the allocating thread. A slot of the free
list is zeroed as it is taken, and in the global allocator the zeroing ends in an empty `asm!`
that may read memory (`zero_freed`): inlined, without it the compiler drops the zeroes that the
value's store overwrites, and a store of a whole value counts as overwriting the bytes it leaves
undefined, an enum's smaller variant's and the padding, so those kept the words of the slot's last
object, which kept what they pointed to alive (the Minecraft server had about 60 MB more alive,
370 MB against 308, and ran slower). For an array and in the global allocator it is zeroed after
the closure that reads the thread's lists, not in it: with the zeroing in it, the closure grew
past what the compiler inlines, every such allocation called it, and ChunkSections ran 16% more
instructions (6.67 G against 5.72 G). Through the global allocator, a call that is not inlined and
works out the class from the size it is given, BinaryTrees spent about 40% of its main thread in
that call; inline, it runs 17.0 G cycles against 19.7 G, SmallObjects 2.06 G against 2.23 G, and
no other benchmark runs more (NodeList, MonitoredObjects, ClassCast and NBody 5-20% fewer). The
global allocator zeroes a slot of up to 128 bytes word by word rather than with a call of
`memset` (BinaryTrees 7.8 s against 9.4 s with the call, alternating runs on a throttled
machine). An earlier inline take, when the search of ObjectDispatch's palette was still a loop
the compiler specialized for the key's class, lost that specialization (4.0 s against 1.8 s);
the search is now a call of the runtime (`java::search_same`), and the benchmarks lose nothing.

The collector keeps, after marking, the slots of each thread's spans not handed out yet, those it
took beyond them in the word of its last run, and the last one handed out, which a thread stopped
in `alloc` may be handing out before its address is in a register; the marks it gives them go
once the sweep is done, so that what the thread makes in them is young. A thread may stop while
it makes a span of the slots beyond its last one, without the lock: the span has no start, which
the collector skips, while its bounds change, and its slots leave the word's bits only once they
are the span, before the first is handed out. A span that handed out slots to Rust code, through
the global allocator's `alloc`, keeps all it handed out until the thread takes a new span, and
their marks stay until the next full collection: some are held only by memory the marking does
not read. A thread's std name is one: std keeps it in the thread's `Thread`, which it allocates
with the C library's allocator, and the test harness names each test's thread. A span that handed
out only objects and arrays keeps only its last slot, so that what weak references refer to dies
(WeakReferencesCleared waited in vain when every span kept all). A name taken from a free list or
an older span is still lost, and the thread's end then frees its slot a second time, so the
runtime gives its own threads no std names; the runtime's tests, whose threads the harness names,
rely on the names coming from the current span. Reading the C library allocator's memory would
close this. A slot that may hold pointers is zeroed whole before it is handed out, so no word
left from an old object, in a part the program does not write (the spare capacity of a `Vec`, an
enum's unused bytes, padding), keeps anything alive: a span's slots all at once when the thread
takes it (`fill_span`), after the heap's lock is let go, as a collection meanwhile keeps them and
scans none, and a slot of the free list as it is taken. A slot of a pointer-free block is zeroed
only for `alloc_zeroed`. Zeroing at allocation rather than when a slot is freed touches memory
the program is about to write anyway: zeroing dead slots in the sweep took 0.8 s of the
translated javac's 30 s on the Minecraft benchmark, mostly on cache lines nothing else touched; at
allocation the sweep takes 11 ms and the run is 0.4 s faster.

## Collection

A collection runs when the program has taken as many bytes as were alive after the last one (at
least 64 MB), so the heap is about twice what is alive, times a scale for programs that spend
most of their time collecting: while a collection takes more than half as much processor time
(all its markers') as the program's threads used since the last one, the bytes taken before the
next grow by a tenth, up to twice, or up to 256 MB where twice is less (`SMALL_ROOM`): a program
with little alive that collects often may take more between collections, while a large heap
stays within twice; while it takes less, they shrink back. The small step keeps a
short phase of costly collections from growing the heap. BinaryTrees, which marks a tree of up
to 85 MB alive on every processor, so collects 38 times instead of 70 and runs 1.06 s instead of
1.18 s, at a peak of 279 MB instead of 217 MB (Java: 2 GB); SmallObjects 0.131 s instead of
0.152 s at 203 MB instead of 171 MB (Java: 812 MB). The room past twice takes BinaryTrees from
38 collections to 26 and 15% fewer instructions (21.8 G against 25.6 G), at a peak of 340 MB
against 245 MB (Java: 1.8 GB); SmallObjects, ManyThreadsAllocating, NodeList and ChunkSections
collect as often as with twice. On the Minecraft benchmark the optimized
translated javac takes 14.5 s and 1.03 GB collecting after half as much as is alive, 13.1 s and
1.36 GB after as much, and 12.4 s and 2.0 GB after twice as much (the Java VM: 11.3 s, 4.1 GB). It:

1. stops every other thread with a signal (`SIGPWR`); each records where its stack is and waits;
2. clears every mark, then marks every slot reachable from the roots, with an explicit stack, so
   a long chain of objects needs no recursion;
3. sweeps: frees the taken slots left unmarked (clears their bits), lists the blocks with free
   slots, and frees the blocks with nothing alive; then gives back the whole free huge pages
   beyond what the next cycle takes (above). The marks stay until the next collection, which is
   what the young generation (below) builds on; the slots threads hold free lose theirs.

The roots are scanned conservatively: any aligned word that points into a taken slot keeps that
slot, which makes interior pointers and pointers held in registers work. The top 16 bits of a
word are ignored, as they hold the class's number in a reference to an object of several
possible classes (`java::Thin`, docs/object-model.md 1.2). The roots are:

- the stacks of the threads, from the stack pointer each recorded when it stopped, and the
  collecting thread's own stack and registers;
- the program's static data: the writable segments of every loaded object;
- each thread's static thread-local storage;
- the free lists of every thread, whose slots are kept without being scanned.

Slots are scanned whole, word by word, except in blocks that cannot hold pointers, and but for
the header in blocks of arrays of numbers (a large array of numbers too: its header, at the
start the block's table records). Any word that points into a slot keeps the slot, wherever it
points: a loop's view of an array's elements or a slice of them lent to the runtime, which point
past the header, keep the whole array. In a program
with 4-byte references (`java::CompactAllocator`, docs/native-speed.md 1.1), each half of each
word scanned, in a slot or a root, is also read as one: it keeps a slot only if, times 8, it is
the slot's start, since such a reference never points inside an object. A number that happens to
name a slot's start keeps that slot too; in the census of a run of the small-objects benchmark,
about 234 of every 10 million halves did (`JAVA_GC_CENSUS`, "lookalikes").

Marking is compiled four times over, for whether other markers run (and mark bits are set
atomically) and whether words are also read as 4-byte references, and picks one per drain of
its stack, so that neither is tested per word and each slot taken off the stack is scanned
inline rather than in a call. A word pointing into a small block, by far the most common, is
tested for that kind first, and the bytes of each slot to scan are kept in the block's table.
With one marker, BinaryTrees runs 34.6 G instructions instead of 43.8 G and SmallObjects 4.9 G
instead of 6.5 G, 1.79 s instead of 2.09 s and 0.22 s instead of 0.24 s (fastest of 5). The other
kinds are marked as rare (`std::hint::cold_path`): without it the compiler tested the kind
through a table of jumps, an indirect branch per word. And the slot's mark bit is read before
its taken bit: a marked slot is always taken, and a word that points to a slot marked already,
as many do, then reads one line of the block's bitmaps instead of two. With one marker, the two
took the marking benchmark (`runtime/examples/marking.rs`: 300 MB alive in tables of chained
entries, 6.7 million slots) from 1,272 M to 1,101 M instructions per collection, and its fewest
cycles from 399 M to 331 M.

## Objects on the stack

An object that never leaves the method that makes it (`analysis/escape.rs`) is made in a
`java::Slot` in that method's frame instead of the heap: the method declares `let slot_1 =
java::Slot::<Point>::new();` at its start, and where Java says `new`, `slot_1.put(Point::new(1,
2))` makes the object there and returns the same `java::Arc` handle that the heap would, pointing
into the frame, so no other code changes. A method that returns a new object of its own, such as
`List.iterator()`, has a second form that makes it in a slot its caller passes,
`iterator_on_stack(this, &slot_2)`, used where the caller keeps the object. So does a
constructor that makes its object itself (docs/object-model.md 1.4).

The collector needs nothing for them:

- **What they point to stays alive.** A slot is in a live frame, between the stack pointer its
  thread stopped at and the stack's top, so its words are scanned as roots like any local.
- **They are never collected.** A handle to a slot points outside the heap's address range, and
  the marker drops every word outside it (`Marker::candidate`): the slot is never marked,
  scanned as a heap slot, or freed.
- **Nothing reaches them once the frame is gone.** The analysis proves that no handle to the
  object is stored in a field, an array element, a static or a closure, returned, thrown, or
  passed to a call that may keep it; anything it does not model counts as kept. A call's
  result may be the arguments its targets return; for a call on an object of the frame, only
  what that object's class runs counts: the element that a list's iterator's `next()` gives
  back is not taken for the iterator, though another iterator class's `next()` returns itself.
  The one field it may be stored in is one of another object of the same frame that only that object's own
  methods read (a lambda's capture, an iterator's list), when that object is kept no longer;
  in a program with 4-byte references not even that, since a field's 4 bytes reach only the
  heap.
  The object is then held only by the method's variables and its other objects.
- **A slot in a loop** is filled again on every iteration, so the object of the previous one
  must be unreachable by then: every variable that may hold the object must be declared inside
  the body of the innermost loop around the `new`, which makes it a new variable on each
  iteration that Java does not let the next iteration read before assigning it, and every
  object of the frame that holds it must be made in that loop and follow the same rule. A
  variable is one declaration, by Java's scopes: another variable of the same name declared
  outside the loop (`for (Node n = head; ..)` before a loop's `Node n = new Node(..)`) is
  another variable, which neither holds the object nor counts against it. A
  factory's object is made outside every loop of the factory, once per call, and the caller's
  slot then follows the caller's loops by the same rule.
- **Moved to the heap where it leaves.** An object that leaves only at some calls or stores may
  still start in a slot: the random `BlockPos` of each random tick of the Minecraft server, which
  only a block that ticks keeps. Before each statement where it may leave, `slot_1.to_heap(&mut
  pos)` copies it into the heap and points the variable there; once the variable names the heap
  it does nothing. The copy is made out of line and gives back the new handle, which `to_heap`
  writes into the variable: the out-of-line code never sees the variable's address, so the
  variable stays a value in a register rather than a place in memory that an unknown function may
  keep, which every acquire read of a field would make the code load again, and clear of its
  class number again, at each use. In a register, it is known to name the slot until the move,
  so the test in `to_heap` and the clearing of the class number before each field read fold
  away: `benchmarks/RandomTickPositions.java`, the random tick's position read and kept on a
  rare path, went from 893 to 866 million instructions. Such a statement is a call that is given the variable, or a store of it into
  a field, an array element or a static, that names the variable nowhere else and assigns it
  nowhere; and the object is held by that one variable, its identity hash is not read before (by
  the method, its callees, or the factory that made it), and with those statements taken for
  none it would stay. Other locals may hold it too when each is declared in a block inside the
  variable's that no move is in, such as the temporary `argument$0` of a call that computes its
  arguments first (below): at a move such a local is not declared yet, and is given the copy, or
  is gone. Until the move nothing that is read after it names the object but the variable, so no
  code can tell the copy from the original: every later read of the variable reads the copy, and
  the original is left in the slot unread. The variable is declared in the loop's body, so a move in one
  iteration never reaches the next one's object, which the slot is filled with again. An object
  that would be moved in the block it is made in, or a plain block inside it, is made in the
  heap, as it would always be copied. A call whose receiver and arguments are computed into
  temporaries first (`argument$0 = supplier;`) counts as one statement with them, so a variable
  given to a temporary there moves before it.
  A move before a call that dispatches can be guarded by the receiver's class: when a profiled
  run saw nearly every receiver of one class, and what that class runs keeps the object nowhere
  and reads its identity hash nowhere, the object is moved only for a receiver of another class
  (`if !matches!(p.0.tag(), 3) { .. to_heap .. }`). Such a move is not counted as always
  copying, so the supplier given to the server's profiler in every entity's tick, which the
  usual profiler ignores, stays in its slot. A move that the generator cannot write before its
  statement makes it write the method again with that object in the heap.
- **Identity.** The identity hash is the address, unique while the object lives; two objects of
  one slot in a loop, never alive at once, may share it, which Java allows. An object moved to
  the heap gets a new address, so it is moved only when its identity hash was not read before.
  A call that hashes it reads its identity hash only if what its class runs for `hashCode` does:
  a `BlockPos`'s own `hashCode` does not, `Object`'s does. A `synchronized` block, `wait` or
  `notify` on an object counts as an escape; a `synchronized` method called on one locks it by
  its address or by the lock inside it, as on the heap, and unlocks it before it returns.
- **Threads.** Passing an object to another thread is an escape, so a stack object is only ever
  used by the thread whose stack holds it. So is every object that stays in its method, in a slot
  or not, and methods called on one of a thread-shared class read and write its fields without
  atomic actions ([multi-threading.md](multi-threading.md) 2.1, "Objects that one thread uses").
- **What it owns.** A slot is never dropped, as a heap object is not: memory its fields own (a
  `Vec`'s buffer, in the heap) is found unreachable once no frame or object points to it.

**Fields read where the object is made.** A handle into a slot hides the slot from the compiler:
a handle of a class with subclasses carries its class number in its top bits, which a read
masks off, and the compiler cannot tell the masked address from the slot's; a field of a class
whose objects threads may share is read with an atomic load, to which it forwards no store
either. So each field read through the handle is a load of what was just stored. So where a local is declared with an object made
in its slot (`P p = new P(..)`, or the call of a factory whose every `return` is the `new` that
makes its object), the primitive fields that the rest of the local's block reads, `p.x` or by a
getter whose body for the object's class is `return this.x;`, are read from the slot into locals
right there, `let p_x = *unsafe { slot_1.object() }.x.fixed();`, and those reads read the
locals, with plain loads. The compiler forwards the slot's stores to them and keeps the values
in registers, as Java's scalar replacement does, and may drop the stores when nothing else reads
the slot. This needs
the object made in a plain value (a constructor that hands out `this` is left out) and put in
the slot for certain (a factory that may return another object, or a call of a dispatcher, is
left out), and a read takes a local only if it comes before every other mention of the variable
(a call given it, a store, an assignment of it or of its fields, which a move to the heap comes
before) and is in no loop that has one. Until that mention nothing but the variable names the
object, so no code can write its fields, and the local is their value. The random `BlockPos` of
each random tick of the Minecraft server is read so, and moves to the heap later, when a block
that ticks keeps it.

**A handle that names no object until the object leaves.** With its fields in locals, a moved
object's slot is read by nothing but a move, yet the compiler still writes every field into it
on every path: the handle that `put` gave holds the slot's address, as a number once it carries
a class number, and a move hands the slot to code out of line, so for the compiler the slot is
memory that any call may read, where a store must be made before the next call whether a move
follows or not. So where the local is mentioned nowhere but in the reads that the locals took
over and in the statements that a move comes before, none of them guarded, its handle is made to
name no object right after those reads, `pos = unsafe { slot_1.unnamed_of_class(pos) };` (`unnamed`
for a handle that is the address alone): its address becomes one that is no object's, its class
number stays. A move, `slot_1.to_heap_unnamed(&mut pos)`, makes a handle that names no object
name a copy of the slot's object in the heap, and leaves one that names the copy as it is; the
copy is made of the object's value, read where the compiler sees the slot, so the slot's address
goes nowhere at all. The slot is then a local like any other: its fields stay in registers, and
the object is written only where it is copied, as Java's compiler writes an object that leaves
on a rare path only there. Nothing reads through the handle before a move, since every mention
of the local is one of those reads or comes after a move in its statement; should the generator
name the local anywhere else while it writes the method, it writes the method again with the
object in the heap (`Gen::local`). `benchmarks/RandomTickPositions.java` runs 754 instead of 843
million instructions. A local that is also read after a place where it may have left, given to
a call that only reads it, or compared, keeps the slot's handle.

A slot holds an object of a class that is not a Rust generic, an owned value or a closure, and
of at most about 128 bytes of fields (16 references, or 32 `int`s), and a method's slots take
at most about 1 KB, eight of the largest, so that a method that recursion runs deep keeps a
small frame. The body of a lambda that is written as a closure
(docs/closures-and-nested-classes.md) declares slots of its own at its start, so each call of
the closure, also one that the closure makes of itself, has its own objects. A call of a
factory is written in its form on the stack only when it is one of the plain forms of a call (a
static method, a method of the receiver's class, or a body on an enum without a dispatcher), or
a call that dispatches among a few classes (the points-to facts' small sets, or a profiled
call's classes before its whole dispatcher, docs/native-speed.md): the arm for the class that
runs the factory passes the slot on. Any other is written as usual, its object goes in the
heap, and the slot stays empty. A profiled call may run several methods, and is a source in its
caller when what its profiled class runs is a factory: the other methods count for its
arguments and what it returns, and their objects go in the heap. Translating with
`JAVA_TO_RUST_HEAP_ONLY=1` puts every object in the heap.

## Marking and sweeping in parallel

Once more than 16 MB was alive after the last collection, helper threads mark and sweep with the
collecting thread: one per processor the program may run on, up to 15. They are started with the
C library's own `pthread_create` at the first such collection, so they are not the program's
threads: they are never stopped, hold no roots and never allocate. Between collections they wait
on a futex.

- A mark bit is set with an atomic `or`, so two markers that reach one object scan it once.
- Each marker has its own stack. When another marker is idle, a marker moves half of its stack,
  the oldest items first (they tend to lead to the most work), up to 4,096 items, to a shared
  pool, from which an idle marker takes up to 256 items. Both are one copy under the pool's spin
  lock, whose waiters read it until it is free before they try to take it.
- A range longer than 64 KB (a large array) is split into 64 KB pieces as it is taken, all of
  which go on the stack at once, so other markers can take them.
- A slot taken off a marker's stack waits in a ring of 16 while its memory is prefetched, its
  first cache line and the line of its last word (within its first 256 bytes, `PREFETCH_LAST`),
  and is scanned as it leaves the ring, so the cache misses of reading sixteen slots overlap
  instead of coming one after another. On the Minecraft benchmark a ring of 8 that fetched first
  lines took the translated javac's marking from 3.9 s to 2.7 s in all (median of runs; 16 such
  slots did no better). On the marking benchmark with one marker, a slot's last line took 5% off
  the fewest cycles of a collection, 16 slots 2%, and both together 12%. With the two changes to
  the test of a word above, the benchmark marks in 479 M cycles instead of 612 M with one marker
  (medians of 8 alternating runs on a busy machine), and its 16 markers take 146 ms of processor
  time per collection instead of 176 ms, and 17.6 ms instead of 22.0 ms at the fastest.
- Holding the words found in a slot that point into the heap in a ring of their own, to fetch
  each one's block description before testing it, made marking slower: 95 to 110 ms instead of
  79 ms with one marker on the marking benchmark, the bitmaps it then waits on being in other
  lines than the description. Marking with plain stores instead of atomic ones, which is
  wrong (two markers may each lose the other's bit), saved no more than 5% with 16 markers, so
  a byte per slot, which would make plain stores safe, is not worth its memory.
- An idle marker spins a little, yields a little, and then sleeps on a futex. A marker putting
  work in the pool wakes one sleeper for each 256 items, and the end of marking wakes them all.
  A heap that hands out no work (one long linked list) is marked by one thread while the others
  sleep. Idle helpers that kept spinning and yielding, and woke all at once to queue on the
  pool's lock, took `SmallObjects` (a linked list of 2 million nodes and an array of 1 million
  objects, collected 22 times) from 0.36 s to 0.64 s and from 1.1 s to 6.0 s of processor time,
  and `BinaryTrees` from 2.16 s to 4.27 s and from 5.7 s to 32.8 s (medians at a load average of
  about 6 on 16 cores).
- Marking ends when every marker is idle and the pool is empty: no marker can then make work.

Sweeping is shared in chunks of 256 blocks: each thread sweeps the blocks of a chunk and records
in each block what lives; the collecting thread then lists the blocks and frees the empty ones.

On a test heap of 545 MB of small objects held by one array, eight threads mark in 23 ms instead
of 104 and sweep in 13 ms instead of 23. A heap that is one long linked list cannot be marked in parallel and marks about 25%
slower than alone, the cost of the atomic mark bits.

## Generations

With `JAVA_GC_GENERATIONS=1`, collections are minor where the system allows it. An object
that lives through two collections keeps its mark and is old. A minor collection marks from the
roots and from the old objects on the pages written since the last collection, stops at marked
objects, and frees the young ones left unmarked. The old objects of a written page are found a
word of the bitmaps at a time, from the slots both taken and marked: a page of young objects
only, as most written pages are, costs a few word reads. Testing each slot's bits one by one
took BinaryTrees' minor collections 10 ms each (at about 850 MHz), for the 64 MB of young
objects written since the last; they now take under a millisecond. A full collection, as
above, runs once a quarter more (plus 1 MB) is alive than after the last full one, at once
after the minor one that finds it, in the same pause.

- **Ageing.** At the end of each collection (`age` in `gc.rs`), an object that lived through
  it for the first time loses its mark and is recorded as having lived through one (a bitmap
  beside the marks, as is one of the old objects); one that had already becomes old. So an
  object alive at a collection only because the program was in the middle of using it does not
  become old: BinaryTrees' tree being built at each collection, which made old objects that
  died at once and a full collection every second time (13 of 77, marking 710 ms in all, at a
  load average of 17), now 3 of 69 and 206 ms. An object that lives on is marked once more,
  at the collection after it was made and the next: SmallObjects, whose 91 MB of objects all
  live to the end, marks them once more.
- **Pointers to objects not old yet.** An old object may point to one that lived through one
  collection. That pointer was written after the younger object was made, so in the interval
  before the last collection or since: on a page written since, or on one the last collection
  found written and protected. A minor collection scans the old objects of both; the runs of
  pages each collection protects are kept for the next (up to a million runs, beyond which the
  next collection is full).

Which pages were written, the kernel says: the heap is registered with a `userfaultfd` in
asynchronous write-protect mode, each collection protects the pages that now hold old objects
that may hold pointers, and a write to one of them unprotects it in the kernel, with no thread
woken; `PAGEMAP_SCAN` on `/proc/self/pagemap` lists the pages not protected (`track_writes`,
`written`, `protect_old` in `gc.rs`). Every write counts, whatever code, thread or system call
makes it, so nothing is emitted in the program. Everything Rust allocates is in the heap (the
collector is the global allocator), and memory outside it that can hold a reference is the
roots, scanned whole at every collection; docs/native-speed.md (1.3) lists it. Where the kernel
refuses (before Linux 6.7, or a seccomp filter), every collection is full. It is opt-in until it
has been measured on the Minecraft server: it saves processor time on programs whose old objects
are many (`SmallObjects`, `BinaryTrees`), but keeps old objects that died until the next full
collection, which raises the peak of some.

## References

Soft, weak and phantom references (`java.lang.ref`, translated from the JDK's sources with a patch
of `Reference` and `ReferenceQueue`) are real: the collector clears a referent that nothing else
keeps (`gc/references.rs`, `weak.rs`).

- **The cell.** The referent of `java.lang.ref.Reference` is a `java::WeakField`, which holds its
  word inverted (null stays 0), so marking, which reads every word of a slot as a possible
  reference, never follows it, and a minor collection's scan of written pages does not either.
  `get` decodes it, and stores the collector's clock into the cell beside it when it differs,
  for the soft policy below.
- **The registry.** As its constructor ends, a reference registers itself
  (`TranslatedReferences.register`): its handle, the address of its cell, its kind and whether
  it has a queue, in a table outside the heap, under the heap's lock. A `FinalReference` is not
  registered: the translation runs no finalizers, so it keeps its referent as long as it lives,
  and is never cleared or enqueued. Nor is a reference to `null`. A reference never lives on a
  method's stack (`analysis/escape.rs`), as the registry holds its address in the heap, and
  neither does what it holds in its other fields, such as a `Cleaner`'s action or the value of a
  `WeakHashMap` entry: the analysis takes a reference as escaping, and so what is stored in it
  (`corpus/whole-program/ObjectsHeldByReferences.java`).
- **A collection**, after marking and before the sweep, (1) marks the references that wait to
  be put on their queues; (2) keeps the soft referents that the policy keeps and marks what they
  reach, until no more are kept, so that an object both softly and weakly reachable stays with
  its soft reference; (3) forgets the references that died and those whose referent is null, and
  clears every other reference whose referent is not marked, adding it to the pending list if it
  has a queue. In a minor collection old objects keep their marks, so only young referents are
  cleared, and every soft referent is kept.
- **The soft policy** is HotSpot's (`SoftRefLRUPolicyMSPerMB`): at a full collection, a soft
  referent that nothing else keeps is cleared when the bytes alive after the last collection are
  past `JAVA_GC_SOFT_LIMIT` MB (by default a quarter of the machine's memory), or when it was last
  read longer ago than `JAVA_GC_SOFT_MS_PER_MB` milliseconds (1000 by default) per MB that the
  live heap is below that limit. The clock is the milliseconds from the heap's start to the start
  of the last collection.
- **Queues.** No Reference Handler thread runs until a thread waits in `ReferenceQueue.remove`,
  which starts it: `ReferenceQueue.poll` first moves the pending references onto their queues
  itself, so a program that only polls, as `WeakHashMap` does, starts no thread. That matters
  beyond the thread: a program that may start one is translated as a threaded program, with
  process-wide statics and shared objects (docs/multi-threading.md), so a `WeakHashMap` must not
  make a single-threaded program threaded. With nothing pending, a poll's check costs one atomic
  load.
- **Threads stopped mid-read.** Threads are stopped by a signal wherever they are, so one may be
  stopped between loading a referent's inverted word and decoding it. Once any reference is
  registered, every word of the threads' stacks and registers is therefore also read inverted,
  which may keep a referent for one more collection, never lose one. Thread-locals, like
  statics, are memory the program writes, never a load in progress, and are read once.

What a program can see: a referent that nothing else keeps is cleared at some collection after
the last strong use, as on a JVM; when depends on when the collector runs, which here is when the
program has allocated enough (`System.gc()` does nothing), and on the conservative scan of the
stacks, which may keep a referent that a copy of its address left on a stack points to. A
`WeakHashMap` loses the entries of keys that nothing else keeps, and a cache of soft references
keeps what it read lately while memory is plentiful.

## Threads

The runtime's `pthread_create` takes the C library's place: it registers the new thread (its
stack and thread pointer) as the first thing the thread does, and no collection runs between the
call and the registration, while the closure the thread runs is only known to the C library. A
thread unregisters when it ends; its later frees during its exit go straight back to the heap.

## Testing it

- `JAVA_GC_EVERY=<bytes>` collects after that many bytes instead, down to a collection every
  few allocations, so that a pointer the collector misses shows quickly as a crash or wrong
  output. A change to the collector or the runtime is checked by running the corpus with it set.
- `JAVA_GC_THREADS=<n>` marks with `n` markers in all, and with more than one marks in parallel
  at every collection however small, which with `JAVA_GC_EVERY` tests the parallel marking.
- `JAVA_GC_LOG=1` writes a line per collection to standard error: whether it was minor or full,
  bytes taken since the last, bytes alive, heap size, the time spent marking and sweeping, the
  scale, the spans the threads took since the last (those under the heap's lock, the rest from
  the bits beyond a run, `span_from_rest`) with their slots on average, and the bytes of each
  kind of root (statics, thread-locals, stacks) and the time their scans took, apart from marking
  what they reach, and how many markers marked and the share of their time they had no work.
  `JAVA_GC_LOG=2` adds, after each full collection, the bytes alive in each slot size and in
  large allocations, and the blocks each takes.
- `JAVA_GC_GENERATIONS=1` with `JAVA_GC_EVERY=1` makes most collections minor ones, at every
  refill of a thread's slots: the corpus run so is the test most likely to catch a write the
  young generation misses. Running a program under `systemd-run --user --wait --pipe -p
  SystemCallFilter=~userfaultfd -p SystemCallErrorNumber=EPERM` tests the fallback to full
  collections.
- `JAVA_GC_CENSUS=<level>` reports after each full collection what the live slots hold
  (`gc/census.rs`): with 1, how many of their words are zero, references (a `java::Thin` or the
  address of a slot's start), the buffers of `Vec`s and boxed slices, numbers below 2^32 and other
  words, and what 4-byte references would save, three ways, from a low bound to a high one, and
  how many 4-byte halves of the other words would read as a 4-byte reference to a live slot (the
  numbers a scan for such references without types would take for references), and in a
  program with 4-byte references the bytes they take; with
  2, also per slot size, and per class number of the objects that `java::Thin`s reach; with 3,
  also per Rust type, each type's objects and the buffers they own (an array's elements, a plain
  string's bytes), and the slots of no type by size, to set beside a JVM's class histogram. Level
  3 records each allocation's type from the start, in a table beside the heap of 2 bytes per 16,
  with spans of one slot, so that every object reaches the out-of-line path that records it
  (`take_slot_cold`); without it the cost is one load and branch per allocation other than an
  object's.
  With 4 (`gc/who.rs`), also what holds the objects of the types that `JAVA_GC_CENSUS_WHO`
  names (parts of type names, comma-separated, or `untyped:<size>` for the slots of that size
  of no type that no object owns): the holders' types (an object, the buffer an object of a type
  owns, a slot of no type), how many objects no slot holds (the roots keep them), and for two of
  them a chain of holders back to a root, naming it (static data, thread-local storage, a
  thread's stack). `JAVA_GC_CENSUS_FROM=<n>` reports only from the nth full collection on.
- `JAVA_GC_GROWTH=<percent>` sets the bytes taken between collections as a percentage of the
  bytes alive (100 by default), which trades memory for time.
- `JAVA_GC_TIME=<percent>` sets the share of processor time above which that grows (50 by
  default); 0 turns the growth off. `JAVA_GC_LOG` shows the scale of each collection.

## Costs and limits

- An integer on a stack that happens to look like an address keeps its slot alive: memory,
  never correctness.
- A pointer kept where the collector does not look would be freed while in use. Everything the
  program allocates is in the heap or, for objects on the stack, in a frame that is scanned;
  statics and thread-local storage are scanned, and the only
  memory outside them is the C library's own, which holds no handles except a starting thread's
  closure (covered above).
- The plan for a faster and smaller heap (objects without headers, 4-byte references) is in
  docs/native-speed.md.
