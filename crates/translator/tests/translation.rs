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
    assert_contains(&rust, &["let s: Option<String> = None;", "if s.as_deref().is_none() {"]);
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
    assert_contains(
        &rust,
        &["fn size(s: Option<String>) -> i32", "java::non_null(s.as_deref()).len() as i32"],
    );
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
    assert_contains(&rust, &["if let Some(s) = s.as_deref() {", "s.len() as i32"]);
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
    assert_contains(&rust, &["let Some(s) = s.as_deref() else {", "return -1;", "s.len() as i32"]);
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
    assert_contains(&rust, &["if !s.as_deref().is_none() {"]);
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
