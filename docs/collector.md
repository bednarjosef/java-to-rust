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

- **Address space.** One range of 64 GB is reserved at start and used as needed, cut into
  blocks of 64 KB. A side table holds each block's kind, size and bitmaps, so objects carry no
  header.
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
- **Small blocks** hold slots of one size: one of 47 sizes from 16 bytes to 32 KB, in steps of
  8 bytes up to 128, where most objects are, so that an object takes at most 7 bytes more than
  it needs, and then of a quarter of a power of two. A slot is aligned to 8 bytes, and to 16 when
  its size is a multiple of 16, which an allocation aligned to 16 takes. Allocations that may
  hold pointers and those that cannot (an alignment below 8, as in the bytes of a string) use
  blocks of their own, and the collector does not scan the second kind.
- **Large allocations** take whole blocks.
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
  a free slot that a thread holds) and a bit per slot for marking.

## Allocation

A thread takes a slot of its size from its own free slots, a few instructions: first from the
list of slots Rust freed (a freed slot goes on the list of the thread that frees it), then from
its span, untaken slots of one 64-slot word of a block's bitmap, held as a mask of bits, the
lowest one handed out next. When both are empty, the thread takes the next word with untaken
slots under the heap's lock: of its span's block, or of a block from its class's list, or of a
new block. That is one lock per span, and touches no slot. A span takes no more
slots than fit in 8 KB (`SPAN_BYTES`), one at least: from slots of 1 KB a word covers its whole
block, and each thread keeps a span in every class it allocates in: with whole words, the
Minecraft server's 41 threads held 28 MB of slots taken and never handed out, about one 64 KB
block per thread and class of the larger sizes. The slots of a new span
are fetched into the cache right away (up to 4 KB), before the program writes them. Linking every
untaken slot of a block into a list, as the allocator did before, cost 5% of the translated
javac's time on the Minecraft benchmark, but it also brought the block into the cache: spans
without the prefetch were no faster, with it they are 3% faster (29.8 s to 28.9 s).

The collector keeps every taken slot of each thread's spans, after marking: the slots not handed
out yet, and the one a thread stopped in `alloc` may be handing out before its address is in a
register; the others the span handed out live one collection longer. Only the span's own
slots are kept, not the rest of its word, which from slots of 1 KB is the whole block. A slot that may hold
pointers is zeroed whole as it is handed out, so no word left from an old object, in a part the
program does not write (the spare capacity of a `Vec`), keeps anything alive; a slot of a
pointer-free block is zeroed only for `alloc_zeroed`. Zeroing there rather than when a slot is
freed touches memory the program is about to write anyway: zeroing dead slots in the sweep took
0.8 s of the translated javac's 30 s on the Minecraft benchmark, mostly on cache lines nothing
else touched; at allocation the sweep takes 11 ms and the run is 0.4 s faster.

## Collection

A collection runs when the program has taken as many bytes as were alive after the last one (at
least 64 MB), so the heap is about twice what is alive. On the Minecraft benchmark the optimized
translated javac takes 14.5 s and 1.03 GB collecting after half as much as is alive, 13.1 s and
1.36 GB after as much, and 12.4 s and 2.0 GB after twice as much (the Java VM: 11.3 s, 4.1 GB). It:

1. stops every other thread with a signal (`SIGPWR`); each records where its stack is and waits;
2. marks every slot reachable from the roots, with an explicit stack, so a long chain of objects
   needs no recursion;
3. sweeps: frees the taken slots left unmarked (clears their bits), clears the marks for the next
   collection, lists the blocks with free slots, and frees the blocks with nothing alive,
   giving their memory back to the system where that empties a whole huge page.

The roots are scanned conservatively: any aligned word that points into a taken slot keeps that
slot, which makes interior pointers and pointers held in registers work. The top 16 bits of a
word are ignored, as they hold the class's number in a reference to an object of several
possible classes (`java::Thin`, docs/object-model.md 1.2). The roots are:

- the stacks of the threads, from the stack pointer each recorded when it stopped, and the
  collecting thread's own stack and registers;
- the program's static data: the writable segments of every loaded object;
- each thread's static thread-local storage;
- the free lists of every thread, whose slots are kept without being scanned.

Slots are scanned whole, word by word, except in blocks that cannot hold pointers.

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
  passed to a call that may keep it; anything it does not model counts as kept. The one field
  it may be stored in is one of another object of the same frame that only that object's own
  methods read (a lambda's capture, an iterator's list), when that object is kept no longer.
  The object is then held only by the method's variables and its other objects.
- **A slot in a loop** is filled again on every iteration, so the object of the previous one
  must be unreachable by then: every variable that may hold the object must be declared inside
  the body of the innermost loop around the `new`, which makes it a new variable on each
  iteration that Java does not let the next iteration read before assigning it, and every
  object of the frame that holds it must be made in that loop and follow the same rule. A
  factory's object is made outside every loop of the factory, once per call, and the caller's
  slot then follows the caller's loops by the same rule.
- **Identity.** The identity hash is the address, unique while the object lives; two objects of
  one slot in a loop, never alive at once, may share it, which Java allows. A `synchronized`
  block, `wait` or `notify` on an object counts as an escape; a `synchronized` method called on
  one locks it by its address or by the lock inside it, as on the heap, and unlocks it before it
  returns.
- **Threads.** Passing an object to another thread is an escape, so a stack object is only ever
  used by the thread whose stack holds it.
- **What it owns.** A slot is never dropped, as a heap object is not: memory its fields own (a
  `Vec`'s buffer, in the heap) is found unreachable once no frame or object points to it.

A method declares at most 8 slots, and only for objects of up to 16 fields of a class that is
not a Rust generic, an owned value or a closure. A call of a factory is written in its form on
the stack only when it is one of the plain forms of a call (a static method, a method of the
receiver's class, or a body on an enum without a dispatcher); any other is written as usual, its
object goes in the heap, and the slot stays empty. Translating with `JAVA_TO_RUST_HEAP_ONLY=1`
puts every object in the heap.

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
- A slot taken off a marker's stack waits in a ring of 8 while its memory is prefetched, and is
  scanned as it leaves the ring, so the cache misses of reading eight slots overlap instead of
  coming one after another. On the Minecraft benchmark this took the translated javac's marking
  from 3.9 s to 2.7 s in all (median of runs; 16 slots did no better).
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
- `JAVA_GC_LOG=1` writes a line per collection to standard error: bytes taken since the last,
  bytes alive, heap size, and the time spent marking and sweeping.
- `JAVA_GC_GROWTH=<percent>` sets the bytes taken between collections as a percentage of the
  bytes alive (100 by default), which trades memory for time.

## Costs and limits

- An integer on a stack that happens to look like an address keeps its slot alive: memory,
  never correctness.
- A pointer kept where the collector does not look would be freed while in use. Everything the
  program allocates is in the heap or, for objects on the stack, in a frame that is scanned;
  statics and thread-local storage are scanned, and the only
  memory outside them is the C library's own, which holds no handles except a starting thread's
  closure (covered above).
- The plan for a faster and smaller heap (objects without headers, 4-byte references,
  generations) is in docs/native-speed.md.
