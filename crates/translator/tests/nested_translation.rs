//! The shape of the generated Rust for inner, local and anonymous classes: each is a struct
//! whose synthetic fields hold its enclosing instance (`outer`) and the locals it captures, set
//! by its constructors from parameters that each `new` passes. (Whether the output matches Java
//! is checked by the corpus test.)

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

#[test]
fn a_local_class_holds_its_captures_as_fields() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            int scale = a.length + 2;
            String unit = \"kg\";
            class Labeler {
                int calls;
                String label(int n) { calls++; return n * scale + unit; }
            }
            Labeler l = new Labeler();
            System.out.println(l.label(3) + l.calls);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "struct ALabeler {\n    scale: i32,\n    unit: String,\n    calls: i32,\n}",
            "fn new(scale: i32, unit: &str) -> ALabeler {",
            "ALabeler::new(scale, &unit)",
            "self.scale",
        ],
    );
}

#[test]
fn a_captured_array_is_shared() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            int[] seen = new int[2];
            class Counter { void see(int i) { seen[i % 2]++; } }
            Counter c = new Counter();
            c.see(1);
            c.see(3);
            System.out.println(seen[1]);
        } }",
    );
    assert_contains(
        &rust,
        &["seen: java::Array<i32>,", "ACounter::new(seen.clone())", "update(&self.seen, i % 2"],
    );
}

#[test]
fn a_method_whose_this_is_captured_keeps_the_handle() {
    let rust = translate(
        "class A {
            int factor = 3;
            String scaled(int v) {
                class Scaler { int apply() { return v * factor; } }
                Scaler s = new Scaler();
                factor++;
                return \"\" + s.apply();
            }
            public static void main(String[] a) {
                A o = new A();
                System.out.println(o.scaled(7));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "outer: Rc<RefCell<A>>,",
            "fn scaled(this: &Rc<RefCell<A>>, v: i32) -> String {",
            "AScaler::new(this.clone(), v)",
            "self.outer.borrow().factor",
        ],
    );
    assert_lacks(&rust, &["fn scaled(&self"]);
}
