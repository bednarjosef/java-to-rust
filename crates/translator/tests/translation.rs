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
    assert_lacks(&rust, &["wrapping", "java::rem", "java::div"]);
}

#[test]
fn counting_loops_with_steps_and_inclusive_bounds_become_ranges() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            String s = \"abc\";
            for (int i = 0; i < 10; i += 3) { System.out.println(i); }
            for (int i = s.length() - 1; i >= 0; i--) { System.out.println(s.charAt(i)); }
            for (int i = 0; i <= 5; i++) { System.out.println(i); }
            for (long i = 1; i <= 5; i++) { System.out.println(i); }
        } }",
    );
    assert_contains(
        &rust,
        &[
            "for i in (0..10).step_by(3) {",
            "for i in (0..s.len() as i32).rev() {",
            "for i in 0..=5 {",
            "for i in 1i64..=5 {",
        ],
    );
}

#[test]
fn counting_loops_that_could_change_or_wrap_stay_while_loops() {
    let changed = translate(
        "class A { public static void main(String[] a) {
            int n = 4;
            for (int i = 0; i < n; i++) { n = 2; }
        } }",
    );
    assert_contains(&changed, &["while i < n {"]);
    let assigned = translate(
        "class A { public static void main(String[] a) {
            for (int i = 0; i < 10; i++) { i += 2; }
        } }",
    );
    assert_contains(&assigned, &["while i < 10 {"]);
    let max = translate(
        "class A { public static void main(String[] a) {
            for (int i = 0; i <= Integer.MAX_VALUE; i++) { if (i == 5) break; }
        } }",
    );
    assert_contains(&max, &["while i <= 2147483647 {"]);
}

#[test]
fn returns_that_end_a_function_become_its_value() {
    let sign = translate(
        "class A { static String sign(int n) {
            if (n > 0) { return \"pos\"; } else if (n < 0) { return \"neg\"; } else { return \"zero\"; }
        } }",
    );
    assert_contains(&sign, &["} else if n < 0 {", "        String::from(\"zero\")\n    }\n}"]);
    assert!(!sign.contains("return"), "{sign}");
    let nested = translate(
        "class A { static int nested(int n) {
            if (n > 0) { int m = n * 2; return m; } else { if (n == 0) { return 7; } else { return -1; } }
        } }",
    );
    assert_contains(&nested, &["        n.wrapping_mul(2)\n", "} else if n == 0 {"]);
    assert!(!nested.contains("return"), "{nested}");
    let partial = translate("class A { static int partial(int n) { if (n > 0) { return 1; } return 2; } }");
    assert_contains(&partial, &["        return 1;\n    }\n    2\n}"]);
    let day = translate(
        "class A { static String day(int d) {
            switch (d) { case 1: return \"Mon\"; default: return \"midweek\"; }
        } }",
    );
    assert_contains(&day, &["    match d {"]);
    assert!(!day.contains("return"), "{day}");
    let open = translate(
        "class A { static String open(int d) {
            switch (d) { case 1: return \"Mon\"; case 2: break; }
            return \"none\";
        } }",
    );
    assert_contains(&open, &["            return String::from(\"Mon\");\n"]);
}

#[test]
fn a_primitive_local_returned_at_the_end_is_the_value() {
    let twice = translate("class A { static int twice(int n) { int r = n + n; return r; } }");
    assert!(!twice.contains("let r") && !twice.contains("return"), "{twice}");
    let text = translate("class A { static String text() { String t = \"a\"; return t; } }");
    assert_contains(&text, &["    let t = String::from(\"a\");\n"]);
}

#[test]
fn a_switch_with_one_label_is_if_let_and_braced_cases_are_plain() {
    let rust = translate(
        "class A { static void f(int n) {
            switch (n) { case 3: System.out.println(\"three\"); break; }
            switch (n) { case 4: { System.out.println(\"four\"); break; } default: break; }
        } }",
    );
    assert_contains(&rust, &["if let 3 = n {", "if let 4 = n {"]);
    assert_lacks(&rust, &["loop {", "match n", "break"]);
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
    assert_lacks(&rust, &["wrapping", "java::rem", "java::div"]);
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
fn strings_that_are_never_null_stay_plain() {
    let rust = translate(
        "class A {
            static String twice(String s) { return s + s; }
            public static void main(String[] a) {
                String s = \"ab\";
                System.out.println(twice(s));
            }
        }",
    );
    assert_contains(&rust, &["fn twice(s: &str) -> String"]);
    assert_lacks(&rust, &["Option"]);
}

#[test]
fn strings_that_may_be_null_are_options() {
    let rust = translate(
        "class A {
            static String find(int n) { return n > 2 ? \"big\" : null; }
            public static void main(String[] a) {
                String s = find(3);
                System.out.println(s);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn find(n: i32) -> Option<String>",
            "let s: Option<String> = find(3);",
            "s.as_deref().unwrap_or(\"null\")",
        ],
    );
}

#[test]
fn null_checks_test_the_option() {
    let rust = translate(
        "class A {
            public static void main(String[] a) {
                String s = null;
                if (s == null) {
                    System.out.println(\"none\");
                }
            }
        }",
    );
    assert_contains(&rust, &["let s: Option<String> = None;", "if s.is_none() {"]);
}

#[test]
fn dereferencing_a_maybe_null_parameter_throws() {
    let rust = translate(
        "class A {
            static int size(String s) { return s.length(); }
            public static void main(String[] a) {
                System.out.println(size(null));
            }
        }",
    );
    assert_contains(&rust, &["fn size(s: Option<&str>) -> i32", "java::non_null(s).len() as i32"]);
    assert_lacks(&rust, &["unwrap()"]);
}

#[test]
fn a_null_check_binds_the_string_in_its_branch() {
    let rust = translate(
        "class A {
            static int size(String s) { if (s != null) { return s.length(); } return -1; }
            public static void main(String[] a) { System.out.println(size(null)); }
        }",
    );
    assert_contains(&rust, &["if let Some(s) = s {", "s.len() as i32"]);
    assert_lacks(&rust, &["non_null"]);
}

#[test]
fn an_early_return_on_null_binds_the_rest_of_the_block() {
    let rust = translate(
        "class A {
            static int size(String s) { if (s == null) return -1; return s.length(); }
            public static void main(String[] a) { System.out.println(size(null)); }
        }",
    );
    assert_contains(&rust, &["let Some(s) = s else {", "return -1;", "s.len() as i32"]);
    assert_lacks(&rust, &["non_null"]);
}

#[test]
fn a_null_check_whose_branch_does_not_read_the_string_does_not_bind_it() {
    let rust = translate(
        "class A {
            static int count(String s) { int n = 0; if (s != null) n++; return n; }
            public static void main(String[] a) { System.out.println(count(null)); }
        }",
    );
    assert_contains(&rust, &["if s.is_some() {"]);
    assert_lacks(&rust, &["if let Some(s)"]);
}

#[test]
fn a_branch_that_assigns_the_string_does_not_narrow_it() {
    let rust = translate(
        "class A {
            static int f(String s) { if (s != null) { s = null; return 1; } return s == null ? 2 : 3; }
            public static void main(String[] a) { System.out.println(f(\"a\")); }
        }",
    );
    assert_lacks(&rust, &["if let Some(s)", "let Some(s)"]);
}

#[test]
fn unaliased_arrays_are_vecs_indexed_in_bounds() {
    let rust = translate(
        "class A { public static void main(String[] args) {
            int[] squares = new int[5];
            for (int i = 0; i < squares.length; i++) { squares[i] = i * i; }
            int sum = 0;
            for (int x : squares) { sum += x; }
            System.out.println(sum);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "let mut squares: Vec<i32> = vec![0; 5];",
            "squares[i as usize] = i.wrapping_mul(i);",
            "for x in squares.iter().cloned() {",
        ],
    );
    assert_lacks(&rust, &["java::Array", "java::get", "java::set"]);
}

#[test]
fn arrays_seen_through_two_names_are_shared() {
    let rust = translate(
        "class A { public static void main(String[] args) {
            int[] a = new int[3];
            int[] b = a;
            b[0] = 5;
            System.out.println(a[0]);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "let a: java::Array<i32> = java::filled(3, 0);",
            "let b: java::Array<i32> = a.clone();",
            "java::set(&b, 0, 5);",
        ],
    );
    assert_lacks(&rust, &["Vec<i32>"]);
}

#[test]
fn arrays_passed_to_methods_are_borrowed() {
    let rust = translate(
        "class A {
            static void fill(int[] v) { v[0] = 1; }
            public static void main(String[] args) {
                int[] a = new int[3];
                fill(a);
                System.out.println(a[0]);
            }
        }",
    );
    assert_contains(&rust, &["fn fill(v: &mut [i32])", "fill(&mut a);", "let mut a: Vec<i32> = vec![0; 3];"]);
    assert_lacks(&rust, &["java::Array"]);
}

#[test]
fn arrays_passed_twice_in_one_call_are_shared() {
    let rust = translate(
        "class A {
            static void two(int[] x, int[] y) { x[0] = y[0]; }
            public static void main(String[] args) {
                int[] a = new int[2];
                two(a, a);
                System.out.println(a[0]);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn two(x: java::Array<i32>, y: java::Array<i32>)",
            "let a: java::Array<i32> = java::filled(2, 0);",
        ],
    );
    assert_lacks(&rust, &["Vec<i32>", "&mut ["]);
}

#[test]
fn counting_loops_that_only_read_elements_become_iterators() {
    let rust = translate(
        "class A { public static void main(String[] args) {
            int[] a = new int[4];
            for (int i = 0; i < a.length; i++) { a[i] = i * 3; }
            int s = 0;
            for (int i = 0; i < a.length; i++) { s += a[i]; }
            System.out.println(s);
        } }",
    );
    assert_contains(&rust, &["for item in a.iter().cloned() {", "s = s.wrapping_add(item);"]);
    assert_lacks(&rust, &["s.wrapping_add(a["]);
}

#[test]
fn element_loops_that_write_or_use_the_index_stay_indexed() {
    let rust = translate(
        "class A { public static void main(String[] args) {
            int[] a = new int[4];
            int s = 0;
            for (int i = 0; i < a.length; i++) { s += a[i]; a[i] = 1; }
            for (int i = 0; i < a.length; i++) { s += a[i] + i; }
            System.out.println(s);
        } }",
    );
    assert_contains(&rust, &["for i in 0..a.len() as i32 {"]);
    assert_lacks(&rust, &["a.iter().cloned()"]);
}

#[test]
fn nullable_parameters_that_are_only_read_are_borrowed() {
    let rust = translate(
        "class A {
            static String describe(String s) {
                if (s == null) { return \"nothing\"; }
                return \"text \" + s;
            }
            public static void main(String[] a) {
                System.out.println(describe(null));
                System.out.println(describe(\"x\"));
            }
        }",
    );
    assert_contains(
        &rust,
        &["fn describe(s: Option<&str>) -> String {", "let Some(s) = s else {", "describe(Some(\"x\"))"],
    );
}

#[test]
fn null_checks_in_and_chains_and_while_conditions_narrow() {
    let rust = translate(
        "class A {
            static String find(int i) { return i % 3 == 0 ? null : \"item\" + i; }
            static int len(String s) { return s != null && s.length() > 3 ? s.length() : -1; }
            public static void main(String[] a) {
                String t = find(1);
                int i = 0;
                while (t != null && i < 3) { System.out.println(t); i++; }
                System.out.println(len(find(2)));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "if let Some(s) = s && s.len() as i32 > 3 { s.len() as i32 } else { -1 }",
            "while let Some(t) = t.as_deref() && i < 3 {",
            "println!(\"{t}\");",
        ],
    );
    assert_lacks(&rust, &["is_none()", "non_null"]);
}

#[test]
fn a_default_for_null_is_unwrap_or() {
    let rust = translate(
        "class A {
            static String orDefault(String m) { return m == null ? \"missing\" : m; }
            public static void main(String[] a) {
                System.out.println(orDefault(null) + orDefault(\"here\"));
            }
        }",
    );
    assert_contains(
        &rust,
        &["fn or_default(m: Option<&str>) -> String {", "m.unwrap_or(\"missing\").to_string()"],
    );
}

#[test]
fn equals_on_null_evaluates_its_argument_first() {
    let rust = translate(
        "class A {
            static String find(int i) { return i > 0 ? \"x\" : null; }
            static String noisy(String s) { System.out.println(s); return s; }
            public static void main(String[] a) {
                String n = find(0);
                System.out.println(n.equals(noisy(\"y\")));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "match (n.as_deref(), noisy(\"y\")) { (receiver_, other_) => java::non_null(receiver_) == other_ }",
        ],
    );
}

#[test]
fn borrows_are_scoped_only_beside_a_mutable_borrow() {
    let rust = translate(
        "class P { int x; int bump() { x++; return x; } }
        class A {
            static void share(P q) {}
            static int f(int v) { return v + 1; }
            public static void main(String[] a) {
                P p = new P();
                System.out.println(p.x);
                System.out.println(f(p.x) + p.bump());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::println!(\"{}\", p.borrow().x);",
            // A lone braced argument would draw rustc's unused_braces lint, so it is a `let`.
            "f({ let value = p.borrow().x; value }).wrapping_add({ let value = p.borrow_mut().bump(); value })",
        ],
    );
}

#[test]
fn arguments_that_read_the_receiver_run_before_it_is_borrowed() {
    let rust = translate(
        "class P { int x; void add(int n) { x += n; } int get() { return x; } }
        class A {
            static void share(P q) {}
            public static void main(String[] a) { P p = new P(); P q = p; q.add(p.get()); q.add(2); }
        }",
    );
    assert_contains(
        &rust,
        &["{ let n = p.borrow().get(); q.borrow_mut().add(n) };", "q.borrow_mut().add(2);"],
    );
}

#[test]
fn to_string_is_display() {
    let rust = translate(
        "class P { int x; public String toString() { return \"P\" + x; } }
        class A { public static void main(String[] a) {
            P p = new P();
            System.out.println(p);
            String s = \"p=\";
            s += p;
            System.out.println(s + p.toString());
        } }",
    );
    assert_contains(
        &rust,
        &[
            "impl fmt::Display for P {\n    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {\n        write!(f, \"P{}\", self.x)\n    }\n}",
            "use std::fmt;",
            "java::println!(\"{p}\");",
            "s += &p.to_string();",
            "java::println!(\"{s}{p}\");",
        ],
    );
    assert_lacks(&rust, &["fn to_string"]);
}

#[test]
fn objects_whose_to_string_can_throw_are_converted_before_printing() {
    let rust = translate(
        "class P { int x; public String toString() { return \"P\" + 10 / x; } }
        class A { public static void main(String[] a) {
            P p = new P();
            System.out.println(\"value \" + p);
        } }",
    );
    // Formatting `{p}` in the line would print `value ` before the division throws.
    assert_contains(&rust, &["java::println!(\"value {}\", p.to_string());"]);
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
            "fn field_inits(&mut self) {",
            "self.y = self.x.wrapping_add(1);",
            "fn new(x: i32) -> P {",
            "let mut this = P { x: 0, y: 0 };",
            "this.field_inits();",
            "this.x = x;",
            "fn sum(&self) -> i32 {",
            "let p = Rc::new(P::new(3));",
            "p.sum().wrapping_add(p.x)",
        ],
    );
    assert_lacks(&rust, &["RefCell", "borrow"]);
}

#[test]
fn methods_borrow_their_shared_object() {
    let rust = translate(
        "class P { int x; P(int x) { this.x = x; } void set(int v) { x = v; } int get() { return x; } }
        class A {
            static void share(P q) {}
            public static void main(String[] a) { P p = new P(1); p.set(2); System.out.println(p.get()); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn new(x: i32) -> P {\n        P { x }\n    }",
            "fn set(&mut self, v: i32) {\n        self.x = v;\n    }",
            "fn get(&self) -> i32 {",
            "let p = Rc::new(RefCell::new(P::new(1)));",
            "p.borrow_mut().set(2);",
            "java::println!(\"{}\", p.borrow().get());",
        ],
    );
}

#[test]
fn methods_that_reach_their_object_another_way_take_the_shared_handle() {
    let rust = translate(
        "class P {
            int x;
            void register() { A.share(this); }
            void copy(P o) { x = o.x; }
        }
        class A {
            static void share(P q) {}
            public static void main(String[] a) { P p = new P(); P q = p; p.register(); q.copy(p); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn register(this: &Rc<RefCell<P>>) {\n        share(this);",
            // `o` may be `this`, which the method then must not hold borrowed.
            "fn copy(this: &Rc<RefCell<P>>, o: &Rc<RefCell<P>>) {\n        this.borrow_mut().x = { o.borrow().x };",
            "P::register(&p);",
            "P::copy(&q, &p);",
        ],
    );
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
            "fn new(x: i32) -> P {\n        P { x }\n    }",
            "fn set(&mut self, v: i32) {",
            "fn get(&self) -> i32 {",
            "let mut p = P::new(1);",
            "p.set(p.x.wrapping_add(1));",
            "java::println!(\"{}\", p.get());",
        ],
    );
    assert_lacks(&rust, &["RefCell", "Rc", "borrow"]);
}

#[test]
fn constructors_keep_java_order_in_their_struct_literal() {
    let rust = translate(
        "class P {
            int a; int b; int c = 7;
            P(int x) { b = x; c = A.next(); a = A.next(); System.out.println(a); }
        }
        class A {
            static int n;
            static int next() { n++; return n; }
            public static void main(String[] a) { P p = new P(5); System.out.println(p.a + p.b + p.c); }
        }",
    );
    // All three are taken into the literal; `a` is assigned by a call after `c`'s, but its field
    // comes first in the literal, so the calls run first, in Java's order.
    assert_contains(
        &rust,
        &["let c = next();\n        let a = next();\n        let this = P { a, b: x, c };", "        this\n"],
    );
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
    assert_contains(
        &rust,
        &["let mut p = P::new();", "java::println!(\"{p}\");", "java::println!(\"p={p}\");"],
    );
    assert_lacks(&rust, &["RefCell", "Rc"]);
}

#[test]
fn a_local_that_no_later_statement_changes_is_not_mut() {
    let rust = translate(
        "class P { int x; P(int x) { this.x = x; } int get() { return x; } void set(int v) { x = v; } }
        class A { public static void main(String[] a) {
            P p = new P(1);
            System.out.println(p.get());
        } }",
    );
    assert_contains(&rust, &["let p = P::new(1);"]);
    assert_lacks(&rust, &["let mut p"]);
}

#[test]
fn a_method_without_parameters_has_a_return_range() {
    let rust = translate(
        "class A {
            static int seven() { return 7; }
            public static void main(String[] a) { int t = seven() * 400000000; System.out.println(t); }
        }",
    );
    assert_contains(&rust, &["seven().wrapping_mul(400000000)"]);
}

#[test]
fn char_arithmetic_in_range_is_plain() {
    let rust = translate(
        "class A {
            static char next(char c) { return (char) (c + 1); }
            static int index(char c) { return c - 'a'; }
            public static void main(String[] a) {
                for (char c = 'a'; c < 'z'; c++) { System.out.print(next(c)); System.out.print(index(c)); }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn next(c: u16) -> u16 {",
            "    c + 1\n",
            "c as i32 - 'a' as i32",
            "for c in 'a' as u16..'z' as u16 {",
        ],
    );
    assert_lacks(&rust, &["wrapping"]);
}

#[test]
fn char_arithmetic_that_may_wrap_wraps_in_the_small_type() {
    let rust = translate(
        "class A {
            static char next(char c) { return (char) (c + 1); }
            public static void main(String[] a) { System.out.println(next(Character.MAX_VALUE)); }
        }",
    );
    assert_contains(&rust, &["c.wrapping_add(1)"]);
}

#[test]
fn small_integers_narrow_implicitly() {
    let rust = translate(
        "class A {
            static byte add(byte b, int x) { b += x; return b; }
            public static void main(String[] a) {
                byte b = 10;
                b += 5;
                short s = 100;
                s++;
                System.out.println(add(b, 300) + s);
            }
        }",
    );
    assert_contains(&rust, &["let mut b: i8 = 10;", "b += 5;", "s += 1;", "b = b.wrapping_add(x as i8);"]);
}

#[test]
fn narrowing_casts_follow_java() {
    let rust = translate(
        "class A {
            static byte low(int i) { return (byte) i; }
            static char fromDouble(double d) { return (char) d; }
            static int truncate(float f) { return (int) f; }
            public static void main(String[] a) {
                System.out.println(low(200) + \" \" + (int) fromDouble(-1.5) + \" \" + truncate(3.9f));
                System.out.println((byte) 200 + \" \" + (char) 65 + \" \" + (short) 70000);
            }
        }",
    );
    assert_contains(&rust, &["i as i8", "d as i32 as u16", "f as i32", "\"-56 A 4464\""]);
}

#[test]
fn chars_compare_and_print_as_chars() {
    let rust = translate(
        "class A {
            static boolean lower(char c) { return c >= 'a' && c <= 'z'; }
            public static void main(String[] a) { char c = 'q'; System.out.println(c + \" \" + lower(c)); }
        }",
    );
    assert_contains(&rust, &["c >= 'a' as u16 && c <= 'z' as u16", "java::Char(c)"]);
}

#[test]
fn char_at_on_an_ascii_string_in_a_counting_loop_indexes_bytes() {
    let rust = translate(
        "class A {
            static int vowels(String s) {
                int n = 0;
                for (int i = 0; i < s.length(); i++) { char c = s.charAt(i); if (c == 'a' || c == 'e') n++; }
                return n;
            }
            public static void main(String[] a) { System.out.println(vowels(\"banana tree\") + \" \" + \"xyz\".charAt(1)); }
        }",
    );
    assert_contains(
        &rust,
        &["for i in 0..s.len() as i32 {", "s.as_bytes()[i as usize] as u16", "java::ascii_char_at("],
    );
}

#[test]
fn char_at_on_text_that_may_not_be_ascii_counts_utf16_units() {
    let rust = translate(
        "class A {
            public static void main(String[] a) { String s = \"héllo\"; System.out.println(s.charAt(1)); }
        }",
    );
    assert_contains(&rust, &["java::char_at(&s, 1)"]);
    assert_lacks(&rust, &["as_bytes"]);
}

#[test]
fn string_built_from_ascii_char_casts_indexes_bytes() {
    let rust = translate(
        "class A {
            static String letters(int n) {
                String s = \"\";
                for (int i = 0; i < n; i++) { s = s + (char) ('a' + (i * 7 + i / 3) % 26); }
                return s;
            }
            static int count(String s, char target) {
                int count = 0;
                for (int i = 0; i < s.length(); i++) { if (s.charAt(i) == target) count++; }
                return count;
            }
            public static void main(String[] a) { System.out.println(count(letters(4000), 'a')); }
        }",
    );
    assert_contains(&rust, &["for i in 0..s.len() as i32 {", "s.as_bytes()[i as usize] as u16"]);
    assert_lacks(&rust, &["java::char_at", "java::string_length"]);
}

#[test]
fn char_cast_that_may_leave_ascii_keeps_utf16_units() {
    let rust = translate(
        "class A {
            static String wide(int n) {
                String s = \"\";
                for (int i = 0; i < n; i++) { s = s + (char) (200 + i); }
                return s;
            }
            public static void main(String[] a) { System.out.println(wide(4).charAt(1)); }
        }",
    );
    assert_contains(&rust, &["java::char_at(&wide(4), 1)"]);
    assert_lacks(&rust, &["as_bytes"]);
}

#[test]
fn primitive_statics_are_cells_and_other_statics_stay_ref_cells() {
    let rust = translate(
        "class A {
            static int count;
            static String name = \"x\";
            static void bump() { count += 2; name += count; }
            public static void main(String[] a) { bump(); System.out.println(count); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "static A_COUNT: Cell<i32> = const { Cell::new(0) };",
            "A_COUNT.get()",
            "A_COUNT.set(value);",
            "static A_NAME: RefCell<",
        ],
    );
    assert_lacks(&rust, &["A_COUNT.with_borrow"]);
}

#[test]
fn floats_are_f32_and_print_like_java() {
    let rust = translate(
        "class A {
            public static void main(String[] a) {
                float f = 1.5f;
                f *= 2;
                f += 0.1;
                System.out.println(f + \" \" + (f == 0.5) + \" \" + Math.max(f, 2f));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "let mut f: f32 = 1.5;",
            "f *= 2.0;",
            "f = (f as f64 + 0.1) as f32;",
            "java::Float(f)",
            "f == 0.5",
            "java::max_f32(f, 2.0)",
        ],
    );
}

#[test]
fn shared_arrays_are_lent_to_slice_parameters() {
    let rust = translate(
        "class A {
            static int sum(int[] v) { int s = 0; for (int x : v) s += x; return s; }
            static void bump(int[] v) { v[0] += 1; }
            public static void main(String[] args) {
                int[] a = {1, 2};
                int[] b = a;
                bump(a);
                System.out.println(sum(a) + sum(b));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn sum(v: &[i32]) -> i32",
            "fn bump(v: &mut [i32])",
            "bump(&mut a.borrow_mut());",
            "sum(&b.borrow())",
        ],
    );
}

#[test]
fn constant_indexes_of_arrays_of_known_length_are_in_bounds() {
    let rust = translate(
        "class A { public static void main(String[] args) {
            double[] d = {1.5, 2.5};
            d[0] *= 2;
            d[1]++;
            int[] q = new int[4];
            int i = 0;
            q[i++] = i;
            System.out.println(d[0] + d[1] + q[0]);
        } }",
    );
    assert_contains(
        &rust,
        &["d[0] *= 2.0;", "d[1] += 1.0;", "java::put(&mut q, { let old = i; i += 1; old }, i);"],
    );
}

#[test]
fn a_last_use_moves_instead_of_cloning() {
    let rust = translate(
        "class A {
            static int[] make(int n) { int[] made = new int[n]; made[0] = n; return made; }
            static String twice(String s) { String t = s + s; return t; }
            public static void main(String[] args) {
                int[] a = make(3);
                int[] b = a;
                System.out.println(b[0] + twice(\"x\"));
            }
        }",
    );
    assert_contains(&rust, &["    made\n", "let b: java::Array<i32> = a;"]);
    assert_lacks(&rust, &["made.clone()", "a.clone()"]);
}

#[test]
fn varargs_that_are_only_read_are_borrowed_slices() {
    let rust = translate(
        "class A {
            static int sum(int... xs) { int s = 0; for (int x : xs) s += x; return s; }
            static void fill(int... xs) { xs[0] = 1; }
            public static void main(String... args) {
                int[] arr = {4, 5};
                System.out.println(sum() + sum(1, 2, 3) + sum(arr));
                fill(arr);
                System.out.println(arr[0]);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn sum(xs: &[i32]) -> i32",
            "sum(&[])",
            "sum(&[1, 2, 3])",
            "sum(&arr)",
            "fn fill(xs: &mut [i32])",
            "fill(&mut arr);",
        ],
    );
    assert_lacks(&rust, &["java::Array"]);
}

#[test]
fn varargs_that_escape_are_shared_arrays() {
    let rust = translate(
        "class A {
            static int[] keep(int... xs) { return xs; }
            public static void main(String[] args) {
                int[] k = keep(7, 8);
                System.out.println(k[0]);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn keep(xs: java::Array<i32>) -> java::Array<i32>",
            "let k: java::Array<i32> = keep(java::from_vec(vec![7, 8]));",
        ],
    );
}

#[test]
fn object_varargs_are_rejected() {
    let err = java_to_rust::translate(
        "class A { static void log(String f, Object... xs) {} public static void main(String[] a) {} }",
    )
    .expect_err("Object varargs must be rejected");
    assert!(err.to_string().contains("Object not supported yet"), "{err}");
}

#[test]
fn annotations_modifiers_and_assertions_leave_no_trace() {
    let rust = translate(
        "@interface Tag { String value() default \"\"; }
        @Tag(\"a\") class A {
            @Deprecated static volatile int n = 0;
            @SuppressWarnings(\"x\") static synchronized void bump(final int by) { n += by; }
            static strictfp double half(@Tag final double x) { return x / 2; }
            public static void main(String[] args) {
                assert n < 0 : \"off\";
                synchronized (args) { bump(1); }
                ;
            }
        }",
    );
    assert_contains(&rust, &["fn bump(by: i32)", "fn half(x: f64) -> f64", "bump(1);", "let _ = args;"]);
    assert_lacks(
        &rust,
        &["Tag", "Deprecated", "SuppressWarnings", "assert", "volatile", "synchronized", "strictfp"],
    );
}

fn translate_project(project: &str, main: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/projects").join(project);
    let program =
        java_to_rust::Program { main: main.into(), sources: vec![root.join("src")], ..Default::default() };
    java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"))
}

#[test]
fn packages_become_nested_modules() {
    let rust = translate_project("imports", "com.example.app.Main");
    assert_contains(
        &rust,
        &[
            "fn main() {\n    crate::com::example::app::main();\n}",
            "mod com {\n    pub(crate) mod example {\n        pub(crate) mod app {",
            "        pub(crate) mod util {\n            use std::fmt;\n\n            use java_runtime as java;",
            // Imported, statically imported and fully qualified names are paths from the crate root.
            "crate::com::example::util::Counter::new(\"clicks\")",
            "crate::com::example::util::square(7)",
            "crate::com::example::util::NUMBERS_LIMIT - crate::com::example::util::square(3)",
            "crate::com::example::util::cube(3)",
            // Names in the same package are not.
            "line(\"total\", clicks.total())",
            "x * square(x)",
            "pub(crate) struct Counter {\n                pub(crate) name: String,",
        ],
    );
}

#[test]
fn unreachable_code_is_not_translated() {
    let rust = translate_project("pruning", "app.Main");
    assert_contains(&rust, &["mod app {", "mod lib {", "fn greet(", "GREETER_MADE"]);
    // The package `extra`, the unused constructor, method and fields, and the second `main`.
    assert_lacks(&rust, &["extra", "Unused", "history", "BANNER", "cache", "fn new_", "crate::lib::main"]);
    assert_eq!(rust.matches("fn main()").count(), 2, "the root `main` and `app::main` only");
}

#[test]
fn classes_with_one_name_in_two_packages() {
    let rust = translate_project("statics", "com.shop.Main");
    assert_contains(
        &rust,
        &[
            "crate::com::shop::model::Item::new_string_i32(\"apple\", 3)",
            "crate::com::shop::legacy::Item::new(7)",
            "crate::com::shop::model::RegistryEntry::new(4)",
            "crate::com::shop::model::REGISTRY_CREATED.get()",
            "pub(crate) static COUNTER_HITS: Cell<i32>",
        ],
    );
}

/// Writes Java files into a fresh directory under the test's temporary directory.
fn write_sources(dir: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(dir);
    let _ = std::fs::remove_dir_all(&dir);
    for (path, java) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, java).unwrap();
    }
    dir
}

#[test]
fn listed_files_without_packages_stay_flat() {
    let dir = write_sources(
        "listed-files",
        &[
            (
                "Main.java",
                "class Main { public static void main(String[] a) { System.out.println(Util.twice(4)); } }",
            ),
            (
                "Util.java",
                "class Util { static int twice(int x) { return 2 * x; } static int unused() { return 0; } }",
            ),
        ],
    );
    let files = vec![dir.join("Main.java"), dir.join("Util.java")];
    let program = java_to_rust::Program { main: "Main".into(), files, ..Default::default() };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &["fn twice(x: i32) -> i32 {", "fn main() {\n    java::println!(\"{}\", twice(4));"],
    );
    assert_lacks(&rust, &["mod ", "pub(crate)", "unused"]);
}

#[test]
fn library_roots_are_read_only_where_used() {
    let dir = write_sources(
        "library-roots",
        &[
            (
                "app/main/Main.java",
                "package main;\nimport lib.Text;\npublic class Main {\n    public static void main(String[] a) { System.out.println(Text.shout(\"hi\")); }\n}\n",
            ),
            (
                "lib/lib/Text.java",
                "package lib;\npublic class Text {\n    public static String shout(String s) { return s + \"!\"; }\n    public static Object unused() { return new Object(); }\n}\n",
            ),
            // Never used, so javac never reads it: not even its syntax error is reported.
            ("lib/lib/Broken.java", "package lib;\npublic class Broken {\n"),
        ],
    );
    let program = java_to_rust::Program {
        main: "main.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &["crate::lib::shout(\"hi\")", "mod lib {", "pub(crate) fn shout(s: &str) -> String"],
    );
    assert_lacks(&rust, &["unused", "Broken"]);
}

#[test]
fn program_errors_name_their_file() {
    let dir = write_sources(
        "program-errors",
        &[
            (
                "p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] a) {\n        Other.run();\n    }\n}\n",
            ),
            (
                "p/Other.java",
                "package p;\nclass Other {\n    static void run() {\n        Object o = null;\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let error = java_to_rust::translate_program(&program).expect_err("Object is not supported").to_string();
    assert!(error.ends_with("Other.java:4:9: Object not supported yet"), "{error}");
}

#[test]
fn arraycopy_copies_plain_arrays_between_slices() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            int[] src = {1, 2, 3, 4};
            int[] dst = new int[4];
            System.arraycopy(src, 1, dst, 0, 3);
            System.arraycopy(dst, 1, dst, 0, dst.length - 1);
            int[] shared = new int[2];
            int[] alias = shared;
            System.arraycopy(src, 0, shared, 0, 2);
            System.out.println(dst[0] + alias[1]);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "let src: Vec<i32> = vec![1, 2, 3, 4];",
            "java::copy_slice(&src, 1, &mut dst, 0, 3);",
            "let length = dst.len() as i32 - 1;\n    java::copy_within(&mut dst, 1, 0, length);",
            "java::copy_slice(&src, 0, &mut shared.borrow_mut(), 0, 2);",
        ],
    );
}

#[test]
fn math_uses_rust_methods_where_they_match_hotspot() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            double x = 0.5;
            System.out.println(Math.sin(x) + StrictMath.sin(x) + Math.asin(x) + Math.log(x) + Math.log10(x));
            System.out.println(Math.floor(x) + Math.rint(x) + Math.round(x) + Math.floorMod(-7, 3) + Math.fma(x, x, x));
            System.out.println(Math.atan2(x, 1.0) + Math.IEEEremainder(x, 0.3) + Math.signum(x) + Math.PI);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "x.sin()",
            "java::fdlibm::sin(x)",
            "java::fdlibm::asin(x)",
            "x.ln()",
            "java::log10(x)",
            "x.floor()",
            "x.round_ties_even()",
            "java::round(x)",
            "java::floor_mod(-7, 3)",
            "x.mul_add(x, x)",
            "java::fdlibm::atan2(x, 1.0)",
            "java::fdlibm::remainder(x, 0.3)",
            "java::signum(x)",
            "3.141592653589793",
        ],
    );
}

#[test]
fn bit_conversions_are_plain_where_java_keeps_the_bits() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            double d = 1.5;
            long bits = Double.doubleToRawLongBits(d);
            float f = Float.intBitsToFloat(7);
            System.out.println(bits + Double.doubleToLongBits(d) + \" \" + Double.longBitsToDouble(bits) + f);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "let bits = d.to_bits() as i64;",
            "let f = f32::from_bits(7i32 as u32);",
            "java::double_to_long_bits(d)",
            "f64::from_bits(bits as u64)",
        ],
    );
}

#[test]
fn system_natives_go_through_the_runtime() {
    let rust = translate(
        "class A {
            int v;
            public static void main(String[] a) {
                A x = new A();
                long t = System.nanoTime() + System.currentTimeMillis();
                System.err.println(\"t \" + t + System.lineSeparator());
                System.out.println(x.hashCode() == System.identityHashCode(x));
                System.out.write(65);
                System.out.flush();
                System.out.println(Runtime.getRuntime().availableProcessors());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::nano_time()",
            "java::current_time_millis()",
            "java::eprintln!(\"t {t}{}\", java::line_separator());",
            "java::identity_hash(&x) == java::identity_hash(&x)",
            "java::write_byte(65);",
            "java::flush();",
            "java::available_processors()",
        ],
    );
    let err = java_to_rust::translate(
        "class A { public static void main(String[] a) { System.out.println(System.getenv(\"HOME\")); } }",
    )
    .unwrap_err();
    assert!(err.to_string().contains("System.getenv` is not supported"), "{err}");
}

#[test]
fn static_final_constants_are_consts_read_without_a_cell() {
    let rust = translate(
        "class A {
            static final int LIMIT = 10;
            static final String NAME = \"a\";
            public static void main(String[] args) {
                System.out.println(LIMIT + NAME);
            }
        }",
    );
    assert_contains(&rust, &["const A_LIMIT: i32 = 10;", "const A_NAME: &str = \"a\";"]);
    assert_lacks(&rust, &["A_LIMIT: RefCell", "A_NAME: RefCell", "A_LIMIT.with"]);
}

#[test]
fn static_blocks_run_once_in_an_init_function_guarded_by_a_flag() {
    let rust = translate(
        "class Helper {
            static int made;
            static { made = 10; }
            static int make() { return made; }
        }
        public class A {
            public static void main(String[] args) { System.out.println(Helper.make()); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "static HELPER__INITIALIZED: Cell<bool> = const { Cell::new(false) };",
            "fn init_helper() {",
            "if HELPER__INITIALIZED.get() {",
            "fn make() -> i32 {\n    init_helper();",
        ],
    );
}

#[test]
fn static_initializers_without_effects_stay_lazy() {
    let rust = translate(
        "class A {
            static int base = 4;
            static int doubled = base * 2;
            public static void main(String[] args) { System.out.println(doubled); }
        }",
    );
    assert_lacks(&rust, &["fn init_a()", "INITIALIZED"]);
}

#[test]
fn an_enum_is_a_copy_rust_enum_with_its_name_and_value_of() {
    let rust = translate(
        "public class A {
            enum Color { RED, VERY_BIG }
            public static void main(String[] args) { System.out.println(Color.valueOf(\"RED\")); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]\nenum AColor {\n    Red,\n    VeryBig,\n}",
            "fn enum_name(self) -> String {",
            "AColor::VeryBig => \"VERY_BIG\",",
            "fn enum_value_of(name: &str) -> AColor {",
            "_ => java::throw(\"IllegalArgumentException\", &format!(\"No enum constant A.Color.{name}\")),",
            "impl fmt::Display for AColor {",
            "write!(f, \"{}\", self.enum_name())",
        ],
    );
}

#[test]
fn an_enum_field_is_an_accessor_that_matches_on_self() {
    let rust = translate(
        "public class A {
            enum Planet {
                MERCURY(3.0), VERY_BIG(5.5);
                private final double mass;
                Planet(double mass) { this.mass = mass; }
                double scaled(double k) { return mass * k; }
            }
            public static void main(String[] args) { System.out.println(Planet.VERY_BIG.scaled(2.0)); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn mass(self) -> f64 {\n        match self {\n            APlanet::Mercury => 3.0,\n            APlanet::VeryBig => 5.5,",
            "fn scaled(self, k: f64) -> f64 {\n        self.mass() * k",
            "APlanet::VeryBig.scaled(2.0)",
        ],
    );
}

#[test]
fn an_enum_field_set_first_by_every_constructor_needs_no_initializer() {
    let rust = translate(
        "public class A {
            enum Color { RED, BLUE }
            static class Paint {
                private final Color color;
                Paint(Color color) { this.color = color; }
                Color color() { return color; }
            }
            public static void main(String[] args) { System.out.println(new Paint(Color.BLUE).color()); }
        }",
    );
    assert_contains(&rust, &["fn new(color: AColor) -> APaint {\n        APaint { color }\n    }"]);
}

#[test]
fn an_enum_field_that_a_constructor_reads_before_setting_it_is_rejected() {
    let error = java_to_rust::translate(
        "public class A {
            enum Color { RED, BLUE }
            static class Paint {
                private Color color;
                Paint(Color color) { System.out.println(this.color); this.color = color; }
            }
            public static void main(String[] args) { new Paint(Color.BLUE); }
        }",
    )
    .expect_err("a field read before it is set is null");
    assert!(error.to_string().contains("null enums not supported yet"), "{error}");
}

#[test]
fn switch_without_fall_through_is_a_match_on_int_char_and_string() {
    let rust = translate(
        "class A {
            static String day(int d) {
                switch (d) {
                    case 1: return \"Mon\";
                    case 6: case 7: return \"weekend\";
                    default: return \"midweek\";
                }
            }
            static int weight(char c) {
                switch (c) {
                    case 'a', 'e' -> { return 1; }
                    default -> { return 0; }
                }
            }
            static void report(String kind) {
                switch (kind) {
                    case \"log\" -> System.out.println(\"log\");
                    default -> System.out.println(\"other\");
                }
            }
            public static void main(String[] a) {
                String s = \"x\";
                switch (s) {
                    case \"x\": System.out.println(1); break;
                    default: System.out.println(2);
                }
                String missing = null;
                switch (missing) {
                    case \"x\" -> System.out.println(3);
                    default -> System.out.println(4);
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "match d {",
            "6 | 7 => {",
            "_ => {",
            "match char::from_u32(u32::from(c)) {",
            "Some('a' | 'e') => {",
            "match kind {",
            "\"log\" => {",
            "match s.as_str() {",
            "match java::non_null(missing.as_deref()) {",
        ],
    );
    assert_lacks(&rust, &["'sw", "let mut case"]);
}

#[test]
fn arrow_switch_expression_is_a_match_that_yields_a_value() {
    let rust = translate(
        "class A {
            static int code(short s) {
                int r = switch (s) {
                    case 100 -> 1;
                    case -4 -> { int t = s * 3; yield t; }
                    default -> 0;
                };
                return r;
            }
            public static void main(String[] a) {
                System.out.println(code((short) 100));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn code(s: i16) -> i32 {\n    {\n",
            "match s {",
            "100 => {",
            "-4 => {\n                s as i32 * 3\n",
        ],
    );
    assert_lacks(&rust, &["'sw"]);
}

#[test]
fn switch_expression_that_yields_from_inside_an_if_is_a_labeled_block() {
    let rust = translate(
        "class A {
            static String size(byte b) {
                return switch (b) {
                    case 0 -> \"zero\";
                    default -> {
                        if (b > 0) {
                            yield \"big\";
                        }
                        yield \"negative\";
                    }
                };
            }
            public static void main(String[] a) {
                System.out.println(size((byte) 9));
            }
        }",
    );
    assert_contains(
        &rust,
        &["'sw1: {", "break 'sw1 String::from(\"big\");", "break 'sw1 String::from(\"negative\");"],
    );
    assert_lacks(&rust, &["loop {"]);
}

#[test]
fn fall_through_runs_a_state_machine_over_the_case_index() {
    let rust = translate(
        "class A {
            static int run(int n) {
                int x = 0;
                switch (n) {
                    case 1:
                        x += 1;
                    case 2:
                        x += 2;
                        break;
                    default:
                        x = 9;
                }
                return x;
            }
            public static void main(String[] a) {
                System.out.println(run(1));
            }
        }",
    );
    assert_contains(
        &rust,
        &["let mut case: usize = match n {", "1 => 0,", "loop {", "match case {", "case = 1;"],
    );
}

#[test]
fn labeled_loops_and_blocks_are_named_by_their_jumps() {
    let rust = translate(
        "class A {
            public static void main(String[] a) {
                int found = 0;
                search:
                for (int i = 0; i < 6; i++) {
                    for (int j = 0; j < 6; j++) {
                        if (i * j == 12) {
                            found = i;
                            break search;
                        }
                    }
                }
                int pairs = 0;
                outer:
                for (int i = 0; i < 5; i++) {
                    for (int j = 0; j < 5; j++) {
                        if (j > i) {
                            continue outer;
                        }
                        pairs++;
                    }
                }
                int n = 0;
                scan:
                while (n < 100) {
                    n += 7;
                    if (n % 5 == 0) {
                        break scan;
                    }
                }
                int stage = 0;
                block: {
                    stage = 1;
                    if (stage == 1) {
                        break block;
                    }
                    stage = 2;
                }
                System.out.println(found + pairs + n + stage);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "'search: for i in 0..6 {",
            "break 'search;",
            "'outer: for i in 0..5 {",
            "continue 'outer;",
            "'block: {",
            "break 'block;",
        ],
    );
    // A `break` directly in the innermost loop needs no label, so `scan` is dropped.
    assert_contains(&rust, &["while n < 100 {"]);
    assert_lacks(&rust, &["'scan"]);
}

#[test]
fn break_in_a_switch_inside_a_loop_leaves_the_switch_only() {
    let rust = translate(
        "class A {
            public static void main(String[] a) {
                int total = 0;
                for (int i = 0; i < 4; i++) {
                    switch (i) {
                        case 1:
                            if (i == 1) {
                                break;
                            }
                            total += 5;
                            break;
                        default:
                            total += i;
                    }
                    total += 10;
                }
                System.out.println(total);
            }
        }",
    );
    assert_contains(&rust, &["'sw1: {", "break 'sw1;"]);
    assert_lacks(&rust, &["break;"]);
}

#[test]
fn boxed_values_without_identity_checks_are_plain_ints() {
    let rust = translate(
        "class A {
            static Integer maybe(int n) { return n > 2 ? n : null; }
            static Integer twice(Integer x) { return x * 2; }
            public static void main(String[] a) {
                Integer plain = 5;
                int sum = plain + twice(3);
                Integer nul = maybe(sum);
                System.out.println(sum + \" \" + nul);
                if (nul != null) {
                    System.out.println(nul.intValue() + 1);
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn maybe(n: i32) -> Option<i32>",
            "fn twice(x: i32) -> i32",
            "let plain: i32 = 5;",
            "let nul: Option<i32> = maybe(sum);",
            "if let Some(nul) = nul {",
        ],
    );
    assert_lacks(&rust, &["Boxed", "box_int", "as_deref"]);
}

#[test]
fn boxed_values_compared_by_identity_are_shared_boxes() {
    let rust = translate(
        "class A {
            static Integer twice(Integer x) { return x * 2; }
            public static void main(String[] a) {
                Integer p = 100, q = 100;
                System.out.println((p == q) + \" \" + twice(p).equals(q));
            }
        }",
    );
    assert_contains(
        &rust,
        &["fn twice(x: java::Boxed<i32>) -> java::Boxed<i32>", "java::box_int(100)", "java::same_optional("],
    );
}

#[test]
fn a_nullable_boxed_identity_is_an_option_of_the_shared_box() {
    let rust = translate(
        "class A {
            static Integer maybe(int n) { return n > 2 ? n : null; }
            public static void main(String[] a) {
                Integer nul = maybe(3);
                Integer other = maybe(4);
                System.out.println((nul == other) + \" \" + nul);
            }
        }",
    );
    assert_contains(
        &rust,
        &["fn maybe(n: i32) -> Option<java::Boxed<i32>>", "let nul: Option<java::Boxed<i32>> = maybe(3);"],
    );
}

#[test]
fn object_fields_that_may_be_null_are_options() {
    let rust = translate(
        "class Node {
            int value; Node next;
            Node(int value, Node next) { this.value = value; this.next = next; }
        }
        class A {
            static int sum(Node head) {
                int s = 0;
                for (Node n = head; n != null; n = n.next) s += n.value;
                return s;
            }
            public static void main(String[] a) { System.out.println(sum(new Node(1, new Node(2, null)))); }
        }",
    );
    // `n` is assigned in the loop, so it is not narrowed there: each use checks it.
    assert_contains(
        &rust,
        &[
            "next: Option<Rc<Node>>,",
            "while n.is_some() {",
            "s = s.wrapping_add(java::non_null(n.as_ref()).value);",
            "n = java::non_null(n.as_ref()).next.clone();",
            "fn sum(head: Rc<Node>) -> i32 {",
            "let mut n: Option<Rc<Node>> = Some(head);",
            "sum(Rc::new(Node::new(1, Some(Rc::new(Node::new(2, None))))))",
        ],
    );
}

#[test]
fn fields_that_every_constructor_sets_are_not_options() {
    let rust = translate(
        "class Node { int v; Node(int v) { this.v = v; } }
        class Edge {
            Node from, to;
            Edge(Node from, Node to) { this.from = from; this.to = to; }
            int span() { return to.v - from.v; }
        }
        class A {
            public static void main(String[] a) {
                Edge e = new Edge(new Node(1), new Node(4));
                System.out.println(e.span());
            }
        }",
    );
    assert_contains(&rust, &["from: Rc<Node>,", "to: Rc<Node>,", "Edge { from, to }"]);
    assert_lacks(&rust, &["Option", "non_null"]);
}

#[test]
fn objects_compare_by_identity() {
    let rust = translate(
        "class P { int x; P(int x) { this.x = x; } }
        class A {
            static P find(P p, int x) { return p.x == x ? p : null; }
            public static void main(String[] a) {
                P p = new P(1);
                P q = new P(1);
                P r = find(p, 1);
                System.out.println(p == q);
                System.out.println(p != r);
                System.out.println(r == null);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::println!(\"{}\", Rc::ptr_eq(&p, &q));",
            "java::println!(\"{}\", !java::same(Some(&p), r.as_ref()));",
        ],
    );
}

#[test]
fn objects_that_may_be_null_print_as_null() {
    let rust = translate(
        "class P {
            int x; P(int x) { this.x = x; }
            public String toString() { return \"P\" + x; }
        }
        class A {
            static P find(int x) { return x > 0 ? new P(x) : null; }
            public static void main(String[] a) {
                P p = find(1);
                P q = find(0);
                System.out.println(p + \" \" + q);
                if (p != null) System.out.println(p.x);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::println!(\"{} {}\", java::or_null(p.as_ref()), java::or_null(q.as_ref()));",
            "if let Some(p) = p.as_ref() {\n        java::println!(\"{}\", p.x);",
        ],
    );
}

#[test]
fn arrays_whose_elements_may_be_null_hold_options() {
    let rust = translate(
        "class A {
            public static void main(String[] a) {
                String[] words = new String[3];
                words[1] = \"mid\";
                System.out.println(words[0] + words[1]);
                int[][] rows = new int[2][];
                rows[0] = new int[] {1, 2};
                System.out.println(rows[1] == null);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "let mut words: Vec<Option<String>> = vec![None; 3];",
            "words[1] = Some(String::from(\"mid\"));",
            "java::println!(\"{}{}\", words[0].as_deref().unwrap_or(\"null\"), words[1].as_deref().unwrap_or(\"null\"));",
            "let rows: java::Array<Option<java::Array<i32>>> = java::filled(2, None);",
            "java::println!(\"{}\", java::get(&rows, 1).is_none());",
        ],
    );
}

#[test]
fn object_parameters_used_only_through_are_borrowed() {
    let rust = translate(
        "class Box { int v; }
        class A {
            static void twice(Box b) { b.v = b.v * 2; }
            static Box keep(Box b) { return b; }
            public static void main(String[] a) {
                Box x = new Box();
                x.v = 3;
                twice(x);
                Box y = keep(x);
                System.out.println(x.v + \" \" + (x == y));
            }
        }",
    );
    // `keep` returns its parameter, so it takes the handle itself.
    assert_contains(
        &rust,
        &["fn twice(b: &Rc<RefCell<Box>>) {", "twice(&x);", "fn keep(b: Rc<RefCell<Box>>)"],
    );
}

#[test]
fn strings_with_unpaired_surrogates_are_rejected() {
    let err = java_to_rust::translate(
        "class A { public static void main(String[] args) { System.out.println(\"a\\uD800b\".length()); } }",
    )
    .unwrap_err();
    assert!(err.to_string().contains("a string with an unpaired surrogate not supported yet"), "{err}");
}

#[test]
fn the_survey_follows_calls_to_the_classes_whose_objects_are_created() {
    let dir = write_sources(
        "survey",
        &[(
            "Main.java",
            "@interface Tag {}
            interface Shape { int area(); }
            class Square implements Shape { public int area() { return 4; } }
            class Circle implements Shape { public int area() { return 3; } }
            class Box<T> { class Inner { int size() { return 1; } } }
            @Tag public class Main {
                static Square unused() { return new Square(); }
                public static void main(String[] args) {
                    Shape s = new Circle();
                    Square none = null;
                    System.out.println(s.area() + new Box<String>().new Inner().size());
                }
            }",
        )],
    );
    let program = java_to_rust::Program { main: "Main".into(), sources: vec![dir], ..Default::default() };
    let report = survey(&program).report;
    // `Circle.area` runs for `s.area()`; `Square` is named but never created, and the annotation
    // does nothing.
    assert_contains(
        &report,
        &[
            "Reachable from Main: 6 classes (3 instantiated), 7 methods",
            "  Circle: interfaces\n",
            "  Box.Inner: inner classes\n",
            "  Shape: interfaces\n",
        ],
    );
    assert_lacks(&report, &["Square:", "Tag"]);
}

/// The survey of a program, which must not fail.
fn survey(program: &java_to_rust::Program) -> java_to_rust::program::Survey {
    java_to_rust::survey_program(program).unwrap_or_else(|e| panic!("survey failed: {e}"))
}

/// The feature table of one part's code in a survey report: its rows up to the blank line.
fn feature_table<'a>(report: &'a str, title: &str) -> &'a str {
    report
        .split(title)
        .nth(1)
        .unwrap_or_else(|| panic!("no {title} in {report}"))
        .split("\n\n")
        .next()
        .unwrap()
}

/// The row of a feature in a feature table.
fn feature_row<'a>(table: &'a str, feature: &str) -> &'a str {
    table.lines().find(|line| line.starts_with(feature)).unwrap_or_else(|| panic!("no {feature} in {table}"))
}

/// A program whose `Main.a`..`Main.d` each hold one lambda, in package `app`, under a source root.
fn lambda_sources(dir: &str) -> std::path::PathBuf {
    let java = [
        "package app;",
        "public class Main {",
        "    static Runnable a() { return () -> {}; }",
        "    static Runnable b() { return () -> {}; }",
        "    static Runnable c() { return () -> {}; }",
        "    static Runnable d() { return () -> {}; }",
        "    public static void main(String[] args) {",
        "        a().run(); b().run(); c().run(); d().run();",
        "    }",
        "}",
    ]
    .join("\n");
    write_sources(dir, &[("app/Main.java", &java)])
}

#[test]
fn local_and_anonymous_classes_are_declared_so_their_calls_are_not_outside() {
    let dir = write_sources(
        "survey_local",
        &[(
            "Main.java",
            "public class Main {
                public static void main(String[] args) {
                    class Local { int n() { return 2; } }
                    Object anon = new Object() { };
                    System.out.println(new Local().n());
                }
            }",
        )],
    );
    let program = java_to_rust::Program { main: "Main".into(), sources: vec![dir], ..Default::default() };
    let report = survey(&program).report;
    // The local class is a feature of the method that declares it; its calls are not calls to
    // classes outside the sources.
    let table = feature_table(&report, "Program code: features");
    assert!(feature_row(table, "local classes").contains("Main.java:"), "{table}");
    assert!(feature_row(table, "anonymous classes").contains("Main.java:"), "{table}");
    assert!(!table.contains("classes outside"), "{table}");
    assert_lacks(&report, &["(outside)"]);
}

#[test]
fn the_survey_reports_the_program_and_its_libraries_in_separate_tables() {
    let dir = write_sources(
        "survey_split",
        &[(
            "Main.java",
            "public class Main {
                public static void main(String[] args) {
                    class Local { int n() { return 1; } }
                    System.out.println(Helper.run() + new Local().n());
                }
            }",
        )],
    );
    let lib = write_sources(
        "survey_split_lib",
        &[(
            "Helper.java",
            "public class Helper { static int run() { Runnable r = () -> {}; r.run(); return 1; } }",
        )],
    );
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir],
        libraries: vec![lib.clone()],
        ..Default::default()
    };
    let report = survey(&program).report;
    let program_table = feature_table(&report, "Program code: features");
    let library_table = feature_table(&report, "Library code: features");
    assert!(report.find("Program code:").unwrap() < report.find("Library code:").unwrap(), "{report}");
    assert!(feature_row(program_table, "local classes").contains("Main.java:"), "{program_table}");
    assert!(!program_table.contains("lambdas"), "{program_table}");
    // The library's example is relative to its root, not the absolute path.
    let row = feature_row(library_table, "lambdas");
    assert!(row.contains("Helper.java:1"), "{row}");
    assert!(!row.contains(lib.to_str().unwrap()), "{row}");
    assert!(!library_table.contains("local classes"), "{library_table}");
}

#[test]
fn the_survey_names_up_to_three_example_places_per_feature() {
    let program = java_to_rust::Program {
        main: "app.Main".into(),
        sources: vec![lambda_sources("survey_examples")],
        ..Default::default()
    };
    let report = survey(&program).report;
    let table = feature_table(&report, "Program code: features");
    // Four pieces use a lambda; the first three are named, relative to the source root.
    assert!(
        feature_row(table, "lambdas").ends_with("app/Main.java:3, app/Main.java:4, app/Main.java:5"),
        "{table}"
    );
    assert_lacks(&report, &["app/Main.java:6"]);
}

#[test]
fn the_survey_json_lists_the_pieces_with_their_example_places() {
    let program = java_to_rust::Program {
        main: "app.Main".into(),
        sources: vec![lambda_sources("survey_json")],
        ..Default::default()
    };
    let json = survey(&program).json;
    assert_eq!(json["main"], "app.Main");
    let pieces = json["pieces"].as_array().expect("pieces");
    let a = pieces.iter().find(|p| p["name"] == "a").expect("a is a piece");
    assert_eq!(a["source"], "program");
    assert_eq!(a["kind"], "method");
    assert_eq!(a["location"], "app/Main.java:3");
    assert_eq!(a["features"]["lambdas"], "app/Main.java:3");
    assert!(json["runtime_calls"].is_array(), "{json}");
    assert!(json["natives"].is_array(), "{json}");
}

#[test]
fn survey_json_writes_the_census_to_the_file_given() {
    let dir = lambda_sources("survey_cli");
    let json = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("survey_cli.json");
    let _ = std::fs::remove_file(&json);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "app.Main", "--source"])
        .arg(&dir)
        .arg("--survey-json")
        .arg(&json)
        .output()
        .expect("runs the translator");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Reachable from app.Main"), "{stdout}");
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(written["main"], "app.Main");
    assert!(written["pieces"].as_array().is_some_and(|p| !p.is_empty()), "{written}");
}

#[test]
fn an_enum_field_assigned_outside_its_constructor_is_rejected() {
    let error = java_to_rust::translate(
        "public class A {
            enum Planet {
                MERCURY(3.0);
                private double mass;
                Planet(double mass) { this.mass = mass; }
                void grow() { mass += 1; }
            }
            public static void main(String[] args) { Planet.MERCURY.grow(); }
        }",
    )
    .expect_err("an enum field that changes is rejected");
    assert!(
        error.to_string().contains("assigning an enum field outside its constructor not supported yet"),
        "{error}"
    );
}

#[test]
fn enum_constant_bodies_become_arms_of_the_enum_method() {
    let rust = translate(
        "public class A {
            enum Op {
                PLUS { int apply(int x, int y) { return x + y; } },
                NEG { int apply(int a, int b) { return -a; } };
                abstract int apply(int a, int b);
            }
            public static void main(String[] args) { System.out.println(Op.PLUS.apply(1, 2) + Op.NEG.apply(3, 0)); }
        }",
    );
    // The arms name the parameters as the override does.
    assert_contains(
        &rust,
        &[
            "fn apply(self, a: i32, b: i32) -> i32 {\n        match self {",
            "AOp::Plus => {\n                let x = a;",
        ],
    );
    let error = java_to_rust::translate(
        "public class A {
            enum Op { PLUS { public String toString() { return \"+\"; } } }
            public static void main(String[] args) { System.out.println(Op.PLUS); }
        }",
    )
    .expect_err("a body that overrides a method the enum does not declare is rejected");
    assert!(
        error.to_string().contains("a method of an enum constant's body that overrides none of the enum's"),
        "{error}"
    );
}

#[test]
fn an_enum_field_with_a_method_of_its_name_gets_another_accessor() {
    let rust = translate(
        "public class A {
            enum Planet {
                MERCURY(3.0), VENUS(4.0);
                private double mass;
                private int rings = 0;
                Planet(double mass) { this.mass = mass; }
                double mass() { return mass; }
                public String toString() { return \"planet \" + name(); }
            }
            public static void main(String[] args) { System.out.println(Planet.VENUS.mass() + \" \" + Planet.MERCURY); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn mass_field(self) -> f64 {",
            "fn mass(self) -> f64 {\n        self.mass_field()\n    }",
            "fn rings(self) -> i32 {",
            "f.write_str(&Self::to_string(*self))",
        ],
    );
}

#[test]
fn string_builders_chain_and_are_shared_handles() {
    let rust = translate(
        "class A {
            static void fill(StringBuilder sb) { sb.append(1).append(',').append(\"x\"); }
            public static void main(String[] a) {
                StringBuilder sb = new StringBuilder();
                fill(sb);
                StringBuilder same = sb.append(2);
                System.out.println(sb.length() + \" \" + same);
            }
        }",
    );
    // A chain is Rust's chain of `&Self`; only where the builder is a value is the handle cloned.
    assert_contains(
        &rust,
        &[
            "fn fill(sb: java::StringBuilder)",
            "sb.append(1).append_char(',' as u16).append_str(\"x\");",
            "let sb = java::StringBuilder::new();",
            "fill(sb.clone());",
            "let same = sb.append(2).clone();",
        ],
    );
}

#[test]
fn string_methods_call_the_runtime_with_borrowed_text() {
    let rust = translate(
        "class A {
            static int count(String text, String part) {
                return text.isEmpty() || !text.contains(part) ? 0 : text.split(part).length;
            }
            public static void main(String[] a) {
                String s = \"a,b\";
                System.out.println(s.substring(1) + s.indexOf(',') + count(s, \",\") + s.toUpperCase());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "text.is_empty() || !text.contains(part)",
            "java::split(text, part, 0)",
            "java::substring_from(&s, 1)",
            "java::index_of_char(&s, ',' as i32, 0)",
            "java::uppercase(&s)",
        ],
    );
}

#[test]
fn format_arguments_keep_their_java_types() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            long big = 1L << 40;
            System.out.printf(\"%d %s %.2f%n\", big, \"x\", 1.5);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "java::print!(\"{}\", java::format(\"%d %s %.2f%n\", &[java::Arg::Long(big), java::Arg::Str(\"x\"), \
           java::Arg::Double(1.5)]));",
        ],
    );
}

#[test]
fn unsupported_regular_expressions_and_formats_are_rejected() {
    let cases = [
        (
            "\"axb\".split(\"(?=x)\").length",
            "lookaround, flags and named groups in the regular expression \"(?=x)\" not supported yet",
        ),
        (
            "\"ab\".replaceAll(\"\\\\bb\", \"c\").length()",
            "the escape \\b in the regular expression \"\\bb\" not supported yet",
        ),
        ("String.format(\"%tY\", 5L).length()", "dates in a format (%t) not supported yet: \"%tY\""),
    ];
    for (expr, message) in cases {
        let java =
            format!("class A {{ public static void main(String[] a) {{ System.out.println({expr}); }} }}");
        let err = java_to_rust::translate(&java).expect_err("must be rejected");
        assert!(err.to_string().contains(message), "{err}");
    }
    // A malformed pattern is not rejected: it throws when the program runs, as in Java.
    translate(
        "class A { public static void main(String[] a) { System.out.println(\"a(b\".split(\"(\").length); } }",
    );
}

#[test]
fn constant_case_labels_and_enum_switches_match_directly() {
    let rust = translate(
        "class A {
            static final int LIMIT = 10;
            enum Dir { UP, DOWN }
            static int f(int n) {
                switch (n) { case LIMIT: return 1; case LIMIT + 1: return 2; default: return 0; }
            }
            static String g(Dir d) {
                switch (d) { case UP: return \"up\"; case DOWN: return \"down\"; }
                return \"\";
            }
            public static void main(String[] a) { System.out.println(f(3) + g(Dir.UP) + g(Dir.DOWN)); }
        }",
    );
    // A constant of the selector's type is matched by name, a folded expression as a literal, and
    // a switch naming every constant of an enum has no `_` arm.
    assert_contains(&rust, &["A_LIMIT => {", "11 => {", "ADir::Up => {", "ADir::Down => {"]);
    assert_lacks(&rust, &["_ => {}"]);
}

#[test]
fn initializer_blocks_run_in_constructors() {
    let rust = translate(
        "class C { int n; { n = 3; } C() {} C(int k) { this(); n += k; } }
        class A { public static void main(String[] a) { System.out.println(new C(2).n); } }",
    );
    // The block becomes part of the struct literal of `C()`; `C(int)` runs it through `C()`.
    assert_contains(&rust, &["C { n: 3 }", "let mut this = C::new();"]);
    let err = java_to_rust::translate(
        "class C { { System.out.println(1); } int n = 2; }
        class A { public static void main(String[] a) { System.out.println(new C().n); } }",
    )
    .expect_err("a field initializer after a block must be rejected");
    assert!(
        err.to_string().contains("a field initializer after an instance initializer block not supported yet")
    );
}
