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
fn a_class_without_to_string_prints_its_name_and_hash() {
    let rust = translate(
        "class P { int x; void bump() { x++; } }
        class A { public static void main(String[] a) { P p = new P(); p.bump(); System.out.println(p); } }",
    );
    // `Object.toString`: the binary name and the identity hash, from the struct's address.
    assert_contains(
        &rust,
        &["java::println!(\"{}\", java::default_text(\"P\", java::address_hash(std::ptr::from_ref(&p))));"],
    );
}

const SHAPES: &str = "
    abstract class Shape {
        private final String name;
        protected int moves;
        Shape(String name) { this.name = name; }
        String name() { return name; }
        void move() { moves++; }
        abstract double area();
        String describe() { return name + \" \" + area(); }
    }
    class Circle extends Shape {
        private final double radius;
        Circle(double radius) { super(\"circle\"); this.radius = radius; }
        @Override double area() { return 3.0 * radius * radius; }
    }
    class A { public static void main(String[] a) {
        Circle c = new Circle(2.0);
        Shape s = c;
        s.move();
        System.out.println(s.name() + \" \" + c.describe() + \" \" + (s == c));
    } }";

#[test]
fn superclass_fields_are_embedded() {
    let rust = translate(SHAPES);
    assert_contains(
        &rust,
        &[
            "struct ShapeFields {\n    name: String,\n    moves: i32,\n}",
            "struct Circle {\n    base: ShapeFields,\n    radius: f64,\n}",
        ],
    );
    // A value of the superclass's type is the one class's object that it can be.
    assert_contains(&rust, &["let s = c.clone();", "Rc::ptr_eq(&s, &c)"]);
    assert_lacks(&rust, &["struct Shape {", "dyn "]);
}

#[test]
fn inherited_field_methods_live_on_the_fields_struct() {
    let rust = translate(SHAPES);
    assert_contains(
        &rust,
        &[
            "impl ShapeFields {",
            "fn name(&self) -> String {",
            "fn move_(&mut self) {",
            "s.borrow_mut().base.move_();",
        ],
    );
    // `describe` calls `area`, which a subclass implements, so it runs on the subclass's object.
    assert_contains(
        &rust,
        &["impl Circle {", "fn describe(&self) -> String {", "self.base.name", "self.area()"],
    );
}

#[test]
fn abstract_methods_are_not_emitted() {
    let rust = translate(SHAPES);
    assert_eq!(rust.matches("fn area(").count(), 1, "{rust}");
}

#[test]
fn a_subclass_constructor_initializes_the_superclass_first() {
    let rust = translate(SHAPES);
    assert_contains(
        &rust,
        &["fn new(name: &str) -> ShapeFields {", "Circle { base: ShapeFields::new(\"circle\"), radius }"],
    );
}

#[test]
fn super_calls_are_non_virtual() {
    let rust = translate(
        "abstract class Account {
            long cents;
            boolean withdraw(long amount) { if (amount > available()) return false; cents -= amount; return true; }
            void deposit(long amount) { cents += amount; }
            abstract long available();
        }
        class Checking extends Account {
            int withdrawals;
            @Override long available() { return cents + 100; }
            @Override boolean withdraw(long amount) { boolean done = super.withdraw(amount); if (done) withdrawals++; return done; }
            @Override void deposit(long amount) { super.deposit(amount + 1); }
        }
        class A { public static void main(String[] a) {
            Checking c = new Checking();
            Account x = c;
            x.deposit(5);
            System.out.println(x.withdraw(50) + \" \" + c.cents + \" \" + c.withdrawals);
        } }",
    );
    // The superclass's `withdraw` needs the whole object (it calls `available`), so it is emitted
    // for the subclass under a name of its own; its `deposit` lives on the fields struct.
    assert_contains(
        &rust,
        &[
            "let done = self.withdraw_in_account(amount);",
            "fn withdraw_in_account(&mut self, amount: i64) -> bool {",
            "self.base.deposit(amount.wrapping_add(1));",
        ],
    );
}

#[test]
fn a_superclass_constructor_that_calls_an_override_runs_on_the_whole_object() {
    let rust = translate(
        "abstract class Base { int x; Base() { x = size(); } abstract int size(); }
        class Impl extends Base { @Override int size() { return 3; } }
        class A { public static void main(String[] a) { System.out.println(new Impl().x); } }",
    );
    // The object is built with every field zero, then the superclass's constructor body runs on
    // it as a method, where `size()` is the subclass's.
    assert_contains(
        &rust,
        &[
            "let mut this = Impl { base: BaseFields::new() };",
            "this.construct_base();",
            "fn construct_base(&mut self) {",
            "self.base.x = self.size();",
        ],
    );
}

#[test]
fn superclass_static_initialization_runs_first() {
    let rust = translate(
        "abstract class Base {
            static final String NAME = make(\"base\");
            static String make(String s) { System.out.println(s); return s; }
        }
        class Sub extends Base {
            static String other = make(\"sub\");
            String both() { return NAME + other; }
        }
        class A { public static void main(String[] a) { System.out.println(new Sub().both()); } }",
    );
    assert_contains(&rust, &["fn init_sub() {", "    init_base();\n    SUB_OTHER.set(make(\"sub\"));"]);
}

const TREE: &str = "
    abstract class Base {
        String name;
        int uses;
        Base(String name) { this.name = name; }
        String getName() { uses++; return name; }
        abstract double area();
        public String toString() { return name + \":\" + area(); }
    }
    class Circle extends Base {
        final double r;
        Circle(double r) { super(\"circle\"); this.r = r; }
        double area() { return 3.0 * r * r; }
        public String toString() { return \"C\" + super.toString(); }
    }
    class Rect extends Base {
        double w, h;
        Rect(double w, double h) { super(\"rect\"); this.w = w; this.h = h; }
        double area() { return w * h; }
    }
    class A { public static void main(String[] a) {
        Circle c = new Circle(1.5);
        Base b = c;
        Base r = new Rect(2, 3);
        System.out.println(b.area() + r.area() + \" \" + b.getName() + \" \" + (b == r) + \" \" + b.name);
        System.out.println(b == c);
        Base last = c;
        System.out.println(r + \" \" + last);
    } }";

#[test]
fn a_type_with_several_concrete_classes_is_an_enum() {
    let rust = translate(TREE);
    assert_contains(
        &rust,
        &[
            "#[derive(Clone)]\nenum Base {\n    Circle(Rc<RefCell<Circle>>),\n    Rect(Rc<RefCell<Rect>>),\n}",
            "    fn area(&self) -> f64 {
        match self {
            Base::Circle(o) => o.borrow().area(),
            Base::Rect(o) => o.borrow().area(),
        }
    }",
            "fn base(&self) -> Ref<'_, BaseFields> {",
            "Base::Circle(o) => Ref::map(o.borrow(), |o| &o.base),",
            // A method that only touches the fields runs on the fields struct of each object.
            "Base::Circle(o) => o.borrow_mut().base.get_name(),",
            "b.base().name",
        ],
    );
    assert_lacks(&rust, &["dyn ", "Box<"]);
}

#[test]
fn inherited_bodies_that_dispatch_live_on_the_enum() {
    let rust = translate(TREE);
    assert_contains(
        &rust,
        &[
            "impl Base {",
            "fn to_string_in_base(&self) -> String {",
            "format!(\"{}:{}\", self.base().name.clone(), java::Double(self.area()))",
            // `super.toString()` wraps the subclass's object in the enum to call the body there.
            "Base::Circle(this.clone()).to_string_in_base()",
            "impl fmt::Display for Base {",
            "Base::Rect(_) => f.write_str(&self.to_string_in_base()),",
        ],
    );
}

#[test]
fn upcasts_wrap_the_handle() {
    let rust = translate(TREE);
    assert_contains(
        &rust,
        &[
            "let b = Base::Circle(c.clone());",
            "let r = Base::Rect(Rc::new(RefCell::new(Rect::new(2.0, 3.0))));",
        ],
    );
    // The last use of `c` moves it into the enum.
    assert_contains(&rust, &["let last = Base::Circle(c);"]);
}

#[test]
fn enum_identity_is_pointer_equality() {
    let rust = translate(TREE);
    assert_contains(
        &rust,
        &[
            "impl PartialEq for Base {",
            "(Base::Circle(a), Base::Circle(b)) => Rc::ptr_eq(a, b),",
            "_ => false,",
            "&b == &r",
            "&b == &Base::Circle(c.clone())",
        ],
    );
}

#[test]
fn an_immutable_family_has_no_refcell() {
    let rust = translate(
        "abstract class N { final int v; N(int v) { this.v = v; } abstract int eval(); }
        class Leaf extends N { Leaf(int v) { super(v); } int eval() { return v; } }
        class Twice extends N { final N inner; Twice(N inner) { super(0); this.inner = inner; } int eval() { return 2 * inner.eval() + inner.v; } }
        class A { public static void main(String[] a) {
            N n = new Twice(new Twice(new Leaf(3)));
            System.out.println(n.eval() + n.v);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "enum N {\n    Leaf(Rc<Leaf>),\n    Twice(Rc<Twice>),\n}",
            "fn base(&self) -> &NFields {",
            "N::Leaf(o) => o.eval(),",
            "self.inner.base().v",
        ],
    );
    assert_lacks(&rust, &["RefCell", "borrow"]);
}

#[test]
fn a_class_that_is_never_upcast_stays_a_plain_value_beside_its_siblings() {
    let rust = translate(
        "abstract class Shape { int moves; abstract int sides(); void move() { moves++; } }
        class Square extends Shape { int sides() { return 4; } }
        class Tri extends Shape { int sides() { return 3; } }
        class A { public static void main(String[] a) {
            Shape s = new Square();
            Tri t = new Tri();
            t.move();
            System.out.println(s.sides() + t.sides() + t.moves);
        } }",
    );
    // `Tri` is only ever a `Tri`, so it is an owned value; only `Square` is a `Shape`.
    assert_contains(&rust, &["let mut t = Tri::new();", "t.base.move_();", "t.base.moves"]);
    // With one class left, the type is that class: no enum.
    assert_contains(&rust, &["let s = Rc::new(RefCell::new(Square::new()));"]);
    assert_lacks(&rust, &["enum Shape"]);
}

#[test]
fn a_class_that_is_upcast_is_shared() {
    let rust = translate(
        "abstract class Shape { abstract int sides(); }
        class Square extends Shape { int sides() { return 4; } }
        class Tri extends Shape { int sides() { return 3; } }
        class A {
            static int total(Shape a, Shape b) { return a.sides() + b.sides(); }
            public static void main(String[] a) {
                Square s = new Square();
                Tri t = new Tri();
                System.out.println(total(s, t) + s.sides());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "enum Shape {\n    Square(Rc<Square>),\n    Tri(Rc<Tri>),\n}",
            "let s = Rc::new(Square::new());",
            "total(&Shape::Square(s.clone()), &Shape::Tri(t))",
        ],
    );
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
    // All three are taken into the literal, whose values are in the order of the assignments;
    // `a` is assigned by a call after `c`'s, and the calls run first, in Java's order.
    assert_contains(
        &rust,
        &["let c = next();\n        let a = next();\n        let this = P { b: x, c, a };", "        this\n"],
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
    assert_contains(&rust, &["    made\n", "let b: Vec<i32> = a;"]);
    assert_lacks(&rust, &["made.clone()", "a.clone()"]);
}

#[test]
fn main_args_are_the_command_line_arguments() {
    let plain = translate(
        "class A {
            public static void main(String[] args) { System.out.println(args.length); }
        }",
    );
    assert_contains(&plain, &["let args: Vec<String> = java::main_args();"]);
    let shared = translate(
        "class A {
            static String[] kept;
            public static void main(String[] args) { kept = args; System.out.println(kept.length); }
        }",
    );
    assert_contains(&shared, &["let args: java::Array<String> = java::from_vec(java::main_args());"]);
    let nullable = translate(
        "class A {
            public static void main(String[] args) { args[0] = null; System.out.println(args.length); }
        }",
    );
    assert_contains(&nullable, &["java::main_args().into_iter().map(Some).collect::<Vec<_>>()"]);
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
fn object_varargs_are_a_slice_of_the_object_enum() {
    let rust = translate(
        "class A {
            static void log(String f, Object... xs) { System.out.println(f + xs.length + xs[1]); }
            public static void main(String[] a) { log(\"x\", 1, \"two\"); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn log(f: &str, xs: &[Object]) {",
            "log(\"x\", &[Object::Integer(1), Object::String(String::from(\"two\"))]);",
        ],
    );
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
fn anonymous_classes_are_reached_through_their_creation_and_calls() {
    let rust = translate_project("anonymous", "app.Main");
    assert_contains(&rust, &["fn run(", "fn area(", "write!(f, \"task {}\", self.name)"]);
    // `audit` and `describe` are never called, nor is `never`, whose anonymous class uses `extra`.
    assert_lacks(&rust, &["extra", "Audit", "audit", "describe", "fn never", "\"never\""]);
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

#[test]
fn objects_created_from_a_class_name_are_of_the_classes_that_code_names() {
    let java = "package app;\n\
        abstract class Shape { abstract String name(); }\n\
        class Circle extends Shape { String name() { return \"circle\"; } }\n\
        class Square extends Shape { String name() { return \"square\"; } }\n\
        public class Main {\n\
            static Shape make(String n) throws Exception {\n\
                return (Shape) Class.forName(n).getDeclaredConstructor().newInstance();\n\
            }\n\
            public static void main(String[] a) throws Exception {\n\
                System.out.println(make(a.length > 0 ? a[0] : \"app.Circle\").name());\n\
            }\n\
        }\n";
    let dir = write_sources("by-name", &[("app/Main.java", java)]);
    let program = java_to_rust::Program { main: "app.Main".into(), sources: vec![dir], ..Default::default() };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &[
            "\"app.Circle\" => {",
            "Rc::new(Circle::new());",
            "java::throw_uncaught(format!(\"java.lang.ClassNotFoundException: {} is not a class of the translated program\"",
        ],
    );
    // No code names `Square`, so no name can create one.
    assert_lacks(&rust, &["Square", "square"]);
}

#[test]
fn class_for_name_of_a_spelled_out_name_initializes_that_class() {
    let registry = "package app;\n\
        class Registry {\n\
            static { Main.value = \"registered\"; System.out.println(\"registry\"); }\n\
        }\n";
    let java = "package app;\n\
        public class Main {\n\
            static String value;\n\
            public static void main(String[] a) {\n\
                try {\n\
                    Class.forName(\"app.Registry\", true, null);\n\
                } catch (ClassNotFoundException e) {}\n\
                System.out.println(value);\n\
            }\n\
        }\n";
    let dir = write_sources("for-name", &[("app/Main.java", java), ("app/Registry.java", registry)]);
    let program = java_to_rust::Program { main: "app.Main".into(), sources: vec![dir], ..Default::default() };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    // Only the string names `Registry`, and the call runs its static initializer.
    assert_contains(&rust, &["init_registry();", "\"registry\""]);
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
fn library_code_compares_strings_by_value_and_the_programs_own_code_cannot() {
    let library = (
        "lib/lib/Tags.java",
        "package lib;\npublic class Tags {\n    public static boolean none(String tag) { return tag == \"\"; }\n    public static boolean differ(String a, String b) { return a != b; }\n}\n",
    );
    let main = "package main;\nimport lib.Tags;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(Tags.none(\"\") + \" \" + Tags.differ(\"x\", a.length > 0 ? a[0] : \"y\"));\n    }\n}\n";
    let dir = write_sources("library-string-equality", &[("app/main/Main.java", main), library]);
    let program = java_to_rust::Program {
        main: "main.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    // Value equality, `Objects.equals`, of the two strings.
    assert_contains(&rust, &["tag == \"\"", "!(a == b)"]);
    // The same comparison in the program's own code compares references, which it cannot.
    let own = "package main;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(a.length > 0 && a[0] == \"x\");\n    }\n}\n";
    let dir = write_sources("program-string-equality", &[("app/main/Main.java", own)]);
    let program = java_to_rust::Program {
        main: "main.Main".into(),
        sources: vec![dir.join("app")],
        ..Default::default()
    };
    let err = java_to_rust::translate_program(&program).expect_err("== on the program's strings");
    assert!(err.to_string().contains("== on strings compares references; not supported yet"), "{err}");
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
                "package p;\nclass Other {\n    static void run() {\n        ProcessBuilder t = null;\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let error =
        java_to_rust::translate_program(&program).expect_err("ProcessBuilder is not supported").to_string();
    assert!(error.ends_with("Other.java:4:9: type `java.lang.ProcessBuilder` not supported yet"), "{error}");
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
            "java::address_hash(Rc::as_ptr(&x)) == java::address_hash(Rc::as_ptr(&x))",
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
            "_ => java::throw(\"java.lang.IllegalArgumentException\", &format!(\"No enum constant A.Color.{name}\")),",
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
fn an_enum_element_that_may_be_null_is_an_option_and_used_as_a_value_is_rejected() {
    let rust = translate(
        "public class A {
            enum Kind { TOP, DEAD }
            public static void main(String[] args) {
                Kind[] kinds = new Kind[2];
                kinds[1] = Kind.DEAD;
                System.out.println(kinds[0] + \" \" + (kinds[1] == Kind.DEAD));
            }
        }",
    );
    assert_contains(&rust, &["vec![None; 2]", "kinds[1] == Some(AKind::Dead)"]);
    for use_ in ["kinds[0].ordinal()", "switch (kinds[0]) { default -> 0; }", "take(kinds[0])"] {
        let error = java_to_rust::translate(&format!(
            "public class A {{
                enum Kind {{ TOP, DEAD }}
                static int take(Kind k) {{ return 1; }}
                public static void main(String[] args) {{
                    Kind[] kinds = new Kind[2];
                    System.out.println({use_});
                }}
            }}"
        ))
        .expect_err("an element that may be null used as an enum value");
        assert!(error.to_string().contains("null enums not supported yet"), "{use_}: {error}");
    }
}

#[test]
fn a_catch_right_around_the_text_of_an_object_whose_to_string_throws_is_rejected() {
    // The text is made with `Display`, which cannot pass the exception on to the `catch`.
    let error = java_to_rust::translate(
        "public class A {
            static class Item {
                public String toString() { throw new IllegalStateException(\"no text\"); }
            }
            public static void main(String[] args) {
                try {
                    System.out.println(\"item \" + new Item());
                } catch (IllegalStateException e) {
                    System.out.println(\"caught\");
                }
            }
        }",
    )
    .expect_err("a toString whose exception a catch around the text sees");
    assert!(
        error.to_string().ends_with("catching an exception that this toString may throw not supported yet"),
        "{error}"
    );
}

#[test]
fn a_catch_on_the_way_to_the_text_of_an_object_whose_to_string_throws_is_rejected() {
    // The `catch` is around a call of the method that makes the text.
    let error = java_to_rust::translate(
        "public class A {
            static class Item {
                public String toString() { throw new IllegalStateException(\"no text\"); }
            }
            static void show(Item item) { System.out.println(\"item \" + item); }
            public static void main(String[] args) {
                try {
                    show(new Item());
                } catch (IllegalStateException e) {
                    System.out.println(\"caught\");
                }
            }
        }",
    )
    .expect_err("a toString whose exception a caller's catch sees");
    assert!(
        error.to_string().ends_with("catching an exception that this toString may throw not supported yet"),
        "{error}"
    );
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
    assert_contains(&report, &["Reachable from Main: 6 classes (3 instantiated), 7 methods"]);
    // The program's own interfaces and inner classes translate.
    assert_lacks(&report, &["Square:", "Tag", "interfaces", "inner classes"]);
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
        "    static Runnable a() { return new StringBuilder()::reverse; }",
        "    static Runnable b() { return new StringBuilder()::reverse; }",
        "    static Runnable c() { return new StringBuilder()::reverse; }",
        "    static Runnable d() { return new StringBuilder()::reverse; }",
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
            "app/Main.java",
            "package app;
            public class Main {
                public static void main(String[] args) {
                    enum Local { A; int n() { return 2; } }
                    Object anon = new Object() { };
                    System.out.println(Local.A.n());
                }
            }",
        )],
    );
    let program = java_to_rust::Program { main: "app.Main".into(), sources: vec![dir], ..Default::default() };
    let report = survey(&program).report;
    // The local enum of a file with a package is hoisted to the top level, so it is no feature of
    // the method that declares it; its calls are not calls to classes outside the sources.
    let table = feature_table(&report, "Program code: features");
    assert!(!table.contains("local interfaces, enums and records"), "{table}");
    assert!(!table.contains("anonymous classes"), "{table}");
    assert!(!table.contains("classes outside"), "{table}");
    assert_lacks(&report, &["(outside)"]);
}

#[test]
fn the_survey_counts_only_a_for_each_over_a_type_outside_the_sources() {
    let dir = write_sources(
        "survey_for_each",
        &[(
            "Main.java",
            "import java.util.Iterator;
            public class Main implements Iterable<String> {
                public Iterator<String> iterator() { return null; }
                static void own(Main m) { for (String s : m) System.out.println(s); }
                static void jdk(java.util.List<String> l) { for (String s : l) System.out.println(s); }
                public static void main(String[] args) { own(new Main()); jdk(null); }
            }",
        )],
    );
    let program = java_to_rust::Program { main: "Main".into(), sources: vec![dir], ..Default::default() };
    let report = survey(&program).report;
    let table = feature_table(&report, "Program code: features");
    let row = feature_row(table, "for-each over types outside the sources");
    assert!(row.contains("Main.java:5") && !row.contains("Main.java:4"), "{table}");
}

#[test]
fn the_survey_counts_only_values_of_generic_types_outside_the_sources() {
    let dir = write_sources(
        "survey_generic_types",
        &[(
            "Main.java",
            "public class Main {
                static <T> Box<T> box(T t) { return new Box<>(t); }
                static int size(Box<?> b) { return 1; }
                static Class<?> kind() { return null; }
                static int a() { return size(box(\"a\")); }
                static Object b() { return kind(); }
                static void c(java.util.List<String> l) { System.out.println(l); }
                public static void main(String[] args) { a(); b(); c(null); }
            }
            class Box<T> { T v; Box(T v) { this.v = v; } }",
        )],
    );
    let program = java_to_rust::Program { main: "Main".into(), sources: vec![dir], ..Default::default() };
    let report = survey(&program).report;
    let table = feature_table(&report, "Program code: features");
    // The names of `size` and `box` have method types (`(Box<?>)int`, `<T>(T)Box<T>`), which are
    // not values; a `Class<?>` is the runtime's class; only the `List<String>` is outside.
    let row = feature_row(table, "generic types");
    assert!(
        row.contains("Main.java:7") && !row.contains("Main.java:5") && !row.contains("Main.java:6"),
        "{table}"
    );
}

#[test]
fn the_survey_reports_the_program_and_its_libraries_in_separate_tables() {
    let dir = write_sources(
        "survey_split",
        &[(
            "app/Main.java",
            "package app;
            public class Main {
                public static void main(String[] args) {
                    enum Local { A; int n() { return 1; } }
                    System.out.println(Helper.run() + Local.A.n());
                }
            }",
        )],
    );
    let lib = write_sources(
        "survey_split_lib",
        &[(
            "app/Helper.java",
            "package app; public class Helper { static int run() { Runnable r = new StringBuilder()::reverse; r.run(); return 1; } }",
        )],
    );
    let program = java_to_rust::Program {
        main: "app.Main".into(),
        sources: vec![dir],
        libraries: vec![lib.clone()],
        ..Default::default()
    };
    let report = survey(&program).report;
    let program_table = feature_table(&report, "Program code: features");
    let library_table = feature_table(&report, "Library code: features");
    assert!(report.find("Program code:").unwrap() < report.find("Library code:").unwrap(), "{report}");
    // The local enum is hoisted, so the program has no blocker of its own.
    assert!(!program_table.contains("local interfaces"), "{program_table}");
    assert!(!program_table.contains("computed method receivers"), "{program_table}");
    // The library's example is relative to its root, not the absolute path.
    let row = feature_row(library_table, "computed method receivers");
    assert!(row.contains("Helper.java:1"), "{row}");
    assert!(!row.contains(lib.to_str().unwrap()), "{row}");
    assert!(!library_table.contains("local interfaces"), "{library_table}");
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
    // Four pieces use a computed receiver; the first three are named, relative to the source root.
    assert!(
        feature_row(table, "computed method receivers")
            .ends_with("app/Main.java:3, app/Main.java:4, app/Main.java:5"),
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
    assert_eq!(a["features"]["computed method receivers"], "app/Main.java:3");
    assert!(json["runtime_calls"].is_array(), "{json}");
    assert!(json["natives"].is_array(), "{json}");
}

#[test]
fn the_survey_names_the_shortest_chain_of_calls_to_each_native_method() {
    let dir = write_sources(
        "survey_chain",
        &[(
            "Main.java",
            "public class Main {
                public static void main(String[] args) {
                    Counter c = new Counter();
                    System.out.println(c.toString() + Clock.now());
                }
            }",
        )],
    );
    // `Clock.now` reaches `raw` directly and through `slow`, and `stamp` is called by the static
    // initializer of `Clock`, which any use of a static member runs.
    let lib = write_sources(
        "survey_chain_lib",
        &[
            (
                "Clock.java",
                "public class Clock {
                    static final long START = stamp();
                    static long now() { return slow() + raw(); }
                    static long slow() { return raw(); }
                    static native long raw();
                    static native long stamp();
                }",
            ),
            (
                "Counter.java",
                "public class Counter { public String toString() { return \"\" + Clock.START; } }",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir],
        libraries: vec![lib],
        ..Default::default()
    };
    let census = survey(&program);
    assert_contains(
        &census.report,
        &[
            "  Clock.raw()long\n      via Main.main -> Clock.now -> Clock.raw\n",
            "  Clock.stamp()long\n      via Main.main -> Clock.<clinit> -> Clock.stamp\n",
        ],
    );
    let natives = census.json["natives"].as_array().expect("natives");
    let raw = natives.iter().find(|n| n["method"] == "Clock.raw()long").expect("raw is reached");
    assert_eq!(raw["chain"], serde_json::json!(["Main.main", "Clock.now", "Clock.raw"]));
    // Calls into the runtime's classes are listed with their chains too.
    let external = census.json["external"].as_array().expect("external");
    let println = external
        .iter()
        .find(|e| e["method"] == "java.io.PrintStream.println(java.lang.String)void")
        .expect("println is called");
    assert_eq!(println["chain"], serde_json::json!(["Main.main", "java.io.PrintStream.println"]));
    // So is each class whose objects are created, with the chain to its constructor.
    let instantiated = census.json["instantiated"].as_array().expect("instantiated");
    let counter = instantiated.iter().find(|c| c["class"] == "Counter").expect("Counter is created");
    assert_eq!(counter["chain"], serde_json::json!(["Main.main", "Counter.<init>"]));
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
fn an_enum_field_assigned_outside_its_constructor_is_a_table() {
    let rust = java_to_rust::translate(
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
    .expect("an enum field that changes is a table");
    assert!(rust.contains("fn init_planet_constant(constant_ordinal: i32, mass: f64)"), "{rust}");
    assert!(rust.contains("init_planet_constant(0, 3.0);"), "{rust}");
    assert!(rust.contains("A_PLANET_MASS_OF_CONSTANTS"), "{rust}");
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

#[test]
fn intern_is_the_string_itself_and_string_identity_stays_rejected() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            String s = \"x\" + a.length;
            String t = s.intern();
            System.out.println(t + s.intern().length());
        } }",
    );
    assert_contains(&rust, &["let t = s.clone();", "s.len() as i32"]);
    // A string is a value in the translation, so `==` on two strings, which compares references
    // in Java (and is what `intern` exists for), is rejected rather than guessed.
    let err = java_to_rust::translate(
        "class A { public static void main(String[] a) { String s = \"x\"; System.out.println(s.intern() == \"x\"); } }",
    )
    .expect_err("== on strings must be rejected");
    assert!(err.to_string().contains("== on strings compares references; not supported yet"), "{err}");
}

/// Stand-ins for JDK classes with natives, in packages that java.base does not export, so that
/// javac reads them from the library root rather than from the JDK.
const NATIVE_STAND_INS: &[(&str, &str)] = &[
    (
        "lib/jdk/internal/misc/CDS.java",
        "package jdk.internal.misc;
        public class CDS {
            private static final int configStatus = getCDSConfigStatus();
            private static native int getCDSConfigStatus();
            public static native long getRandomSeedForDumping();
            public static native void initializeFromArchive(Class<?> c);
            static { initializeFromArchive(CDS.class); }
            public static boolean usingArchive() { return (configStatus & 2) != 0; }
        }",
    ),
    (
        "lib/jdk/internal/misc/Unsafe.java",
        "package jdk.internal.misc;
        public final class Unsafe {
            private static native void registerNatives();
            static { registerNatives(); }
            private Unsafe() {}
            private static final Unsafe theUnsafe = new Unsafe();
            public static Unsafe getUnsafe() { return theUnsafe; }
            public long arrayBaseOffset(Class<?> c) {
                if (c == null) { throw new NullPointerException(); }
                return arrayBaseOffset0(c);
            }
            public int arrayIndexScale(Class<?> c) {
                if (c == null) { throw new NullPointerException(); }
                return arrayIndexScale0(c);
            }
            public static final long ARRAY_INT_BASE_OFFSET = theUnsafe.arrayBaseOffset(int[].class);
            public static final int ARRAY_BOOLEAN_INDEX_SCALE = theUnsafe.arrayIndexScale(boolean[].class);
            public static final int ARRAY_OBJECT_INDEX_SCALE = getUnsafe().arrayIndexScale(String[][].class);
            private native int arrayBaseOffset0(Class<?> c);
            private native int arrayIndexScale0(Class<?> c);
            public static native long getuid();
            public static long badOffset() { return theUnsafe.arrayBaseOffset(new int[0].getClass()); }
            public static int badScale() { return theUnsafe.arrayIndexScale(String.class); }
            public long objectFieldOffset(Class<?> c, String name) {
                if (c == null || name == null) { throw new NullPointerException(); }
                return objectFieldOffset1(c, name);
            }
            private native long objectFieldOffset1(Class<?> c, String name);
            public native Object getReferenceVolatile(Object o, long offset);
            public native void putReferenceVolatile(Object o, long offset, Object x);
            public final Object getReferenceAcquire(Object o, long offset) { return getReferenceVolatile(o, offset); }
            public final void putReferenceRelease(Object o, long offset, Object x) { putReferenceVolatile(o, offset, x); }
            public native int getInt(Object o, long offset);
            public native void putIntVolatile(Object o, long offset, int x);
        }",
    ),
    (
        "lib/jdk/internal/misc/Lazy.java",
        "package jdk.internal.misc;
        public final class Lazy {
            private static final Unsafe U = Unsafe.getUnsafe();
            private static final long VALUE = U.objectFieldOffset(Lazy.class, \"value\");
            private static final long COUNT = U.objectFieldOffset(Lazy.class, \"count\");
            private String value;
            private int count;
            public String get() {
                String v = (String) U.getReferenceAcquire(this, VALUE);
                if (v == null) {
                    v = \"computed\";
                    U.putReferenceRelease(this, VALUE, v);
                    U.putIntVolatile(this, COUNT, U.getInt(this, COUNT) + 1);
                }
                return v;
            }
            public int count() { return count; }
            public static long offset() { return VALUE; }
        }",
    ),
    (
        "lib/jdk/internal/misc/VM.java",
        "package jdk.internal.misc;
        public class VM {
            static { initialize(); }
            private static native void initialize();
            public static native long getNanoTimeAdjustment(long offsetInSeconds);
        }",
    ),
    (
        "lib/jdk/internal/vm/ContinuationSupport.java",
        "package jdk.internal.vm;
        public class ContinuationSupport {
            private static final boolean SUPPORTED = isSupported0();
            public static boolean isSupported() { return SUPPORTED; }
            private static native boolean isSupported0();
        }",
    ),
];

/// Translates a `main` of these statements over the native stand-ins.
fn translate_with_natives(dir: &str, statements: &str) -> Result<String, String> {
    let main = format!(
        "import jdk.internal.misc.*;\nimport jdk.internal.vm.ContinuationSupport;\npublic class Main {{\n    public static void main(String[] a) {{\n        {statements}\n    }}\n}}\n"
    );
    let mut files = NATIVE_STAND_INS.to_vec();
    files.push(("app/Main.java", &main));
    let dir = write_sources(dir, &files);
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    java_to_rust::translate_program(&program).map_err(|e| e.to_string())
}

#[test]
fn natives_of_jdk_classes_become_runtime_calls() {
    let rust = translate_with_natives(
        "jdk-natives",
        "System.out.println(CDS.usingArchive() + \" \" + CDS.getRandomSeedForDumping());
        System.out.println(Unsafe.ARRAY_INT_BASE_OFFSET + Unsafe.ARRAY_BOOLEAN_INDEX_SCALE + Unsafe.ARRAY_OBJECT_INDEX_SCALE);
        System.out.println(VM.getNanoTimeAdjustment(0) > 0);
        System.out.println(ContinuationSupport.isSupported());",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &[
            "CDS_CONFIG_STATUS.set(java::cds_config_status());",
            "java::cds_random_seed_for_dumping()",
            // `Unsafe`'s array layout, from the class literals; the receiver is not evaluated.
            "UNSAFE_ARRAY_INT_BASE_OFFSET.set(java::array_base_offset());",
            "UNSAFE_ARRAY_BOOLEAN_INDEX_SCALE.set(java::array_index_scale(\"boolean\"));",
            "UNSAFE_ARRAY_OBJECT_INDEX_SCALE.set(java::array_index_scale(\"java.lang.Object\"));",
            "java::nano_time_adjustment(0)",
            "CONTINUATIONSUPPORT_SUPPORTED.set(java::continuations_supported());",
            // `main` initializes `Unsafe` in its package before reading its statics.
            "{ crate::jdk::internal::misc::init_unsafe_(); crate::jdk::internal::misc::UNSAFE_ARRAY_INT_BASE_OFFSET.get() }",
        ],
    );
    // Registering natives, `VM.initialize` and restoring from an archive do nothing, and neither
    // the native declarations nor the methods replaced at their calls are translated.
    assert_lacks(
        &rust,
        &["register_natives", "initialize", "from_archive", "fn array_base_offset", "fn array_index_scale"],
    );
}

#[test]
fn natives_that_only_answer_a_value_cannot_fail_in_a_try() {
    // A `catch` around them sees nothing they raise, as in `ImmutableCollections`'s static
    // initializer, which reads the seed of CDS.
    let rust = translate_with_natives(
        "jdk-natives-caught",
        "try {
            System.out.println(CDS.getRandomSeedForDumping() + VM.getNanoTimeAdjustment(0));
            System.out.println(System.nanoTime() > 0);
        } catch (Throwable t) {
            System.out.println(\"caught\");
        }",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(&rust, &["java::cds_random_seed_for_dumping()"]);
    assert_lacks(&rust, &["java::catching("]);
}

#[test]
fn an_access_through_a_field_offset_is_the_field() {
    let rust = translate_with_natives(
        "jdk-field-handles",
        "Lazy l = new Lazy(); System.out.println(l.get() + l.get() + l.count());",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    // The offsets are no fields, and `Unsafe`'s accessors are not translated.
    assert_lacks(
        &rust,
        &["LAZY_VALUE", "LAZY_COUNT", "field_offset", "fn get_reference_acquire", "fn get_int"],
    );
    assert_contains(&rust, &["self.value", "self.count"]);
}

#[test]
fn natives_the_runtime_lacks_are_rejected_by_name() {
    let rejection =
        |dir: &str, statements: &str| translate_with_natives(dir, statements).expect_err("must be rejected");
    let error = rejection("jdk-natives-missing", "System.out.println(Unsafe.getuid());");
    assert!(
        error.ends_with("native method `jdk.internal.misc.Unsafe.getuid()long` not supported yet"),
        "{error}"
    );
    let error = rejection("jdk-natives-no-literal", "System.out.println(Unsafe.badOffset());");
    assert!(
        error.ends_with("`Unsafe.arrayBaseOffset` of anything but a class literal not supported yet"),
        "{error}"
    );
    let error = rejection("jdk-natives-no-array", "System.out.println(Unsafe.badScale());");
    assert!(
        error.ends_with("`Unsafe.arrayIndexScale` of a class that is not an array not supported yet"),
        "{error}"
    );
    let error = rejection("jdk-natives-offset-value", "System.out.println(Lazy.offset());");
    assert!(error.ends_with("a field offset used as a value not supported yet"), "{error}");
}

const INTERFACES: &str = "
    interface Shape {
        int UNKNOWN = -1;
        double area();
        default String kind() { return \"shape\"; }
        default String describe() { return kind() + \" \" + area(); }
        static Shape larger(Shape a, Shape b) { return a.area() >= b.area() ? a : b; }
    }
    class Circle implements Shape {
        final double r;
        Circle(double r) { this.r = r; }
        public double area() { return 3.0 * r * r; }
        public String kind() { return \"circle\"; }
    }
    class Rect implements Shape {
        double w, h;
        Rect(double w, double h) { this.w = w; this.h = h; }
        public double area() { return w * h; }
    }
    class Square extends Rect {
        Square(double s) { super(s, s); }
    }
    enum Color implements Shape {
        RED, BLUE;
        public double area() { return ordinal(); }
    }
    interface Named { String name(); default String hello(int times) { return \"hello \" + name(); } }
    class Person implements Named {
        public String name() { return \"ann\"; }
    }
    class A { public static void main(String[] a) {
        Rect r = new Rect(2, 3);
        Shape s = Shape.larger(new Circle(1), r);
        Shape sq = new Square(2);
        Shape c = Color.BLUE;
        System.out.println(s.describe() + \" \" + sq.describe() + \" \" + c.describe() + \" \" + (s == r)
            + \" \" + (c == Color.BLUE) + \" \" + Shape.UNKNOWN);
        Named n = new Person();
        System.out.println(n.hello(2) + \" \" + r.area());
    } }";

#[test]
fn an_interface_is_an_enum_over_its_implementors() {
    let rust = translate(INTERFACES);
    assert_contains(
        &rust,
        &[
            "#[derive(Clone)]\nenum Shape {\n    Circle(Rc<Circle>),\n    Rect(Rc<RectFields>),\n    Square(Rc<Square>),\n    Color(Color),\n}",
            "            Shape::Square(o) => o.base.area(),\n            Shape::Color(o) => o.area(),",
            "Shape::Circle(Rc::new(Circle::new(1.0))), Shape::Rect(r.clone())",
            "&s == &Shape::Rect(r.clone())",
        ],
    );
}

#[test]
fn default_methods_live_on_the_interface_enum() {
    let rust = translate(INTERFACES);
    // `kind` is overridden in one class, so the enum has a dispatcher and the default body under
    // its own name; every class runs `describe` itself, which the enum has once.
    assert_contains(
        &rust,
        &[
            "            Shape::Circle(o) => o.kind(),\n            Shape::Rect(_) => self.kind_in_shape(),",
            "    fn kind_in_shape(&self) -> String {\n        String::from(\"shape\")\n    }",
            "    fn describe(&self) -> String {\n        format!(\"{} {}\", self.kind(), java::Double(self.area()))\n    }",
        ],
    );
}

#[test]
fn interface_statics_are_free_functions_and_consts() {
    let rust = translate(INTERFACES);
    assert_contains(
        &rust,
        &["const SHAPE_UNKNOWN: i32 = -1;\n\nfn larger(a: Shape, b: Shape) -> Shape {", "{SHAPE_UNKNOWN}"],
    );
}

#[test]
fn an_enum_constant_implements_an_interface() {
    let rust = translate(INTERFACES);
    // The variant holds the constant itself, which is `Copy`; equal constants are the same.
    assert_contains(
        &rust,
        &[
            "    Color(Color),",
            "(Shape::Color(a), Shape::Color(b)) => a == b,",
            "let c = Shape::Color(Color::Blue);",
            "    fn area(self) -> f64 {",
        ],
    );
}

#[test]
fn an_interface_with_one_implementor_is_that_class() {
    let rust = translate(INTERFACES);
    // `Named` has only `Person`: its default method is a method of `Person`, whose parameter the
    // body ignores.
    assert_contains(
        &rust,
        &["let n = Rc::new(Person::new());", "    fn hello(&self, _times: i32) -> String {"],
    );
    assert_lacks(&rust, &["enum Named"]);
}

#[test]
fn a_class_whose_subclasses_never_become_its_values_is_not_an_enum() {
    let rust = translate(INTERFACES);
    // A `Square` is a `Shape` but never a `Rect`, so a `Rect` is always a `Rect` object.
    assert_contains(&rust, &["let r = Rc::new(RectFields::new(2.0, 3.0));", "java::Double(r.area())"]);
    assert_lacks(&rust, &["enum Rect"]);
}

#[test]
fn interfaces_outside_the_program_are_rejected_except_comparable() {
    let err = java_to_rust::translate(
        "class C implements java.util.EventListener { public void close() {} }
        class A { public static void main(String[] a) { new C().close(); } }",
    )
    .unwrap_err();
    assert!(err.to_string().contains("implementing `java.util.EventListener` not supported yet"), "{err}");
    let rust = translate(
        "class V implements Comparable<V> {
            final int n;
            V(int n) { this.n = n; }
            public int compareTo(V o) { return Integer.compare(n, o.n); }
        }
        class A { public static void main(String[] a) { System.out.println(new V(1).compareTo(new V(2))); } }",
    );
    assert_contains(&rust, &["fn compare_to(&self, o: &Rc<V>) -> i32 {"]);
}

#[test]
fn a_method_whose_exception_is_caught_returns_a_result() {
    let rust = translate(
        "class Missing extends Exception { Missing(String m) { super(m); } }
        class A {
            static int find(int[] xs, int x) throws Missing {
                for (int i = 0; i < xs.length; i++) { if (xs[i] == x) return i; }
                throw new Missing(\"no \" + x);
            }
            static int twice(int[] xs, int x) throws Missing { return find(xs, x) * 2; }
            public static void main(String[] a) {
                int[] xs = {4, 5};
                try {
                    System.out.println(twice(xs, 5));
                    System.out.println(twice(xs, 6));
                } catch (Missing e) {
                    System.out.println(e.getMessage());
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn find(xs: &[i32], x: i32) -> Result<i32, Throwable> {",
            "Err(Throwable::Missing(Rc::new(",
            "fn twice(xs: &[i32], x: i32) -> Result<i32, Throwable> {",
            "find(xs, x)?",
            "let attempt: Result<(), Throwable> = 'attempt: {",
            "java::or_break!(twice(&xs, 5), 'attempt)",
            "Err(Throwable::Missing(e)) => {",
        ],
    );
}

#[test]
fn an_exception_that_nothing_catches_ends_the_program_where_it_is_thrown() {
    let rust = translate(
        "class A {
            static int check(int x) {
                if (x < 0) throw new IllegalArgumentException(\"negative: \" + x);
                return x;
            }
            public static void main(String[] a) {
                System.out.println(check(3));
                System.out.println(check(-3));
            }
        }",
    );
    // Nothing observes the exception, so `check` keeps its plain signature.
    assert_contains(
        &rust,
        &["fn check(x: i32) -> i32 {", "java::throw_uncaught(Throwable::IllegalArgumentException("],
    );
    assert_lacks(&rust, &["Result<", "'attempt"]);
}

#[test]
fn a_call_outside_any_try_of_a_method_that_throws_ends_the_program_on_failure() {
    let rust = translate(
        "class A {
            static int check(int x) {
                if (x < 0) throw new IllegalStateException(\"negative\");
                return x;
            }
            public static void main(String[] a) {
                try { check(-1); } catch (IllegalStateException e) { System.out.println(\"caught\"); }
                System.out.println(check(-2));
            }
        }",
    );
    assert_contains(
        &rust,
        &["fn check(x: i32) -> Result<i32, Throwable> {", "check(-2).unwrap_or_else(java::uncaught)"],
    );
}

#[test]
fn catch_clauses_match_in_order_and_a_try_that_catches_nothing_is_a_block() {
    let rust = translate(
        "class A {
            static void run(int x) {
                if (x == 1) throw new IllegalArgumentException(\"one\");
                if (x == 2) throw new IllegalStateException(\"two\");
                if (x == 3) throw new UnsupportedOperationException(\"three\");
            }
            public static void main(String[] a) {
                for (int i = 0; i < 4; i++) {
                    try {
                        run(i);
                    } catch (IllegalStateException e) {
                        System.out.println(\"state \" + e.getMessage());
                    } catch (RuntimeException e) {
                        System.out.println(\"other \" + e);
                    }
                }
                try { System.out.println(\"quiet\"); } catch (RuntimeException e) { System.out.println(\"never\"); }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "Err(Throwable::IllegalStateException(e)) => {",
            "Err(e @ (Throwable::IllegalArgumentException(_) | Throwable::UnsupportedOperationException(_))) => {",
        ],
    );
    assert_lacks(&rust, &["'attempt2"]);
}

#[test]
fn a_finally_runs_after_the_block_and_then_the_exception_goes_on() {
    let rust = translate(
        "class A {
            static int log = 0;
            static void withdraw(int amount) {
                try {
                    if (amount > 100) throw new IllegalStateException(\"too much\");
                    System.out.println(amount);
                } finally {
                    log++;
                }
            }
            public static void main(String[] a) {
                try { withdraw(500); } catch (IllegalStateException e) { System.out.println(e.getMessage()); }
                try { System.out.println(1); } finally { System.out.println(2); }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn withdraw(amount: i32) -> Result<(), Throwable> {",
            "let attempt: Result<(), Throwable> = 'attempt: {",
            "break 'attempt Err(Throwable::IllegalStateException(",
            "attempt?;",
        ],
    );
    // A `try` that nothing leaves early is its block, then the `finally`.
    assert_contains(&rust, &["{\n        java::println!(\"1\");\n    }\n    java::println!(\"2\");"]);
}

#[test]
fn a_return_or_a_jump_through_a_finally_happens_after_it() {
    let rust = translate(
        "class A {
            static int find(int[] values, int target) {
                try {
                    for (int i = 0; i < values.length; i++) {
                        if (values[i] == target) return i;
                    }
                    return -1;
                } finally {
                    System.out.println(\"searched\");
                }
            }
            public static void main(String[] a) {
                System.out.println(find(new int[] {4, 5}, 5));
                for (int i = 0; i < 5; i++) {
                    try {
                        if (i == 1) continue;
                        if (i == 3) break;
                        System.out.println(i);
                    } finally {
                        System.out.println(\"after \" + i);
                    }
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "let attempt: java::Completion<i32, std::convert::Infallible, std::convert::Infallible> = 'attempt: {",
            "break 'attempt java::Completion::Return(i);",
            "java::Completion::Return(value) => {\n            return value;\n        }",
            "break 'attempt java::Completion::Jump(0);",
            "java::Completion::Jump(0) => {\n                continue;",
            "java::Completion::Jump(_) => {\n                break;",
        ],
    );
}

#[test]
fn a_runtime_failure_that_a_catch_sees_is_a_checked_call_that_throws() {
    let rust = translate(
        "class A {
            static int ratio(int a, int b) { return a / b; }
            public static void main(String[] args) {
                int[] v = new int[2];
                try {
                    v[args.length + 2] = ratio(1, args.length);
                } catch (ArithmeticException e) {
                    System.out.println(\"zero: \" + e.getMessage());
                } catch (ArrayIndexOutOfBoundsException e) {
                    System.out.println(\"out\");
                }
                System.out.println(\"a b\".split(\" +\").length);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn ratio(a: i32, b: i32) -> Result<i32, Throwable> {",
            "java::try_div(a, b)?",
            "java::or_break!(java::try_put(&mut v, ",
            "impl From<java::Exception> for Throwable {",
            "\"java.lang.ArithmeticException\" => ",
            "_ => java::uncaught(e),",
        ],
    );
}

#[test]
fn a_null_dereference_that_a_catch_sees_is_a_checked_non_null() {
    let rust = translate(
        "class A {
            int v;
            A next;
            int get() { return v; }
            static int second(A a) {
                try {
                    return a.next.get() + a.next.v;
                } catch (NullPointerException e) {
                    return -1;
                }
            }
            public static void main(String[] args) {
                A a = new A();
                System.out.println(second(a));
                System.out.println(a.get());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::or_break!(java::try_non_null(a.next.as_ref()), 'attempt).get()",
            "\"java.lang.NullPointerException\" => ",
        ],
    );
    // A dereference that no handler sees keeps the plain check.
    assert_lacks(&rust, &["try_non_null(a.as_ref())"]);
}

#[test]
fn the_message_of_a_null_dereference_is_rejected_and_a_created_ones_is_not() {
    let program = |message: &str| {
        format!(
            "class A {{
                A next;
                int v;
                static int second(A a) {{
                    try {{
                        if (a.v > 0) {{ throw new NullPointerException(\"given\"); }}
                        return a.next.v;
                    }} catch (NullPointerException e) {{
                        System.out.println({message});
                        return -1;
                    }}
                }}
                public static void main(String[] args) {{
                    System.out.println(second(new A()));
                }}
            }}"
        )
    };
    let err = java_to_rust::translate(&program("e.getMessage()")).unwrap_err().to_string();
    assert!(
        err.contains("the message of a `NullPointerException` that the runtime raises not supported yet"),
        "{err}"
    );
    java_to_rust::translate(&program("\"caught\"")).expect("a catch that does not read the message");
}

#[test]
fn an_eager_static_object_starts_empty_and_is_not_null_once_initialized() {
    let rust = translate(
        "class A {
            static class Empty {
                static final Empty INSTANCE = new Empty();
                boolean any() { return false; }
            }
            static int count() {
                try {
                    return Empty.INSTANCE.any() ? 1 : 0;
                } catch (NullPointerException e) {
                    return -1;
                }
            }
            public static void main(String[] args) {
                System.out.println(count());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "static A_EMPTY_INSTANCE: RefCell<Option<Rc<AEmpty>>> = const { RefCell::new(None) };",
            "A_EMPTY_INSTANCE.set(Some(Rc::new(AEmpty::new())));",
            "java::non_null(A_EMPTY_INSTANCE.with_borrow(|v| v.clone()))",
        ],
    );
    assert_lacks(&rust, &["try_non_null"]);
}

#[test]
fn exceptions_the_translation_cannot_reproduce_are_rejected() {
    let reject = |java: &str| java_to_rust::translate(java).unwrap_err().to_string();
    // A failure without a checked form of its own runs inside `java::catching`.
    let rust = translate(
        "class A { public static void main(String[] a) {
            try { int[] v = new int[a.length - 1]; System.out.println(v.length); }
            catch (NegativeArraySizeException e) { System.out.println(\"negative\"); }
        } }",
    );
    assert_contains(&rust, &["java::or_break!(java::catching(|| "]);
    // One whose class is not known is not.
    let err = reject(
        "class A { public static void main(String[] a) {
            try { System.out.println(\"a,b\".split(a[0]).length); }
            catch (RuntimeException e) { System.out.println(\"bad pattern\"); }
        } }",
    );
    assert!(
        err.contains("catching an exception that this call of the library may throw not supported yet"),
        "{err}"
    );
    // A string that may be null is dereferenced with the checked form where a catch sees it.
    let rust = translate(
        "class A { static String s(int n) { return n > 0 ? \"x\" : null; }
            public static void main(String[] a) {
                try { System.out.println(s(a.length).length()); }
                catch (NullPointerException e) { System.out.println(\"null\"); }
            } }",
    );
    assert_contains(
        &rust,
        &["java::or_break!(java::try_non_null(s(a.len() as i32).as_deref()), 'attempt).len()"],
    );
    let err = reject(
        "class Bad extends RuntimeException {
            Bad(int x) { super(\"bad\"); if (x < 0) throw new IllegalStateException(\"negative\"); }
        }
        class A { public static void main(String[] a) {
            try { throw new Bad(-1); } catch (IllegalStateException e) { System.out.println(\"caught\"); }
        } }",
    );
    assert!(err.contains("a constructor of an exception that throws not supported yet"), "{err}");
}

#[test]
fn a_try_with_resources_closes_each_resource_in_a_finally() {
    let rust = translate(
        "class A {
            static class R implements AutoCloseable {
                final String name;
                R(String name) { this.name = name; }
                public void close() { System.out.println(\"close \" + name); }
            }
            public static void main(String[] args) {
                try (R a = new R(\"a\"); R b = new R(\"b\")) {
                    System.out.println(a.name + b.name);
                }
            }
        }",
    );
    // `b` is closed first, then `a`, however the body ends; this `close` cannot throw, so no
    // `addSuppressed` is needed.
    assert_contains(&rust, &["let a = ", "let b = ", "a_failed", "b_failed", ".close();"]);
    assert!(!rust.contains(".add_suppressed("));
    let rust = translate(
        "class A {
            static class R implements AutoCloseable {
                public void close() { throw new IllegalStateException(\"closing\"); }
            }
            public static void main(String[] args) {
                try (R r = new R()) { throw new RuntimeException(\"body\"); }
            }
        }",
    );
    assert_contains(&rust, &["r_failed", "add_suppressed"]);
    let rust = translate(
        "class A {
            static class R implements AutoCloseable { public void close() { System.out.println(2); } }
            public static void main(String[] args) {
                R r = new R();
                try (r) { System.out.println(1); }
            }
        }",
    );
    assert_contains(&rust, &["r_resource", "r_resource_failed", "close()"]);
    let reject = |java: &str| java_to_rust::translate(java).unwrap_err().to_string();
    let err = reject(
        "class A {
            public static void main(String[] args) throws Exception {
                AutoCloseable tick = new java.io.StringWriter();
                try (tick) { System.out.println(2); }
            }
        }",
    );
    assert!(err.contains("not supported yet"), "{err}");
}

#[test]
fn a_constructor_that_throws_returns_a_result_that_new_passes_on() {
    let rust = translate(
        "class A {
            static class Range {
                final int low, high;
                Range(int low, int high) {
                    if (low > high) throw new IllegalArgumentException(\"empty\");
                    this.low = low;
                    this.high = high;
                }
                Range(int high) { this(0, high); }
            }
            public static void main(String[] args) {
                try {
                    Range r = new Range(args.length - 1);
                    System.out.println(r.high - r.low);
                } catch (IllegalArgumentException e) {
                    System.out.println(e.getMessage());
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn new_i32_i32(low: i32, high: i32) -> Result<ARange, Throwable> {",
            "return Err(Throwable::IllegalArgumentException(",
            // A constructor that only delegates returns the other's `Result`.
            "fn new_i32(high: i32) -> Result<ARange, Throwable> {\n        ARange::new_i32_i32(0, high)\n    }",
            "java::or_break!(ARange::new_i32(",
        ],
    );
}

const PATTERNS: &str = "
    interface Shape { double area(); }
    class Circle implements Shape {
        final double r;
        Circle(double r) { this.r = r; }
        public double area() { return 3.0 * r * r; }
    }
    class Rect implements Shape {
        final double w, h;
        Rect(double w, double h) { this.w = w; this.h = h; }
        public double area() { return w * h; }
    }
    enum Unit implements Shape { ONE; public double area() { return 1; } }
    class Main {
        static String show(Shape s) {
            if (s instanceof Circle c) return \"circle \" + c.r;
            else if (s instanceof Rect r && r.w == r.h) return \"square \" + r.w;
            if (!(s instanceof Rect r)) return \"unit \" + ((Unit) s).ordinal();
            return \"rect \" + r.w;
        }
        static boolean round(Shape s) { return s instanceof Circle; }
        static boolean wide(Shape s) { return s instanceof Rect r && r.w > r.h; }
        public static void main(String[] args) {
            Shape[] all = { new Circle(1), new Rect(2, 2), new Rect(3, 1), Unit.ONE };
            for (Shape s : all) System.out.println(show(s) + \" \" + round(s) + \" \" + wide(s));
            Rect r = (Rect) all[1];
            System.out.println(r.h);
        }
    }";

#[test]
fn instanceof_is_a_match_on_the_enum() {
    let rust = translate(PATTERNS);
    // A test borrows its value.
    assert_contains(&rust, &["fn round(s: &Shape) -> bool {\n    matches!(s, Shape::Circle(_))\n}"]);
}

#[test]
fn a_pattern_binds_the_object_with_if_let() {
    let rust = translate(PATTERNS);
    assert_contains(
        &rust,
        &[
            "    fn as_rect(&self) -> Option<Rc<Rect>> {\n        match self {\n            Shape::Rect(o) => Some(o.clone()),\n            _ => None,\n        }\n    }",
            "    if let Some(c) = s.as_circle() {\n",
            "    } else if let Some(r) = s.as_rect() && r.w == r.h {\n",
            "    s.as_rect().is_some_and(|r| r.w > r.h)",
        ],
    );
}

#[test]
fn a_negated_pattern_that_returns_is_let_else() {
    let rust = translate(PATTERNS);
    assert_contains(
        &rust,
        &["    let Some(r) = s.as_rect() else {\n", "    format!(\"rect {}\", java::Double(r.w))"],
    );
}

#[test]
fn downcasts_are_checked() {
    let rust = translate(PATTERNS);
    assert_contains(
        &rust,
        &[
            "let r = java::cast(all[1].as_rect(), \"Rect\");",
            "java::cast(s.as_unit(), \"Unit\") as i32",
            "    fn as_unit(&self) -> Option<Unit> {\n        match self {\n            Shape::Unit(o) => Some(*o),",
        ],
    );
}

#[test]
fn pattern_variables_are_read_only_where_the_translation_binds_them() {
    let err = java_to_rust::translate(
        "interface S {}
        class A implements S { int v = 1; }
        class B implements S {}
        class M {
            static int f(S s) {
                if (!(s instanceof A a) || a.v == 0) return 0;
                return a.v;
            }
            public static void main(String[] x) { System.out.println(f(new A()) + f(new B())); }
        }",
    )
    .unwrap_err();
    assert!(err.to_string().contains("a pattern variable used here not supported yet"), "{err}");
}

#[test]
fn a_cast_of_a_value_that_may_be_null_to_an_enum_ends_the_program_on_null() {
    let rust = java_to_rust::translate(
        "interface S {}
        enum E implements S { X }
        class M {
            static S pick(int i) { return i == 0 ? null : E.X; }
            public static void main(String[] x) { E e = (E) pick(1); System.out.println(e); }
        }",
    )
    .unwrap();
    assert!(rust.contains("java::null_enum("), "{rust}");
}

const OBJECTS: &str = "
    class Key {
        final int k;
        Key(int k) { this.k = k; }
        public boolean equals(Object o) { return o instanceof Key other && other.k == k; }
        public int hashCode() { return k; }
    }
    class Plain {}
    class M {
        static String show(Object o) {
            if (o instanceof String s) return s;
            if (o instanceof Integer i) return \"int \" + i;
            return \"other \" + o;
        }
        public static void main(String[] a) {
            Object k = new Key(1);
            Object s = \"text\";
            Object n = 5;
            Object p = new Plain();
            System.out.println(k.equals(new Key(1)) + \" \" + k.hashCode() + \" \" + show(s) + show(n) + p);
            int back = (Integer) n;
            System.out.println(back + ((String) s).length());
        }
    }";

#[test]
fn object_values_are_an_enum() {
    let rust = translate(OBJECTS);
    // A variant per class whose objects become `Object`s, and one per runtime class.
    assert_contains(
        &rust,
        &[
            "enum Object {\n    Key(Rc<Key>),\n    Plain(Rc<Plain>),\n    String(String),\n    Integer(i32),\n}",
            "let s = Object::String(String::from(\"text\"));",
            "let n = Object::Integer(5);",
            "let back = java::cast(n.as_integer(), \"java.lang.Integer\");",
            "if let Some(s) = o.as_string() {",
        ],
    );
}

#[test]
fn equals_takes_the_object_enum() {
    let rust = translate(OBJECTS);
    // A class's own `equals` and `hashCode` run for its variant; every other class compares and
    // hashes its identity, and a string or a box its value.
    assert_contains(
        &rust,
        &[
            "fn equals(&self, o: &Object) -> bool {",
            "fn equals(&self, other: &Object) -> bool {\n        match self {\n            Object::Key(o) => o.equals(other),\n            Object::Plain(o) => matches!(other, Object::Plain(b) if Rc::ptr_eq(o, b)),\n            Object::String(o) => matches!(other, Object::String(b) if o == b),",
            "Object::Plain(o) => java::address_hash(Rc::as_ptr(o)),",
            "Object::String(o) => java::string_hash(o),",
            "Object::Plain(o) => f.write_str(&java::default_text(\"Plain\", java::address_hash(Rc::as_ptr(o)))),",
        ],
    );
}

#[test]
fn a_class_extending_number_implements_it_and_byte_value_narrows_int_value() {
    let class = |extra: &str| {
        format!(
            "class M {{ static class Cell extends Number {{ int v = 300;
                public int intValue() {{ return v; }} public long longValue() {{ return v; }}
                public float floatValue() {{ return v; }} public double doubleValue() {{ return v; }} {extra} }}
                public static void main(String[] x) {{ Number n = new Cell(); System.out.println(n.byteValue() + n.intValue()); }} }}"
        )
    };
    let rust = translate(&class(""));
    assert_contains(&rust, &["(n.int_value() as i8 as i32)"]);
    let err = java_to_rust::translate(&class("public byte byteValue() { return 1; }")).unwrap_err();
    assert!(err.to_string().contains("overriding `Number.byteValue` not supported yet"), "{err}");
}

#[test]
fn identity_of_object_values_that_may_be_strings_is_rejected() {
    let err = java_to_rust::translate(
        "class M { public static void main(String[] x) { Object a = \"s\"; Object b = \"s\"; System.out.println(a == b); } }",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("`==` on `Object` values that may be strings or boxes not supported yet"),
        "{err}"
    );
}

#[test]
fn identity_of_object_values_follows_what_flows_into_them() {
    // Only program objects reach `same`'s parameters, so `==` is plain identity; once a box may
    // reach both sides, it is rejected again.
    let program = |arg: &str| {
        format!(
            "class M {{ static boolean same(Object a, Object b) {{ return a == b; }}
                public static void main(String[] x) {{ Object m = {arg}; System.out.println(same(m, {arg})); }} }}"
        )
    };
    java_to_rust::translate(&program("new M()")).unwrap();
    let err = java_to_rust::translate(&program("1")).unwrap_err();
    assert!(
        err.to_string().contains("`==` on `Object` values that may be strings or boxes not supported yet"),
        "{err}"
    );
}

#[test]
fn an_array_as_an_object_prints_as_its_jvm_name_and_hash() {
    let rust = translate(
        "class M { public static void main(String[] x) { Object a = new int[2]; System.out.println(a); } }",
    );
    assert_contains(
        &rust,
        &["Object::IntArray(o) => f.write_str(&java::default_text(\"[I\", java::identity_hash(o))),"],
    );
}

const GENERICS: &str = "
    class Box<T> {
        T value;
        Box(T value) { this.value = value; }
        T get() { return value; }
        void set(T v) { value = v; }
    }
    interface Source<T> { T next(); }
    class Counter implements Source<Integer> {
        int n;
        public Integer next() { return n++; }
    }
    abstract class Shape { abstract int sides(); }
    class Tri extends Shape { int sides() { return 3; } }
    class Point implements Comparable<Point> {
        final int x;
        Point(int x) { this.x = x; }
        public int compareTo(Point o) { return x - o.x; }
    }
    class M {
        static <C extends Comparable<C>> C max(C a, C b) { return a.compareTo(b) >= 0 ? a : b; }
        static int sides(Box<? extends Shape> b) { return b.get().sides(); }
        public static void main(String[] args) {
            Box<Integer> b = new Box<>(3);
            int x = b.get() + 1;
            b.set(x);
            Source<Integer> source = new Counter();
            int y = source.next();
            Box raw = b;
            System.out.println(x + y + \" \" + raw.get());
            System.out.println(max(new Point(1), new Point(2)).x + max(\"a\", \"b\"));
            System.out.println(sides(new Box<>(new Tri())));
        }
    }
";

#[test]
fn type_variables_erase_to_object() {
    let rust = translate(GENERICS);
    // A type variable without a bound is `Object`, and the class is a plain struct.
    assert_contains(
        &rust,
        &[
            "struct Box {\n    value: Object,\n}",
            "fn get(&self) -> Object {",
            "fn set(&mut self, v: Object) {",
            "Box::new(Object::Integer(3))",
            "b.borrow_mut().set(Object::Integer(x));",
        ],
    );
}

#[test]
fn use_sites_check_cast() {
    let rust = translate(GENERICS);
    // Where javac substituted a type argument, the erased value is cast back, as javac's checked
    // cast does.
    assert_contains(
        &rust,
        &["let x = java::cast(b.borrow().get().as_integer(), \"java.lang.Integer\").wrapping_add(1);"],
    );
}

#[test]
fn bounded_type_variables_erase_to_the_bound() {
    let rust = translate(GENERICS);
    // `Comparable` is an enum like `Object`: a string compares by its text, a class by its own
    // `compareTo`.
    assert_contains(
        &rust,
        &[
            "fn max(a: Comparable, b: Comparable) -> Comparable {",
            "enum Comparable {\n    Point(Rc<Point>),\n    String(String),\n}",
            "Comparable::Point(o) => o.compare_to(other),",
            "Comparable::String(o) => match &other { Comparable::String(b) => java::compare_to(o, b), _ => java::cast_failed(\"java.lang.String\") },",
        ],
    );
}

#[test]
fn a_type_variable_with_several_bounds_is_cast_to_the_later_bound_it_calls() {
    let rust = translate(
        "interface Named { String name(); }
        interface Sized { int size(); }
        class Parcel implements Named, Sized {
            public String name() { return \"box\"; }
            public int size() { return 2; }
        }
        public class Main {
            static <T extends Object & Comparable<? super T>> T max(T a, T b) { return a.compareTo(b) >= 0 ? a : b; }
            static <T extends Named & Sized> String show(T t) { return t.name() + t.size(); }
            public static void main(String[] args) {
                System.out.println(max(\"a\", \"b\"));
                System.out.println(show(new Parcel()));
            }
        }",
    );
    // `T` erases to its first bound; a call of a later bound's method casts to it first, as
    // javac's erased code does.
    assert_contains(
        &rust,
        &[
            "fn max(a: Object, b: Object) -> Object {",
            "java::cast(a.as_comparable(), \"Comparable\").compare_to(",
            "t.name()",
        ],
    );
}

#[test]
fn an_array_held_as_an_object_is_a_shared_variant_compared_by_identity() {
    let rust = translate(
        "public class Main {
            public static void main(String[] args) {
                int[] cell = {1, 2};
                Object held = cell;
                int[] back = (int[]) held;
                back[0] = 5;
                System.out.println(cell[0] + \" \" + (held == cell) + \" \" + (held instanceof String[]));
                System.out.println(held.hashCode() == cell.hashCode());
            }
        }",
    );
    // The variant holds the array's handle, so the array keeps its identity in the `Object`.
    assert_contains(
        &rust,
        &[
            "IntArray(java::Array<i32>),",
            "let held = Object::IntArray(cell.clone());",
            "java::cast(held.as_int_array(), \"[I\")",
            "(Object::IntArray(a), Object::IntArray(b)) => Rc::ptr_eq(a, b),",
            "Object::IntArray(o) => java::identity_hash(o),",
        ],
    );
}

#[test]
fn a_type_variables_array_passed_for_an_erased_array_erases_only_its_generic() {
    let rust = translate(
        "public class Main {
            static class Fixed<E> {
                final E[] items;
                Fixed(E[] items) { this.items = items; }
            }
            @SafeVarargs
            static <T> Fixed<T> fixed(T... a) { return new Fixed<>(a); }
            static <T> T either(boolean first, T a, T b) { return first ? a : b; }
            public static void main(String[] args) {
                System.out.println(fixed(1, 2).items.length + either(true, \"x\", \"y\"));
            }
        }",
    );
    // `fixed`'s `T[]` is `Fixed`'s erased `Object[]`, so `fixed` is erased; `either` stays generic.
    assert_contains(
        &rust,
        &[
            "fn fixed(a: java::Array<Object>) -> Rc<MainFixed> {",
            "fn either<T: Clone>(first: bool, a: T, b: T) -> T {",
        ],
    );
}

#[test]
fn a_string_array_passed_for_a_type_variables_array_is_an_object_array_copied_at_builtins() {
    let rust = translate(
        "public class Main {
            static <T> T first(T[] a) { return a[0]; }
            public static void main(String[] args) {
                String[] words = \"b a\".split(\" \");
                Integer[] numbers = {1, 2};
                System.out.println(first(words) + first(numbers) + String.join(\",\", words));
            }
        }",
    );
    // `String[]` meets `Integer[]` at `T[]`, so it is an `Object[]` of strings; the runtime's
    // string builtins take and give copies as `String[]`.
    assert_contains(
        &rust,
        &[
            "String(String),",
            "fn first(a: &[Object]) -> Object {",
            "let words: java::Array<Object> = java::from_vec((&java::from_vec(java::split(\"b a\", \" \", 0)).borrow()).iter().map(|s| Object::String(s.clone())).collect::<Vec<_>>());",
            "java::join(\",\", &java::from_vec((&words.borrow()).iter().map(|s| java::cast(s.as_string(), \"java.lang.String\")).collect::<Vec<_>>()).borrow())",
        ],
    );
}

#[test]
fn main_arguments_that_are_an_object_array_are_strings_as_objects() {
    let rust = translate(
        "public class Main {
            static <T> int count(T[] a) { return a.length; }
            public static void main(String[] args) {
                System.out.println(count(args) + count(new Integer[] {1}));
            }
        }",
    );
    assert_contains(
        &rust,
        &["String(String),", "main_args().into_iter().map(|s| Object::String(s)).collect::<Vec<_>>()"],
    );
}

#[test]
fn sorting_a_string_array_that_is_an_object_array_in_place_is_rejected() {
    let err = java_to_rust::translate(
        "public class Main {
            static <T> T first(T[] a) { return a[0]; }
            public static void main(String[] args) {
                String[] words = {\"b\", \"a\"};
                java.util.Arrays.sort(words);
                System.out.println(first(words) + first(new Integer[] {1}));
            }
        }",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("a change in place of a `String[]` that is also an `Object[]`"),
        "{err}"
    );
}

#[test]
fn a_string_builder_as_an_object_is_a_shared_variant_compared_by_identity() {
    let rust = translate(
        "public class Main {
            public static void main(String[] args) {
                StringBuilder sb = new StringBuilder(\"a\");
                Object held = sb;
                Object text = \"a\";
                ((StringBuilder) held).append('b');
                System.out.println(held + \" \" + (held == sb) + \" \" + held.equals(text) + \" \" + text);
            }
        }",
    );
    // The variant holds the builder's handle, so it is the same builder, which prints its text.
    assert_contains(
        &rust,
        &[
            "StringBuilder(java::StringBuilder),",
            "let held = Object::StringBuilder(sb.clone());",
            "Object::StringBuilder(o) => matches!(other, Object::StringBuilder(b) if o.same(b)),",
            "Object::StringBuilder(o) => fmt::Display::fmt(o, f),",
        ],
    );
}

#[test]
fn an_arraycopy_between_objects_matches_the_types_of_their_arrays() {
    let rust = translate(
        "public class Main {
            static void copy(Object src, Object dest, int length) {
                System.arraycopy(src, 0, dest, 0, length);
            }
            public static void main(String[] args) {
                int[] a = {1, 2};
                int[] b = new int[2];
                copy(a, b, 2);
                copy(new long[] {3L}, new long[1], 1);
                System.out.println(b[1]);
            }
        }",
    );
    // The positions are computed before the arrays are looked at, under names no local has.
    assert_contains(
        &rust,
        &[
            "let (src_pos, dest_pos, length_2) = (0, 0, length); match (&src, &dest) {",
            "(Object::IntArray(src), Object::IntArray(dest)) => java::arraycopy(src, src_pos, dest, dest_pos, length_2),",
            "(Object::LongArray(src), Object::LongArray(dest)) => java::arraycopy(src, src_pos, dest, dest_pos, length_2),",
            "_ => java::throw(\"java.lang.ArrayStoreException\", \"arraycopy: type mismatch\"),",
        ],
    );
}

#[test]
fn an_array_of_references_used_as_an_object_array_is_one_everywhere() {
    let rust = translate(
        "class V { int n; V(int n) { this.n = n; } }
        public class Main {
            static int count(Object[] xs) { return xs.length; }
            public static void main(String[] args) {
                V[] vs = { new V(1) };
                Object[] mixed = { \"s\", vs[0] };
                System.out.println(count(vs) + count(mixed) + vs[0].n);
            }
        }",
    );
    // `V[]` meets `Object[]`, so it is one: the same array is passed, and a read casts back.
    assert_contains(
        &rust,
        &[
            "fn count(xs: &[Object]) -> i32 {",
            "let vs: Vec<Object> = vec![Object::V(Rc::new(V::new(1)))];",
            "count(&vs)",
            "java::cast(vs[0].as_v(), \"V\").n",
        ],
    );
}

#[test]
fn a_generic_interface_bridge_converts_the_argument_and_the_result() {
    let rust = translate(GENERICS);
    // The method with the substituted types keeps its own signature; a bridge with the erased
    // one converts, as the bridge javac writes does.
    assert_contains(
        &rust,
        &[
            "fn next_integer(&mut self) -> i32 {",
            "fn next(&mut self) -> Object {\n        Object::Integer(self.next_integer())\n    }",
            "fn compare_to_point(&self, o: &Rc<Point>) -> i32 {",
            "fn compare_to(&self, other: Comparable) -> i32 {\n        self.compare_to_point(&java::cast(other.as_point(), \"Point\"))\n    }",
        ],
    );
}

#[test]
fn wildcards_and_raw_types_erase() {
    let rust = translate(GENERICS);
    // `? extends Shape` is `Shape`, and a raw `Box` is the class itself.
    assert_contains(&rust, &["java::cast(b.borrow().get().as_shape(), \"Shape\").sides()", "let raw = b;"]);
}

#[test]
fn a_string_array_passed_as_an_array_of_a_type_variable_is_an_object_array() {
    let rust = translate(
        "class M { static <T> T first(T[] xs) { return xs[0]; }
            public static void main(String[] x) { String[] s = {\"a\"}; System.out.println(first(s)); } }",
    );
    // The erased `T[]` is an `Object[]`, which the `String[]` meets, so it is one too.
    assert_contains(
        &rust,
        &[
            "fn first(xs: &[Object]) -> Object {",
            "let s: Vec<Object> = vec![Object::String(String::from(\"a\"))];",
            "java::cast(first(&s).as_string(), \"java.lang.String\")",
        ],
    );
}

#[test]
fn a_type_variable_hides_a_class_of_its_name() {
    let rust = translate(
        "class M { public static void main(String[] x) { System.out.println(new Holder<>(new T()).get().n + new Holder<>(\"s\").get()); } }
        class T { int n = 1; }
        class Holder<T> { T held; Holder(T held) { this.held = held; } T get() { return held; } }",
    );
    // The type variable is the Rust type parameter, and `T` the class only outside `Holder`.
    assert_contains(&rust, &["struct Holder<T> {\n    held: T,\n}", "Holder::new(Rc::new(T::new()))"]);
}

const REAL_GENERICS: &str = "
    class Stack<T> {
        Node<T> head;
        void push(T value) { head = new Node<>(value, head); }
        T pop() { T value = head.value; head = head.next; return value; }
        boolean isEmpty() { return head == null; }
    }
    class Node<T> {
        final T value;
        final Node<T> next;
        Node(T value, Node<T> next) { this.value = value; this.next = next; }
    }
    class Pair<A, B> {
        final A first;
        final B second;
        Pair(A first, B second) { this.first = first; this.second = second; }
        Pair<B, A> swap() { return new Pair<>(second, first); }
    }
    class M {
        static <T> T choose(boolean first, T a, T b) { return first ? a : b; }
        public static void main(String[] args) {
            Stack<String> words = new Stack<>();
            words.push(\"a\");
            System.out.println(words.pop().length());
            Stack<Integer> numbers = new Stack<>();
            numbers.push(4);
            int n = numbers.pop() + 1;
            Pair<String, Integer> p = new Pair<>(\"one\", 1).swap().swap();
            System.out.println(n + p.first + choose(false, 1, 2));
        }
    }
";

#[test]
fn a_simple_container_is_a_rust_generic() {
    let rust = translate(REAL_GENERICS);
    // A class whose type variables are only stored, returned and passed on is a Rust generic, and
    // its use sites keep the substituted types, with no casts.
    assert_contains(
        &rust,
        &[
            "struct Stack<T> {\n    head: Option<Rc<Node<T>>>,\n}",
            "impl<T: Clone> Stack<T> {",
            "fn push(&mut self, value: T) {",
            "fn pop(&mut self) -> T {",
            "struct Pair<A, B> {\n    first: A,\n    second: B,\n}",
            "fn swap(&self) -> Rc<Pair<B, A>> {",
            "words.push(String::from(\"a\"));",
        ],
    );
    assert!(!rust.contains("enum Object"), "{rust}");
}

#[test]
fn a_generic_method_infers_its_type_parameter() {
    let rust = translate(REAL_GENERICS);
    // A literal given for a type variable states its type, which Rust would otherwise not know.
    assert_contains(
        &rust,
        &["fn choose<T: Clone>(first: bool, a: T, b: T) -> T {", "choose(false, 1i32, 2i32)"],
    );
}

#[test]
fn a_class_that_reads_its_type_variable_stays_erased() {
    let rust = translate(
        "class Box<T> { T v; Box(T v) { this.v = v; } String show() { return \"<\" + v + \">\"; } }
        class Bag<T> { Object[] items = new Object[2]; T get(int i) { return (T) items[i]; } }
        class M { public static void main(String[] x) {
            Bag<String> bag = new Bag<>();
            bag.items[0] = \"s\";
            System.out.println(new Box<>(3).show() + bag.get(0)); } }",
    );
    // Text of a type variable's value and an unchecked cast need the erased classes.
    assert_contains(&rust, &["struct Box {\n    v: Object,\n}", "struct Bag {"]);
    assert!(!rust.contains("struct Box<T>") && !rust.contains("struct Bag<T>"), "{rust}");
}

#[test]
fn a_method_whose_type_parameter_is_only_its_result_stays_erased() {
    let rust = translate(
        "class M { static <T> T nothing() { return null; }
            public static void main(String[] x) { String s = nothing(); System.out.println(s == null); } }",
    );
    // Rust cannot infer a type parameter that no parameter mentions.
    assert_contains(&rust, &["fn nothing() -> Option<Object> {"]);
}

#[test]
fn a_field_assigned_from_a_borrow_of_itself_ends_the_borrow_first() {
    let rust = translate(
        "class Node { int v; Node next; Node(int v) { this.v = v; } }
        class Chain { Node head;
            void add(int v) { Node n = new Node(v); n.next = head; head = n; }
            int pop() { int v = head.v; head = head.next; return v; } }
        class M { public static void main(String[] x) { Chain c = new Chain(); c.add(1); c.add(2);
            System.out.println(c.pop() + c.pop()); } }",
    );
    // The `Ref` of the old head must end before the field is written.
    assert_contains(&rust, &["self.head = { java::non_null(self.head.as_ref()).borrow().next.clone() };"]);
}

#[test]
fn new_object_creates_a_plain_object_with_its_identity() {
    let rust = translate(
        "class A {
            static final Object LOCK = new Object();
            public static void main(String[] args) {
                Object o = new Object();
                System.out.println(o.equals(LOCK) + \" \" + o.toString().startsWith(\"java.lang.Object@\"));
            }
        }",
    );
    assert_contains(
        &rust,
        &["struct PlainObject {", "let o = Rc::new(PlainObject::new());", "java.lang.Object"],
    );
}

#[test]
fn an_element_that_may_be_null_is_passed_as_the_receiver_that_it_is() {
    // `start` uses `this` as a value, so it takes the element's `&Rc<..>`, which the null check
    // already gives.
    let rust = translate(
        "class P {
            static class Box { int v; static void register(Box b) { b.v++; } void start() { register(this); } }
            public static void main(String[] args) {
                Box[] boxes = new Box[2];
                for (int i = 0; i < 2; i++) { boxes[i] = new Box(); boxes[i].start(); }
                System.out.println(boxes[0].v + boxes[1].v);
            }
        }",
    );
    assert_contains(&rust, &["PBox::start(java::non_null(boxes[i as usize].as_ref()));"]);
}

#[test]
fn string_streams_need_the_jdk_sources() {
    for method in ["chars", "lines"] {
        let source = format!("class A {{ public static void main(String[] a) {{ \"abc\".{method}(); }} }}");
        let err = java_to_rust::translate(&source).expect_err("a string's stream runs the JDK's code");
        assert!(err.to_string().contains(&format!("`String.{method}` streams with the JDK's code")), "{err}");
    }
}

#[test]
fn clone_in_a_class_tree_copies_each_runtime_class() {
    // `Object.clone` copies every field of the runtime class, the embedded superclass fields
    // too, and refuses a class that is not `Cloneable` when the copy is made.
    let rust = translate(
        "class A {
            static class Shape {
                int x;
                Shape copy() throws CloneNotSupportedException { return (Shape) super.clone(); }
            }
            static class Circle extends Shape implements Cloneable { int r; }
            public static void main(String[] a) throws CloneNotSupportedException {
                Shape s = new Circle();
                System.out.println(s.copy().x);
                try { new Shape().copy(); } catch (CloneNotSupportedException e) { System.out.println(e); }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "AShape::Shape(_) => java::try_clone_refused(\"A$Shape\")?",
            "AShape::Circle(o) => { let o = o.borrow(); AShape::Circle(Rc::new(RefCell::new(ACircle { base: AShapeFields { x: o.base.x }, r: o.r }))) }",
        ],
    );
}

#[test]
fn clone_of_another_object_is_rejected() {
    let err = java_to_rust::translate(
        "class A implements Cloneable {
            int x;
            Object other(A a) throws CloneNotSupportedException { return a.clone(); }
            public static void main(String[] args) {}
        }",
    )
    .expect_err("only `super.clone()` is translated");
    assert!(err.to_string().contains("`clone` of an object other than `this`"), "{err}");
}
