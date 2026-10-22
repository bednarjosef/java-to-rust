# Multi-threading: threads, monitors, shared objects

How a Java program that starts threads is translated: which objects become thread-shared, how
monitors, statics, interrupts and uncaught exceptions work, and why the result is a legal
execution of the Java program. It builds on the object model (`docs/object-model.md`: class
trees as enums, objects as `Rc<RefCell<C>>`, `Rc<C>` when immutable, plain values when owned) and
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
   cannot start a thread is translated exactly as without this work: `Rc`, `RefCell`,
   thread-local statics, `synchronized` as a null check. It pays nothing.
2. **Stronger synchronization than Java's is always allowed, and it is all the translation
   uses.** Every access to memory that another thread can reach goes through a lock or an atomic
   with at least acquire/release ordering, never `Relaxed`. Every execution of the Rust program
   is then a sequentially consistent interleaving of the Java program's actions, which Java
   allows (section 1). A lock the translation adds is never held while the thread waits for
   anything else, so it cannot add a deadlock (1.3).
3. **Thread-shared objects swap `Rc` for `Arc` and `RefCell` for `java::Locked<C>`**, a `RefCell`
   behind a reentrant lock with `RefCell`'s API. The generated text is the same as for a
   thread-local object; only the type names differ. An immutable shared class is `Arc<C>`, read
   without a lock. Which classes are shared is decided per class, by type (section 4).
4. **An object's monitor is its data lock.** `synchronized` holds the reentrant lock of the
   object's `java::Locked` for the region; the field borrows inside take it once more. Monitors
   are kept only where the program can tell them from none (section 3).
5. **Statics of a multi-threaded program are process-wide `static` items**: `java::Atomic<T>`
   for primitives (acquire/release), `java::Locked<T>` for objects and strings, and a
   `java::ClassInit` per class that runs its static initialization once for the process.
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
| read/write of a field of a thread-shared object (plain or `volatile`) | under the object's `java::Locked` |
| read/write of an element of an array of a threaded program | under the array's lock (`java::SharedArray`) |
| `Unsafe`/`VarHandle` get, put, compare-and-set, get-and-add on a field or element | the same lock, held for the whole read-compare-write (5.3) |
| monitor enter/exit, `wait`, `notify` | the object's reentrant lock and its wait set (section 3) |
| `Thread.start`, `join`, `park`/`unpark` | `std::thread` spawn, the thread record's condition variables |
| static field access | `java::Atomic` (acquire/release) or `java::Locked` statics |
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
  field is under a lock, so the referent's fields are published with it. The racy idiom becomes
  correct; two threads may still both compute, as in Java.
- **Compound operations on racy fields** (`count++` without synchronization): each read and
  write is one locked access, so updates may be lost between them, as Java allows. Where one
  statement holds the lock across both, none is lost: fewer behaviours.
- **Granularity.** A translation may hold a lock across several Java actions, which removes
  interleavings; it must never implement one Java action as several unsynchronized steps that
  another thread can observe half-done. Every multi-step form is inside one lock acquisition.
- **Identity hash codes** of shared objects come from the `Arc` payload's address: stable while
  the object lives, and never equal for two live objects.

### 1.3 Progress and deadlock

Java guarantees no fairness, so a thread that never gets a lock is legal; a deadlock, though, is
observable, and the data lock is a lock Java does not have. The rule that keeps it invisible is
the one `RefCell` already imposes: **a data lock is held only while fields are read or written,
or for a short `&self`/`&mut self` call, and never while the thread waits for anything else.**

- A method of a shared class may borrow its object for the whole call only if it is short: no
  loop, no call of a method or constructor, no access to another object or array, no blocking
  native (`analysis/threads.rs`, `long_methods`). Any other method takes the object's handle and
  locks it per access, as Java reads a field each time. So a thread waiting in `join` never holds
  a lock that the thread it waits for needs.
- In a threaded program two borrows in one statement count as conflicting even when neither is
  mutable (`codegen/objects.rs`, `conflicts`), and a blocking native counts as one, so the
  operands are read into locals first. A conditional expression whose condition reads two shared
  objects is guarded the same way, so it never holds both locks at once.
- A lock read through another object (`synchronized (this.lock)`) is taken out of it before the
  thread waits for its monitor (`java::enter(&{ this.borrow().lock.clone() })`); the same holds
  for `wait` and `notify`.

In a build with debug assertions (the corpus test's), a wait of more than `WATCHDOG` (20
seconds) for a data lock is this rule broken and panics with "translation bug: ..." instead of
hanging (`crates/runtime/src/locked.rs`). A thread waiting for a lock held as a monitor is not
watched, since Java may hold a monitor across anything.

A monitor is Java's own lock: a deadlock on monitors is one the Java program has too. Java's
monitor is reentrant and so is ours; `wait` releases all recursion levels and takes them back.

## 2. Representations

### 2.1 Thread-shared classes

Besides shared (`Rc<RefCell<C>>`), immutable (`Rc<C>`) and owned (a plain value), a class may be
*thread-shared*: some object of it may be reached by two threads (section 4).

| | thread-local | thread-shared |
| --- | --- | --- |
| mutable | `Rc<RefCell<C>>` | `Arc<java::Locked<C>>` |
| immutable (no field written after construction) | `Rc<C>` | `Arc<C>` |
| owned | plain value | plain value, moved into the thread |
| closure type (`docs/closures-and-nested-classes.md` 4.2) | `Rc<dyn Fn(..) -> R>` | `Arc<dyn Fn(..) -> R + Send + Sync>` |

`java::Locked<C>` keeps `RefCell`'s API (`borrow`, `borrow_mut`, `with_borrow`,
`with_borrow_mut`, mapped guards), so the generator writes the same text for both forms. Because
the lock is reentrant, one statement may borrow the same object several times
(`o.borrow_mut().n = o.borrow().n + 1`), which a plain `Mutex` would deadlock on; a borrow that
conflicts with another of the same thread panics as `RefCell`'s does. A poisoned lock is used
anyway: a panic is a translation bug and the program is ending. Enums of class trees wrap
whatever each variant's class uses. Identity is `Arc::ptr_eq`; `Option` for null is unchanged.

**Why a lock per object rather than an atomic per field.** The lock keeps the generator's shapes
and the `&mut self` methods; per-field atomics would need `&self` methods everywhere, an explicit
`load`/`store` on every access, and a lock anyway for references (std has no atomic `Arc`). A
plain field of a thread-shared object is a racy field in Java, which well-written programs avoid,
so its lock is rarely hot. Atomic fields remain a refinement for when a profile shows a lock
(section 7).

**Volatile fields** are plain fields of the object, read and written under its lock, which is at
least as strong as `volatile` asks. A `volatile` static is an ordinary `java::Atomic`
(acquire/release); Java's volatile is sequentially consistent, so `SeqCst` statics are a known
gap (section 7). In a single-threaded program `volatile` is nothing.

### 2.2 Arrays and builders

In a threaded program an array that is not a plain `Vec` (see the array analysis) is a
`java::SharedArray<T>`, an `Arc<java::Locked<Vec<T>>>`, when two threads may reach arrays of its
family, and a `java::Array<T>` otherwise. The family is the type of the innermost elements, so
`int[]` and `int[][]` are decided together and a `SharedArray` never holds an `Array`. A family is
shared by the same rule as builders below: a static holds such an array, or a field or capture of
a shared class does, or such an array becomes a shared `Object` (`analysis/threads.rs`,
`shared_arrays`). In a whole program the JDK's statics hold arrays of the common families (`int`,
`char`, `byte`, `String`, `Object`), so those stay shared there. The runtime's array functions take
either form through the `Elements<T>` trait (`crates/runtime/src/array.rs`).

A `StringBuilder` is a `java::SharedStringBuilder` (its units in an `Arc<java::Locked<..>>`,
whose monitor is that lock) when two threads may reach builders: a static holds one, or a field
or capture of a shared class does (in an array or as a type argument too), or a builder becomes a
shared `Object` or `CharSequence` (`analysis/threads.rs`, `shares_builders`). Otherwise every
builder stays a `java::StringBuilder`, even in a threaded program, and its monitor is the address
of its `Rc`.

### 2.3 Statics

A thread-local static would give each thread its own copy, so in a multi-threaded program every
static is process-wide (`crates/runtime/src/statics.rs`, `codegen/statics.rs`):

| static | form | access |
| --- | --- | --- |
| compile-time constant | `const` | `X` |
| primitive | `static X: java::Atomic<i32> = java::Atomic::new(0);` | `X.get()`, `X.set(v)`, acquire/release |
| object or string | `java::Locked<T>`, behind a `LazyLock` when its initializer is not a constant | `with_borrow`, `with_borrow_mut` |
| a class's "initialized" flag | `static C_INIT: java::ClassInit` | `if !C_INIT.start() { return; }` .. `C_INIT.finish();` |

`java::ClassInit` is the JLS 12.4.2 procedure: the thread that finds the class not started runs
the initializer, another thread finding it in progress waits, and the same thread finding its
own initialization in progress returns at once (a recursive initialization sees defaults, as in
Java). `std::sync::Once` cannot do the last case. `start`/`finish` have the shape of the
thread-local flag's guard, so the initializer body is generated once for both forms.

A static that only the main thread touches could stay thread-local; that needs the set of
methods each thread runs, which does not exist yet (section 7).

### 2.4 Objects used as locks

`new Object()` used as a lock is an `Arc<java::Locked<PlainObject>>`: a class whose values may be
a kept lock is neither immutable nor owned, and neither is any class of its tree, so that an enum
over the tree reaches all its objects one way and each has a lock. Class monitors (static synchronized
methods, `synchronized (C.class)`) are statics of the translation, `static C__MONITOR:
java::Locked<()>`, entered with `java::enter_class(&C__MONITOR)`.

## 3. Monitors

Code: `analysis/monitors.rs` (which monitors are kept), `lower/sync.rs`, `codegen/sync.rs`,
`crates/runtime/src/monitor.rs` and `locked.rs`.

### 3.1 Which monitors are kept

A monitor is kept where the program can tell it from none:

- in a program that waits, notifies or asks `Thread.holdsLock`, every object's monitor, since
  those answer as Java's only when the monitor is held;
- otherwise, the monitors of objects whose class threads may share, and of builders where they
  are shared;
- class monitors in any program with threads.

Everywhere else `synchronized` is the null check of its lock, so a program without threads or
`wait` translates as before. `synchronized (this)` in a constructor keeps no monitor: no other
thread has the object yet.

Rejected, by name: `synchronized` on a string, a box, an array, a Java enum's constant, or the
class of a value (`o.getClass()`) where the monitor would be kept (they are values or `Rc`s
without a lock), and `wait`, `notify` or `holdsLock` on a string, box or array anywhere;
`wait(millis, nanos)`. A string, box or `Class` held as an `Object` has no monitor either, since
the translation keeps them as values; where a locked `Object` may hold one of them as well as the
program's objects (as `Collections.synchronizedSet(set, mutex)` locks what it is given), the
objects keep their monitors, and locking, waiting on or notifying such a value at run time ends
the program with "synchronizing on, waiting on or notifying a String, a boxed value or a Class
held as an Object is not supported by the translation" (`java::unkept_monitor`).

### 3.2 One form: the monitor is the data lock

The data lock of `java::Locked` is reentrant, so it serves as the monitor too; there is no
separate monitor type and no per-class choice.

- `synchronized (x) { .. }` is `{ let _monitor = java::enter(&x); { .. } }`. The guard drops
  however the block ends, so `?`, `break` and `return` release it as `monitorexit` does.
- A synchronized method's body starts with `let _monitor = java::enter(this);` (or
  `java::enter_class(&C__MONITOR)` for a static one). A method that uses its object's monitor
  takes the handle rather than `&self`, so a synchronized method calling another, or `wait`
  inside one, holds no borrow across the call. `wait` asserts that the thread holds no borrow of
  the object, which would let another thread in while it lasts.
- **The monitor of a thread-local object** (an `Rc`, in a program that observes monitors) is a
  thread-local list of the addresses the thread has entered. `holdsLock` and the
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

A thread reaches its `Thread` object and the statics. So the roots are the `Thread` class family
and the types of every static field.

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

The cost of overestimating is `Arc` and a lock where `Rc` and `RefCell` would do: speed, not
correctness. A type-based closure makes, for example, every `Node` of every list shared when one
list is handed to a thread. Per allocation site, with the points-to analysis of thorough mode,
is the planned refinement (section 7).

Rejected (`check`): a box compared by identity (an `Rc`) in a static or in a field of a shared
class, which has no shared form yet: "a box compared by identity that threads may share not
supported yet".

### 4.3 rustc as the verifier

The analysis over-approximates, and rustc checks it: an `Rc`, `RefCell` or `Cell` inside an
`Arc`'s payload or a spawn closure is a compile error (`Rc<..> cannot be sent between threads
safely`), never a miscompilation. A bug in the analysis therefore shows in the corpus build as an
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
  `getState`, `isVirtual` (always false), the context class loader, the uncaught-exception
  handlers, and `toString` (`Thread[#3,main,5,main]`).
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
  `LockSupport` reach through `Unsafe` offsets; `threadLocals` and `terminatingThreadLocals`, in
  which the JDK's own `ThreadLocal` keeps a thread's values (`java/ThreadLocalStub.java` gives
  `Threads.java` the map's name); the scoped-value bindings and cache that `ScopedValue` keeps
  in the thread (a new thread inherits none). `inheritableThreadLocals` is only a field, which
  `ForkJoinWorkerThread` clears; a new thread copies nothing from its parent, so
  `InheritableThreadLocal` is rejected rather than wrong.
- **`ThreadGroup`** has a name, a parent and `maxpri` 10, prints as
  `java.lang.ThreadGroup[name=main,maxpri=10]`, and a thread joins the group of the thread that
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
| `currentThread` | a thread-local current `Thread`; the main thread's is made on first use |
| `alive0`, `state0` | the record's alive flag and state byte |
| `join0(tid, millis)` | wait on the record's condition variable until it is not alive or the timeout passes, interruptible |
| `interrupt0`, `isInterrupted0`, `clearInterrupt0` | the interrupt status (3.3) |
| `sleep`, `yield`, `onSpinWait` | an interruptible wait on the record's park condition variable; `std::thread::yield_now`; `std::hint::spin_loop` (`system.rs`) |
| `Unsafe.park`, `unpark` | a permit per record (a mutex and a condition variable); `std::thread::park` has no handle before `spawn` returns |
| `Object.wait`, `notify`, `notifyAll`, `holdsLock` | the monitor functions of section 3 (`monitor.rs`, `locked.rs`) |
| `installUncaught0`, `uncaught0`, `printUncaught0` | the uncaught-exception hook (section 6) |
| `System.exit`, `Runtime.halt` | flush the output, `process::exit` |

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
  `getDeclaredField`, or assigned once in a static block). It is no field of the translation. A
  read or write through it, in any memory mode, is a read or write of `o.f`; the field may be one
  of another class (`ThreadLocalRandom`'s offsets into `Thread`) as long as the object is of that
  class. Static fields (`staticFieldOffset`, `staticFieldBase`) work the same way.
- **An element offset** is `base + (index << shift)` or `base + index * scale`, with `base` and
  `scale` the `arrayBaseOffset` and `arrayIndexScale` of one array class, written out, read from
  a static final, computed by a one-line method (`ForkJoinPool.slotOffset`) or held in a `long`
  local. An access at it is `a[index]`.
- **`VarHandle` constants** of fields, of array elements and of `byte[]` views as numbers
  (`byteArrayViewVarHandle`) are handles too, as is a `MethodHandle` constant of a constructor.
- **Atomic updates** (compare-and-set, compare-and-exchange, get-and-add, get-and-set,
  get-and-bitwise, every memory mode) of a field become a method that lowering adds to the
  field's class, one per field and form (`compareAndSetValue(expected, value)`); the method runs
  with the object borrowed, or locked in a threaded program, for the whole call, so no other
  access comes between the read and the write. Of an element, they are runtime functions
  (`java::compare_and_set_element` and its kin in `array.rs`) that hold the array's borrow or lock
  for the whole update. References compare by identity. The weak forms never fail spuriously,
  which Java allows.
- Any other use of a handle (printing it, arithmetic other than the forms above, passing it on)
  is rejected: "a field offset used as a value".

The fields and arrays themselves stay plain: an access holds the object's or array's lock, which
orders it at least as the plain, opaque, acquire, release and volatile modes ask.

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
  Translated programs have no shutdown hooks. A deadlocked thread keeps the process alive, as in
  Java.
- `InterruptedException`, `IllegalMonitorStateException` and `IllegalThreadStateException` are
  standard classes of `Standard.java`.

## 7. Not done, and plans

- **Per-site sharing.** Sharing is decided per class, arrays per element family and
  builders by type. With the points-to analysis of thorough mode, sharing would be decided per
  allocation site; a class with both shared and thread-local sites would get two structs, and an
  array of a shared family that no other thread reaches would stay a `java::Array`. The escape analysis should then
  be a client of a "heap edges" interface so that only the edge provider changes.
- **Main-only statics.** A static that only main-thread code touches could stay thread-local;
  this needs the set of methods each thread runs.
- **`SeqCst` volatile statics, and atomic fields.** A `volatile` static should be sequentially
  consistent. Per-field atomics (`AtomicI32`, ...; a lock-free `Arc<C>` with `&self` methods
  for a class whose every field is final or atomic) are the refinement for when a profile shows
  a data lock on a hot path. Shared primitive arrays that workers write would be
  `Arc<Vec<AtomicI32>>` and the like, and arrays filled before sharing and never written after
  `Arc<Vec<T>>`, so that neither needs a lock per element.
- **Splitting final from mutable fields** (`struct C { name: String, state: Locked<CState> }`)
  would let final fields be read without the lock.
- **The rest of `java.util.concurrent`.** The pieces exercised by the whole-program corpus are
  the atomics (`AtomicInteger`, `AtomicLong`, atomic arrays), `LockSupport`, `ConcurrentHashMap`,
  `ConcurrentSkipListMap`/`Set`, `ThreadLocal`, `ThreadLocalRandom`, `ScopedValue`,
  `ForkJoinPool` with `RecursiveTask`. Executors, `CompletableFuture`, the blocking queues and
  the other synchronizers go through the same path but are untested. Field updaters
  (`AtomicIntegerFieldUpdater`) use `getDeclaredField` reflectively and are not expected to
  translate.
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
JDK's concurrency classes are in `corpus/whole-program/` (`AtomicNumbers`, `ConcurrentCounters`,
`ConcurrentMaps`, `ForkJoinTasks`, `ThreadLocals`, `ThreadStates`, `ParkWithPermit`,
`LockAmongValues`, `VarHandleViews`, `ScopedValues`). Shape tests of the generated Rust are in
`crates/translator/tests/thread_translation.rs`; among them, that a single-threaded program is
unchanged and that a class no thread reaches keeps `Rc`.

## 9. Cost

- **Single-threaded programs pay nothing**: the output is what it was without this work.
- **A thread-shared mutable object** costs an uncontended lock and unlock per field access or
  short method call (10 to 20 ns on Linux's futex mutex, against about 1 ns for a `RefCell`
  borrow) and an atomic increment per handle clone. An immutable shared object costs nothing on
  reads beyond the `Arc` count.
- **A monitor** costs one lock per synchronized region, like HotSpot's thin lock.
- **Primitive statics** are plain loads and stores on x86-64 (acquire/release); object statics
  cost a lock per access.
- The common case stays fast because classes no thread reaches keep `Rc`/`RefCell`, immutable
  classes have no lock, and a `Vec` or plain value moved into a thread is not shared at all.
