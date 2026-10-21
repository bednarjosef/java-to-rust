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
fn a_string_builder_as_an_object_in_a_program_with_threads_is_rejected() {
    // `Object`'s variant would hold the builder's `Rc`, and an `Object` may cross threads.
    let error = java_to_rust::translate(
        "class A {
            static Object held;
            public static void main(String[] args) throws InterruptedException {
                held = new StringBuilder(\"a\");
                Thread t = new Thread(() -> System.out.println(\"x\"));
                t.start();
                t.join();
                System.out.println(held);
            }
        }",
    )
    .expect_err("a StringBuilder as an Object with threads is not supported")
    .to_string();
    assert!(
        error.ends_with("a `StringBuilder` as an `Object` in a program with threads not supported yet"),
        "{error}"
    );
}
