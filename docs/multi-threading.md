# Multi-threading: threads, monitors, shared objects and parallel execution

This plan builds on the object model (`docs/object-model.md`: class trees as enums, objects as
`Rc<RefCell<C>>`, `Rc<C>` when immutable, plain values when owned, no `RefCell` borrow
outliving the expression that made it) and on the exceptions design (`docs/exceptions.md`:
exceptions are `Result`, `?` propagates, a method returns `Result` only where a caller observes
what escapes it). It is written for an agent that implements it from this document, and is kept
up to date as decisions change (see "Deviations" at the end).

Decided by the user and kept here: programs are normally multi-threaded, so real threads must
work and the design must allow optimizations that use threads; class trees are enums (closed
world); generics are idiomatic where proven and erased otherwise; exceptions are `Result`;
`String` is hand-written. Hand-written is only what bottoms out in the JVM, the core types
mapped to Rust types, and calls with a direct Rust equivalent. `java.util.concurrent` is
translated from the JDK's source over hand-written natives. Generated code is safe Rust, prints
what Java prints, and whatever cannot be proven keeps a correct general representation.

## 0. The decisions in one page

1. **A program is multi-threaded exactly when the native `Thread.start0` is reachable from
   `main`** (`program/reach.rs`). Every way of running code on another thread, executors,
   `CompletableFuture`, parallel streams, `ForkJoinPool`, bottoms out there once
   `java.util.concurrent` is translated. A program that cannot start a thread is translated
   exactly as today: `Rc`, `RefCell`, `thread_local!` statics, `synchronized` as a null check.
   It pays nothing for this work.
2. **Stronger synchronization than Java's is always allowed, and it is all we use.** Every
   access to memory that another thread can reach goes through a lock or an atomic with at least
   acquire/release ordering, never `Relaxed`. Then every execution of the Rust program is a
   sequentially consistent interleaving of the Java program's actions, which is a legal Java
   execution (section 1). Deadlocks the Java program can have, ours can have; deadlocks it
   cannot have, ours must not introduce, so a lock the translation adds is never held across a
   call that could take it again (the `RefCell` discipline, kept word for word).
3. **Thread-shared objects swap `Rc` for `Arc` and `RefCell` for a data lock,
   `java::Locked<C>`** (a `Mutex` without poisoning), with the same shapes as today's code:
   `o.lock().count += 1` where today it is `o.borrow_mut().count += 1`. An immutable shared class
   is `Arc<C>` and its reads take no lock. A class whose every field is final or atomic (volatile
   primitive, or reached through `Unsafe`/`VarHandle`) is `Arc<C>` too, lock-free, with `&self`
   methods: that is what `AtomicInteger` and the lock internals of `java.util.concurrent` become.
   Whether a class is thread-shared is decided per class by the thread-escape analysis (section
   4); per allocation site comes with the points-to analysis of thorough mode.
4. **Volatile fields are per-field atomics with `SeqCst`** (Java's volatile is sequentially
   consistent): `AtomicI32`, `AtomicI64`, `AtomicBool`, `AtomicU64` holding `f64::to_bits`, and
   `java::AtomicRef<T>` (a small lock around an `Option<Arc<..>>` or an enum value) for
   references. Plain fields of shared objects are under the data lock. Shared primitive arrays
   that some thread writes after publication are `Arc<Vec<AtomicI32>>` and the like: a lock per
   element access would serialize exactly the loops people parallelize.
5. **Monitors exist only where the closed world shows an object is locked.** The general form is
   a reentrant `java::Monitor` (owner thread, count, `Condvar`) kept beside the data lock in
   `Arc<java::Monitored<C>>`. The common form, chosen when no synchronized region of a class
   re-enters a region of the same class and the class's `wait`/`notify` are on `this`, fuses
   the monitor with the data lock: `synchronized void increment()` is `fn increment(&mut self)`
   called as `counter.lock().increment()`, and `wait()` is `guard = this.wait(guard)?`. Static
   synchronized methods and `synchronized (C.class)` lock a `static` monitor of the class.
6. **Statics in a multi-threaded program are process-wide `static` items**: `java::Atomic<i32>`
   with `get()`/`set()` for primitives (acquire/release; a plain `mov` on x86-64),
   `java::Locked<T>` for objects and strings, `LazyLock` for a `static final` object, and a
   `java::ClassInit` for each class whose initialization is eager (one thread runs it, others
   wait, the same thread re-entering sees it as done, as the JLS says). A static that only
   methods running on the main thread touch keeps the thread-local form.
7. **`Thread`, `ThreadGroup` and `ThreadLocal` are synthetic classes written as Java source in
   the translator** (`crates/translator/java/Threads.java`, like `Standard.java` for the
   exceptions), with their public API and bodies that call the natives of section 5. A program
   may extend `Thread`. Everything else, `LockSupport`, `AbstractQueuedSynchronizer`, the
   atomics, `ConcurrentHashMap`, `ThreadPoolExecutor`, `CompletableFuture`, `ForkJoinPool`, is
   translated from the JDK over the natives: `Unsafe` and `VarHandle` operations on a field
   named by a constant handle become atomic operations on that field.
8. **An uncaught exception in a thread ends that thread, not the program**: the thread's entry
   observes everything that escapes `run()`, prints `Exception in thread "Thread-0" ` and the
   exception as Java does, and the other threads continue. `main` prints its own uncaught
   exception at once, then waits for the non-daemon threads as the JVM does, and exits with 1.
   `System.exit` from any thread flushes the output and ends the process at once.
9. **Parallel streams run on the translated `ForkJoinPool` and stream code**, not on Rayon: the
   JDK's splitting is deterministic for a given input size and parallelism, so even a
   floating-point reduction prints what Java prints on the same machine; Rayon's adaptive
   splitting would not, and it would be a dependency. Loops the analysis proves independent are
   parallelized through a small runtime helper over `std::thread::scope`, only above a work
   threshold, and rustc's `Send`, `Sync`, `Fn` and disjoint `&mut` checks verify the proof.
10. **Milestones** (section 10): the thread-aware runtime and process-wide statics; threads by
    subclassing `Thread` (before lambdas); the thread-escape analysis and shared objects;
    monitors; exceptions across threads; volatile fields and the atomic natives; executors and
    the concurrency library (needs lambdas and inner classes); thread-local values; parallel
    streams; automatic loop parallelization (independent of lambdas); per-site sharing with the
    points-to analysis.

## 1. Correctness basis: the Java memory model in Rust

### 1.1 The argument

The Java memory model (JLS 17.4) allows every sequentially consistent execution of a program,
and more (executions in which racy reads see stale values). Rust's model is C++20's: a data race
is undefined behaviour, and safe Rust cannot write one, because every value two threads can
reach is behind a `Sync` type (a lock or an atomic). So a translated program has no undefined
behaviour by construction; what remains to show is that each of its executions is one Java
allows.

The translation maps every Java memory action to one of:

| Java action | Rust |
| --- | --- |
| read/write of a field of an object only one thread reaches | plain read/write (as today) |
| read/write of a plain field of a thread-shared object | under the object's data lock |
| read/write of a `volatile` field | atomic load/store, `SeqCst` |
| read/write of a plain field of a lock-free class (`Unsafe`/`VarHandle` field), `Unsafe.get/put{,Opaque,Acquire,Release}` | atomic load with `Acquire`, store with `Release` |
| `compareAndSet`, `getAndAdd`, `getAndSet`, ... | the atomic read-modify-write, `SeqCst` |
| monitor enter/exit, `wait`, `notify` | `java::Monitor`, a `Mutex` and a `Condvar` |
| `Thread.start`, `join`, `park`/`unpark` | `std::thread::spawn`, `join`, the thread record's permit |
| static field access | `java::Atomic`/`java::Locked` statics, acquire/release or the lock |

Every one of these is at least as strong as the Java action: a lock or `SeqCst` makes the
access part of a total order; acquire/release makes every read-from edge a happens-before
edge. An execution of the Rust program is therefore an interleaving of the Java program's
actions in which every read sees the last write before it in that order: a sequentially
consistent execution, which JLS 17.4.8 admits. Adding synchronization removes behaviours and
never adds one, with one exception, progress, handled in 1.3.

### 1.2 Where the argument needs care

- **`long` and `double` tearing.** Java allows a non-volatile 64-bit write to be two 32-bit
  writes (JLS 17.7). Rust's `AtomicI64` never tears, and a value under a lock never tears. Fewer
  behaviours: fine. On targets without 64-bit atomics (none we target: x86-64 and aarch64), the
  lock form is used.
- **Final fields.** Java guarantees that a thread seeing a reference to an object sees its final
  fields initialized (JLS 17.5), even through a race. In safe Rust a reference can only cross
  threads through a lock, an atomic store with release ordering, or a `spawn` closure, each of
  which publishes everything the constructing thread did before. So final fields, and
  non-final ones too, are always seen initialized. Fewer behaviours.
- **Out-of-thin-air values.** The JMM forbids them (causality, 17.4.8); C++20 formally allows
  them for cycles of `Relaxed` accesses, though no compiler produces them. We never emit
  `Relaxed`. With every load `Acquire` and every store `Release`, a read that sees a write makes
  the write happen-before the read, and a cycle of such reads would make happens-before cyclic,
  which the model forbids. So the formal model excludes out-of-thin-air values for our output,
  without relying on compiler behaviour. On x86-64 acquire loads and release stores are plain
  `mov`s, so this costs nothing there.
- **Lazily published objects** (double-checked locking without `volatile`, a racy `if (cache ==
  null) cache = compute();`). In Java another thread may see the reference and stale field
  contents. Ours: the field is under a data lock or is an `AtomicRef`, the referent's fields are
  published with it. The racy Java idiom becomes correct; fewer behaviours. Two threads may both
  compute, as in Java.
- **Races on references.** A Java racy read of a reference field sees some write of that field,
  atomically. Ours sees it through the lock: the same set of values, minus stale ones. A racy
  `==` on identities compares `Arc` pointers: unchanged.
- **Compound operations on racy plain fields** (`count++` without synchronization). Java may lose
  updates. Under the data lock each read and write is one locked access, two accesses per
  `++`, so updates may still be lost between them, exactly as Java allows; nothing forces the
  increment atomic. (Where the generator fuses `count++` into one `lock()` statement it holds
  the lock across both, which loses no update: fewer behaviours again.)
- **Granularity.** A translation may hold a lock across several Java actions (a `&mut self`
  method on a fused monitor class), which removes interleavings; it must never implement one
  Java action as several unsynchronized steps that another thread can observe half-done. The
  only multi-step forms we emit are inside one lock acquisition.
- **Identity hash codes and `toString` of objects without `toString`** differ from Java's, as
  they already do; the corpus does not print them.

### 1.3 Progress and deadlock

Java guarantees no fairness, so a thread that never gets a lock is a legal Java behaviour; but a
deadlock is observable (the program hangs where Java would finish), and the data lock is a lock
Java does not have. The rule that keeps it invisible is the one `RefCell` already imposes: a
data lock is held for one field access or one `&mut self` call and is released before any call
that could reach the same object again (`analysis/borrowing.rs` decides `&self`/`&mut self`
only when "nothing they run reaches an object of their class another way"). A violation panics
today ("already borrowed"); with a `Mutex` it would hang. So in the corpus build (overflow
checks on) `java::Locked::lock` is `try_lock` with a one-second timeout that panics with
"translation bug: the data lock of this object is held across a call that locks it again".
Release builds use the plain lock.

The monitor is Java's own lock: a deadlock on monitors is one the Java program has too. Java's
monitor is reentrant and ours is; `wait` releases all recursion levels and reacquires them.

## 2. Representations

### 2.1 The fourth axis: thread-shared

Today a class is shared (`Rc<RefCell<C>>`), immutable (`Rc<C>`) or owned (a plain value). This
work adds whether the class is *thread-shared*: some object of it may be reached by two threads
(section 4). The combinations:

| | thread-local (as today) | thread-shared |
| --- | --- | --- |
| mutable | `Rc<RefCell<C>>`, `o.borrow()`, `o.borrow_mut()` | `Arc<java::Locked<C>>`, `o.lock()` |
| immutable (no field written after construction) | `Rc<C>` | `Arc<C>` |
| lock-free (every field final or atomic) | `Rc<C>` with atomic fields | `Arc<C>`, `&self` methods |
| owned | plain value, moved | plain value, moved into the thread (`Send`) |
| locked (has a monitor) | no monitor needed: `synchronized` is a null check | `Arc<java::Locked<C>>` fused, or `Arc<java::Monitored<C>>` |

`java::Locked<C>` is a `std::sync::Mutex<C>` whose `lock()` returns the guard directly (a
poisoned lock is unlocked and used: a panic is a translation bug and the program is ending
anyway). The generator's handle forms (`ty_name`, `handle_text`, `borrow forms` in
`codegen/objects.rs`) get `Arc`/`lock` twins of their `Rc`/`borrow` forms; nothing else in the
shapes changes. Enums of class trees wrap whatever the variant's class uses:
`enum Shape { Circle(Arc<Locked<Circle>>), Rect(Arc<Rect>) }`; `base()` returns a
`MappedMutexGuard` instead of a `Ref` for a locked family (`MutexGuard::map` is on
`std::sync::MappedMutexGuard`, stable since 1.86; or the arm clones the fields out, which the
generator already does for enum reads where a `Ref` would not do). Identity is `Arc::ptr_eq`.
`Option` for null is unchanged. The borrow discipline is unchanged: a `MutexGuard` is a
temporary of the statement that made it, and the `conflicts`/`guarded` machinery that scopes
field reads in braces applies to it as to a `Ref`.

Why a data lock per object rather than an atomic per field, for plain fields:

| | per-object data lock | per-field atomics |
| --- | --- | --- |
| cost of a read | uncontended `Mutex` lock and unlock, about 10 to 20 ns | `Acquire` load: a plain `mov` on x86-64 |
| cost of a write | the same | `Release` store: a plain `mov`; `SeqCst`: `xchg`, 5 to 20 ns |
| references | fine | need a lock anyway (`AtomicRef`); no `AtomicArc` in std |
| `&mut self` methods | work as today: the guard is the `&mut` | impossible: every method is `&self`, every field access an explicit `load`/`store` |
| readability | `o.lock().x`, the same shape as `o.borrow().x` | `o.x.load(Ordering::Acquire)` on every read |
| generator | the `RefCell` shapes with new names | a second set of field-access shapes |

The lock is the general representation because it keeps the generator's shapes and the
`&mut self` methods. Per-field atomics are used where Java itself asks for them (`volatile`,
`Unsafe`, `VarHandle`) and so cover the lock-free classes of `java.util.concurrent`; a plain
field of a thread-shared object is a racy field in Java, which well-written programs avoid, so
its lock rarely sits on a hot path. If profiling shows one, the lock-free form (all fields
atomic, `&self` methods, `java::Atomic<T>` with `get()`/`set()` for readability) is the
refinement, class by class.

### 2.2 Volatile fields

A `volatile` field of any class in a multi-threaded program is an atomic: `AtomicI32` (`int`,
and `byte`, `short`, `char` through `AtomicI8`, `AtomicI16`, `AtomicU16`), `AtomicI64`,
`AtomicBool`, `AtomicU32`/`AtomicU64` holding `f32::to_bits`/`f64::to_bits`, and
`java::AtomicRef<T>` for a reference (an `Option<Arc<..>>`, an enum value, a `String`), which
is a `Locked<T>` inside until an `ArcSwap`-like form is worth writing. Reads are
`self.flag.load(SeqCst)`, writes `self.flag.store(v, SeqCst)`; the runtime gives
`java::Volatile<T>` with `get()`/`set()` so the text stays `self.flag.get()`. A volatile field
never needs the data lock, so a class whose only writable fields are volatile is lock-free
(`Arc<C>`), and `while (!done) {}` spins on a load as in Java. In a single-threaded program
`volatile` is nothing, as today.

### 2.3 Final fields

A final field of a thread-shared class is a plain field: a `&self` read needs no lock, and the
generator reads it as `o.x` on an `Arc<C>` and as `o.lock().x` on an `Arc<Locked<C>>` only
because the struct is behind the lock. To avoid that lock, a class with both final and mutable
fields may split: `struct C { name: String, state: Locked<CState> }`. That is the "fields
struct" machinery of the object model applied to mutability; it is a refinement for later and
not needed for correctness.

### 2.4 Arrays shared between threads

A `java::Array<T>` is `Rc<RefCell<Vec<T>>>`. A thread-shared array is:

| writes after it is shared | form | element access |
| --- | --- | --- |
| none (the creating method fills it, then shares; no thread writes through the captured path) | `Arc<Vec<T>>` | `a[i as usize]`, or `java::at(&a, i)` with the check |
| by some thread, primitive elements | `Arc<Vec<AtomicI32>>` (`AtomicI64`, `AtomicU64` bits, `AtomicBool`, ...) | `a[i].load(Acquire)`, `a[i].store(v, Release)`; `java::at_atomic` with the bounds check |
| by some thread, reference elements | `Arc<Vec<java::AtomicRef<T>>>` | the same through the small lock |
| passed to `java.util.Arrays` helpers or `System.arraycopy` while shared and written | `Arc<java::Locked<Vec<T>>>` | `a.lock()[i]`, the helpers take the guard's slice |

The first row is the common one for inputs (a table every worker reads) and the second for
outputs (each worker fills its own slice). The length never changes, so `a.length` is `a.len()`
in every form. The plain `Vec<T>` of a local that is later captured by a thread is converted at
the capture (`java::share_atomic(v)`), as `java::share` converts today.

### 2.5 Statics

Today's `thread_local! { static X: Cell<i32> }` is wrong with threads: each thread would get its
own copy. In a multi-threaded program:

| static | form | access |
| --- | --- | --- |
| compile-time constant | `const`, as today | `X` |
| primitive | `static X: java::Atomic<i32> = java::Atomic::new(0);` | `X.get()`, `X.set(v)` (acquire/release) |
| `volatile` primitive | the same with `SeqCst` (`java::Volatile<i32>`) | the same |
| `static final` object or string with an initializer | `static X: LazyLock<Arc<C>> = LazyLock::new(\|\| ..);` | `X.clone()`, `X.lock()` |
| other object or string | `static X: java::Locked<Option<Arc<C>>> = java::Locked::new(None);` | `X.lock().clone()` |
| the "initialized" flag of an eager class | `static C_INIT: java::ClassInit = java::ClassInit::new();` | `C_INIT.run(\|\| { .. })` |

`java::ClassInit::run` is the JLS 12.4.2 procedure without the error cases: a `Mutex<State>`
(`NotStarted`, `InProgress(thread id)`, `Done`) and a `Condvar`; the thread that finds
`NotStarted` runs the initializer, another thread finding `InProgress` waits, the same thread
finding its own `InProgress` returns at once (recursive initialization sees defaults, as in
Java). `std::sync::Once` cannot do the last case.

Statics accessed only by methods that run on the main thread (no method reachable from a
thread's `run()` or a submitted task reads or writes them; section 4.3) keep the thread-local
forms. In a single-threaded program every static keeps them.

### 2.6 Objects used as locks

`new Object()` used as a lock becomes a variant `Object::Lock(Arc<java::Monitor>)` (the object
model's rejection of `new Object()` is lifted here, for this use: identity and locking only).
`synchronized` on a `String`, a box or a class literal of a runtime type is rejected ("locking
on a string"): their identity is not kept. `synchronized (C.class)` and static synchronized
methods use `static C_MONITOR: java::Monitor = java::Monitor::new();` (`new` is `const`).

### 2.7 Examples

**A counter shared by two threads, with `synchronized`** (the fused monitor form: every
synchronized region of `Counter` is a method on `this` that re-enters nothing):

```java
class Counter {
    private int count;
    synchronized void increment() { count++; }
    synchronized int get() { return count; }
}
class Main {
    public static void main(String[] args) throws InterruptedException {
        Counter c = new Counter();
        Thread a = new Thread(() -> { for (int i = 0; i < 100000; i++) c.increment(); });
        Thread b = new Thread(() -> { for (int i = 0; i < 100000; i++) c.increment(); });
        a.start(); b.start(); a.join(); b.join();
        System.out.println(c.get());
    }
}
```

```rust
struct Counter {
    count: i32,
}

impl Counter {
    fn new() -> Counter {
        Counter { count: 0 }
    }

    fn increment(&mut self) {
        self.count = self.count.wrapping_add(1);
    }

    fn get(&self) -> i32 {
        self.count
    }
}

fn main() {
    let result: Result<(), Throwable> = 'main: {
        let c = Arc::new(java::Locked::new(Counter::new()));
        let a = java::Thread::new(java::runnable({
            let c = c.clone();
            move || { for _i in 0..100000 { c.lock().increment(); } Ok(()) }
        }));
        let b = java::Thread::new(java::runnable({
            let c = c.clone();
            move || { for _i in 0..100000 { c.lock().increment(); } Ok(()) }
        }));
        a.start()?; b.start()?; a.join()?; b.join()?;
        java::println!("{}", c.lock().get());
        Ok(())
    };
    java::end_main(result);
}
```

`java::Thread` here is the translated synthetic `Thread` class (an `Arc<java::Locked<Thread>>`
with the thread record inside); `start`, `join` and `main` return `Result` because `join` may
throw `InterruptedException`, which `main` declares and so its epilogue observes. The lambda
translation (`java::runnable`, a boxed `Fn() -> Result<(), Throwable> + Send`) is the lambdas
work's; the shape of the capture is what matters here: `c` is cloned into the closure, so
`c: Arc<..>` must be `Send + Sync`, which rustc checks.

**A volatile flag:**

```java
class Worker extends Thread {
    volatile boolean stop;
    int steps;
    public void run() { while (!stop) steps++; }
}
// main: w.start(); Thread.sleep(10); w.stop = true; w.join(); System.out.println(w.steps > 0);
```

```rust
struct Worker {
    base: ThreadFields,
    stop: java::Volatile<bool>,
    steps: i32,
}

impl Worker {
    fn run(this: &Arc<java::Locked<Worker>>) -> Result<(), Throwable> {
        while !this.lock().stop.get() {
            this.lock().steps = this.lock().steps.wrapping_add(1);
        }
        Ok(())
    }
}
```

`stop` is read through the data lock here only because the struct is behind it; `steps` is a
plain field written by one thread and read by another after `join` (a happens-before edge in
Java too), and the lock makes that visible. The split of 2.3 would make `stop.get()` lock-free.
`w.stop = true` in `main` is `w.lock().stop.set(true)`.

**Producer and consumer with `wait`/`notify`** (fused form with a condition variable):

```java
class Queue {
    private final int[] items = new int[4]; private int head, size;
    synchronized void put(int v) throws InterruptedException {
        while (size == items.length) wait();
        items[(head + size++) % items.length] = v;
        notifyAll();
    }
    synchronized int take() throws InterruptedException {
        while (size == 0) wait();
        int v = items[head]; head = (head + 1) % items.length; size--;
        notifyAll();
        return v;
    }
}
```

```rust
impl Queue {
    fn put(this: &Arc<java::Monitor<Queue>>, v: i32) -> Result<(), Throwable> {
        let mut q = this.lock();
        while q.size == q.items.len() as i32 {
            q = this.wait(q)?;
        }
        let i = java::rem(q.head.wrapping_add(q.size), q.items.len() as i32);
        q.size = q.size.wrapping_add(1);
        q.items[i as usize] = v;
        this.notify_all();
        Ok(())
    }

    fn take(this: &Arc<java::Monitor<Queue>>) -> Result<i32, Throwable> {
        let mut q = this.lock();
        while q.size == 0 {
            q = this.wait(q)?;
        }
        let v = q.items[q.head as usize];
        q.head = java::rem(q.head.wrapping_add(1), q.items.len() as i32);
        q.size = q.size.wrapping_sub(1);
        this.notify_all();
        Ok(v)
    }
}
```

`java::Monitor<C>` is `Locked<C>` plus a `Condvar`; `wait(guard)` releases the lock, waits
(waking on `notify`, `notifyAll`, a timeout, an interrupt or spuriously, as Java allows) and
returns the reacquired guard, or `Err(InterruptedException)` with the lock reacquired, as Java
does. A method that calls `wait` takes the handle (`this: &Arc<..>`) rather than `&mut self`,
because it must hand the guard back; `items` is a plain `Vec<i32>` (owned by the queue, never
escapes) and its index is in bounds by the range analysis (`% length`).

## 3. Monitors

### 3.1 Which objects have one

A class *is locked* when some object of it may be the operand of `synchronized (x)`, the
receiver of a synchronized instance method, or the receiver of `wait`, `notify` or
`notifyAll`, found through the flows of the object model (the concrete classes whose values
reach the static type of the lock expression). Only thread-shared locked classes, and classes
whose objects are waited on, get a monitor; `synchronized` on an object of any other class is
the null check it is today (a lock no other thread can contend is invisible, and Java's
`IllegalMonitorStateException` for `notify` outside `synchronized` is still raised, by a
`java::check_owner` call when the analysis cannot see the enclosing region).

### 3.2 Two forms

**Fused** (`Arc<java::Locked<C>>`, or `Arc<java::Monitor<C>>` when the class uses `wait`):
the data lock is the monitor. Conditions, over the whole class tree (family) since the enum's
variants share one form:

1. every synchronized region on an object of the family is a synchronized instance method or a
   `synchronized (this)` block inside the family's own methods (no `synchronized (other)` where
   `other` may be of the family);
2. no synchronized region, through any call it may make (`callees` over the hierarchy), enters
   another synchronized region on an object of the family, or any method that locks an object of
   the family (reentry would deadlock a plain `Mutex`); and a region reaches no object of the
   family through another handle (the `isolated`/`writers` test of `analysis/borrowing.rs`, which
   today decides `&mut self`);
3. `wait`, `notify` and `notifyAll` in the family are on `this` inside such a region.

Then a synchronized method is a `&self`/`&mut self` method (or takes the handle when it calls
`wait`), and the call site locks: `c.lock().increment()`. Unsynchronized accesses to the same
object lock the same mutex per access, which is correct and stronger than Java.

**Separate** (`Arc<java::Monitored<C>>` = `{ monitor: java::ReentrantMonitor, data:
java::Locked<C> }`, with `Deref` to the data so `o.lock()` keeps working): everything else.
`synchronized (x) { body }` is `{ let _monitor = x.monitor.enter(); body }`; a synchronized
method's body is wrapped the same way, and the method takes the handle. The guard's `Drop`
exits, so `?` and `break` release it as Java's `monitorexit` does. `java::ReentrantMonitor` is
`Mutex<(owner: Option<ThreadId>, count: u32)>` and a `Condvar`: `enter` increments the count
when the owner is the current thread, waits on the condvar otherwise; `exit` decrements and
notifies when the count reaches zero; `wait` checks ownership (`IllegalMonitorStateException`
otherwise), saves the count, gives up ownership, waits on a second `Condvar` for `notify`, then
retakes ownership with the saved count. `Thread.holdsLock(o)` is `o.monitor.held_by_current()`.

The fused form is the one to expect in programs; the separate form exists so that nothing is
rejected. A class with no fields whose only use is locking (`new Object()`) is the bare
`Arc<java::ReentrantMonitor>` of 2.6.

### 3.3 Interrupt

`Thread.interrupt()` sets the target's `interrupted` flag (a volatile in the synthetic
`Thread`) and wakes it from wherever it blocks: `unpark` (which ends `park`, `sleep` and
`join`, all implemented over the thread record's permit) and `notify_all` on the monitor the
target is waiting on (the record holds a pointer to it while in `wait`). The woken `wait`,
`sleep` or `join` finds the flag, clears it and returns `Err(InterruptedException)`.
`Thread.interrupted()` reads and clears, `isInterrupted()` reads. Interrupting a thread blocked
in a read of standard input is not supported (Java does not interrupt it either).

## 4. Thread-escape analysis

### 4.1 Roots

An object may reach another thread when it is, or is reachable from:

- a `Thread` object (the starting thread and the started one both hold it, so every `Thread`
  subclass is thread-shared, and so is everything its fields reach);
- a value captured by a `Runnable`, `Callable`, `Supplier`, ... (a lambda, a method reference's
  receiver, an anonymous or local class's captured variables and `this`) that is passed to
  `new Thread(..)`, an executor's `execute`/`submit`/`invokeAll`, a `CompletableFuture`
  `*Async` method, `ForkJoinTask.fork`/`invoke`, a parallel stream's operations, or any
  parameter of a translated library method that stores it (found as: the parameter flows into a
  field of a thread-shared class, which is the general rule below);
- a static field, in a method that may run on more than one thread (4.3);
- an argument or return value crossing a thread boundary through a translated library object
  that is thread-shared (a `BlockingQueue`, a `ConcurrentHashMap`, a `Future`): covered by the
  general rule, since the library object's fields are typed and the element types flow in.

### 4.2 Propagation

Without a points-to analysis (thorough mode's first item) the closure is **type-based**, like
every existing per-class fact: a class `C` is thread-shared when an object of `C` is a root, or
`C` is a variant (in the object model's `flows` sense) of the declared type of a field, array
element type or type argument of a thread-shared class, or of a local, parameter or return type
that a root's method body stores into one. Owned (plain value) classes are not shared by
reference: a plain value moved into a closure is simply `Send`. Immutable classes become `Arc<C>`
without a lock.

The result is a set `thread_shared: HashSet<usize>` in `Facts` (per class), `shared_arrays`
per array allocation site, `atomic_fields: HashSet<(usize, String)>` (volatile or handle-
accessed), `locked_classes`, `fused_monitor` and `waited_classes`, and `thread_methods` (4.3).

Precision needed: the cost of overestimating is `Arc` and a lock where `Rc` and `RefCell`
would do, which is speed, not correctness; rustc verifies the other direction. A type-based
closure makes, for example, every `Node` of a linked list shared when one list is handed to a
thread. That is acceptable for the first version and is measured (section 9). The points-to
analysis of thorough mode replaces the type-based closure by reachability over the points-to
graph, per allocation site, and then a class whose sites split gets two structs (as a real
generic gets one per instantiation); the escape analysis is written as a client of an abstract
"heap edges" interface so that only the edge provider changes.

### 4.3 Which methods run on which threads

`thread_methods` is the set of methods reachable (through `callees`, over all override targets)
from the `run()`/`call()`/`get()`/`accept()` implementation of any root task, plus everything
reachable from a translated library method that a worker runs. A method not in it runs only on
the main thread. This decides the thread-local statics of 2.5, and it lets a class whose objects
are created and used only in main-only methods stay thread-local even if its type appears in a
shared class's field that no thread-side code reads (a refinement to add when the report shows
it matters, not in the first version).

### 4.4 rustc as the verifier

The analysis is sound by construction (it over-approximates), and rustc checks it: an `Rc`,
`RefCell` or `Cell` inside an `Arc`'s payload, or inside a `spawn` closure, is a compile error
(`Rc<..> cannot be sent between threads safely`), never a miscompilation. So a bug in the
analysis shows up in the corpus build as an error naming the type, and the fix is in the
analysis, never a silent fallback. The thorough-mode idea of speculating and letting rustc reject
does not apply to sharing: the general form must be chosen up front, because a wrong `Rc` would
be an error rather than a slower program.

## 5. The hand-written native layer

Everything below lives in `crates/runtime/src/thread.rs`, `monitor.rs`, `atomic.rs` and
`statics.rs`, behind a cargo feature `threads` of the runtime crate that the generated
`Cargo.toml` enables for multi-threaded programs (so single-threaded programs keep the
thread-local output buffer and descriptor table).

### 5.1 `Thread`, `ThreadGroup`, `ThreadLocal` as synthetic classes

`crates/translator/java/Threads.java`, printed to a tree at build time like `Standard.java`,
declares the classes a program sees; the members the program and the reached library use are
lowered. `java.lang.Thread`:

- fields: `name` (volatile `String`), `daemon`, `priority` (stored, no effect), `interrupted`
  (volatile `boolean`), `task` (`Runnable`, may be null), `tid` (`long`), `group`
  (`ThreadGroup`), `uncaughtExceptionHandler`, `parkBlocker` (volatile `Object`),
  `threadLocalRandomSeed` (`long`), `threadLocalRandomProbe`, `threadLocalRandomSecondarySeed`
  (`int`; `ThreadLocalRandom` and `Striped64` reach them through `Unsafe` offsets), `locals`
  (a `long` key for the runtime's thread-local store), `handle` (the native record: alive flag,
  park permit, waiting-on pointer, join condvar);
- constructors: `()`, `(Runnable)`, `(Runnable, String)`, `(String)`, `(ThreadGroup, Runnable)`,
  `(ThreadGroup, Runnable, String)`, `(ThreadGroup, Runnable, String, long)` (the one
  `Executors.DefaultThreadFactory` and `ForkJoinWorkerThread` call), naming unnamed threads
  `Thread-N` from a static counter as Java does;
- methods: `start`, `run` (runs `task` if not null), `join()`, `join(long)`, `isAlive`,
  `interrupt`, `isInterrupted`, `interrupted`, `currentThread`, `sleep` (`long`, `long,int`,
  `Duration`), `yield`, `onSpinWait`, `holdsLock`, `getName`/`setName`, `isDaemon`/`setDaemon`
  (`IllegalThreadStateException` after start), `getId`/`threadId`, `getPriority`/`setPriority`,
  `getThreadGroup`, `getState` (`NEW`, `RUNNABLE`, `TERMINATED`; `WAITING`, `TIMED_WAITING` and
  `BLOCKED` from the record's blocking state), `getUncaughtExceptionHandler`/`set...`,
  `getDefaultUncaughtExceptionHandler`/`set...`, `toString` (`Thread[#1,main,5,main]`),
  `dispatchUncaughtException` (the handler chain ending in `ThreadGroup.uncaughtException`,
  which prints `Exception in thread "<name>" ` and `e.printStackTrace()` to standard error
  exactly as the JDK's does), `checkAccess` (nothing);
- `java.lang.ThreadGroup`: `name`, `parent`, `getName`, `getParent`, `uncaughtException`,
  `getMaxPriority`. The main thread's group is `main`, as in Java.
- `java.lang.ThreadLocal<T>`: `get`, `set`, `remove`, `initialValue`, `withInitial(Supplier)`
  (a subclass whose `initialValue` calls the supplier). Storage is the runtime's per-thread map
  from the `ThreadLocal` object's key to a type-erased value (`thread_local_get::<T>(key)`,
  `thread_local_set`, `thread_local_remove`), so the JDK's `ThreadLocalMap` with its weak
  references is not translated. `InheritableThreadLocal` is rejected.

Rejected, each with a message naming it: virtual threads (`Thread.ofVirtual`,
`startVirtualThread`, `Executors.newVirtualThreadPerTaskExecutor`), `Thread.Builder`,
`ScopedValue`, `StructuredTaskScope`, `getStackTrace`, `getAllStackTraces`, `dumpStack`,
`stop`/`suspend`/`resume`, `ThreadGroup` enumeration, `setContextClassLoader`.

### 5.2 Natives and their Rust

| native | Rust |
| --- | --- |
| `Thread.start0` | `std::thread::Builder::new().name(name).spawn(move \|\| { set CURRENT; let r = Thread::run(&t); if let Err(e) = r { t.dispatch_uncaught(e) } t.record.terminated(); })`; a daemon thread is not counted in the non-daemon registry |
| `Thread.currentThread` | a `thread_local!` `CURRENT: OnceCell<Arc<..>>`; the main thread's record is created on first use with name `main` and id 1 |
| `isAlive` | the record's `alive: AtomicBool` |
| `join`, `join(millis)` | wait on the record's `Condvar` until `alive` is false or the timeout, interruptible; the synthetic `join` is `while (isAlive()) wait(..)` on the `Thread` object as in the JDK, so no native beyond `isAlive` and the monitor is needed |
| `interrupt0` | wake the record: `unpark`, and `notify_all` on the monitor it waits on |
| `sleep`, `yield`, `onSpinWait` | `park_timeout` loop until the deadline, returning `InterruptedException` if interrupted (`sleep` today is a plain `std::thread::sleep`; it becomes interruptible); `std::thread::yield_now`; `std::hint::spin_loop` |
| `Unsafe.park(absolute, time)`, `Unsafe.unpark(thread)` | a permit per record: `Mutex<bool>` and a `Condvar` (`std::thread::park` has the right semantics but no handle before `spawn` returns; the record is created first, so our own permit is simpler) |
| `Object.wait(millis)`, `notify`, `notifyAll` | `java::Monitor`/`ReentrantMonitor` (section 3) |
| `Thread.holdsLock` | `monitor.held_by_current()` |
| `Unsafe.objectFieldOffset(C.class, "f")`, `MethodHandles.lookup().findVarHandle(C.class, "f", T.class)`, `MhUtil.findVarHandle` | not a value: a *field handle* constant (5.3) |
| `Unsafe.arrayBaseOffset`/`arrayIndexScale(T[].class)`, `MethodHandles.arrayElementVarHandle` | an *array handle* constant |
| `Unsafe.get{Int,Long,Reference,Boolean,Byte,Short,Char,Float,Double}` and `put...`, plain, `Opaque`, `Acquire`, `Release`; `VarHandle.get/set{,Opaque,Acquire,Release}` | `load(Acquire)` / `store(Release)` on the field's atomic |
| `...Volatile` forms; `VarHandle.getVolatile/setVolatile` | `load(SeqCst)` / `store(SeqCst)` |
| `compareAndSet*`, `compareAndExchange*{,Acquire,Release}`, `weakCompareAndSet*{,Plain,Acquire,Release}` | `compare_exchange(expected, new, SeqCst, SeqCst)` (the strong form: a weak CAS may fail spuriously in Java, so never failing is fewer behaviours); references compare by identity (`Arc::ptr_eq`, the enum's `PartialEq`) |
| `getAndAdd{Int,Long}`, `getAndSet*`, `getAndBitwise{And,Or,Xor}*` | `fetch_add`, `swap`, `fetch_and`/`fetch_or`/`fetch_xor`, `SeqCst` |
| `fullFence`, `loadFence`/`acquireFence`, `storeFence`/`releaseFence`, `loadLoadFence`, `storeStoreFence` | `fence(SeqCst)`, `fence(Acquire)`, `fence(Release)`, `fence(Acquire)`, `fence(Release)` |
| `Unsafe.ensureClassInitialized`, `shouldBeInitialized` | the class's `init` call; `false` |
| `Thread.yield0`, `setPriority0`, `setNativeName`, `registerNatives`, `ensureMaterializedForStackWalk`, `scopedValueCache` | nothing (the synthetic class does not declare them) |
| `System.exit`, `Runtime.halt` | flush, `process::exit` (already) |
| `System.out` | the buffer becomes `static OUT: Mutex<Buffer>`, locked for one whole `print`/`println` so lines from two threads never interleave inside a line, as `PrintStream`'s synchronized methods guarantee; flushed by `end_main` and `exit`; `fd.rs`'s table likewise |

### 5.3 Field and array handles

`java.util.concurrent` reaches its fields through `Unsafe` offsets and `VarHandle`s:

```java
private static final long STATE = U.objectFieldOffset(AbstractQueuedSynchronizer.class, "state");
... U.compareAndSetInt(this, STATE, expect, update) ...
private static final VarHandle RESULT = MhUtil.findVarHandle(l, "result", Object.class);
... RESULT.compareAndSet(this, null, r) ...
private static final int ABASE = U.arrayBaseOffset(Node[].class), ASHIFT = 31 - Integer.numberOfLeadingZeros(U.arrayIndexScale(Node[].class));
... U.getReferenceAcquire(tab, ((long)i << ASHIFT) + ABASE) ...
```

Lowering (`lower/handles.rs`) gives a `static final` whose initializer is one of the handle
calls the type `Type::FieldHandle(class, field)` or `Type::ArrayHandle(elem)` instead of
`long`/`VarHandle`; it is a compile-time constant of the lowering, not a Rust item. Every use
must be as the offset or handle argument of one of the operations of 5.2, whose receiver's
static type is the handle's class (or a subtype) and whose value type matches the field's; any
other use (printing it, arithmetic, passing it on) is rejected: "a field offset used as a
value". An array offset argument must be `ABASE + (index << ASHIFT)`, `ABASE + index * ASCALE`
or a call of a one-line method returning such an expression (`ForkJoinPool.slotOffset`), and
lowers to the element index. The field named by a handle becomes an atomic field
(`atomic_fields`) in every class of the family, and the array an atomic array. `java.lang.invoke`
stays `UNTRANSLATED` except for `VarHandle`, `MethodHandles.lookup`, `findVarHandle`,
`findStaticVarHandle`, `arrayElementVarHandle` and `MhUtil.findVarHandle`, which are handles;
`jdk.internal.misc.Unsafe` is a runtime class whose other methods (`allocateMemory`,
`allocateInstance`, `defineClass`, raw addresses, `getLoadAverage`, `copyMemory`) are rejected
as "raw memory". `jdk.internal.access.SharedSecrets.getJavaLangAccess()` is a runtime class
too: `currentCarrierThread()` is `Thread.currentThread()`, `parkVirtualThread`/`unpark...` are
unreachable because `isVirtual()` is `false` in the synthetic class (reachability still sees
them; they lower to `unreachable!`). `jdk.internal.vm.SharedThreadContainer` (which
`ThreadPoolExecutor` and `ForkJoinPool` use to start threads) is a synthetic class with
`create(name)`, `start(thread)` (= `thread.start()`), `close()`, `threadCount()`.
`@Contended` and `@ReservedStackAccess` are annotations and reach nothing.

### 5.4 What then translates as-is, and what is pruned

Translates from the JDK's source: `LockSupport`, `AbstractOwnableSynchronizer`,
`AbstractQueuedSynchronizer` (and `...LongSynchronizer`), `ReentrantLock`, `Condition`,
`ReentrantReadWriteLock`, `StampedLock`, `Semaphore`, `CountDownLatch`, `CyclicBarrier`,
`Phaser`, `Exchanger`, the `atomic` package (`AtomicInteger`, `AtomicLong`, `AtomicBoolean`,
`AtomicReference`, the arrays, the field updaters once reflection-free: the updaters use
`Class.getDeclaredField`, so they are rejected as reflection; `Striped64`, `LongAdder`,
`LongAccumulator`, `DoubleAdder`), `ConcurrentHashMap`, `ConcurrentLinkedQueue`/`Deque`,
`ConcurrentSkipListMap`/`Set`, `CopyOnWriteArrayList`/`Set`, `LinkedBlockingQueue`,
`ArrayBlockingQueue`, `PriorityBlockingQueue`, `LinkedBlockingDeque`, `SynchronousQueue`,
`LinkedTransferQueue`, `DelayQueue`, `Executors`, `ThreadPoolExecutor`,
`ScheduledThreadPoolExecutor`, `FutureTask`, `ExecutorCompletionService`, `CompletableFuture`,
`ForkJoinPool`, `ForkJoinTask`, `RecursiveTask`, `RecursiveAction`, `CountedCompleter`,
`ForkJoinWorkerThread` (extends the synthetic `Thread`), `TimeUnit`, `ThreadLocalRandom` (its
`SecureRandom` branch is behind a property that is unset, so the `java.security` boundary is not
crossed), `Thread.State` as an enum of the synthetic class.

Pruned by the existing boundary and left so: `java.lang.ref` (runtime classes), reflection,
serialization, `java.security`, `jdk.internal.vm.annotation`. Unsupported, rejected with a
message: `Executors.privilegedThreadFactory` (security), `ForkJoinPool` with a custom
`ForkJoinWorkerThreadFactory` that uses `Thread.Builder`, `Thread.getAllStackTraces`,
`ThreadMXBean` and everything under `java.lang.management`, timed waits with `Instant`
clocks that are not `System.nanoTime` (none in `java.util.concurrent`).

## 6. Exceptions across threads, daemon threads, exit

The exceptions design makes a method return `Result` only where a caller observes what escapes
it, and `java::uncaught(e)` prints and exits with 1 where nothing does. In a multi-threaded
program two more observers exist:

- **The entry of a thread observes everything.** `Thread.run()` and every implementation of
  `Runnable.run`/`Callable.call`/`ForkJoinTask.exec` that a task may run count as called from a
  `try { .. } catch (Throwable)` in the analysis (`observed(m)` gets a root per thread entry),
  so every exception reachable from them propagates by `?` to the spawn closure, which hands it
  to `dispatchUncaughtException`: the thread's handler, else the default handler, else
  `ThreadGroup.uncaughtException`, which prints `Exception in thread "Thread-0"
  java.lang.IllegalStateException: boom` (no stack frames, as the exceptions design decided) to
  standard error. The thread ends; the others continue; the exit code is not affected, as in
  Java. Inside an executor, `FutureTask.run` catches `Throwable` itself and `get()` rethrows an
  `ExecutionException`; this is the translated JDK code and needs no special case, but it means
  every runtime failure (division, index, null, cast, parse) inside a task must have its checked
  form (`java::try_div(..)?`), which the exceptions design provides.
- **`main` observes everything too**, with its epilogue `java::end_main(result)`: on `Err(e)`,
  print `Exception in thread "main" ` and `e` at once; then wait until every non-daemon thread
  has ended (the runtime counts live non-daemon threads in `java::threads::LIVE`, decremented
  when a thread's closure returns; `end_main` waits on its condvar), flush, and exit with 1 if
  `main` threw and 0 otherwise. This is what the `java` launcher does (`DestroyJavaVM` waits for
  the non-daemon threads). In a single-threaded program `end_main` is today's immediate exit.

Daemon threads (`setDaemon(true)` before `start`) are not counted, so the process exits while
they run and they die with it, as in Java. `System.exit(n)` and `Runtime.halt(n)` from any
thread flush the output buffer and call `process::exit(n)`, which ends every thread at once;
`System.exit` inside a `synchronized` region or with other threads mid-print behaves as Java's
(their partial output is lost in both). Shutdown hooks are rejected as today. A thread that is
blocked forever (a deadlock) keeps the process alive, as in Java.

`InterruptedException` is a standard class of `Standard.java` already; `sleep`, `join`, `wait`,
`park`-based library methods and `BlockingQueue` operations throw it as a value through the
natives' `Result<_, java::Exception>`, and `?` converts it. `IllegalMonitorStateException` and
`IllegalThreadStateException` join the standard table.

## 7. Testing

Differential testing compares one Java run with one Rust run, which assumes a deterministic
output. Thread programs are tested in four ways:

1. **Deterministic thread programs in the corpus** (`corpus/threads/`): programs whose output
   does not depend on scheduling. Each thread computes into its own slot or object; `join`,
   `CountDownLatch`, `Future.get` or `invokeAll` order the output after the work; what a thread
   prints, if anything, is printed after a `join` in a fixed order. Most real programs are of
   this kind. The corpus test runs them like every other program.
2. **Programs with an allowed set of outputs.** A thread program may legitimately print in
   several orders. Such a program carries a header comment `// output: sorted lines` (or
   `// output: multiset of lines`), and the corpus harness sorts both sides' lines before
   comparing; the exit code must still match. A program whose outputs differ in more than order
   is written as a **self-checking program** instead: it computes an invariant (the counter
   equals the number of increments, every produced item was consumed once, the sum is the
   expected sum) and prints `ok` or the violation, so Java and Rust print `ok`, and a wrong
   translation prints a diagnosis. Both kinds are also deterministic from the harness's point of
   view, so no second Java run is needed.
3. **Stress runs** (`scripts/stress-threads.sh`): every program of `corpus/threads/` runs
   twenty times on the Rust side, once pinned to one CPU (`taskset -c 0`, where races surface
   that parallel runs hide), once on all CPUs, and once with a `sched_yield` injected into
   `java::Locked::lock` and `Monitor::enter` under an environment variable (the runtime's
   "chaos" mode, a hundred lines in the runtime), comparing against the Java output or the
   invariant each time. A failure is reproduced with `rr record --chaos`, which randomizes
   scheduling and records deterministically; no code changes are needed for it.
4. **The runtime's primitives are tested with loom**: `java::Monitor`, `ReentrantMonitor`,
   `ClassInit`, the park permit and `end_main`'s thread count are written against `loom`'s
   `Mutex`/`Condvar`/`thread` under `#[cfg(loom)]` and model-checked for the two- and
   three-thread cases (lost wakeups, reentrancy, interrupt during `wait`, recursive class
   initialization). loom is a dev-dependency only.

A deterministic scheduler for the Rust side (one thread at a time with explicit switch points)
is **not** built: it would need every blocking primitive, including the data lock on every
shared field access, to go through the runtime's own scheduler, which is a second runtime,
and `rr --chaos`, `taskset -c 0` and the yield injection find the same bugs for a fraction of
the work. Revisit only if a flaky failure resists those three.

Measurement: the translation report gains rows for thread-shared classes (how many, and which
sites made them so), monitors by form, atomic fields, process-wide statics, and parallelized
loops with their thresholds; the benchmarks gain thread programs (section 9).

## 8. Optimizations that use threads

### 8.1 Parallel streams: translated `ForkJoinPool`, not Rayon

`list.parallelStream().map(..).reduce(..)` runs the JDK's own stream and `ForkJoinPool` code
over the natives, with the common pool's parallelism `availableProcessors() - 1` as Java's. The
reasons against Rayon: the JDK splits a sized source deterministically (midpoints down to
`size / (parallelism * 4)`) and combines in a fixed tree, so a parallel `reduce` over doubles
prints exactly what Java prints on the same machine, and the corpus test runs both on one
machine; Rayon's splitting follows stealing, so a floating-point result would vary run to run
and differ from Java's; and Rayon is a dependency on the generated crate, which has none
today. The costs: parallel stream code reads like the JDK, not like Rust, and the translated
pool is slower than Rayon (atomics through our natives, `Arc` and locks around tasks). Both
are acceptable for the faithful form.

The refinement, after the sequential stream-to-iterator work of `docs/lists-and-streams.md`:
a parallel pipeline whose terminal operation is order- and split-independent (`sum` of
integers, `count`, `min`/`max`, `anyMatch`/`allMatch`/`noneMatch` over side-effect-free
predicates, `collect(toSet())`, `forEach` whose body writes only its own element) over a sized
source, with side-effect-free lambdas, is written with the chunked helper of 8.2 instead of the
translated pool, which makes it both readable and fast. Floating-point `sum`, `average` and
`reduce` are never moved: `DoubleStream.sum` uses compensated summation whose result depends on
the split, so only the translated code reproduces it.

### 8.2 Automatic parallelization of independent loops

A counting loop `for (int i = a; i < b; i++) body` (already a `for i in a..b` by the loop
analysis) is run in parallel when all of these are proven:

1. **Independent iterations.** The body writes only (a) elements `out[i]` or `out[i + c]` of
   plain (owned, non-aliased: `analysis/arrays.rs`) arrays, where the index is `i` plus a
   loop-invariant constant, so no two iterations write the same element, (b) locals declared in
   the body, and (c) reduction variables (below). It reads only loop-invariant values (locals
   not assigned in the loop, arrays the loop does not write, immutable objects, final fields,
   `static final` constants) and elements of the arrays it writes at the same index it writes
   (`a[i] = f(a[i])`).
2. **No other effects.** No printing, no field or static writes, no allocation of shared
   objects, no calls except `Math`/`StrictMath`, the string and character builtins without
   effects, and methods the effect analysis proves pure (no writes, no calls with effects, no
   throws); until thorough mode's "effects across calls" exists, that means static methods whose
   bodies pass the same test recursively.
3. **No exception.** Every index is in bounds (`analysis/arrays/bounds.rs`), every division has
   a provably nonzero divisor (`analysis/ranges.rs`), nothing dereferenced may be null, no
   checked cast. A Java exception at iteration `i` leaves iterations before `i` done and the
   rest not; parallel execution would do some of the rest, which the program could see in the
   array afterwards.
4. **Reductions.** `acc += e`, `acc *= e`, `acc &= e`, `|=`, `^=`, `acc = Math.min(acc, e)`,
   `max`, on `int`, `long`, `short`, `byte`, `char` or `boolean`: wrapping addition and
   multiplication in Z/2^n are associative and commutative, and so are the bit operations and
   `min`/`max`, so every order of combining the chunks' partial results gives the same value,
   overflow included. `acc += e` on `float` or `double` is **not** a reduction: IEEE addition is
   commutative but not associative, so a different grouping gives a different result, visible
   when printed. The relaxation `floating-point reductions in this method may be reordered`
   (added to thorough mode's table) lets the user allow it per method, listed in the report.
   Divisions, remainders and string concatenation are never reductions.
5. **Worth it.** The generator weighs the body (operations, array accesses, calls; `w ≥ 1`) and
   the helper runs the loop in parallel only when `(b - a) * w >= 500_000` (about a millisecond
   of work at a nanosecond or two per operation, against 15 to 30 µs to spawn each scoped
   thread), otherwise sequentially in the same closure. A loop nested in a parallel region runs
   sequentially (a thread-local flag in the helper).

Output: the loop becomes a call of a runtime helper over `std::thread::scope`, which splits the
range into `available_parallelism()` chunks:

```java
double[] out = new double[n];
long checksum = 0;
for (int i = 0; i < n; i++) {
    double x = i * 0.001;
    out[i] = Math.sin(x) * Math.cos(x);
    checksum += (long) (out[i] * 1000);
}
```

```rust
let mut out = vec![0.0f64; n as usize];
let checksum: i64 = java::par_for(0, n, 12, &mut out, |i, out_i| {
    let x = i as f64 * 0.001;
    *out_i = x.sin() * x.cos();
    (*out_i * 1000.0) as i64
}, 0i64, i64::wrapping_add);
```

`par_for(from, to, weight, out: &mut [T], body: impl Fn(i32, &mut T) -> R + Sync, zero: R,
combine: fn(R, R) -> R) -> R` hands each chunk a disjoint `&mut [T]` (`chunks_mut`) and folds
each chunk's results with `combine`, then the chunks' results in index order; with several
output arrays a tuple of slices; with no output array `par_reduce`; with no reduction `()`.
rustc verifies the proof: the body must be `Fn` (no writes to captured locals), `Sync` (no
`Rc`, `RefCell` or `Cell` captured), and can only write through the `&mut T` it is given
(disjoint by `chunks_mut`). What rustc cannot check: a captured `Arc<Locked<..>>` would let the
body write a shared object, so condition 2 excludes shared objects itself. The sequential path
of the helper runs the same closure in index order, so the parallel and sequential results are
identical for integer reductions by construction.

Note that loops that print or that run an index-dependent effect are never parallelized, and
`out` must be a plain `Vec` (an aliased `java::Array` would need a lock per element).

## 9. Performance

- **Single-threaded programs pay nothing**: with `Thread.start0` unreachable, the output is
  byte-for-byte today's (`Rc`, `RefCell`, `thread_local!`, the thread-local output buffer
  behind the runtime's default features). The corpus and `scripts/benchmark.py` check this: the
  translation of every existing benchmark must not change.
- **Multi-threaded programs**: a thread-shared mutable object costs an uncontended lock and
  unlock per field access or `&mut self` call (10 to 20 ns on Linux's futex `Mutex`, against
  about 1 ns for a `RefCell` borrow) and an atomic increment per handle clone. A thread-shared
  immutable or lock-free object costs nothing on reads beyond `Arc`'s refcount. A fused monitor
  costs one lock per synchronized call, like HotSpot's thin lock. A `volatile` read is a plain
  load, a write an `xchg`, as HotSpot emits. Process-wide primitive statics are plain loads and
  stores on x86-64 (acquire/release), the same as today's `Cell`; object statics cost a lock per
  access unless `static final` (`LazyLock`, no lock on read).
- **Keeping the common case fast**: (1) thread-local classes keep `Rc`/`RefCell`, which the
  type-based escape analysis decides for every class no thread reaches; (2) immutability and
  the lock-free rule remove the lock from most shared classes of `java.util.concurrent` and from
  shared inputs; (3) shared primitive arrays written by workers are atomic per element, so
  `out[i] = ..` is a `mov`; (4) a `Vec<T>` or plain value moved into a thread is not shared at
  all; (5) the thread-method set keeps main-only statics thread-local. The report lists what
  became shared and why, so a slow case can be traced to its root.
- **Benchmarks to add**: `ParallelSum` (an `ExecutorService` summing slices), `MonitorCounter`
  (two threads on a synchronized counter), `ProducerConsumer` (`LinkedBlockingQueue`),
  `ParallelStreamSum`, `AutoParallelLoop` (a sequential Java loop that the translation
  parallelizes, timed against Java's single thread: the one case where the Rust side should be
  several times faster), `ConcurrentMapCount` (`ConcurrentHashMap.merge` from eight threads).

### 9.1 Two more examples

**An `ExecutorService` summing in parallel** (translated library, lambdas, `Future`):

```java
int[] data = ...; int parts = 4; ExecutorService pool = Executors.newFixedThreadPool(parts);
List<Future<Long>> results = new ArrayList<>();
for (int p = 0; p < parts; p++) {
    final int from = p * data.length / parts, to = (p + 1) * data.length / parts;
    results.add(pool.submit(() -> { long s = 0; for (int i = from; i < to; i++) s += data[i]; return s; }));
}
long total = 0;
for (Future<Long> f : results) total += f.get();
pool.shutdown();
System.out.println(total);
```

```rust
let data: Arc<Vec<i32>> = java::share_frozen(data);   // filled before, never written after
let pool = Executors::new_fixed_thread_pool(parts)?;
let results: Vec<Future> = Vec::new();               // the list representation of lists-and-streams
for p in 0..parts {
    let from = p.wrapping_mul(data.len() as i32) / parts;
    let to = p.wrapping_add(1).wrapping_mul(data.len() as i32) / parts;
    results.push(pool.submit(java::callable({
        let data = data.clone();
        move || {
            let mut s: i64 = 0;
            for i in from..to {
                s = s.wrapping_add(data[i as usize] as i64);
            }
            Ok(Object::Long(s))
        }
    }))?);
}
let mut total: i64 = 0;
for f in &results {
    total = total.wrapping_add(f.get()?.to_long());
}
pool.shutdown();
java::println!("{}", total);
```

`pool` is the translated `ThreadPoolExecutor` behind the `ExecutorService` enum (an interface
with several implementors in the reached code), `Future` the enum over `FutureTask` and the
`CompletableFuture` variants reached; `data` is captured by a task, so its allocation site is
thread-shared, and since nothing writes it after `share_frozen` it is `Arc<Vec<i32>>` read
without a lock and in bounds (`from..to` within `data.length` by the range analysis). The
erased `Callable` returns `Object`; the real-generic refinement makes it `i64`.

**A parallel stream:**

```java
long evens = IntStream.range(0, n).parallel().filter(i -> i % 2 == 0).count();
```

```rust
// faithful: the translated stream pipeline over the translated common ForkJoinPool
let evens = IntPipeline::range(0, n).parallel().filter(java::int_predicate(|i| java::rem(i, 2) == 0)).count()?;
// refinement (8.1): count is split-independent and the predicate has no effects
let evens = java::par_count(0, n, |i| java::rem(i, 2) == 0);
```

## 10. Milestones

Each milestone is mergeable on its own with `cargo fmt`, clippy clean, `cargo test` green, the
corpus extended, the README's "Current state" updated and the report's new rows recorded. The
dependencies name the other plans' milestones: object model (inheritance and enums: done;
interfaces: done; `Object`: done; erased generics: its fifth milestone), exceptions (steps 3 to 5),
lambdas and inner classes (not planned yet; "Runnable lambdas" means lambdas or method
references converting to a functional interface, and "anonymous classes" the `new Thread() {
public void run() .. }` form).

### The thread-aware runtime and process-wide statics

Can start now; needs nothing. Runtime, behind the `threads` feature: `java::Locked<T>`,
`java::Monitor<T>` (lock plus condvar: `wait`, `wait_timeout`, `notify`, `notify_all`),
`java::ReentrantMonitor`, `java::Atomic<T>`/`java::Volatile<T>` for the primitive types and
`f32`/`f64` through bits, `java::AtomicRef<T>`, `java::ClassInit`, the thread record (alive
flag, park permit, interrupt flag, waiting-on pointer, live non-daemon count), `park`/`unpark`,
interruptible `sleep`, `end_main`, the process-wide output buffer and descriptor table, and the
debug-mode `try_lock` timeout of 1.3. Translator: the predicate "multi-threaded program"
(`Thread.start0` reachable; for single-file programs, `Thread` mentioned), and
`codegen/statics.rs` writing `static` items with `java::Atomic`/`java::Locked`/`LazyLock` and
`ClassInit` when it holds. Tests: loom tests of the primitives; shape tests
`statics_are_process_wide_in_a_threaded_program`, `a_single_threaded_program_is_unchanged`
(the translation of every existing corpus program is byte-identical); corpus:
`corpus/threads/StaticsAcrossThreads.java` waits for the threads-by-subclassing milestone to run.

### Threads by subclassing `Thread`

Before lambdas and inner classes. The synthetic `Threads.java` (`Thread` with the members of
5.1 except the handler interfaces, `ThreadGroup`); `extends Thread` accepted (the object model's
"extending `java.lang.Thread` not supported yet" lifted); `Thread` objects are thread-shared;
`start`, `run` override, `join`, `sleep`, `yield`, `currentThread().getName()`, `setName`,
`isAlive`, `setDaemon`, `getId`; the spawn closure; a `Thread` subclass's fields reach the
started thread, so the thread-escape analysis is needed in its simplest form: every class a
`Thread` subclass's fields reach is thread-shared, with `Arc<java::Locked<C>>`/`Arc<C>`, and
shared arrays of 2.4. `synchronized` stays the null check (correct for programs whose shared
state is written by one thread and read after `join`). Corpus (`corpus/threads/`):
`StartAndJoin.java` (two workers filling their own slots, printed after join),
`ThreadNames.java` (`Thread-0`, `Thread-1`, `main`, `toString`), `SleepAndJoin.java`,
`DaemonExit.java` (a daemon thread sleeping forever does not keep the program alive),
`StaticsAcrossThreads.java` (a static written by main before `start`, read by the worker),
`SharedArraySlices.java` (each worker fills its slice of one `long[]`: the atomic array form),
`programs/ParallelMatrixRows.java`. Survey: "threads" stops counting `extends Thread`,
`Thread.start`/`join`.

### The thread-escape analysis and shared representations

`analysis/threads.rs`: roots of 4.1 (with `Thread` subclasses and, when lambdas land, captures),
the type-based closure of 4.2, `thread_methods` of 4.3, `shared_arrays`; the generator's `Arc`
and `lock` twins for every handle form, enum variants over shared classes, `base()` through a
guard, `Arc::ptr_eq` identity, moves of `Arc`s into closures; the report rows. Corpus:
`SharedGraph.java` (a tree built by main, walked by two threads, totals printed after join),
`ThreadLocalClassesStayRc.java` (shape test: a class no thread reaches keeps `Rc`),
`MainOnlyStatics.java` (shape test: a static only main touches stays thread-local).

### Monitors

Before lambdas. `synchronized` methods and blocks, the fused and separate forms of 3.2 with
the family-wide decision, `wait`/`notify`/`notifyAll` with timeouts, `interrupt` with
`InterruptedException` from `wait`/`sleep`/`join`, `holdsLock`, static synchronized methods
and `synchronized (C.class)`, `new Object()` as a lock, `IllegalMonitorStateException`.
Needs exceptions step 3 (checked `InterruptedException` as a value) for `wait` and `join`
inside `try`. Corpus: `SynchronizedCounter.java` (the counter of 2.7 with `Thread` subclasses),
`ProducerConsumer.java` (2.7), `ReentrantSynchronized.java` (a synchronized method calling
another: the separate form), `StaticSynchronized.java`, `WaitTimeout.java`,
`InterruptWait.java`, `LockObject.java` (`new Object()` locks guarding two counters),
`programs/BankTransfersThreaded.java` (transfers between accounts from several threads with
ordered locking, balances summed after join), `programs/DiningPhilosophers.java` (ordered
forks, a fixed number of meals, counts printed).

### Exceptions across threads

Needs exceptions steps 3 to 5 and the threads-by-subclassing milestone. Thread entries as observers, `end_main` waiting
for non-daemon threads and exiting with 1 after `main`'s uncaught exception, uncaught
exceptions in threads printed through the `ThreadGroup` chain, `System.exit` from a worker,
`Runtime.halt`. Corpus: `UncaughtInThread.java` (a worker throws; main joins and prints; exit
0), `UncaughtInMainWithWorkers.java` (main throws while a worker still prints; exit 1 after the
worker ends), `ExitFromWorker.java` (`System.exit(3)` from a thread), `InterruptedSleep.java`.

### Monitors

- **Always the fused form, on a reentrant lock.** The data lock of `java::Locked` is reentrant
  (`RawLock`), so the reason for the separate form of 3.2 (a plain `Mutex` would deadlock on
  reentry) is gone, and there is one form: an object's monitor is its data lock. No
  `java::Monitor<C>`, `java::Monitored<C>` or `java::ReentrantMonitor`, and no family-wide
  choice. `synchronized (x) { .. }` is `{ let _monitor = java::enter(&x); { .. } }`, a
  synchronized method's body starts with `let _monitor = java::enter(this);`, and the field
  borrows inside take the lock once more. The call site does not lock (`c.lock().increment()` of
  3.2): a method that uses its object's monitor takes the handle (it joins `long_methods`), so a
  synchronized method calling another, or `wait` inside one, holds no `Ref` across the call.
  `wait` asserts that the thread holds no borrow of the object, which would let another thread
  in while it lasts. A thread waiting for a lock held as a monitor is not watched by the
  watchdog, since Java may hold a monitor across anything.
- **Which monitors are kept** (`analysis/monitors.rs`) is decided per program, not per class as
  in 3.1: in a program that waits, notifies or asks `holdsLock`, every monitor; otherwise those
  of objects whose class threads may share (the type-based walk), and the class monitors in a
  program with threads. The classes whose values may be a kept lock (through the flows computed
  without the fields-struct methods, a superset) are neither immutable nor owned, so
  `new Object()` as a lock is `Arc<java::Locked<PlainObject>>`, not the bare monitor of 2.6.
  Elsewhere `synchronized` stays the null check, so a program without threads or `wait` is
  byte-identical to before.
- **A monitor of one thread's object** (an `Rc`, in a program that observes monitors) is a
  thread-local list of the addresses the thread has entered: `holdsLock` and the
  `IllegalMonitorStateException` of `wait` and `notify` answer from it, `notify` wakes nobody,
  and `wait` lasts its timeout, or for ever without one, as in Java where no other thread can
  reach the object. No `java::check_owner` was needed: the runtime checks ownership in every
  `wait` and `notify`.
- **The wait set** is one entry per waiting thread in the lock's parking bucket: `notify` marks
  the one that has waited longest (HotSpot's order), `notifyAll` all of them, and a timed-out
  waiter removes its own entry, so a later waiter cannot take a notification meant for an
  earlier one. No spurious wakeups are produced. An interrupt ends a `wait` (see "Interrupts"
  below), and `wait(millis, nanos)` is rejected.
- **Class monitors** are statics of the translation, `static C__MONITOR: java::Locked<()>`,
  entered with `java::enter_class(&C__MONITOR)` by static synchronized methods and
  `synchronized (C.class)`. `synchronized (o.getClass())` is rejected where it would be kept.
- **Rejected where the monitor would be kept**: `synchronized` on a string, a box, an array, a
  `StringBuilder` or a Java enum's constant (values, or `Rc`s without a lock), and `wait`,
  `notify` or `holdsLock` on them anywhere. `synchronized (this)` in a constructor keeps no
  monitor: no other thread has the object yet.
- **A lock read through another object** is taken out of it before the thread waits for its
  monitor (`java::enter(&{ this.borrow().lock.clone() })`), so that the thread does not hold the
  first object's data lock while it waits; the same holds for `wait` and `notify`.
- **Found on the way**: a conditional expression's condition with two shared objects
  (`a.number < b.number ? a : b`) held both data locks at once, and two threads could wait for
  each other there; in a threaded program such a condition is now guarded like a statement. A
  static field of reference type initialized eagerly in a threaded program is stored with
  `java::Locked::set`, which the runtime lacked.
- **Corpus**: `corpus/threads/` holds `SynchronizedCounter`, `ProducerConsumer`,
  `ReentrantSynchronized`, `StaticSynchronized`, `WaitTimeout`, `LockObject`, and, instead of
  `programs/BankTransfersThreaded.java` and `programs/DiningPhilosophers.java`, `BankTransfers`
  and `DiningPhilosophers` there with the other thread programs; plus `SynchronizedSubclasses`
  (inherited synchronized methods through a variable of the base type), `ExceptionInSynchronized`
  (an exception leaving a region exits the monitor) and `MonitorsOneThread` (the monitors of one
  thread).

### Volatile fields and the atomic natives

Needs the object model's erased generics (its fifth milestone, for `AtomicReference<V>` and the collections)
and whole-program mode with the JDK sources. `volatile` fields as atomics, the lock-free class
rule, `lower/handles.rs` (field and array handles), the `Unsafe`/`VarHandle` operations and
fences, `jdk.internal.misc.Unsafe`, `SharedSecrets`, `SharedThreadContainer` as runtime or
synthetic classes, `java.lang.invoke` carve-out. Translates: the `atomic` package,
`LockSupport`, `AbstractQueuedSynchronizer`, `ReentrantLock`, `CountDownLatch`, `Semaphore`,
`ConcurrentHashMap`, `ConcurrentLinkedQueue`, `CopyOnWriteArrayList`, `ThreadLocalRandom`.
Corpus: `VolatileFlag.java` (2.7), `AtomicCounter.java` (`AtomicInteger` from four `Thread`
subclasses), `LongAdderCount.java`, `ReentrantLockCounter.java`, `LatchStart.java`
(`CountDownLatch` releasing workers together), `ConcurrentMapMerge.java` (word counts from
threads, printed sorted), `programs/WorkQueue.java` (`ConcurrentLinkedQueue` of tasks drained by
workers). Lambdas are not needed: the tasks are `Thread` subclasses and the atomics' basic
methods take no functions (`updateAndGet` waits for the executors milestone).

### Executors and the concurrency library

Needs lambdas and inner classes (Runnable lambdas, anonymous `Thread` and `Runnable` classes,
`Callable`, functional interfaces), for-each over `Iterable`, and the list representation of
`docs/lists-and-streams.md` for `List<Future<T>>`. Translates: `Executors`,
`ThreadPoolExecutor`, `ScheduledThreadPoolExecutor`, `FutureTask`, `CompletableFuture`,
`ForkJoinPool`/`RecursiveTask`, the blocking queues, `CyclicBarrier`, `Phaser`, `Condition`,
`ReentrantReadWriteLock`, `StampedLock`, `Exchanger`; the capture roots of 4.1;
`UncaughtExceptionHandler`. Corpus: `ExecutorSum.java` (9.1), `InvokeAll.java`,
`CompletableFutureChain.java` (`supplyAsync().thenApply().join()`), `ForkJoinSum.java`
(`RecursiveTask` over an array), `BlockingQueuePipeline.java`, `ScheduledOnce.java`
(`schedule` with a delay, then `shutdown`), `UncaughtHandler.java`,
`programs/WebCrawlerSimulation.java` (a bounded pool over a synthetic link graph, results in a
`ConcurrentHashMap`, printed sorted), `programs/ParallelMergeSort.java` (`ForkJoinPool`).

### Thread-local values

Needs the executors milestone for `withInitial`. `ThreadLocal` (synthetic class and the runtime's
per-thread store), `ThreadLocalRandom` end to end. Corpus: `ThreadLocalCounter.java`,
`ThreadLocalRandomSums.java` (self-checking: sums within bounds, `ok`).

### Parallel streams

Needs the sequential stream translation of `docs/lists-and-streams.md` and the executors
milestone. Translated `java.util.stream` parallel evaluation over the translated `ForkJoinPool`, with the
common pool's parallelism as Java's; then the refinement of 8.1 for split-independent terminal
operations. Corpus: `ParallelStreamCount.java`, `ParallelStreamCollect.java` (an ordered
`collect(toList())`), `ParallelDoubleSum.java` (exact against Java on the same machine),
`ParallelForEachOrdered.java`.

### Automatic loop parallelization

Independent of lambdas and of the exceptions, atomics, executors, thread-local and
parallel-stream milestones; needs the runtime and the thread-escape analysis (the runtime
helper and the `Sync` facts) and the existing array, range and bounds analyses.
`analysis/parallel.rs` with the conditions of 8.2 and the body weight; `java::par_for`/`par_reduce`;
`codegen/parallel.rs`; the relaxation for floating-point reductions in the report; a loop-level
row in the report. Corpus: `AutoParallelFill.java` (`out[i] = f(i)` over a million elements,
checksum printed), `AutoParallelIntSum.java` (a wrapping sum that overflows: the same value
either way), `NotParallelFloatSum.java` (shape test: stays sequential),
`NotParallelPrints.java`, `NotParallelMayThrow.java` (an unproven index keeps the loop
sequential); benchmark `AutoParallelLoop`.

### Per-site sharing with the points-to analysis

Thorough mode's first item. The escape analysis's edge provider becomes the points-to graph,
sharing is decided per allocation site, a class with both shared and thread-local sites is
split into two structs, the "frozen after publication" test for arrays and objects becomes
flow-based, and the thread-method set becomes a per-site "which threads reach this object".

Order: runtime → threads by subclassing → thread-escape analysis → monitors → exceptions across
threads → volatile fields and atomics → executors → thread-local values → parallel streams, with
automatic loop parallelization any time after the thread-escape analysis and per-site sharing
last. Everything up to and including the atomics, and the loop parallelization, needs neither
lambdas nor inner classes.

## 11. Risks and open questions

- **The type-based escape analysis shares too much.** A whole class tree becomes `Arc` and
  locks when one object of it is handed to a thread (every `Node` of every list). Risk: speed,
  not correctness; the report measures it. Recommendation: accept until per-site sharing, and
  make per-site sharing the first thorough-mode deliverable, since the same
  points-to analysis also drives ownership.
- **A data lock held across a call that locks the same object hangs** where `RefCell` panics.
  The discipline is the same and the corpus build's `try_lock` timeout panics with a message
  naming the bug. Recommendation: keep the timeout in debug builds for good, and add a shape
  test that every `lock()` in generated code is a temporary of its statement.
- **Fused monitors and reentrancy.** Misclassifying a reentrant class as fused deadlocks it.
  The condition is checked over all callees through the hierarchy; when in doubt (a call whose
  targets include library code not yet analysed) the separate form is chosen, which is always
  correct. Recommendation: fused only when every callee is in the program or in translated
  library code whose regions are known; the separate form costs one extra lock per region.
- **`java.util.concurrent` uses something the lowering rejects.** Known: `AtomicIntegerFieldUpdater`
  (reflection), `ForkJoinPool`'s `@Contended` layout (ignored), `Thread.ofPlatform` in some
  factories, `SharedThreadContainer`, `JavaLangAccess`. Unknown constructs will appear when the
  classes are first reached; the survey (`--survey --main` on the corpus thread programs with
  the JDK as library) is run at the start of the atomics milestone to list them before the work.
  Recommendation: add stubs as synthetic classes only for `jdk.internal` plumbing, never for
  `java.util.concurrent` classes themselves.
- **Parallel-stream determinism depends on parallelism matching Java's.** The common pool's size
  is `availableProcessors() - 1` on both sides, but a `taskset`-restricted run or a cgroup quota
  changes both the same way (the runtime's `available_processors` already follows affinity).
  If a difference appears, the test pins both sides with the same `taskset`.
- **Thread-local output buffer versus process-wide.** `PrintStream` locks per call; a Java
  program whose threads print concurrently interleaves whole lines, and ours does the same with
  the buffer lock held per `println`. `print` followed by `println` from one thread can be split
  by another thread in Java too. No risk, but the `eprintln!` ordering trick (flush `out` before
  `err`) stays per call.
- **Spurious wakeups.** Java permits them in `wait`; so does `Condvar`. Translated JDK code
  loops as it must; a user program with `if (cond) wait();` instead of `while` is as wrong on
  Java as on ours, and may fail the same way or not. Acceptable: the behaviour is legal Java.
- **Identity hash of shared objects** uses the `Arc` payload's address: stable while the object
  lives, as `Rc`'s is; two live objects never share an address.
- **32-bit targets** lack 64-bit atomics; not targeted. If needed, `long`/`double` volatiles fall
  back to `Locked<i64>`, still correct.
- **Fairness and timing.** `Thread.sleep` and timed waits have platform precision; programs that
  print timing are nondeterministic on Java too and belong in the self-checking set.
- **Open: should `Thread` subclasses with no fields of their own avoid the data lock?** The
  `Thread` record is lock-free (atomics) and the synthetic fields are volatile or final except
  `name` and `daemon`; a `Thread` subclass whose own fields are all read after `join` still goes
  through `lock()`. Recommendation: accept; measure in `MonitorCounter`; apply the 2.3 split if
  it shows.
- **Open: lambdas' representation.** The examples use `java::runnable(move || ..)`, a boxed
  `Fn + Send + Sync` returning `Result`. The lambdas plan may choose closures per functional
  interface or an enum of the program's lambda classes; either works here as long as a lambda
  that reaches a thread is `Send` and the escape analysis sees its captures as roots.
  Recommendation: the lambdas plan records a lambda's captured variables and `this` in the AST
  so that `analysis/threads.rs` reads them directly.
- **Open: should the general form use `parking_lot`?** Faster uncontended locks and a
  reentrant mutex for free, but a dependency of every generated crate. Recommendation: std
  only; revisit with benchmark numbers.

## 12. Deviations

Decisions that change during the implementation are recorded here, milestone by milestone.

### The thread-aware runtime and process-wide statics

- **No `threads` cargo feature.** The runtime decides at run time: the output buffer and the
  descriptor table are thread-local until `start_thread` first runs, then process-wide
  (`out::share_output`), so a single-threaded program pays nothing and needs no build switch.
- **`java::Locked<T>` keeps `RefCell`'s API** (`borrow`, `borrow_mut`, `with_borrow`,
  `with_borrow_mut`, `Ref::map`) instead of the `lock()` of section 2: it is a `RefCell` behind a
  reentrant lock. The examples of 2.7 lock `this` twice in one statement (`this.lock().n =
  this.lock().n + 1`), which a plain `Mutex` deadlocks on; the reentrant lock allows it, and the
  generator emits the same text for `Rc<RefCell<C>>` and `Arc<java::Locked<C>>`, differing only
  in the type names. The guards are the runtime's own, since `MutexGuard::map` is unstable.
- **The watchdog** of 1.3 is a wait of more than `WATCHDOG` on a data lock in a build with debug
  assertions, not a `try_lock` loop of the release lock.
- **`java::ClassInit` is `start()`/`finish()`**, not `run(closure)`: the class's `init` function
  begins with `if !C_INIT.start() { return; }` and ends with `C_INIT.finish();`, the shape of the
  thread-local flag's guard, so the initializer body is generated once for both forms.
- **Every static of a threaded program is process-wide.** The main-only refinement of 2.5 (a
  static no thread's methods touch stays thread-local) waits for `thread_methods` of 4.3.

### Threads by subclassing `Thread`

- **`Thread` is smaller than 5.1**: no state, `ThreadLocal` or `Duration` sleep yet, and
  `ThreadGroup` only as far as handlers need it (interrupts and handlers have their own sections
  below); each lands with the milestone that needs it. `join` is
  the native `join0` on the runtime's record (a condvar on its alive flag), not `while
  (isAlive()) wait()`, because monitors come later. `start` passes the new thread the `Thread`
  object itself: `start0` gets a fifth argument, the call `t.run()`, which codegen puts in the
  closure (`move || { let t = &t; java::set_current_thread(t.clone()); t.run(); }`).
- **The main thread is `#3`** and prints as `Thread[#3,main,5,main]`, as on JDK 26 (the JVM's
  own threads take 1 and 2); `Thread-N` numbering starts at 0. The null-name messages are the
  JDK's (`'name' is null`, `name cannot be null`).
- **An exception that ends a worker** prints `Exception in thread "<name>" ...` and unwinds the
  worker with a private panic payload, which the spawn closure catches; the other threads go on
  and the exit code is unaffected, as in Java. Any other panic in a worker is a translation bug
  and ends the process with 101.
- **`main` holds a `java::MainThread`** (`let _main = java::main_thread();`) instead of calling
  `end_main`: its drop waits for the non-daemon threads and flushes, also when `main` returns
  early.
- **Shared arrays are chosen per program, not per site**: in a threaded program every array
  that is not a plain `Vec` is a `java::SharedArray<T>` (`Arc<java::Locked<Vec<T>>>`, the last
  row of 2.4), and the runtime's array functions take either form through the `Elements<T>`
  trait. The atomic and read-only forms of 2.4 need the per-site facts of the escape milestone.
- **The type-based closure walks superclasses except `Object`, and subtypes**, but not
  interfaces upward: going up through `Object` or an interface would make every class shared.
  Java enums are never shared: their values are plain Rust values, which every thread may hold.
- **Methods that hold no lock across a wait.** A method of a shared class that loops, calls, reads
  another object or an array, switches, or calls a blocking native (`sleep`, `join`, `start`) is
  in `long_methods` and is never a `&self`/`&mut self` method of the fields struct, which would
  hold the object's lock for the whole call; it takes the handle and locks per access. And in a
  threaded program two borrows in one statement count as conflicting even when neither is
  mutable (`codegen/objects.rs`, `conflicts`), with a blocking native counting as one, so
  `java::join_thread(self.base().tid, 0)` reads the id in its own block first: the lock is never
  held while the thread waits for another.
- **Not yet shared**: a `StringBuilder` or a box compared by identity (both `Rc`s) in a static or
  in a field of a shared class is rejected (`analysis/threads.rs`, `check`) until the escape
  milestone gives them shared forms. The closure does not follow the type arguments of a generic
  class yet; a program that needs it fails in rustc with `Rc<..> cannot be sent between threads
  safely` (4.4).
- **A closure type that threads may share** (a functional interface whose values are only
  lambdas, `docs/closures-and-nested-classes.md` 4.2, reached through a capture, a field or a
  static) is `Arc<dyn Fn(..) -> R + Send + Sync>` and its lambdas `Arc::new(move |..| ..)`; the
  closure's captures are of shared types by the same type-based walk, so the closure is `Send`
  and `Sync`. A closure type no thread reaches keeps its `Rc<dyn Fn>`, also in a threaded program.
- **A `volatile` static is an ordinary `java::Atomic`** (acquire and release) until the volatile
  milestone gives it `SeqCst`; a `volatile` field of a shared object is read and written under
  the object's lock.

### Interrupts

- **The status is the runtime's**, by thread id, not a field of the synthetic `Thread`: the
  record of a running thread holds it, and a set of ids holds it for threads that have no record
  (interrupted before `start`, or ended with the status set, which Java keeps in the `Thread`
  object). Moving it between the two happens under the registry's lock, so an interrupt racing a
  start or an end is not lost. `Thread.java`'s `interrupt`, `isInterrupted` and `interrupted` call
  the natives `interrupt0`, `isInterrupted0` and `clearInterrupt0`.
- **Waking**: `interrupt` sets the status under the record's park mutex and notifies its
  condition variable (which `sleep` and `park` wait on), then the parking bucket of the monitor
  the thread waits on in `wait` (the record holds its index), then the record of the thread it
  joins. Each waiter checks the status under the mutex it waits on, so no wakeup is lost. `park`
  returns on an interrupt and leaves the status set, as `LockSupport.park` does.
- **Java's order of checks**: `sleep` checks a negative time before the interrupt; `wait` checks
  ownership (`IllegalMonitorStateException`) before it, and takes the monitor again before it
  throws; a waiter both notified and interrupted returns normally with the status set; `join` of
  a thread that is not alive returns at once even when interrupted (Java's `join` waits only
  while the thread is alive). `sleep`'s exception says `sleep interrupted`; `wait`'s and `join`'s
  have no message, so `InterruptedException` is raised by its constructor without arguments
  where the runtime gives no message.
- **Only a program that interrupts pays**: `Facts::interrupts` holds when some method calls
  `interrupt0`, and only then do `sleep`, `join` and `wait` have `InterruptedException` among
  their failures (with the checked forms `java::try_sleep` and `java::try_join_thread`), and does
  lowering declare it as raised. `sleep`, `join` and `wait` register where an interrupt would wake
  them even before the program's first interrupt, since one may come while they wait.
- **Found on the way**: the `Link` of an enum whose variants are thread-shared pushed `Arc`s with
  the `Rc` form; it now uses `Chain::push_shared` for them (a lambda run by a thread that
  captures a `Thread`).

### Uncaught-exception handlers

- **The runtime calls the translation back.** `setUncaughtExceptionHandler` and
  `setDefaultUncaughtExceptionHandler` call the native `installUncaught0`, which hands the runtime
  a function (`java::set_uncaught_hook`) that turns what ends a thread into a `Throwable`, keeps it
  for `uncaught0()` and runs `Thread.dispatchUncaught()`, which is
  `currentThread().dispatchUncaughtException(uncaught0())` in `Threads.java`: the thread's handler,
  else its group, whose `uncaughtException` goes to the parent, else the default handler, else
  `printUncaught0`, the message the runtime printed before. Until a program installs a handler,
  `java::die` prints as before, and a program that never sets one has no hook at all.
- **What a handler sees**: an exception that the program throws, and a failure of the runtime
  (an index, a division, a cast) whose class lowering declares; in a program that installs a
  handler, every class of `RAISED` whose operation the program uses is declared, as if a `catch`
  could see it. A failure of the runtime without a declared class skips the handlers and ends
  the thread with the message.
- **A handler that throws** ends the thread with the JVM's `Exception: <class> thrown from the
  UncaughtExceptionHandler in thread "<name>"` (`DISPATCHING` marks the thread while its handler
  runs). `main`'s uncaught exception goes through the same chain and the program then exits
  with 1, as Java's does.
- **Thread groups only as far as handlers observe them**: a group has a name, a parent and
  `maxpri` 10, prints as `java.lang.ThreadGroup[name=main,maxpri=10]`, and a thread joins the
  group of the thread that creates it (`main`, whose parent is `system`); a thread that has
  ended has no group and no handler. `Thread.group` is null for the main group, so a program
  that never creates a group only gains a field that stays null.
- **Nested classes in the templates**: `Thread.UncaughtExceptionHandler` made the selection of
  template members declare nested types with their outer class, keep the interfaces a used
  class implements, the abstract methods of an interface the program uses, and the methods of
  a class whose constructor is used that implement a used interface method
  (`ThreadGroup.uncaughtException`).

### Volatile fields and the atomic natives

- **Field handles came first**, ahead of monitors: `Charset.defaultCharset()` keeps its charset
  in a `LazyConstant`, whose `get` reads its field with `Unsafe.getReferenceAcquire` at an
  `objectFieldOffset`, so every reader, writer and `getBytes()` without a charset needs them.
  `lower/handles.rs` makes a `static final` whose initializer is `objectFieldOffset(C.class, "f")`
  a handle with no field of its own, as 5.3 says, and lowers `get<Kind><Mode>` and
  `put<Kind><Mode>` through it to a plain read or write of `o.f`; reachability enters neither
  `objectFieldOffset` nor the accessors. The field is not made atomic yet: in a program without
  threads a plain access is exact, and in one with threads the access is under the object's lock,
  which orders it at least as acquire, release and volatile ask. `VarHandle`s and array handles
  come with `java.util.concurrent`.
- **The atomic updates of a field handle came next**, for `AtomicInteger`, `AtomicLong` and
  `Striped64` once classes extending `Number` translate (`ThreadLocalRandom`'s `<clinit>`, which
  `Locale`'s statics reach through `ConcurrentHashMap`, creates an `AtomicInteger`).
  `getAndAdd`, `getAndSet`, `compareAndSet`, `weakCompareAndSet` and `compareAndExchange` of an
  `int`, `long` or reference field, in every memory-order mode, become a call of a method that
  lowering adds to the field's class, one per field and form (`compareAndSetValue(expected,
  value)`), whose body reads, compares and writes the field. A method runs with its object
  borrowed, or in a program with threads with the object's lock held (`this.borrow_mut()` of an
  `Arc<java::Locked<C>>`), for the whole call, so no other access of the object comes between
  the read and the write: the update is atomic without atomic fields, and four threads
  incrementing one `AtomicInteger` count every increment (`corpus/whole-program/AtomicNumbers.java`).
  The weak forms never fail spuriously, which Java allows. Reachability does not enter
  `Unsafe`'s own updates, whose loops pass the offset on as a value. Atomic fields (2.2) remain
  the refinement for when the lock shows in a profile.
