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
            "let mut squares: Vec<i32> = java::new_vec(5, 0);",
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
    assert_contains(
        &rust,
        &["fn fill(v: &mut [i32])", "fill(&mut a);", "let mut a: Vec<i32> = java::new_vec(3, 0);"],
    );
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
