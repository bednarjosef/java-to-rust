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
            "enum Thread {\n    Worker(Arc<java::Locked<AWorker>>),",
            "fn run(this: &Arc<java::Locked<AWorker>>) {",
            "let w = Arc::new(java::Locked::new(AWorker::new(10)));",
            "(Thread::Worker(a), Thread::Worker(b)) => Arc::ptr_eq(a, b),",
            "fn base(&self) -> java::Ref<'_, ThreadFields> {",
        ],
    );
}

#[test]
fn a_class_no_thread_reaches_is_translated_as_before() {
    // `Local` never leaves `main`, so it is a plain value, as in a program without threads.
    let rust = translate(WORKER);
    assert_contains(&rust, &["let mut l = ALocal::new();", "fn add(&mut self, k: i32) {"]);
    assert_lacks(&rust, &["Arc<java::Locked<ALocal>>"]);
}

#[test]
fn statics_are_process_wide_in_a_threaded_program() {
    let rust = translate(WORKER);
    assert_contains(
        &rust,
        &[
            "static A_BASE: java::Atomic<i32> = java::Atomic::new(3);",
            "static A_LABEL: java::Locked<Option<String>> = java::Locked::new(None);",
            "let _main = java::main_thread();",
        ],
    );
    assert_lacks(&rust, &["thread_local!"]);
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
            "java::join_thread({ let value = self.base().tid; value }, 0);",
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
    assert_contains(&rust, &["thread_local! {", "Rc<RefCell<ANode>>"]);
    assert_lacks(&rust, &["Arc", "java::Locked", "java::Atomic", "main_thread"]);
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
fn arrays_of_a_threaded_program_are_shared_arrays() {
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
    assert_contains(&rust, &["out: java::SharedArray<i64>,", "java::shared_filled(4, 0i64)"]);
}

#[test]
fn a_string_builder_that_threads_share_is_rejected() {
    // A `StringBuilder` is an `Rc`, which no other thread may hold.
    let error = java_to_rust::translate(
        "class A {
            static StringBuilder log = new StringBuilder();
            public static void main(String[] args) throws InterruptedException {
                Thread t = new Thread(() -> log.append(1));
                t.start();
                t.join();
                System.out.println(log);
            }
        }",
    )
    .expect_err("a shared StringBuilder is not supported")
    .to_string();
    assert!(error.ends_with("a `StringBuilder` that threads may share not supported yet"), "{error}");
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
                System.out.println(l.apply(out[0]));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "type AStep = Arc<dyn Fn(i32) -> i32 + Send + Sync>;",
            "let s = Arc::new(move |x: i32| -> i32 { x.wrapping_add(k) }) as AStep;",
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
            "fn add(this: &Arc<java::Locked<ACounter>>) {\n        let _monitor = java::enter(this);",
            "let _monitor = java::enter(&{ this.borrow().lock.clone() });",
            "static A__MONITOR: java::Locked<()> = java::Locked::new(());",
            "fn hit() {\n    let _monitor = java::enter_class(&A__MONITOR);",
            "java::monitor_wait(this, 0);",
            "java::monitor_notify(this, true);",
            // A lock object has a lock of its own.
            "lock: Arc<java::Locked<PlainObject>>,",
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
    assert_contains(&rust, &["fn add(&mut self) {"]);
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
