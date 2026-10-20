//! The shape of the generated Rust: where the analyses prove it safe, the translation must read
//! like hand-written Rust, and where they cannot, it must keep Java's semantics. (Whether the
//! code behaves like Java is checked by the corpus test.)

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
fn counters_in_counting_loops_use_plain_arithmetic() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            int count = 0;
            for (int i = 0; i < 1000; i++) { if (i % 7 == 0) count++; }
            System.out.println(count);
        } }",
    );
    assert_contains(&rust, &["let mut count = 0;", "for i in 0..1000 {", "i % 7 == 0", "count += 1;"]);
    assert_lacks(&rust, &["wrapping", "java::"]);
}

#[test]
fn trial_division_is_plain() {
    let rust = translate(
        "class A {
            static boolean isPrime(int n) {
                if (n < 2) return false;
                for (int d = 2; d * d <= n; d++) { if (n % d == 0) return false; }
                return true;
            }
            public static void main(String[] a) {
                int primes = 0;
                for (int i = 0; i < 100000; i++) { if (isPrime(i)) primes++; }
                System.out.println(primes);
            }
        }",
    );
    assert_contains(&rust, &["while d * d <= n {", "if n % d == 0 {", "d += 1;", "primes += 1;"]);
    assert_lacks(&rust, &["wrapping", "java::rem"]);
}

#[test]
fn modular_arithmetic_stays_in_range() {
    let rust = translate(
        "class A {
            static long modPow(long base, long exp, long mod) {
                long result = 1;
                base %= mod;
                while (exp > 0) {
                    if (exp % 2 == 1) result = result * base % mod;
                    base = base * base % mod;
                    exp /= 2;
                }
                return result;
            }
            public static void main(String[] a) { System.out.println(modPow(3, 200, 1000000007L)); }
        }",
    );
    assert_contains(&rust, &["result = result * base % mod_;", "base = base * base % mod_;", "exp /= 2;"]);
    assert_lacks(&rust, &["wrapping", "java::"]);
}

#[test]
fn possible_overflow_keeps_wrapping() {
    let rust = translate(
        "class A {
            static long fib(int n) { if (n < 2) return n; return fib(n - 1) + fib(n - 2); }
            static int next(int x) { return x + 1; }
            public static void main(String[] a) {
                System.out.println(fib(40) + next(5));
            }
        }",
    );
    // `n - 1` cannot overflow after `n < 2` returned, but the sum of two results can.
    assert_contains(&rust, &["fib(n - 1).wrapping_add(fib(n - 2))", "x + 1"]);
}

#[test]
fn unknown_divisors_keep_java_division() {
    let rust = translate(
        "class A {
            static int divide(int a, int b) { return a / b; }
            public static void main(String[] a) { System.out.println(divide(7, 0)); }
        }",
    );
    assert_contains(&rust, &["java::div(a, b)"]);
}

#[test]
fn string_parameters_are_borrowed() {
    let rust = translate(
        "class A {
            static String greet(String name) { return \"Hi \" + name + \" (\" + name.length() + \")\"; }
            public static void main(String[] a) {
                String n = \"Ada\";
                System.out.println(greet(n));
                System.out.println(greet(\"Bob\"));
            }
        }",
    );
    assert_contains(
        &rust,
        &["fn greet(name: &str) -> String", "name.len() as i32", "greet(&n)", "greet(\"Bob\")"],
    );
    assert_lacks(&rust, &["clone", "string_length"]);
}

#[test]
fn non_ascii_strings_count_utf16_units() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            String s = \"h\\u00e9llo\";
            System.out.println(s.length());
        } }",
    );
    assert_contains(&rust, &["java::string_length(&s)"]);
}

#[test]
fn string_building_appends_in_place() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            String s = \"\";
            for (int i = 0; i < 10; i++) { s += i; s += \",\"; }
            System.out.println(s);
        } }",
    );
    assert_contains(&rust, &["let mut s = String::new();", "s += &i.to_string();", "s += \",\";"]);
}

#[test]
fn field_reads_are_braced_only_beside_a_mutable_borrow() {
    let rust = translate(
        "class P { int x; int y = x + 1; void set(int v) { x = v; } }
        class A {
            static void share(P q) {}
            public static void main(String[] a) {
                P p = new P();
                System.out.println(p.x);
            }
        }",
    );
    assert_contains(&rust, &["println!(\"{}\", p.borrow().x);", "this.borrow_mut().y = { this.borrow().x }"]);
    assert_lacks(&rust, &["{ p.borrow()"]);
}

#[test]
fn field_read_arguments_beside_a_call_bind_in_a_let() {
    let rust = translate(
        "class P { int x; void set(int v) { x = v; } }
        class A {
            static int f(int v) { return v + 1; }
            static void share(P q) {}
            public static void main(String[] a) { P p = new P(); System.out.println(f(p.x)); }
        }",
    );
    assert_contains(&rust, &["f({ let value = p.borrow().x; value })"]);
    assert_lacks(&rust, &["f({ p.borrow()"]);
}

#[test]
fn objects_print_through_their_to_string() {
    let rust = translate(
        "class P { int x; public String toString() { return \"P\" + x; } }
        class A { public static void main(String[] a) {
            P p = new P();
            System.out.println(p);
            String s = \"p=\";
            s += p;
        } }",
    );
    assert_contains(&rust, &["println!(\"{}\", P::to_string(&p));", "s += &P::to_string(&p);"]);
    assert_lacks(&rust, &["P::to_string(&p).to_string()"]);
}

#[test]
fn printing_an_object_without_to_string_is_rejected() {
    let err = java_to_rust::translate(
        "class P { int x; }
        class A { public static void main(String[] a) { System.out.println(new P()); } }",
    )
    .expect_err("printing an object without toString must be rejected");
    assert!(err.to_string().contains("printing an object without toString not supported yet"), "{err}");
}

#[test]
fn immutable_objects_are_shared_without_a_refcell() {
    let rust = translate(
        "class P { int x; int y = x + 1; P(int x) { this.x = x; } int sum() { return x + y; } }
        class A { public static void main(String[] a) { P p = new P(3); System.out.println(p.sum() + p.x); } }",
    );
    assert_contains(
        &rust,
        &[
            "struct P {",
            "impl P {",
            "fn field_inits(this: &mut P) {",
            "this.y = this.x.wrapping_add(1);",
            "fn new(x: i32) -> Rc<P> {",
            "let mut obj = P { x: 0, y: 0 };",
            "let this = &mut obj;",
            "P::field_inits(this);",
            "this.x = value;",
            "Rc::new(obj)",
            "fn sum(this: &P) -> i32 {",
            "P::sum(&p)",
            "p.x",
        ],
    );
    assert_lacks(&rust, &["RefCell", "borrow"]);
}

#[test]
fn an_object_written_outside_its_constructor_keeps_its_refcell() {
    let rust = translate(
        "class P { int x; void set(int v) { x = v; } }
        class A {
            static void share(P q) {}
            public static void main(String[] a) { P p = new P(); p.set(2); }
        }",
    );
    assert_contains(&rust, &["Rc<RefCell<P>>", "fn set(this: &Rc<RefCell<P>>, v: i32)"]);
}

#[test]
fn an_object_that_is_never_shared_is_a_plain_value() {
    let rust = translate(
        "class P { int x; P(int x) { this.x = x; } void set(int v) { x = v; } int get() { return x; } }
        class A { public static void main(String[] a) {
            P p = new P(1);
            p.set(p.x + 1);
            System.out.println(p.get());
        } }",
    );
    assert_contains(
        &rust,
        &[
            "fn new(x: i32) -> P {",
            "let mut this = P { x: 0 };",
            "fn set(&mut self, v: i32) {",
            "fn get(&self) -> i32 {",
            "let mut p = P::new(1);",
            "p.set(p.x.wrapping_add(1));",
            "println!(\"{}\", p.get());",
        ],
    );
    assert_lacks(&rust, &["RefCell", "Rc", "borrow"]);
}

#[test]
fn printing_an_object_does_not_share_it() {
    let rust = translate(
        "class P { int x; void set(int v) { x = v; } public String toString() { return \"p\" + x; } }
        class A { public static void main(String[] a) {
            P p = new P();
            p.set(2);
            System.out.println(p);
            System.out.println(\"p=\" + p);
        } }",
    );
    assert_contains(&rust, &["let mut p = P::new();", "println!(\"{}\", P::to_string(&p));"]);
    assert_lacks(&rust, &["RefCell", "Rc"]);
}

#[test]
fn a_local_that_no_later_statement_changes_is_not_mut() {
    let rust = translate(
        "class P { int x; P(int x) { this.x = x; } int get() { return x; } }
        class A { public static void main(String[] a) {
            P p = new P(1);
            System.out.println(p.get());
        } }",
    );
    assert_contains(&rust, &["let p = P::new(1);"]);
    assert_lacks(&rust, &["let mut p"]);
}
