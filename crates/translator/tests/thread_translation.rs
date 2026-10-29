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
            "enum ThreadView<'a> {\n    Worker(&'a Arc<java::Monitored<AWorker>>) = 11,",
            "fn run(this: &Arc<java::Monitored<AWorker>>) {",
            "let w = Arc::new(java::Monitored::new(AWorker::new(10)));",
            "impl PartialEq for Thread {\n    fn eq(&self, other: &Thread) -> bool {\n        self.0 == other.0",
            "fn base(&self) -> &ThreadFields {",
        ],
    );
}

#[test]
fn a_class_no_thread_reaches_is_translated_as_before() {
    // `Local` never leaves `main`, so it is a plain value, as in a program without threads.
    let rust = translate(WORKER);
    assert_contains(&rust, &["let mut l = ALocal::new();", "fn add(&mut self, k: i32) {"]);
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
            "static A_LABEL: java::Locked<Option<java::JString>> = java::Locked::new(None);",
            "let _main = java::main_thread();",
            "static CLASSLOADER_SYSTEM: RefCell<Option<Arc<ClassLoader>>> = const { RefCell::new(None) };",
        ],
    );
}

#[test]
fn a_static_final_that_threads_read_is_read_without_a_lock() {
    // `TABLE` and `NAME` are assigned once, by `A`'s static initialization: `java::Final`s, which a
    // thread lends in place. `log` is assigned again, so it keeps its lock.
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
            "static A_LOG: java::Locked<Option<java::JString>>",
            "java::len(java::non_null((*{ init_a(); A_TABLE.get() }).as_ref()))",
            "{ init_a(); A_DESCRIBED.get().clone() }",
            "A_TABLE.set(Some(java::shared_from_vec(vec![1, 2, 3])));",
        ],
    );
    assert_lacks(&rust, &["A_TABLE.with_borrow", "A_DESCRIBED.with_borrow"]);
}

#[test]
fn the_started_thread_runs_the_thread_object_it_was_given() {
    let rust = translate(WORKER);
    // The handle is cloned into the new thread, which makes it its current thread and runs it;
    // the fields that the start reads are read before, without holding the object's lock.
    assert_contains(
        &rust,
        &[
            "{ let t = t.clone(); move || { let t = &t; java::set_current_thread(t.clone()); t.run(); } }",
            "java::join_thread(self.base().tid.get(), 0);",
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
    assert_contains(&rust, &["Thread::Worker(w.clone()).join();"]);
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
            "static A_LOG: LazyLock<java::Locked<java::SharedStringBuilder>>",
            "StringBuilder(&'a java::SharedStringBuilder) = 22,",
            "22 => { let o: &java::SharedStringBuilder = unsafe { self.0.as_pointer() }; java::HasMonitor::monitor(o) }",
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
            "fn add(this: &Arc<java::Monitored<ACounter>>) {\n        let _monitor = java::enter(this);",
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
