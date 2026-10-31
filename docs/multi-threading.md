# Multi-threading: threads, monitors, shared objects

How a Java program that starts threads is translated: which objects become thread-shared, how
monitors, statics, interrupts and uncaught exceptions work, and why the result is a legal
execution of the Java program. It builds on the object model (`docs/object-model.md`: class
trees as enums, objects as `Arc<RefCell<C>>`, `Arc<C>` when immutable, plain values when owned) and
on the exceptions design (`docs/exceptions.md`: exceptions are `Result`, `?` propagates).

The user's requirements: programs are normally multi-threaded, so real threads must work and the
design must leave room for optimizations that use threads. Generated code is safe Rust, prints
what Java prints, and whatever cannot be proven keeps a correct general representation.
`java.util.concurrent` is translated from the JDK's source over hand-written natives, not
hand-written itself.

## 0. The decisions in one page

1. **A program is multi-threaded exactly when it can start a thread**: some reachable method
   calls the native behind `Thread.start()` (`analysis/threads.rs`, `threaded`). Every other way
   of running code on another thread (executors, `ForkJoinPool`) ends there. A program that
   cannot start a thread is translated exactly as without this work: `RefCell`, statics as
   plain `java::Global` cells, `synchronized` as a null check. It pays nothing.
2. **Stronger synchronization than Java's is always allowed, and it is all the translation
   uses.** Every access to memory that another thread can reach goes through a lock or an atomic
   with at least acquire/release ordering, never `Relaxed`. Every execution of the Rust program
   is then a sequentially consistent interleaving of the Java program's actions, which Java
   allows (section 1). A lock the translation adds is never held while the thread waits for
   anything else, so it cannot add a deadlock (1.3).
3. **The fields of a thread-shared object are `java::SharedField` cells**, read and written by
   value, each access one atomic action: acquire loads and release stores of a word, sequentially
   consistent ones for a `volatile` field, a striped lock for a wider value. The object is an
   `Arc<C>`, or where a monitor site may lock it an `Arc<java::Monitored<C>>`, its fields and its
   monitor's lock. Every object is held by a `java::Arc` (`shared.rs`, docs/collector.md),
   thread-shared or not. The generated text is the same as for an object of one thread, whose
   fields are `java::Field` cells; only the type names differ. An immutable shared class is
   `Arc<C>`, read without a lock. Which classes are shared is decided per class, by type (section
   4).
4. **An object's monitor is the lock of its `java::Monitored`.** `synchronized` holds it for the
   region; field accesses take no lock. Monitors are kept only where the program can tell them
   from none, and only the objects that such a site may lock have one (section 3).
5. **The statics that a started thread can touch are process-wide `static` items**:
   `java::Atomic<T>` for primitives, `java::SharedField<T>` for objects and strings, each access
   one atomic action with no lock (sequentially consistent for a `volatile` static), and a
   `java::ClassInit` per class that runs its static initialization once for the process.
   The statics of every other class stay thread-local cells, which only the main thread uses.
6. **`Thread` and `ThreadGroup` are classes written in Java in the translator**
   (`crates/translator/java/Threads.java`), over natives in the runtime
   (`crates/runtime/src/thread.rs`). A program may extend `Thread`. Everything else in
   `java.util.concurrent` (`LockSupport`, the atomics, `ConcurrentHashMap`, `ForkJoinPool`,
   `ThreadLocal`) is the JDK's own code, translated in a whole program; its `Unsafe` and
   `VarHandle` accesses are lowered to accesses of the field or element they name (5.3).
7. **An uncaught exception in a thread ends that thread, not the program**: it goes through the
   uncaught-exception handlers and prints `Exception in thread "Thread-0" ...` as Java does, and
   the other threads go on. `main` waits for the non-daemon threads before the process ends.

## 1. Correctness basis: the Java memory model in Rust

### 1.1 The argument

The Java memory model (JLS 17.4) allows every sequentially consistent execution of a program,
and more (racy reads may see stale values). Rust's model is C++20's: a data race is undefined
behaviour, and safe Rust cannot write one, since every value two threads can reach is behind a
`Sync` type. So a translated program has no undefined behaviour by construction; what remains to
show is that each of its executions is one Java allows.

The translation maps every Java memory action to one of:

| Java action | Rust |
| --- | --- |
| read/write of a field of an object only one thread reaches | plain read/write |
| read/write of a field of a thread-shared object | its `java::SharedField`: an acquire load or release store of a word, under a striped lock for a wider value; a fixed field's read borrows the value |
| read/write of a `volatile` field of a thread-shared object | its `java::SharedField`, sequentially consistent |
| read/write of an element of an array that threads share | the element's cell (`java::SharedArray`): an acquire load or release store of a word, under a striped lock for a wider value |
| `Unsafe`/`VarHandle` get, put, compare-and-set, get-and-add on a field | one atomic action of its `java::SharedField` (5.3) |
| `Unsafe`/`VarHandle` get, put, compare-and-set, get-and-add on an element | one atomic action of the element's cell, sequentially consistent in the volatile mode (5.3) |
| monitor enter/exit, `wait`, `notify` | the object's reentrant lock and its wait set (section 3) |
| `Thread.start`, `join`, `park`/`unpark` | `std::thread` spawn, the thread record's condition variables |
| static field access | `java::Atomic` or `java::SharedField` statics: acquire/release, sequentially consistent for a `volatile` static; a `java::Final` static's one write is a release, and its reads acquire |
| `Unsafe.fullFence` and the `VarHandle` fences | `fence(SeqCst)` |

Each is at least as strong as the Java action: a lock makes the access part of a total order,
and acquire/release makes every read-from edge a happens-before edge. An execution of the Rust
program is therefore an interleaving of the Java program's actions in which every read sees the
last write before it: a sequentially consistent execution, which JLS 17.4.8 admits. Adding
synchronization removes behaviours and never adds one, except for progress (1.3).

### 1.2 Where the argument needs care

- **`long` and `double` tearing.** Java allows a non-volatile 64-bit write to be two 32-bit
  writes. A value under a lock or in a 64-bit atomic never tears: fewer behaviours. Only 64-bit
  targets (x86-64, aarch64) are supported.
- **Final fields.** In safe Rust a reference crosses threads only through a lock, a release
  store or a spawn closure, each of which publishes everything the constructing thread did
  before, so every field is seen initialized. Fewer behaviours.
- **Out-of-thin-air values.** No `Relaxed` access is emitted for program memory. With every load
  acquire and every store release, a cycle of reads seeing writes would make happens-before
  cyclic, which the model forbids. On x86-64 acquire loads and release stores are plain `mov`s.
- **Lazily published objects** (`if (cache == null) cache = compute();` without `volatile`): the
  field's store is a release, and the read that finds it an acquire, so the referent's fields are
  published with it. The racy idiom becomes correct; two threads may still both compute, as in
  Java.
- **Compound operations on racy fields** (`count++` without synchronization): the read and the
  write are two atomic actions, so updates may be lost between them, as Java allows.
- **Granularity.** A translation may hold a lock across several Java actions, which removes
  interleavings; it must never implement one Java action as several unsynchronized steps that
  another thread can observe half-done. Every multi-step form is inside one lock acquisition.
- **Identity hash codes** of shared objects come from the `Arc` payload's address: stable while
  the object lives, and never equal for two live objects.

### 1.3 Progress and deadlock

Java guarantees no fairness, so a thread that never gets a lock is legal; a deadlock, though, is
observable, and the data locks are locks Java does not have: those of shared builders and of
the objects of a Java enum's class tree (`java::Locked`), and the striped locks of wide fields,
statics and array elements. The rule that keeps them invisible is the one `RefCell` already
imposes: **a data lock is held only while values are read or written, and never while the thread
waits for anything else.**

- An object's fields and a shared array's elements take no data lock: an access is one atomic
  action of the field's or element's cell, and a striped lock is held only to copy or replace one
  value, whose copy reaches no other cell. A bulk operation on a shared array (a copy, a fill, a
  sort) takes each element on its own.
- A method of a shared class takes `&self` only if it is short: no loop, no call of a method or
  constructor, no access to another object or array, no blocking native (`analysis/threads.rs`,
  `long_methods`). Any other method takes the object's handle.
- In a threaded program two borrows of shared objects, builders or statics in one statement count
  as conflicting even when neither is mutable (`codegen/objects.rs`, `conflicts`), and a blocking
  native counts as one, so the operands are read into locals first. A conditional expression
  whose condition reads two of them is guarded the same way, so it never holds both locks at
  once.
- A lock read through another object (`synchronized (this.lock)`) is copied out of its cell
  before the thread waits for its monitor (`java::enter(&this.lock.get())`); the same holds for
  `wait` and `notify`.

In a build with debug assertions (the corpus test's), a wait of more than `WATCHDOG` (20
seconds) for a data lock is this rule broken and panics with "translation bug: ..." instead of
hanging (`crates/runtime/src/locked.rs`). A thread waiting for a lock held as a monitor is not
watched, since Java may hold a monitor across anything.

A monitor is Java's own lock: a deadlock on monitors is one the Java program has too. Java's
monitor is reentrant and so is ours; `wait` releases all recursion levels and takes them back.

## 2. Representations

### 2.1 Thread-shared classes

Besides shared (`Arc<RefCell<C>>`), immutable (`Arc<C>`) and owned (a plain value), a class may be
*thread-shared*: some object of it may be reached by two threads (section 4).

| | thread-local | thread-shared |
| --- | --- | --- |
| mutable | `Arc<C>` of `java::Field`s | `Arc<C>` of `java::SharedField`s; `Arc<java::Monitored<C>>` of them where a monitor site may lock it (3.1) |
| immutable (no field written after construction) | `Arc<C>` | `Arc<C>` |
| owned | plain value | plain value, moved into the thread |
| closure type (`docs/closures-and-nested-classes.md` 4.2) | `Rc<dyn Fn(..) -> R>` | `std::sync::Arc<dyn Fn(..) -> R + Send + Sync>` |

The `Arc` of objects is the runtime's `java::Arc`: `std::sync::Arc`'s shape without weak
references, whose count changes are plain loads and stores until the program starts its first
thread and atomic from then on, by the same argument as the locks' plain path (`locked.rs`,
`THREADS`). Closures keep `std::sync::Arc`, because making a `dyn` handle from a sized one needs
the unstable `CoerceUnsized`.

A `java::SharedField<T>` has `java::Field`'s API (`get`, `set`), so the generator writes the same
text for both forms: `o.n.set(o.n.get() + 1)`. A value of 1, 2, 4 or 8 bytes without drop glue
(a primitive, an object's handle, a string with an identity) is an atomic word, loaded with
acquire and stored with release, which on x86-64 are plain moves; any other value (a closure, a
string without an identity) is copied and replaced under one of 64 striped spin locks, picked by
the field's address and taken only once a second thread runs. A field that only constructors
write (a fixed field, [object-model.md](object-model.md) 1.8) is read where it is, `fixed()`,
with no lock and no copy, outside the constructors and field initializers; a fixed number,
boolean, char or enum so read is a plain load, which the optimizer can merge and hoist. A
`java::Monitored` object holds the fields first and then a pointer to the monitor's lock, made
the first time a thread takes it (`LazyMonitor`), so the object's address is its fields', as
`&self` of a method sees it. A class tree is celled, shared and monitored together, so an enum over it
lends every variant's fields struct as `&XFields`. A class tree with a Java enum keeps its
`java::Locked`, whose `RefCell` API (`borrow`, `borrow_mut`, mapped guards) the generator writes
as for a `RefCell`. Identity is `Arc::ptr_eq`; `Option` for null is unchanged.

**Volatile fields** of a thread-shared object and **volatile statics** of the process are read
and written in sequential consistency (`get_volatile`, `set_volatile`), as Java's are. In a
single-threaded program `volatile` is nothing.

**Objects that one thread uses.** An object of a thread-shared class that never leaves the method
that makes it (`StackObjects::objects` of the escape analysis, [collector.md](collector.md),
"Objects on the stack") and is never moved to the heap is used only by the thread that runs that
method: handing an object to another thread is a way of leaving, since a thread reaches objects
only through what the runtime is given when it starts and through fields and statics. Its fields
then need none of the atomic actions above. A call on a receiver that always holds such an object
(`analysis/escape/confined.rs`: a local whose every declaration and assignment is such a `new`,
or in a lambda's method the field that captured one) calls the method's confined form,
`<name>_confined`, which reads and writes the fields of `this` with `get_confined`,
`set_confined` and the plain forms of the atomic updates (`compare_and_set_by_confined`,
`get_and_update_confined`, ...), and calls the confined forms of what it calls on `this`: a
counter `AtomicInteger` that a method keeps for a lambda it passes down sets it with a plain
store, not an exchange, and its `incrementAndGet` is a plain increment, not a locked one. Only
the methods that such calls may run and that do something atomic to `this` (an access of a
`volatile` field, an update through `Unsafe`, or a call of such a method on `this`) get the
form, since the other accesses are plain moves already, and only those that make no object (no
lambda, whose `this` is another object). The decision is per object, but
only for objects that stay in their method; an object that a field holds, such as a random
generator that one thread's code reaches through a shared object, keeps the atomic accesses
(section 7).

### 2.2 Arrays and builders

In a threaded program an array that is not a plain `Vec` (see the array analysis) is a
`java::SharedArray<T>` when two threads may reach arrays of its family, and a `java::Array<T>`
(an `Arc` of a `RefCell<Vec<T>>`) otherwise. The family is the type of the innermost elements, so
`int[]` and `int[][]` are decided together and a `SharedArray` never holds an `Array`. A family is
shared by the same rule as builders below: a static holds such an array, or a field or capture of
a shared class does, or such an array becomes a shared `Object` (`analysis/threads.rs`,
`shared_arrays`). In a whole program the JDK's statics hold arrays of the common families (`int`,
`char`, `byte`, `String`, `Object`), so those stay shared there. The runtime's array functions take
either form through the `Elements<T>` trait (`crates/runtime/src/array.rs`).

A `SharedArray` keeps each element in a cell of its own, a `java::SharedField` (an `Arc` of
`java::SharedElements`, a boxed slice of cells, and no lock), as a thread-shared object keeps its
fields: `a[i]` and `a[i] = v` are one atomic action each, an acquire load or a release store of
a word for an element of 1, 2, 4 or 8 bytes without drop glue (a primitive, `long` and `double`
too, or a reference), and a copy under a striped lock for a wider one (a string without an
identity). No access takes a lock or checks a borrow flag. Java makes each element access atomic
(a `long` or `double` one may even tear) and promises nothing about the order or atomicity of a
bulk operation, so `System.arraycopy`, `Arrays.fill`, a sort or a copy reads and writes a shared
array one element at a time, and another thread may see it half done, as on the JVM.

No slice of a shared array's elements is ever lent, since another thread may write them while it
is in use. A runtime function that the generated code gives an array as a slice (`&a.borrow()`
of an `Array`, `&v` of a plain array) gets a `java::Lent` of a shared array instead
(`crates/runtime/src/lent.rs`): a function that reads or writes a range of it
(`new String(chars, offset, count)`, `System.arraycopy` with a plain array, a file read into a
buffer, `getChars`) takes it as a `java::Source` or `java::Target`, reads only that range out of
the cells and stores only the elements it writes; any other function gets a copy of the whole
array through `Deref`, and a `LentMut` stores back the elements that changed when the call ends. A
method parameter that the array analysis borrows as a slice (`&[T]`) is never given a shared
array: such a parameter stays a handle (`analysis/arrays.rs`, `lends`).

A `StringBuilder` is a `java::SharedStringBuilder` (its units in an `Arc<java::Locked<..>>`,
whose monitor is that lock) when two threads may reach builders: a static holds one, or a field
or capture of a shared class does (in an array or as a type argument too), or a builder becomes a
shared `Object` or `CharSequence` (`analysis/threads.rs`, `shares_builders`). Otherwise every
builder stays a `java::StringBuilder`, even in a threaded program, and its monitor is the address
of its cell.

### 2.3 Statics

A thread-local static gives each thread its own copy, so a static that a started thread can touch
is process-wide (`crates/runtime/src/statics.rs`, `codegen/statics.rs`):

| static | form | access |
| --- | --- | --- |
| compile-time constant | `const` | `X` |
| primitive | `static X: java::Atomic<i32> = java::Atomic::new(0);` | `X.get()`, `X.set(v)`, acquire/release; `get_volatile`, `set_volatile` for a `volatile` one |
| object or string | `java::SharedField<T>`, behind a `LazyLock` when its initializer is not a constant | `X.get()`, `X.set(v)`; `get_volatile`, `set_volatile` for a `volatile` one |
| object or string that only its class's initialization assigns | `java::Final<T>`, made with `java::Final::lazy` when its initializer is not a constant | `X.get().clone()`, or the place `(*X.get())` (`get_prepared` for an eager class's, which `main` prepares, `get_settled` for one that `main` initializes first); `X.set(v)` |
| array that only its class's initialization makes, and that the program only indexes to read and measures | `java::Final<java::Frozen<T>>`, or `java::Frozen<T, N>` when every array it is given has the constant length `N` | `java::get(&(*X.get()), i)`, `java::len(&(*X.get()))`; `X.set(java::freeze(a))`, `X.set(java::freeze_fixed(a))` |
| a class's "initialized" flag | `static C_INIT: java::ClassInit` | `if !C_INIT.start() { return; }` .. `C_INIT.finish();`; a use tests `C_INIT.done()` (`C_INIT.ready()` where the `<clinit>` keeps its own state) |

A static field's read or write is one action in Java, never a region, so a static of the process
is a cell of one value, as a field of a thread-shared object is (2.1): an object's handle, an
array's, a builder's or a string with an identity is a word, loaded with acquire and stored with
release, and any other value (a string without an identity, a closure) is copied or replaced
under a striped spin lock. No access takes a lock that another could wait for, and none runs Java
code, so a read cannot see half a write. A compound assignment (`log += s`, `n++`) reads and then
writes, two actions, as Java's does; `volatile` makes each of them sequentially consistent, not
the pair atomic. Such a static is never lent where it is: a read copies its value out, so a later
write cannot change what an earlier read holds. Behind a lock, each read would take it with a
compare-and-swap and give it back with an exchange, two full fences and two calls: the Minecraft
server's main thread reads `Util.timeSource` and `BlockableEventLoop.delayedCrash` on every turn
of the loop that waits for the next tick, and there the exchange alone was 0.8% of its
instructions.

A `static final` of object, array or string type that no code but its class's static
initialization assigns (`Facts::final_statics`, from `analysis/statics.rs`) is a `java::Final`:
the value the initializer's one assignment sets, the value it starts with (Java's default, or the
initializer's value for a class initialized lazily, built on the first read when it is not a
constant), and a pointer to whichever is current, which the assignment sets with release. Its
value never changes once the class is initialized, so a read takes no lock: `get()` loads the
pointer, tests that it is set and lends what it points at, the value set or the starting value
before that, which is what a read during the initializer sees (null for a static not yet
assigned, as in Java). There is no `LazyLock` around it, whose own test each read would pay. An
assignment made before the program starts its first thread also writes the value's address to
`settled`, a plain field that a read tries first: every thread starts after that write, and
nothing writes it once one has, so the plain read races with nothing, and the compiler may keep
it from one read to the next, which it does not do with an acquire load. The slower path, an
acquire load of the pointer, is a call out of line, so that a function reading the static several
times reaches each later read only from the first read or from that call, and the compiler reads
`settled` once.

A class initialized once threads run never has its statics settled, and there the test and its
call cost more than the load: they split the code around each read, and the values that are live
across the call leave their registers. A noise function that reads a table 8 times a call ran
290 instructions a call with them and 221 without. So an eager class's `java::Final` statics are
prepared instead (`prepared_static` in `codegen/statics.rs`): `main` first calls `X.prepare()` on
each, which points the pointer at the starting value, before any thread exists, and a read is
`X.get_prepared()`, the acquire load alone, with no test, since nothing unsets the pointer. The
starting value of an eager class's static is Java's default, cheap to make; a frozen array of a
constant length `N` starts with `N` default elements. One whose elements are objects that are
never null has no default to fill it with, and is read with `get()`.

A class whose static initialization Java could not tell from an earlier one is initialized by
`main` before anything else, before any thread exists (`Facts::early_init`, `statics::early`), so
that its statics are settled, and a read is `X.get_settled()`: the plain load of `settled`, with
no test, which the compiler keeps from one read to the next. After running these
initializations, `main` checks that each such static is settled (`check_settled`) and panics
otherwise, so that no read without a test ever finds it unset; the class's own `<clinit>`, which
runs before that check, reads its statics with `get()`. NoiseTree's gradient table, read 8 times
a noise call, is such a static: its run took 6.81 G instructions with the test of each read and
5.64 G without. The test of a class is conservative, a program in which nothing of the
initialization is observable:

- its `<clinit>` (the code inside the `try` of a rewritten one, `lower::clinit_errors`) is only
  assignments to statics of its class, and, for an enum whose fields are tables
  (`lower/enum_tables.rs`), calls of its constant builders that only fill rows of its tables
  with their parameters or literals (`inert_builder`);
- each assigned value or argument reads only literals, constants of any class, statics of its
  class assigned before, and constants of enums, its own or one that is not eager or is
  initialized this way too,
  with operators that cannot throw (no integer division or remainder), new arrays of a literal
  size or of literal elements, and `new` of objects whose constructor (`plain_constructor`)
  only assigns fields of the new object its parameters or literals, of a class that has no
  superclass in the program, no field initializer but a literal, and no static initialization
  that runs eagerly: no other call, no lambda, no other class's static but constants, nothing
  that throws but `OutOfMemoryError`;
- no superclass of it, nor a superinterface that it initializes with it, initializes eagerly.

Since such a class is initialized before any code that could use it runs, nothing tests whether
it is: its static methods and constructors start with no call of its `init`, and a read of its
statics or constants from another class has none either (`Facts::initialized_first`). The code
that runs before, the other such initializations, names nothing of it but constants and the
values of its enum constants, which need no initialization. In javac, the enums of tree tags and
type tags are such enums, whose constants nearly every method reads.

A class that prints, reads a static another class changes, or may throw keeps its initialization
at its first use (corpus `threads/EarlyStaticInitializers`), and so does an enum whose constants
are built from such an enum's constants (corpus `threads/EarlyEnumTables`). Only a program that
starts threads does this, as only its statics are `java::Final`s. Where the read is only borrowed
(the object of a call or a field access,
an array that is indexed or measured, a string that is lent, a null test), the generator writes
the place `(*X.get())` (`final_place` in `codegen/statics.rs`) instead of a clone; a static that
starts empty is `(*java::non_null(X.get().as_ref()))`. The protocol of `ClassInit` around it is
unchanged: a reader from another class calls the class's `init` first, which waits for a run in
progress. The Minecraft server's noise functions index a `static final` array millions of times
from every worker thread; behind a lock, most of those threads stood in it. A small program of
eight threads doing the same runs 25 times faster in a debug build, and over 100 times faster in
a release one, with a `java::Final` than with a lock. Against a `java::SharedField`, a read of a
`java::Final` copies nothing and lends the value in place.

Such a `static final` that is a one-dimensional array, which every assignment gives a new array
(`{..}`, `new T[] {..}`, `new T[n]`) and which every expression of the program only indexes to
read an element or measures (`S[i]`, `S.length`, never `S` alone, `S[i] = v` or `S[i]++`), is a
frozen array (`Facts::frozen_statics`, `statics::frozen`): no name but the static reaches it, so
nothing writes its elements once the initializer has made it. Its `java::Final` holds a
`java::Frozen<T>`, the elements in a slice of cells, which the initializer's assignment makes from
the new array (`java::freeze`) and publishes with the `java::Final`'s release. A read is a plain
load from the slice, with no atomic load: an acquire load of a `SharedArray`'s element would keep
the compiler from reusing the array's address and length across reads. Where every array the
static is given has the same constant length (`new T[16]`, a literal of 16; `Facts::frozen_lengths`,
`frozen_fields::static_lengths`), it is a `java::Frozen<T, 16>` (`java::freeze_fixed`), whose
length is a constant: an index the compiler can bound below it, as `h & 15`, needs no check, as
the JIT, which takes the array for a constant, checks none. A static that may be read before it
is assigned (null then) keeps its `SharedArray`.

An instance field that is a one-dimensional array, which only its class's constructors give
arrays, each a new one, and write elements of, through `this`, and which the rest of the program
only indexes to read and measures, is frozen too (`Facts::frozen_fields`,
`analysis/frozen_fields.rs`): a `java::Frozen<T>` or `java::Frozen<T, N>` field, which the
constructor writes in place while the object is a value it holds (a class whose constructors
build a plain value, not celled; the field never null). Once the object is built nothing writes
the array, and the object is published to other threads with the array in it, so its elements
are read with no atomic load, as the JIT reads a `final` field's array. A store `this.f[i] = v`
whose value may have an effect holds the array in a local first (`lower::argument_order`), a
copy of the `java::Frozen` handle that shares its elements.

`java::ClassInit` is the JLS 12.4.2 procedure: the thread that finds the class not started runs
the initializer, another thread finding it in progress waits, and the same thread finding its
own initialization in progress returns at once (a recursive initialization sees defaults, as in
Java). `std::sync::Once` cannot do the last case. `start`/`finish` have the shape of the
thread-local flag's guard, so the initializer body is generated once for both forms. A use of
the class tests one word before anything else, one acquire load (a plain move on x86-64) in the
inlined `init` function: `done()`, or `ready()` for a class whose `<clinit>` keeps its own state
of failure (`docs/exceptions.md`), whose run finishes as `READY` only when that state says it
succeeded. A loop that uses several classes tests each on every pass, so the test is kept to the
one load: a program whose loop calls a static method of each of two such classes ran 28
instructions a pass when the test also read the kept state, and 22 with `ready()`.

The statics that are process-wide are decided per class (`Facts::process_statics`): those of
every class whose statics the code of a started thread may read or write, or whose static
initialization it may run. That code is what a second walk of `analysis/live_methods.rs` finds
(`thread_statics`), after the walk from `main` that decides which methods can run. It starts from
what the runtime runs on a new thread (the call of `run` that lowering puts into
`Native::ThreadStart`), from the dispatch of an exception that ends a thread
(`Native::InstallUncaught`) and from the exceptions the runtime raises, and follows calls with the
first walk's facts: the values known for every call and write of running code, the started
threads' included, and every class that running code creates objects of, since a thread may be
handed any of them. It starts with no class initialized, as a thread may be the first to use one:
a static read or write, a static call, a `new` or an enum constant runs the class's static
initialization in the walk, and the class's statics become process-wide. A class's statics are
process-wide or thread-local together, with its flag, so that the thread that initializes a class
writes the copies that every thread reads. When the walk meets something its facts do not answer,
every static is process-wide.

Every other static stays a thread-local cell (and an eager class's flag a `Cell<bool>`), which only
the main thread uses: no code of another thread reads or writes it, and no other thread runs its
class's initialization. The walk follows calls by the facts of `object_values`, which do not tell
one call of a method from another: a thread's code that calls a library method that the program
also calls elsewhere (`Iterable.forEach`, `ThreadLocal.get`, a class's static initializer) reaches
every lambda and class that any caller hands it. In javac, whose one thread is the file manager's
deferred close (`BaseFileManager.deferredClose`), that is nearly all of the compiler, and about 200
of its 3,000 statics stay thread-local.

### 2.4 Objects used as locks

`new Object()` used as a lock is an `Arc<java::Monitored<PlainObject>>`: a class whose values may be
a kept lock is neither immutable nor owned, and neither is any class of its tree, so that an enum
over the tree reaches all its objects one way and each has a lock. Class monitors (static
synchronized methods, `synchronized (C.class)`) are statics of the translation, `static C__MONITOR:
java::Locked<()>`, entered with `java::enter_class(&C__MONITOR)`.

## 3. Monitors

Code: `analysis/monitors.rs` (which monitors are kept), `lower/sync.rs`, `codegen/sync.rs`,
`crates/runtime/src/monitor.rs` and `locked.rs`.

### 3.1 Which monitors are kept

A monitor site (a `synchronized` block or method, `wait`, `notify`, `holdsLock`) keeps the
monitor of its lock where the program can tell it from none:

- in a program that waits, notifies or asks `Thread.holdsLock`, at every site, since those answer
  as Java's only when the monitor is held;
- otherwise, at the sites whose lock is of a class that threads may share, or a builder where
  builders are shared;
- class monitors in any program with threads.

Everywhere else `synchronized` is the null check of its lock, so a program without threads or
`wait` translates as before. `synchronized (this)` in a constructor keeps no monitor: no other
thread has the object yet.

Only the objects that a site keeping its monitor may lock have one (`analysis/lock_values.rs`):
the classes of the lock's type, of `this` in a synchronized method, and of the objects the lock
can be, found from where its value comes from. `new C()` is an object of `C`; a field holds what
any assignment to it stores (reflection, deserialization and `Unsafe` write fields in code that
lowering wrote out, so their stores are among them); a local variable what the method gives it;
a constructor's or static method's parameter what its callers pass; a cast, `?:` and an
assignment pass their operands' on. What is not followed (another method's parameter, which a
call through an interface or the runtime may pass, an array's element, a call's result) is any
object of its type (`hierarchy::flows`), so a lock of type `Object` that a call returns makes
every class whose objects become `Object`s keep monitors. A class tree keeps them together, so
the other objects of a locked class's tree have one too. `corpus/threads/WaitThroughInterface`
and `WaitThroughArraysAndCalls` wait on objects reached each of these ways.

An array's monitor is kept in any program with threads. An array that threads share
(`java::SharedArray`), like a `java::Monitored` object, makes its lock the first time a thread
takes it (`LazyMonitor` in `monitor.rs`, a pointer that stays null for the many objects and
arrays never locked, eight bytes where the lock is sixteen); one that a single thread uses keeps
its monitor by address, as a `RefCell` object does.

Rejected, by name: `synchronized` on a string, a box, a Java enum's constant, or the class of a
value (`o.getClass()`) where the monitor would be kept (they are values without a lock), and
`wait`, `notify` or `holdsLock` on a string or box anywhere. A string, box or `Class` held as an `Object` has no monitor either, since
the translation keeps them as values; where a locked `Object` may hold one of them as well as the
program's objects (as `Collections.synchronizedSet(set, mutex)` locks what it is given), the
objects keep their monitors, and locking, waiting on or notifying such a value at run time ends
the program with "synchronizing on, waiting on or notifying a String, a boxed value or a Class
held as an Object is not supported by the translation" (`java::unkept_monitor`).

### 3.2 One form: the monitor is the object's lock

An object that a site keeping its monitor may lock (3.1) is an `Arc<java::Monitored<C>>`, whose
reentrant lock is its monitor. Its fields take no lock, so the monitor is the only lock its
object has. (A builder and
a `java::Locked` object have their data lock as their monitor, and an array its elements', as 3.1
says.)

- `synchronized (x) { .. }` is `{ let _monitor = java::enter(&x); { .. } }`. The guard drops
  however the block ends, so `?`, `break` and `return` release it as `monitorexit` does.
- A synchronized method's body starts with `let _monitor = java::enter(this);` (or
  `java::enter_class(&C__MONITOR)` for a static one). A method that uses its object's monitor
  takes the handle rather than `&self`; a method that lives on the enum of a class tree,
  synchronized or holding `synchronized (this)`, enters the monitor of the enum's value, whose
  variant has it. `wait` asserts that the thread holds no
  borrow of the object, which would let another thread in while it lasts.
- **The monitor of a thread-local `RefCell` object** (a builder's cell, in a program that
  observes monitors) is a thread-local list of the addresses the thread has entered. `holdsLock` and the
  `IllegalMonitorStateException` of `wait` and `notify` answer from it, `notify` wakes nobody,
  and `wait` lasts its timeout, or for ever without one, as in Java where no other thread can
  reach the object. The runtime checks ownership in every `wait` and `notify`.
- **The wait set** is one entry per waiting thread in the lock's parking bucket: `notify` picks
  the one that has waited longest (HotSpot's order), `notifyAll` all of them, and a timed-out
  waiter removes its own entry, so a later waiter cannot take a notification meant for an
  earlier one. No spurious wakeups are produced (Java allows them; fewer behaviours).
- `synchronized` on an object of a class with subclasses holds the monitor of the object's
  variant.

### 3.3 Interrupt

- **The status is the runtime's**, by thread id: the record of a running thread holds it, and a
  set of ids holds it for threads with no record (interrupted before `start`, or ended with the
  status set, which Java keeps in the `Thread` object). Moving it between the two happens under
  the registry's lock, so an interrupt racing a start or an end is not lost. `Threads.java`'s
  `interrupt`, `isInterrupted` and `interrupted` call `interrupt0`, `isInterrupted0` and
  `clearInterrupt0`.
- **Waking**: `interrupt` sets the status under the record's park mutex and notifies its
  condition variable (which `sleep` and `park` wait on), then the parking bucket of the monitor
  the thread waits on, then the record of the thread it joins. Each waiter checks the status
  under the mutex it waits on, so no wakeup is lost. `park` returns on an interrupt and leaves
  the status set, as `LockSupport.park` does.
- **Java's order of checks**: `sleep` checks a negative time before the interrupt; `wait` checks
  ownership before it and takes the monitor again before it throws; a waiter both notified and
  interrupted returns normally with the status set; `join` of a thread that is not alive returns
  at once even when interrupted. `sleep`'s exception says `sleep interrupted`; `wait`'s and
  `join`'s have no message.
- **Only a program that interrupts pays**: `Facts::interrupts` holds when some method calls
  `interrupt0`, and only then do `sleep`, `join` and `wait` have `InterruptedException` among
  their failures (with the checked forms `java::try_sleep` and `java::try_join_thread`). They
  register where an interrupt would wake them even before the program's first interrupt.
- Interrupting a thread blocked reading standard input does not wake it (Java's does not either).

## 4. Thread-escape analysis

`analysis/threads.rs` decides which classes two threads may reach (`Facts::thread_shared`).

### 4.1 Roots

A thread reaches its `Thread` object and the statics that it can touch. So the roots are the
`Thread` class family and the types of the static fields that are process-wide (section 2.3).

### 4.2 Propagation

The closure is **by type**, like every other per-class fact: from a shared class it adds

- its superclasses (but not `Object`) and its subclasses, since the enum of a class tree holds
  all of its classes the same way;
- for an interface, the classes whose objects become values of it (`hierarchy::flows`); going up
  through an interface or `Object` is not done, since that would make every class shared;
- the classes of the types of its fields and of its captures (local and anonymous classes,
  lambdas), through arrays and type arguments.

Java enums are never shared: their values are plain Rust values, which any thread may hold. A
closure type that a thread may reach (through a capture, a field or a static) becomes
`Arc<dyn Fn .. + Send + Sync>`, and its captures are of shared types by the same walk.

The cost of overestimating is a lock where a `RefCell` would do: speed, not
correctness. A type-based closure makes, for example, every `Node` of every list shared when one
list is handed to a thread. Per allocation site, with the points-to analysis of thorough mode,
is the planned refinement (section 7).

Rejected (`check`): a box compared by identity (a `java::Boxed`) in a static or in a field of a shared
class, which has no shared form yet: "a box compared by identity that threads may share not
supported yet".

### 4.3 rustc as the verifier

The analysis over-approximates, and rustc checks it: an `Rc`, `RefCell` or `Cell` inside a
thread-shared `Arc`'s payload or a spawn closure is a compile error (`RefCell<..> cannot be shared
between threads safely`), never a miscompilation. A bug in the analysis therefore shows in the corpus build as an
error naming the type, and the fix belongs in the analysis, never in a silent fallback.
Speculating and letting rustc reject (as thorough mode does elsewhere) does not apply to sharing:
the general form must be chosen up front.

## 5. The hand-written native layer

The runtime decides at run time, not by a cargo feature: the output buffer and descriptor table
are thread-local until the first `start_thread`, then process-wide (`out::share_output`), so a
single-threaded program pays nothing. Process-wide output is locked for one whole `print` or
`println`, so lines from two threads never interleave inside a line, as `PrintStream`'s
synchronized methods guarantee.

### 5.1 `Thread` and `ThreadGroup`

`crates/translator/java/Threads.java` is compiled to a tree at build time, like `Standard.java`,
and the members a program uses are added to it. It holds `Thread`, `Thread.UncaughtExceptionHandler`,
`ThreadGroup` and a `ClassLoader` that is a value only.

- **`Thread`**: the constructors of `java.lang.Thread` with a group, task, name and stack size,
  plus `ForkJoinWorkerThread`'s `(ThreadGroup, Runnable, String, long, boolean)`; `start`, `run`,
  `join()` and `join(long)`, `isAlive`, `interrupt`, `isInterrupted`, `interrupted`,
  `currentThread`, `sleep` (millis, and millis with nanos), `yield`, `onSpinWait`, `holdsLock`,
  `getName`/`setName`, `isDaemon`/`setDaemon` (`IllegalThreadStateException` after start),
  `getId`/`threadId`, `getPriority`/`setPriority` (stored, no effect), `getThreadGroup`,
  the shutdown hooks of `Runtime` (section 6),
  `getState`, `isVirtual` (always false), the context class loader, the uncaught-exception
  handlers, `toString` (`Thread[#3,main,5,main]`), and the stack without frames, as an
  exception's (see [exceptions.md](exceptions.md)): `getStackTrace` is empty, Java's shared empty
  array for a thread that is not alive, and `dumpStack` prints `java.lang.Exception: Stack trace`.
- **Naming and numbering are Java's**: unnamed threads are `Thread-0`, `Thread-1`, ...; the main
  thread has id 3, as on current JDKs where the JVM's own threads take 1 and 2. The null-name
  messages are the JDK's.
- **`getState`** is `NEW` before the start and `TERMINATED` after the end; in between it is the
  state byte in the runtime's record, which `sleep`, `park`, `join`, `wait` and a contended
  monitor entry set around their waits (`TIMED_WAITING`, `WAITING`, `BLOCKED`), else `RUNNABLE`.
- **The context class loader** is a slot: the main thread's is the application's loader, and a
  new thread starts with its parent's. Loading a class through it is rejected
  (`docs/reflection-and-plugins.md`).
- **Fields for the JDK's code**: `threadLocalRandomSeed`, `threadLocalRandomProbe`,
  `threadLocalRandomSecondarySeed` and `parkBlocker`, which `ThreadLocalRandom` and
  `LockSupport` reach through `Unsafe` offsets; `threadLocals`, `inheritableThreadLocals` and
  `terminatingThreadLocals`, in which the JDK's own `ThreadLocal` keeps a thread's values
  (`java/ThreadLocalStub.java` gives `Threads.java` the map's name); the scoped-value bindings
  and cache that `ScopedValue` keeps in the thread (a new thread inherits none).
- **Thread-locals by slot number**: the JDK's `ThreadLocal` is patched
  (`java/patches/java/lang/ThreadLocal.java`) so that `get` and `set` find a value without
  hashing. Each of the first 4,096 thread-locals made gets a slot number of its own, never given
  again, and each thread's map keeps an array from those numbers to the entries of its hash table,
  next to the table. A `get` is then the current thread (a thread-local type compared and a
  pointer read), its map, and an indexed load from the array: no hash, no probe and no test of the
  entry's weak referent, which a live thread-local's entry cannot have lost. The table still decides what the map holds: an entry
  goes into the array where it goes into the table and leaves it wherever it leaves the table
  (`remove`, a stale entry expunged or replaced, a stale entry dropped as the table grows), so
  `initialValue` runs once per thread, `remove` brings it back, values stay in their thread's map,
  and the array keeps nothing alive that the table does not. Inheritance, a thread's exit and the
  erasing of a pool thread's thread-locals replace or drop the whole map, array included.
  Thread-locals made after the first 4,096 are found by hashing, as in the JDK.
- **Inheritable thread locals**: as in Java, a new thread starts with a copy of its parent's
  `InheritableThreadLocal` values, each through `childValue`, run in the parent; a
  `ForkJoinWorkerThread` that asks not to inherit them gets none, and the application's class
  loader as its context loader. Only a program that sets an inheritable value
  (`setInheritableThreadLocals`) has the copy (`inheritThreadLocals`); in any other the
  constructor's call does nothing (`lower/exceptions.rs`), so that threads do not bring in the
  JDK's `ThreadLocal`.
- **`ThreadGroup`** has a name, a parent and `maxpri` 10, prints as
  `java.lang.ThreadGroup[name=main,maxpri=10]`, answers `parentOf` by its parents and
  `isDestroyed` with false, as Java's always does, and a thread joins the group of the thread that
  creates it (`main`, whose parent is `system`). A thread that has ended has no group and no
  handler. `Thread.group` is null for the main group, so a program that never creates a group
  only gains a field that stays null.
- **Thread containers**: `SharedThreadContainer.start` reaches `JavaLangAccess.start`, which is
  the overlay `TranslatedLangAccess.start`, a plain `thread.start()`.
- `ForkJoinTask.getException`, for an exception thrown in another thread, makes a new one of its
  class by reflection with the original as its cause; the translation tests the class against
  each exception class reflection could use and calls the constructor reflection would choose
  (`lower/recreate.rs`).

Members that `Threads.java` does not declare are not translated: virtual threads,
`Thread.Builder`, `getStackTrace` and `getAllStackTraces`, `stop`/`suspend`/`resume`, thread
group enumeration.

### 5.2 Natives

| native | Rust (`crates/runtime/src/thread.rs` unless noted) |
| --- | --- |
| `start0(t, tid, name, daemon)` | `java::start_thread`: a `std::thread` with that name, which sets the current thread and calls `t.run()`; a daemon is not counted among the live threads |
| `currentThread` | a thread-local current `Thread`, found through one thread-local pointer and its type, compared with the program's `Thread` type (no borrow and no virtual call, for code that checks its owner thread, or reads a thread-local, on every call); the main thread's is made on first use |
| `alive0`, `state0` | the record's alive flag and state byte |
| `join0(tid, millis)` | wait on the record's condition variable until it is not alive or the timeout passes, interruptible |
| `interrupt0`, `isInterrupted0`, `clearInterrupt0` | the interrupt status (3.3) |
| `sleep`, `yield`, `onSpinWait` | an interruptible wait on the record's park condition variable; `std::thread::yield_now`; `std::hint::spin_loop` (`system.rs`) |
| `Unsafe.park`, `unpark` | a permit per record (a mutex and a condition variable); `std::thread::park` has no handle before `spawn` returns |
| `Object.wait`, `notify`, `notifyAll`, `holdsLock` | the monitor functions of section 3 (`monitor.rs`, `locked.rs`) |
| `installUncaught0`, `uncaught0`, `printUncaught0` | the uncaught-exception hook (section 6) |
| `installShutdown0` | `java::set_shutdown_hook`: the function that runs `Thread.runShutdownHooks()`, which lowering adds as the native's argument |
| `System.exit`, `Runtime.halt` | `exit` runs the shutdown hooks; both flush the output, `process::exit` |

### 5.3 Field and array handles

`java.util.concurrent` and other JDK code reach fields and array elements through `Unsafe`
offsets and `VarHandle`s:

```java
private static final long STATE = U.objectFieldOffset(AbstractQueuedSynchronizer.class, "state");
... U.compareAndSetInt(this, STATE, expect, update) ...
private static final int ABASE = U.arrayBaseOffset(Node[].class), ASHIFT = 31 - Integer.numberOfLeadingZeros(U.arrayIndexScale(Node[].class));
... U.getReferenceAcquire(tab, ((long)i << ASHIFT) + ABASE) ...
```

`lower/handles.rs` (its module comment has the full rules) turns these into accesses of what they
name:

- **A field handle** is a static final whose value is `objectFieldOffset(C.class, "f")` (or of
  `getDeclaredField`, or assigned once in a static block, or made by a static helper of a class
  and a name, as JCTools' `UnsafeAccess.fieldOffset` in netty). It is no field of the translation. A
  read or write through it, in any memory mode, is a read or write of `o.f`; the field may be one
  of another class (`ThreadLocalRandom`'s offsets into `Thread`) as long as the object is of that
  class. Static fields (`staticFieldOffset`, `staticFieldBase`) work the same way.
- **An offset used as a number**, `objectFieldOffset(f)` or `staticFieldOffset(f)` of a `Field`
  that is no handle (joml's `MemUtil` finds a matrix's offsets so to check that its floats are 4
  bytes apart; netty finds `java.nio.Bits`' `MAX_MEMORY` or `maxMemory`, by a name that a
  condition picks), is a number of the field's own, the same for every `Field` of it and 16 apart
  from the others, and `staticFieldBase(f)` is the field's class (`Field.staticFieldOffset`,
  `staticFieldBase` in `java/Reflection.java`). Code that checks the layout before reading at
  offsets of its own computing finds it is not the one it expects, as on a JVM that lays fields
  out otherwise, and takes its other way. A read or write at an offset that is none of the above,
  where the program has such numbers or uses `sun.misc.Unsafe`, is the translator's
  `Field.unsafeGetLong(base, offset, access)` (and its kin of each type): of the field whose
  number the offset is, when the base holds it (its class for a static field) and it has the
  access's type. Otherwise it throws `UnsupportedOperationException` naming the access, where
  Java would read or write whatever memory is there; so does an atomic update at such an
  offset, and an access in a program without such numbers (which is rejected if the program has
  no `UnsupportedOperationException` to throw). A census lists these accesses
  ([minecraft-server.md](minecraft-server.md)), except those whose offset is by its definition
  (a call, or a local or static final that holds one) a field's number. A plain read or write of a primitive in a base held as
  an `Object` (or a primitive array, or null) at such an offset, unless the offset is a field's
  number, reads or writes the bytes of the array there, or memory outside the heap by address. A
  record's field gets a number too, where `sun.misc.Unsafe` throws.
- **An element offset** is `base + (index << shift)` or `base + index * scale`, with `base` and
  `scale` the `arrayBaseOffset` and `arrayIndexScale` of one array class, written out, read from
  a static final, computed by a one-line method (`ForkJoinPool.slotOffset`), held in a `long`
  local, or passed to a `long` parameter of a private method by each of its calls
  (`LazyCollections`). An access at it is `a[index]`.
- **`VarHandle` constants** of fields (`findVarHandle`, `ConstantBootstraps.fieldVarHandle`), of
  static fields (`findStaticVarHandle`), of array elements, and of the bytes of a `byte[]` or a
  `ByteBuffer` viewed as numbers (`byteArrayViewVarHandle`, `byteBufferViewVarHandle`, whose
  accesses are the translator's methods in `java/overlay/java/nio/TranslatedBufferViews.java`,
  with the handle's checks and byte order) are handles too, as is a `MethodHandle` constant of a constructor.
- **Field updater constants** (`AtomicIntegerFieldUpdater.newUpdater(C.class, "f")`, the `long`
  one, and `AtomicReferenceFieldUpdater.newUpdater(C.class, V.class, "f")`, cast to a raw
  updater or held in a static block's local first, as netty does) are handles of a field for
  which `newUpdater` succeeds (`volatile`, not static, of the updater's type, and
  accessible to the class that makes it). The classes may be class literals, locals of the
  static block that hold them, or calls of a static method that returns its argument cast
  (netty's `cls(Node.class)`); the value class of a field whose type is a type variable is that
  variable's erasure. `get`, `set`, `lazySet`, the compare-and-set forms,
  `getAndSet` and `getAndAdd` are the field's access or update; `getAndIncrement`,
  `incrementAndGet`, `addAndGet` and their kin are its `getAndAdd`, with the new value computed
  from the old as the JDK's updater does. `getAndUpdate`, `updateAndGet`, `getAndAccumulate`
  and `accumulateAndGet` are a method that lowering adds to the field's class, one per field and
  form (`getAndUpdateCount(function)`), whose body is the JDK's loop: read the field, apply the
  function (`applyAsInt`, `applyAsLong` or `apply`, which reachability keeps for the call),
  and repeat until the field's compare-and-set succeeds. The updater throws
  `ClassCastException` without a message for a null object, where the field's access would
  throw `NullPointerException`: an object other than `this` is tested first
  (`Builtin::UpdaterRefused`), so it must be a local or an enclosing instance. The test may
  come before other arguments that are literals, constant fields, locals, lambdas, references to
  static methods, or arithmetic of those that cannot throw; with any other argument, the object
  and the arguments are first computed into locals (`receiver$`, `argument$`), in Java's order,
  for the field's access, though not for the forms that compute a new value or loop. Other objects and arguments, and an updater that is not a constant, are rejected.
- **Atomic updates** (compare-and-set, compare-and-exchange, get-and-add, get-and-set,
  get-and-bitwise, every memory mode) of a field become a method that lowering adds to the
  field's class, one per field and form (`compareAndSetValue(expected, value)`), whose body
  reads, compares and writes the field. On a thread-shared object the generator writes that body
  as one atomic action of the field's `java::SharedField` (`compare_and_set_by`,
  `compare_exchange_by`, `replace`, `get_and_update`: a compare-and-swap of a word, or the
  striped lock for a wider value), so no other access comes between the read and the write; on
  an object of one thread no other access can, and the method's confined form, called on an
  object that never leaves its method (2.1), writes the plain read, comparison and write. Of an element, they are runtime functions
  (`java::compare_and_set_element` and its kin in `array.rs`): one borrow of an array of one
  thread, and one atomic action of the element's cell in a shared array (a compare-and-swap of a
  word, or the striped lock for a wider element). References compare by identity, and `float`
  and `double` values by their raw bits, as `Unsafe` does. The weak forms never fail spuriously,
  which Java allows. `Unsafe`'s `float` and `double` updates and the compare forms and
  get-and-set of `byte`, `short`, `char` and `boolean`, which the JDK writes in Java over the
  `int` and `long` ones (a compare-and-set of the bits, or of the word that holds the byte), are
  the same updates of the field or element, so their bodies, which compute offsets, are not
  translated.
- **`sun.misc.Unsafe`**, which libraries use, forwards its accesses and updates at an offset and
  its `objectFieldOffset(Field)` to `jdk.internal.misc.Unsafe`'s (`compareAndSwapInt` to
  `compareAndSetInt`, `getObject` to `getReference`, `putOrderedInt` to `putIntRelease`), after
  a warning that the first such call of a run prints on standard error: the method, the class
  that called it and where that class was loaded from. A call of one of them is the forwarded
  access, after that warning (`lower/sun_unsafe.rs`, `java::memory_access_warning`). The calling
  class is the one whose code makes the call, and its location is the working directory as a
  `file:` URL, as for `java Main` run from the folder of the program's classes. Java evaluates
  the arguments before the warning, so every argument but the offset must be one whose
  evaluation does nothing (a literal, `this`, a local, a field of the calling class); other
  arguments and a method reference to such a method are rejected. Its methods of raw memory
  (`allocateMemory`, `freeMemory`, `setMemory`, `copyMemory`, accesses at an address,
  `invokeCleaner`), which netty uses, call `jdk.internal.misc.Unsafe`'s after the same warning,
  with their arguments evaluated first as in Java; `addressSize`, `pageSize` and
  `arrayIndexScale` do not warn. Accesses at an offset that is not a field's or an array
  element's, found at run time (`staticFieldOffset` of a reflected field, an offset passed in),
  are rejected.
- **Handles found at run time**, as netty finds its own (`lower/handle_values.rs`): a
  `VarHandle`, `MethodHandle`, `MethodHandles.Lookup` or `MethodType` kept in a local or a field,
  passed to a method or returned from one, made in a `try` whose `catch` sets it to null. At run
  time such a value only tells whether it is null; which handle it holds, lowering finds when
  translating, following assignments, static finals, the `return`s of static, private and final
  methods and the arguments of every call of such a method. The lookups, `methodType` of class
  literals, the factories of field, static field, element and view handles, `findVirtual` and
  `findStatic` of those factories, and `invokeExact` of the `MethodHandle`s that finds, are
  followed when none of them can fail. So is `findStatic` of a static method of the program
  (reached by its name) or of `VarHandle`'s fences: `invokeExact` or `invoke` with exactly the
  method's types is a direct call of it (a full fence for a fence). `invoke` may also differ from
  the method in reference types, as netty takes `getRuntimeMXBean()` as an `Object`: an argument
  or the result that is not an upcast gets `Class.cast`'s check and message (`Cannot cast A to
  B`), as `MethodHandle.asType` gives it; primitive types must match. A handle that may be one of
  several such methods, as netty's skip list takes `VarHandle.acquireFence` or its own fallback,
  holds its method's class as its token, and the call tests it. An access through a value that holds one handle, or null,
  is that handle's access after a null check that throws Java's `NullPointerException`, message
  included; Java checks after evaluating the arguments, so such an access takes only arguments
  whose evaluation does nothing. `lookupClass()` of a lookup that is followed is its class. A
  call of a lookup or a `MethodHandle` that is not followed, or
  that may fail, throws an `UnsupportedOperationException` that says so, which netty's `catch`
  turns into going without that handle (no virtual-thread check through a handle, no cleaner
  linked to `malloc`); a census lists each such call
  ([minecraft-server.md](minecraft-server.md#going-on-past-compile-errors)). A value that may be one of several handles and a handle kept anywhere else
  (a collection, an `Object`) are rejected.
- Any other use of a handle (printing it, arithmetic other than the forms above, passing it on)
  is rejected: "a field offset used as a value".

The fields and arrays themselves stay plain: an access through a handle is an ordinary access of
the field or element, an acquire load or a release store of its cell, which orders it at least
as the plain, opaque, acquire and release modes ask. An element's access in the volatile mode
(`getVolatile` and `setVolatile`, which `AtomicIntegerArray.get` and `set` and their kin make,
or `Unsafe.getIntVolatile`) is sequentially consistent, as Java's is: lowering records it
(`Class::volatile_elements`), and the generator makes it a `SeqCst` load or store of the
element's cell (`java::get_volatile`, `java::set_volatile`). A field's accesses are as its
declaration makes them, sequentially consistent for a `volatile` field; the volatile mode on a
field that is not `volatile` asks for more (section 7).

## 6. Exceptions across threads, daemon threads, exit

- **An exception that ends a thread** goes through the handler chain: the thread's own handler,
  else its group's `uncaughtException` (which a subclass may override, and which goes to the
  parent), else the default handler, else the runtime prints `Exception in thread "<name>" ` and
  the exception as Java does. The thread ends with a private panic payload that the spawn
  closure catches; the other threads go on and the exit code is unaffected. Any other panic in a
  worker is a translation bug and ends the process with 101.
- **The runtime calls the translation back**: `setUncaughtExceptionHandler` and
  `setDefaultUncaughtExceptionHandler` call `installUncaught0`, which hands the runtime a function
  (`java::set_uncaught_hook`) that turns what ends a thread into a `Throwable` and runs
  `Thread.dispatchUncaught()`. A program that never sets a handler has no hook, and the runtime
  prints the message itself.
- **What a handler sees**: an exception the program throws, and a runtime failure (an index, a
  division, a cast) whose class lowering declares; in a program that installs a handler, every
  failure class whose operation the program uses is declared, as if a `catch` could see it. A
  runtime failure without a declared class skips the handlers and ends the thread with the
  message.
- **A handler that throws** ends the thread with the JVM's `Exception: <class> thrown from the
  UncaughtExceptionHandler in thread "<name>"`.
- **`main`** holds a `java::MainThread` (`let _main = java::main_thread();`). Its drop, also when
  `main` returns early, waits until every non-daemon thread has ended and then flushes, as the
  `java` launcher waits in `DestroyJavaVM`. `main`'s uncaught exception goes through the same
  handler chain, and the program then exits with 1.
- **Daemon threads** are not counted, so the process exits while they run, as in Java.
  `System.exit(n)` and `Runtime.halt(n)` from any thread flush and end every thread at once.
  A deadlocked thread keeps the process alive, as in Java.
- **Shutdown hooks**: `Runtime.getRuntime().addShutdownHook(t)` and `removeShutdownHook(t)` are
  calls of the translator's `Thread.addShutdownHook` and `removeShutdownHook`, which keep the
  hooks, with the JDK's checks and messages. Once a hook is added, the runtime has
  `runShutdownHooks()`, and runs it on the first exit, as Java's shutdown sequence: `main`
  returning (after the other non-daemon threads), `System.exit`, an uncaught exception that ends
  `main`, a failing static initializer. Every hook starts, and the exit waits until all of them
  have ended; no hook can be added or removed from then on (`Shutdown in progress`).
  The JDK's own hooks (`JavaLangAccess.registerShutdownHook`, which `File.deleteOnExit`'s
  `DeleteOnExitHook` registers) are the translator's `Thread.registerShutdownHook`'s, kept by
  slot with the JDK's checks: the exit runs them in the order of their slots, the program's hooks
  in slot 1 as the JDK's `ApplicationShutdownHooks` does, so the files go after every hook of the
  program has ended. `Runtime.halt` runs none, as in Java. `SIGTERM`, `SIGINT` and `SIGHUP`
  (unless it is ignored, as under `nohup`) end a program with hooks as the JDK's `Terminator`
  does: a thread named `SIGTERM handler` (`SIGINT`, `SIGHUP`) exits with status 128 + the
  signal, running the hooks; a later signal changes nothing (`runtime/src/signals.rs`). A
  program without hooks ends as the signal ends any process. An exit while the hooks run ends
  the process at once, where Java's blocks.
- `InterruptedException`, `IllegalMonitorStateException` and `IllegalThreadStateException` are
  standard classes of `Standard.java`.

## 7. Not done, and plans

- **Per-site sharing.** Sharing is decided per class, arrays per element family and
  builders by type; only objects that never leave their method are told apart (2.1). With the
  points-to analysis of thorough mode, sharing would be decided per allocation site: an object
  held in a field of an object that one thread reaches would keep plain accesses. That needs the
  points-to sets of each thread's code apart (what the main thread's methods reach, what each
  started thread's `run` reaches), since a field of an object that both reach is shared whatever
  the thread that made it; and code that is kept off a thread only by a run-time test of
  `Thread.currentThread()` (a check that a packet is handled on the main thread) is reachable
  from that thread to any static analysis. Then a class with both shared and thread-local sites
  would get two structs, and an array of a shared family that no other thread reaches would stay
  a `java::Array`. The escape analysis should then be a client of a "heap edges" interface so
  that only the edge provider changes.
- **`SeqCst` volatile-mode accesses of fields.** A volatile-mode access through `Unsafe` or a
  `VarHandle` (`getVolatile`, `setVolatile`) of a field that is not `volatile` should be
  sequentially consistent, but lowering turns it into an ordinary access, acquire or release
  (5.3). The atomic updates, and the volatile-mode accesses of array elements, are sequentially
  consistent already.
- **The rest of `java.util.concurrent`.** The pieces exercised by the whole-program corpus are
  the atomics (`AtomicInteger`, `AtomicLong`, atomic arrays), `LockSupport`, `ConcurrentHashMap`,
  `ConcurrentSkipListMap`/`Set`, `ThreadLocal`, `ThreadLocalRandom`, `ScopedValue`,
  `ForkJoinPool` with `RecursiveTask`. Executors, `CompletableFuture`, the blocking queues and
  the other synchronizers go through the same path but are untested. Field updaters translate
  only as constants (5.3).
- **Parallel streams** should run on the translated `ForkJoinPool` and the JDK's stream code,
  not on Rayon: the JDK splits a sized source deterministically and combines in a fixed tree, so
  a floating-point `reduce` prints what Java prints on the same machine, while Rayon's splitting
  follows work stealing and would vary; and Rayon would be a dependency of every generated crate,
  which has none. A later refinement may move split-independent terminal operations (integer
  `sum`, `count`, `min`/`max`, `anyMatch`, ...) with side-effect-free lambdas to a chunked
  helper; floating-point `sum`, `average` and `reduce` are never moved, since `DoubleStream.sum`'s
  compensated summation depends on the split.
- **Automatic parallelization of independent loops** (plan). A counting loop would run on a
  runtime helper over `std::thread::scope` when the analyses prove: iterations write only
  `out[i + c]` of plain arrays, body locals and reductions; no other effects (no printing, field
  or static writes, calls other than pure ones); no exception (indexes in bounds, divisors
  nonzero, nothing null); and enough work (a weight threshold, against the cost of spawning).
  Reductions are only those whose result does not depend on the grouping: wrapping `+` and `*`,
  bit operations, `min`/`max` on integers. Floating-point `+=` is never a reduction (IEEE
  addition is not associative), unless a thorough-mode relaxation allows it per method. rustc
  would verify the proof: the body must be `Fn` and `Sync`, and writes only through the disjoint
  `&mut` chunks it is given.
- **Testing aids** (plan): programs whose output is legitimately unordered could carry a header
  that makes the corpus harness compare sorted lines; a stress script could rerun the thread
  corpus pinned to one CPU and on all CPUs. A deterministic scheduler for the Rust side is not
  planned: it would need every lock to go through a second runtime, and `rr record --chaos`,
  `taskset -c 0` and repeated runs find the same bugs for far less work.
- **Open: `parking_lot`.** Faster uncontended locks, but a dependency of every generated crate.
  Std only, until benchmark numbers say otherwise.

## 8. Testing

Thread programs are tested by the same differential test as all others, so their output must
not depend on scheduling. `corpus/threads/` holds programs where each thread computes into its
own slot or object and `join` orders the output, or where the program checks its own invariant
(every increment counted, every item consumed once) and prints the result. Programs that use the
JDK's concurrency classes are in `corpus/whole-program/` (`AtomicNumbers`, `AtomicArrayDekker`,
`ConcurrentCounters`, `ConcurrentMaps`, `ForkJoinTasks`, `ThreadLocals`, `ThreadLocalSlots`, `ThreadStates`,
`ParkWithPermit`, `LockAmongValues`, `ConfinedAtomics`, `VarHandleViews`, `ByteBufferViews`, `StaticVarHandles`, `RunTimeHandles`, `ScopedValues`). Shape tests of the generated Rust are in
`crates/translator/tests/thread_translation.rs`; among them, that a single-threaded program is
unchanged and that a class no thread reaches has no monitor.

## 9. Cost

- **Single-threaded programs pay nothing**: the output is what it was without this work.
- **A thread-shared mutable object** costs nothing more than an object of one thread on a field
  access of a word: an acquire load or a release store is a plain move on x86-64. A `volatile`
  store is an exchange, and an atomic update a locked compare-and-swap, except on an object that
  never leaves its method, whose confined forms use plain moves (2.1). A wider value costs a striped spin lock once a second thread runs. Each
  object carries a pointer to its monitor's lock, 8 bytes, and the 16 bytes of the lock once a
  thread first takes it. An immutable shared object costs nothing on reads,
  and copying a handle is copying a pointer, shared or not.
- **A monitor** costs one lock per synchronized region, like HotSpot's thin lock. Until the
  program starts its first thread, taking the lock and giving it back are a plain increment and
  decrement of the lock's word (`locked.rs`, `RawLock` and `THREADS`): a program whose threads
  start late, or never (javac's), pays no atomic operation until then. The lock's word holds the
  holder's number and the count of its holds together, and the thread that ran alone is holder
  0, so the locks it holds when it starts a thread are its own without being recorded.
- **Statics** of the process are plain loads and stores on x86-64 (acquire/release), a
  `volatile` one's store an exchange, primitives and object handles alike; a string without an
  identity or a closure is copied under a striped spin lock, as a wide field is.
- The common case stays fast because classes no thread reaches have plain cells, immutable
  classes have no lock, and a `Vec` or plain value moved into a thread is not shared at all.
