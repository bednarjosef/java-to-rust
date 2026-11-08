//! The shape of the translation of programs that start threads (see docs/multi-threading.md): the
//! objects that threads share are `Arc`s, the statics are the process's, and a program without
//! threads is translated as before.

fn translate(java: &str) -> String {
    java_to_rust::translate(java).unwrap_or_else(|e| panic!("translation failed: {e}"))
}

#[track_caller]
fn assert_contains(rust: &str, expected: &[&str]) {
    for e in expected {
        assert!(rust.contains(e), "expected `{e}` in:\n{rust}");
    }
}

#[track_caller]
fn assert_lacks(rust: &str, unexpected: &[&str]) {
    for u in unexpected {
        assert!(!rust.contains(u), "did not expect `{u}` in:\n{rust}");
    }
}

/// A worker that sums a range into its own field, and a class that only the main thread uses.
const WORKER: &str = "class A {
    static int base = 3;
    static String label;
    static class Worker extends Thread {
        private final int to;
        long sum;
        Worker(int to) { this.to = to; }
        public void run() {
            long s = 0;
            for (int i = 0; i < to; i++) s += base + i;
            sum = s;
        }
    }
    static class Local {
        int n;
        void add(int k) { n += k; }
    }
    public static void main(String[] args) throws InterruptedException {
        label = \"sum\";
        Worker w = new Worker(10);
        w.start();
        w.join();
        Local l = new Local();
        l.add((int) w.sum);
        System.out.println(label + \" \" + l.n);
    }
}";

#[test]
fn a_thread_and_what_it_reaches_are_arcs() {
    let rust = translate(WORKER);
    assert_contains(
        &rust,
        &[
            // Threads share a worker, but no monitor site locks one (`join` is the runtime's),
            // so it has `java::SharedField`s and no lock word.
            "enum ThreadView {\n    Worker(Arc<AWorker>) = 11,",
            "fn AWorker__run(this: &Arc<AWorker>) {",
            "let w = Arc::new(AWorker__new(10));",
            "sum: java::SharedField<i64>,",
            "impl PartialEq for Thread {\n    fn eq(&self, other: &Thread) -> bool {\n        self.0 == other.0",
            "fn base(&self) -> &ThreadFields {",
        ],
    );
}

#[test]
fn a_class_no_thread_reaches_is_translated_as_before() {
    // `Local` never leaves `main`, so it is a plain value, as in a program without threads.
    let rust = translate(WORKER);
    assert_contains(&rust, &["let mut l = ALocal__new();", "fn add(&mut self, k: i32) {"]);
    assert_lacks(&rust, &["Arc<java::Monitored<ALocal>>"]);
}

#[test]
fn statics_that_a_started_thread_touches_are_process_wide() {
    let rust = translate(WORKER);
    // The worker reads `base`, so `A`'s statics are the process's; only main touches
    // `ClassLoader`'s, which stay thread-local.
    assert_contains(
        &rust,
        &[
            "static A_BASE: java::Atomic<i32> = java::Atomic::new(3);",
            "static A_LABEL: java::SharedField<Option<java::JString>> = java::SharedField::new(None);",
            "let _main = java::main_thread();",
            "static CLASSLOADER_SYSTEM: RefCell<Option<Arc<ClassLoader>>> = const { RefCell::new(None) };",
        ],
    );
}

#[test]
fn a_static_of_the_process_is_read_and_written_without_a_lock() {
    // `current` and `hits` are assigned again after initialization, so they are not
    // `java::Final`s; each access is still one atomic action with no lock, sequentially
    // consistent for the `volatile` ones, as Java's are.
    let rust = translate(
        "class A {
            static String current;
            static volatile Object stop;
            static int hits;
            static volatile int turns;
            public static void main(String[] args) throws InterruptedException {
                Thread t = new Thread(() -> {
                    while (stop == null) {
                        current = \"seen \" + turns;
                        hits++;
                        turns += 2;
                    }
                });
                t.start();
                current += \"!\";
                stop = new Object();
                t.join();
                System.out.println(current != null);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "static A_CURRENT: java::SharedField<Option<java::JString>> = java::SharedField::new(None);",
            "static A_STOP: java::SharedField<Option<Arc<PlainObject>>> = java::SharedField::new(None);",
            "static A_HITS: java::Atomic<i32> = java::Atomic::new(0);",
            "A_STOP.get_volatile().is_none()",
            "A_CURRENT.set(",
            "A_CURRENT.get()",
            "A_STOP.set_volatile(",
            "A_TURNS.get_volatile()",
            "A_TURNS.set_volatile(",
            "A_CURRENT.get().is_some()",
        ],
    );
    assert_lacks(&rust, &["java::Locked<Option", "A_CURRENT.with_borrow", "A_STOP.with_borrow"]);
}

#[test]
fn a_static_final_that_threads_read_is_read_without_a_lock() {
    // `TABLE` and `NAME` are assigned once, by `A`'s static initialization: `java::Final`s, which a
    // thread lends in place, with no test, since `main` prepares them first. `log` is assigned
    // again, so it is a `java::SharedField`, read and written by value.
    let rust = translate(
        "class A {
            static final int[] TABLE;
            static final String NAME = \"table\";
            static final String DESCRIBED = NAME + A.TABLE;
            static String log;
            static {
                TABLE = new int[] {1, 2, 3};
            }
            public static void main(String[] args) throws InterruptedException {
                Thread t = new Thread(() -> log = DESCRIBED + TABLE[1] + TABLE.length);
                t.start();
                t.join();
                System.out.println(log);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            // `DESCRIBED`'s initializer reads it before it is set, so it may be null.
            "static A_TABLE: java::Final<Option<java::SharedArray<i32>>> = java::Final::new(None);",
            "static A_DESCRIBED: java::Final<java::JString> = java::Final::new(java::JString::new());",
            "static A_LOG: java::SharedField<Option<java::JString>>",
            "java::len(java::non_null((*{ init_a(); A_TABLE.get_prepared() }).as_ref()))",
            "{ init_a(); A_DESCRIBED.get_prepared().clone() }",
            "A_TABLE.prepare();",
            "A_DESCRIBED.prepare();",
            "A_TABLE.set(Some(java::shared_from_vec(vec![1, 2, 3])));",
        ],
    );
    assert_lacks(&rust, &["A_TABLE.with_borrow", "A_DESCRIBED.with_borrow"]);
}

#[test]
fn a_static_final_array_that_is_only_read_is_frozen() {
    // `PRIMES` and `NAMES` are only indexed and measured: frozen, read with no atomic load.
    // `COUNTS` has its elements written, and `SHOWN` is used as a value: shared arrays.
    let rust = translate(
        "class A {
            static final int[] PRIMES = {2, 3, 5, 7};
            static final String[] NAMES = new String[] {\"a\", \"b\"};
            static final int[] COUNTS = new int[2];
            static final int[] SHOWN = {1};
            static String log;
            public static void main(String[] args) throws InterruptedException {
                Thread t = new Thread(() -> {
                    COUNTS[0]++;
                    log = NAMES[1] + PRIMES[3] + PRIMES.length + COUNTS[0] + java.util.Arrays.toString(SHOWN);
                });
                t.start();
                t.join();
                System.out.println(log);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "static A_PRIMES: java::Final<java::Frozen<i32, 4>> = java::Final::lazy(|| java::freeze_fixed(java::shared_from_vec(vec![2, 3, 5, 7])));",
            "static A_NAMES: java::Final<java::Frozen<",
            "static A_COUNTS: java::Final<java::SharedArray<i32>>",
            "static A_SHOWN: java::Final<java::SharedArray<i32>>",
        ],
    );
}

#[test]
fn an_array_field_only_its_constructor_makes_and_writes_is_frozen() {
    // `table` and `sized` are made and filled by the constructor only, then only indexed and
    // measured: frozen, of length 16 and of no constant length. `counts` has its elements written
    // by a method, and `given` leaves its object: shared arrays. `table` and `counts` always have
    // one length, which their indexes stay within: in bounds.
    let rust = translate(
        "class A {
            final int[] table = new int[16];
            final long[] sized;
            final int[] counts = new int[2];
            final int[] given = {1, 2};
            A(int n) {
                sized = new long[n];
                for (int i = 0; i < 16; i++) {
                    table[i] = i * i;
                }
                sized[0] = n;
            }
            void count() { counts[0]++; }
            int[] given() { return given; }
            int read(int h) { return table[h & 15] + (int) sized[h % sized.length] + counts[0]; }
            public static void main(String[] args) throws InterruptedException {
                A a = new A(3);
                int[] seen = new int[1];
                Thread t = new Thread(() -> { a.count(); seen[0] = a.read(5) + a.given()[1]; });
                t.start();
                t.join();
                System.out.println(seen[0]);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "table: java::Frozen<i32, 16>,",
            "sized: java::Frozen<i64>,",
            "table: java::freeze_fixed(java::shared_filled(16, 0))",
            "sized: java::freeze(java::shared_filled(n, 0i64))",
            "java::set_in_bounds(&this.table, i, i * i);",
            "java::get_in_bounds(&self.table, h & 15)",
            "java::get_in_bounds(&self.counts, 0)",
            "java::get(&self.sized, java::rem(h, java::len(&self.sized)))",
        ],
    );
    assert_lacks(&rust, &["counts: java::Frozen", "given: java::Frozen"]);
}

#[test]
fn final_array_fields_that_threads_share_bound_their_indexes() {
    // The constructor starts a thread before it writes `sections`, so `sections` is read from its
    // cell at each use; it is final, so the array a read finds is the same at every later read,
    // and the guard on its length proves the index.
    let rust = translate(
        "class A {
            private final int[] sections;
            private final Thread worker;
            A(int height) {
                this.worker = new Thread(this::report);
                this.worker.start();
                this.sections = new int[height];
            }
            void report() { System.out.println(this.worker == Thread.currentThread()); }
            synchronized int at(int y) {
                if (y >= 0 && y < this.sections.length) {
                    return this.sections[y];
                }
                return -1;
            }
            synchronized int below(int y) { return this.sections[y - 1]; }
            synchronized void put(int y) { this.sections[y] = y; }
            public static void main(String[] args) throws InterruptedException {
                A a = new A(6);
                a.worker.join();
                a.put(2);
                System.out.println(a.at(2) + a.below(6));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::get_in_bounds(java::non_null(this.sections.get().as_ref()), y)",
            "(java::non_null(this.sections.get().as_ref()), y - 1)",
        ],
    );
    assert_lacks(&rust, &["java::get_in_bounds(java::non_null(this.sections.get().as_ref()), y - 1)"]);
}

#[test]
fn the_started_thread_runs_the_thread_object_it_was_given() {
    let rust = translate(WORKER);
    // The handle is cloned into the new thread, which makes it its current thread and runs it;
    // the fields that the start reads are read before, without holding the object's lock.
    assert_contains(
        &rust,
        &[
            "{ let t = t.clone(); move || { java::set_current_thread(t.clone()); Thread__run(t); } }",
            "java::join_thread(*this.base().tid.fixed(), 0);",
        ],
    );
}

#[test]
fn a_single_threaded_program_is_unchanged() {
    let rust = translate(
        "class A {
            static int count;
            static class Node { int v; Node next; Node(int v) { this.v = v; } }
            public static void main(String[] args) {
                Node n = new Node(1);
                n.next = new Node(2);
                count = n.v + n.next.v;
                System.out.println(count);
            }
        }",
    );
    assert_contains(&rust, &["static A_COUNT: java::Global<Cell<i32>>", "Arc<ANode>"]);
    assert_lacks(&rust, &["java::Locked", "java::Atomic", "main_thread"]);
}

#[test]
fn a_thread_object_is_never_borrowed_across_a_join() {
    // `join` waits for the other thread, which may need the object's lock: it is no method of the
    // fields struct, which would hold the lock for the call.
    let rust = translate(WORKER);
    assert_contains(&rust, &["Thread__join(Thread::Worker(w.clone()));"]);
    assert_lacks(&rust, &[".borrow().base.join()"]);
}

#[test]
fn arrays_that_another_thread_reaches_are_shared_arrays() {
    let rust = translate(
        "class A {
            static class Filler extends Thread {
                private final long[] out;
                Filler(long[] out) { this.out = out; }
                public void run() { for (int i = 0; i < out.length; i++) out[i] = i; }
            }
            public static void main(String[] args) throws InterruptedException {
                long[] values = new long[4];
                Filler f = new Filler(values);
                f.start();
                f.join();
                System.out.println(values[3]);
            }
        }",
    );
    assert_contains(
        &rust,
        &["out: java::SharedField<java::SharedArray<i64>>,", "java::shared_filled(4, 0i64)"],
    );
}

#[test]
fn arrays_that_no_other_thread_reaches_stay_local() {
    // Only `long[]`s cross to the worker; the main thread's `int[]` and `char[]` stay `Rc`s.
    let rust = translate(
        "class A {
            static class Filler extends Thread {
                private final long[] out;
                Filler(long[] out) { this.out = out; }
                public void run() { for (int i = 0; i < out.length; i++) out[i] = i; }
            }
            public static void main(String[] args) throws InterruptedException {
                long[] values = new long[4];
                Filler f = new Filler(values);
                f.start();
                f.join();
                int[] counts = new int[3];
                char[] word = \"abc\".toCharArray();
                for (char c : word) counts[c - 'a'] += values[3];
                int[] alias = counts;
                System.out.println(counts[2] + \" \" + (alias == counts) + \" \" + word.length);
            }
        }",
    );
    assert_contains(
        &rust,
        &["out: java::SharedField<java::SharedArray<i64>>,", "java::filled(3, 0)", "Arc::ptr_eq("],
    );
    assert_lacks(&rust, &["java::SharedArray<i32>", "java::SharedArray<u16>", "shared_filled(3"]);
}

#[test]
fn a_string_builder_of_a_program_with_threads_is_shared() {
    // Every builder of a program with threads is a `java::SharedStringBuilder`, its units behind a
    // lock: in a static, in `Object`'s variant, and as the monitor of a `synchronized` block.
    let rust = translate(
        "class A {
            static StringBuilder log = new StringBuilder();
            static Object held;
            public static void main(String[] args) throws InterruptedException {
                held = new StringBuilder(\"a\");
                Thread t = new Thread(() -> log.append(1));
                t.start();
                t.join();
                synchronized (log) {
                    System.out.println(log);
                }
                System.out.println(held);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "static A_LOG: LazyLock<java::SharedField<java::SharedStringBuilder>>",
            "StringBuilder(java::SharedStringBuilder) = 22,",
            "unsafe { self.0.monitor::<java::SharedStringBuilder>() }",
            "let _monitor = java::enter(&A_LOG",
        ],
    );
    assert_lacks(&rust, &["java::StringBuilder"]);
}

#[test]
fn a_string_builder_of_a_program_without_threads_stays_local() {
    let rust = translate(
        "class A {
            public static void main(String[] args) {
                StringBuilder sb = new StringBuilder(\"a\");
                synchronized (sb) {
                    sb.append(1);
                }
                System.out.println(sb);
            }
        }",
    );
    assert_contains(&rust, &["java::StringBuilder::from_text(java::wtf8(\"a\"))"]);
    assert_lacks(&rust, &["SharedStringBuilder", "java::enter"]);
}

#[test]
fn a_string_builder_that_no_other_thread_reaches_stays_local() {
    // The worker and main build their own texts; only strings cross threads.
    let rust = translate(
        "class A {
            static String result;
            public static void main(String[] args) throws InterruptedException {
                Thread t = new Thread(() -> {
                    StringBuilder sb = new StringBuilder();
                    for (int i = 0; i < 3; i++) sb.append(i);
                    result = sb.toString();
                });
                t.start();
                t.join();
                StringBuilder mine = new StringBuilder(\"main \");
                synchronized (mine) {
                    mine.append(result);
                }
                System.out.println(mine);
            }
        }",
    );
    assert_contains(
        &rust,
        &["java::StringBuilder::new()", "java::StringBuilder::from_text(java::wtf8(\"main \"))"],
    );
    assert_lacks(&rust, &["SharedStringBuilder", "java::enter("]);
}

#[test]
fn a_closure_that_threads_share_is_an_arc_that_any_thread_may_call() {
    // `Step` reaches the thread through the lambda's capture; `Local` stays on the main thread.
    let rust = translate(
        "class A {
            interface Step { int apply(int x); }
            interface Local { int apply(int x); }
            public static void main(String[] args) throws InterruptedException {
                int k = 3;
                Step s = x -> x + k;
                int[] out = new int[1];
                Thread t = new Thread(() -> out[0] = s.apply(4));
                t.start();
                t.join();
                Local l = x -> x * 2;
                Local m = l;
                System.out.println(m.apply(out[0]));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "type AStep = std::sync::Arc<dyn Fn(i32) -> i32 + Send + Sync>;",
            "let s = std::sync::Arc::new(move |x: i32| -> i32 { x.wrapping_add(k) }) as AStep;",
            "type ALocal = Rc<dyn Fn(i32) -> i32>;",
        ],
    );
}

/// A counter that threads share, with synchronized methods, a lock in a field, and a static
/// synchronized method.
const MONITORS: &str = "class A {
    static int hits;
    static synchronized void hit() { hits++; }
    static class Counter {
        private final Object lock = new Object();
        private int n;
        private int m;
        synchronized void add() { n++; }
        void addOther() { synchronized (lock) { m++; } }
        synchronized void await(int k) throws InterruptedException { while (n < k) wait(); notifyAll(); }
    }
    static class Adder extends Thread {
        private final Counter c;
        Adder(Counter c) { this.c = c; }
        public void run() { c.add(); c.addOther(); hit(); }
    }
    public static void main(String[] args) throws InterruptedException {
        Counter c = new Counter();
        Adder a = new Adder(c);
        a.start();
        c.await(1);
        a.join();
        System.out.println(c.n + c.m + hits);
    }
}";

#[test]
fn a_synchronized_method_of_a_shared_object_holds_its_monitor() {
    let rust = translate(MONITORS);
    assert_contains(
        &rust,
        &[
            "fn ACounter__add(this: &Arc<java::Monitored<ACounter>>) {\n    let _monitor = java::enter(this);",
            "let _monitor = java::enter(&(*this.lock.fixed()));",
            "static A__MONITOR: java::Locked<()> = java::Locked::new(());",
            "fn hit() {\n    let _monitor = java::enter_class(&A__MONITOR);",
            "java::monitor_wait(this, 0);",
            "java::monitor_notify(this, true);",
            // A lock object has a lock of its own.
            "lock: java::SharedField<Arc<java::Monitored<PlainObject>>>,",
        ],
    );
}

#[test]
fn synchronized_without_threads_or_wait_is_the_null_check_it_was() {
    let rust = translate(
        "class A {
            static class Counter {
                private int n;
                synchronized void add() { n++; }
            }
            public static void main(String[] args) {
                Counter c = new Counter();
                synchronized (c) { c.add(); }
                System.out.println(c.n);
            }
        }",
    );
    assert_lacks(&rust, &["java::enter", "_monitor"]);
    assert_contains(&rust, &["fn add(&self) {"]);
}

#[test]
fn a_monitor_of_one_thread_is_kept_where_the_program_waits() {
    let rust = translate(
        "class A {
            public static void main(String[] args) throws InterruptedException {
                Object o = new Object();
                synchronized (o) {
                    o.wait(5);
                    System.out.println(Thread.holdsLock(o));
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &["let _monitor = java::enter(&o);", "java::monitor_wait(&o, 5);", "java::holds_lock(&o)"],
    );
}

#[test]
fn a_string_that_threads_lock_is_rejected() {
    let error = java_to_rust::translate(
        "class A {
            static String name = \"a\";
            static int n;
            public static void main(String[] args) throws InterruptedException {
                Thread t = new Thread(() -> { synchronized (name) { n++; } });
                t.start();
                t.join();
                System.out.println(n);
            }
        }",
    )
    .expect_err("a string has no monitor in the translation")
    .to_string();
    assert!(error.ends_with("`synchronized` on a string not supported yet"), "{error}");
}

/// A worker that sleeps, which main interrupts when `interrupt` is set.
fn sleeper(interrupt: bool) -> String {
    let stop = if interrupt { "w.interrupt();" } else { "" };
    format!(
        "class A {{
    static class Worker extends Thread {{
        String end = \"slept\";
        public void run() {{
            try {{ Thread.sleep(1000); }} catch (InterruptedException e) {{ end = e.getMessage(); }}
        }}
    }}
    public static void main(String[] args) throws InterruptedException {{
        Worker w = new Worker();
        w.start();
        {stop}
        try {{ w.join(); }} catch (InterruptedException e) {{ System.out.println(e); }}
        System.out.println(w.end);
    }}
}}"
    )
}

#[test]
fn sleep_and_join_throw_only_in_a_program_that_interrupts() {
    let quiet = translate(&sleeper(false));
    assert_contains(&quiet, &["java::sleep(1000);", "java::join_thread("]);
    assert_lacks(&quiet, &["try_sleep", "try_join_thread"]);
    let interrupted = translate(&sleeper(true));
    assert_contains(
        &interrupted,
        &[
            "java::try_sleep(1000)",
            "java::try_join_thread(",
            "java::interrupt_thread(",
            "if e.message.is_none() =>",
        ],
    );
}

#[test]
fn a_string_field_of_a_shared_object_is_hashed_where_it_is() {
    let rust = translate(
        "class A {
    static class Id {
        final String name;
        String alias;
        Id(String name) { this.name = name; }
        public int hashCode() { return 31 * name.hashCode() + alias.hashCode(); }
        public boolean equals(Object o) { return o instanceof Id id && name.equals(id.name); }
    }
    public static void main(String[] args) throws InterruptedException {
        Id id = new Id(args.length > 0 ? args[0] : \"stone\");
        Thread t = new Thread(() -> { id.alias = \"rock\"; System.out.println(id.hashCode()); });
        t.start();
        t.join();
        System.out.println(id.equals(new Id(\"stone\")));
    }
}",
    );
    // A fixed field is borrowed where it is, not copied; a field that may change is read by value,
    // and both are hashed as the string itself, which keeps its hash, not as its text.
    assert_contains(&rust, &["java::string_hash(&(*self.name.fixed()))", "self.alias.get().as_ref()"]);
    assert_lacks(&rust, &["(*self.name.fixed()).clone()", "self.alias.get().as_deref()"]);
}

#[test]
fn a_static_initializer_that_nothing_observes_runs_first_in_main() {
    // `Table`'s initialization only makes objects whose constructor assigns a field: `main` runs it
    // before anything else, and its static is then read with no test. `Loud`'s prints, so it runs
    // at its first use, and its static keeps the test.
    let rust = translate(
        "class A {
            static final class P {
                final int x;
                P(int x) { this.x = x; }
            }
            static final class Table {
                static final P[] POINTS = {new P(1), new P(2)};
            }
            static final class Loud {
                static final P[] POINTS = {new P(3)};
                static { System.out.println(\"loud\"); }
            }
            static int sum;
            public static void main(String[] args) throws InterruptedException {
                Thread t = new Thread(() -> sum = Table.POINTS[1].x + Loud.POINTS[0].x);
                t.start();
                t.join();
                System.out.println(sum);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn main() {\n    init_a_table();\n    A_TABLE_POINTS.check_settled();\n    let _main = java::main_thread();",
            "A_TABLE_POINTS.get_settled()",
            "A_LOUD_POINTS.get()",
        ],
    );
    assert_lacks(&rust, &["init_a_loud();\n    A_LOUD"]);
}

#[test]
fn a_constructor_that_starts_a_thread_without_letting_this_leave_keeps_its_fields_fixed() {
    let rust = translate(
        "class A {
    static final class Pool {
        final int[] slots;
        final Thread worker;
        Pool(int n) {
            slots = new int[n];
            worker = new Thread(() -> System.out.println(\"worker\"));
            worker.start();
        }
        int first() { return slots[0]; }
    }
    static final class Reporter {
        final int[] counts;
        final Thread thread;
        Reporter(int n) {
            thread = new Thread(this::report);
            counts = new int[n];
            thread.start();
        }
        void report() { System.out.println(counts.length); }
        int first() { return counts[0]; }
    }
    public static void main(String[] args) throws InterruptedException {
        Pool p = new Pool(2);
        p.worker.join();
        Reporter r = new Reporter(3);
        r.thread.join();
        System.out.println(p.first() + r.first());
    }
}",
    );
    assert_contains(
        &rust,
        &[
            // `Pool`'s constructor starts a thread, but no other code holds the pool until it
            // returns: its array is frozen and read without an atomic load.
            "slots: java::Frozen<i32>,",
            // `Reporter`'s hands `this` to its thread before it writes `counts`.
            "counts: java::SharedField<Option<java::SharedArray<i32>>>,",
            "this.counts.get()",
        ],
    );
    assert_lacks(&rust, &["slots: java::SharedField"]);
}

#[test]
fn a_fixed_number_boolean_or_enum_field_of_a_shared_object_is_read_with_a_plain_load() {
    let rust = translate(
        "class A {
    enum Shape { SQUARE, CIRCLE; boolean round() { return this == CIRCLE; } }
    static final class Spec {
        final int width;
        final long seed;
        final double scale;
        final boolean mirrored;
        final char mark;
        final Shape shape;
        final int area;
        int uses;
        Spec(int width, long seed, double scale, boolean mirrored, Shape shape) {
            this.width = width;
            this.seed = seed;
            this.scale = scale;
            this.mirrored = mirrored;
            this.mark = (char) ('a' + width);
            this.shape = shape;
            this.area = this.width * this.width;
        }
        long step(long x) {
            long y = x * seed + width + mark + area;
            if (mirrored) y = -y;
            return shape == Shape.CIRCLE && shape.round() ? y + (long) scale : y;
        }
    }
    static final class Leaky {
        final Thread pinger;
        final int value;
        Leaky(int value) {
            pinger = new Thread(this::ping);
            pinger.start();
            this.value = value;
        }
        void ping() { System.out.println(\"ping\"); }
    }
    static volatile Spec published;
    public static void main(String[] args) throws InterruptedException {
        Thread t = new Thread(() -> {
            Spec s;
            while ((s = published) == null) Thread.onSpinWait();
            long total = 0;
            for (int i = 0; i < 1000; i++) total += s.step(i);
            s.uses++;
            System.out.println(total);
        });
        t.start();
        published = new Spec(3, 7L, 1.5, true, Shape.CIRCLE);
        t.join();
        Leaky l = new Leaky(4);
        l.pinger.join();
        System.out.println(l.value);
    }
}",
    );
    assert_contains(
        &rust,
        &[
            // The fields keep their atomic cells, which the constructor fills; elsewhere they are
            // read where they are, a plain load.
            "width: java::SharedField<i32>,",
            "*this.seed.fixed()",
            "*this.width.fixed()",
            "*this.mirrored.fixed()",
            "*this.mark.fixed()",
            "*this.area.fixed()",
            "*this.scale.fixed()",
            "*this.shape.fixed()",
            // A field that is not fixed is still an atomic load, and so is a fixed field of an
            // object whose constructor hands `this` to a thread before writing it.
            "uses.get()",
            "value.get()",
        ],
    );
    // An enum's read is parenthesized where a method is called on it.
    assert_lacks(
        &rust,
        &["this.seed.get()", "this.mirrored.get()", "this.shape.get()", "value.fixed()", ".fixed().round()"],
    );
}

#[test]
fn fields_written_before_this_leaves_the_constructor_stay_fixed() {
    let rust = translate(
        "class A {
    static Object keep;
    static void aside(Runnable r) {
        Thread t = new Thread(r);
        t.start();
        try { t.join(); } catch (InterruptedException e) { }
    }
    static final class Grid {
        final int[] cells = new int[4];
        final StringBuilder log;
        final StringBuilder after;
        final Inner inner;
        Grid() {
            log = new StringBuilder(\"g\");
            aside(this::fill);
            after = new StringBuilder(\"a\");
            inner = new Inner();
        }
        void fill() { cells[0] = log.length(); }
        final class Inner {
            int reads;
            Inner() { aside(this::count); }
            void count() { reads++; }
            int read() { count(); return cells[0] + after.length() + reads; }
        }
    }
    static class Base {
        Base() { keep = this; }
    }
    static final class Sub extends Base {
        final StringBuilder name;
        Sub() { super(); name = new StringBuilder(\"s\"); aside(this::len); }
        void len() { System.out.println(name.length()); }
    }
    static final class Pair {
        final StringBuilder a;
        StringBuilder c;
        Pair() { this(new StringBuilder(\"p\")); c = new StringBuilder(\"c\"); }
        Pair(StringBuilder a) { this.a = a; aside(this::len); }
        void len() { System.out.println(a.length() + (c == null ? 0 : c.length())); }
    }
    public static void main(String[] args) {
        Grid g = new Grid();
        System.out.println(g.inner.read());
        new Sub();
        new Pair();
    }
}",
    );
    assert_contains(
        &rust,
        &[
            // Written by an initializer and before the statement that hands `this` to a thread,
            // and the enclosing instance, which the inner class's constructor writes first.
            ".cells.fixed()",
            ".log.fixed()",
            "(*this.outer.fixed())",
            "self.a.fixed()",
            // Written after `this` left: in a later statement, after a superclass's constructor
            // that stores it, and after `this(..)`.
            ".after.get()",
            "self.name.get()",
            "self.c.get()",
        ],
    );
    assert_lacks(&rust, &[".cells.get()", ".log.get()", ".outer.get()", "self.a.get()"]);
}
