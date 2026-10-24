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
- **Small blocks** hold slots of one size: one of 40 sizes from 16 bytes to 32 KB. Allocations
  that may hold pointers and those that cannot (an alignment below 8, as in the bytes of a
  string) use blocks of their own, and the collector does not scan the second kind.
- **Large allocations** take whole blocks.
- **Bitmaps.** Each small block has a bit per slot saying that the slot is taken (an object, or
  a free slot that a thread holds) and a bit per slot for marking.

## Allocation

A thread takes a slot from its own list of free slots of that size: a few instructions. A slot
freed by Rust is zeroed and goes on the list of the thread that frees it. When a list is empty,
the thread takes all the untaken slots of a block of that size under the heap's lock. Every
untaken slot is zero, so memory is handed out zeroed.

## Collection

A collection runs when the program has taken half as many bytes as were alive after the last one
(at least 64 MB), so the heap is about one and a half times what is alive. Collecting after as
much as is alive instead took the translated javac on the Minecraft benchmark from 59.7 s and
4.1 GB to 56.1 s and 5.4 GB. It:

1. stops every other thread with a signal (`SIGPWR`); each records where its stack is and waits;
2. marks every slot reachable from the roots, with an explicit stack, so a long chain of objects
   needs no recursion;
3. sweeps: zeroes the taken slots left unmarked and frees them, clears the marks for the next
   collection, lists the blocks with free slots, and gives the memory of blocks with nothing
   alive back to the system (neighbouring ones in one call).

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

## Marking and sweeping in parallel

Once more than 16 MB was alive after the last collection, helper threads mark and sweep with the
collecting thread: one per processor the program may run on, up to 15. They are started with the
C library's own `pthread_create` at the first such collection, so they are not the program's
threads: they are never stopped, hold no roots and never allocate. Between collections they wait
on a futex.

- A mark bit is set with an atomic `or`, so two markers that reach one object scan it once.
- Each marker has its own stack. When another marker is idle, a marker moves half of its stack,
  the oldest items first (they tend to lead to the most work), to a shared pool, from which an
  idle marker takes up to 256 items.
- A range longer than 64 KB (a large array) is split into 64 KB pieces as it is taken, all of
  which go on the stack at once, so other markers can take them.
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
  bytes alive (50 by default), which trades memory for time.

## Costs and limits

- An integer on a stack that happens to look like an address keeps its slot alive: memory,
  never correctness.
- A pointer kept where the collector does not look would be freed while in use. Everything the
  program allocates is in the heap, statics and thread-local storage are scanned, and the only
  memory outside them is the C library's own, which holds no handles except a starting thread's
  closure (covered above).
- The plan for a faster and smaller heap (objects without headers, 4-byte references,
  generations) is in docs/native-speed.md.
