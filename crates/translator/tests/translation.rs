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
    assert_contains(&sign, &["} else if n < 0 {", "        java::JString::from(\"zero\")\n    }\n}"]);
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
    assert_contains(&open, &["            return java::JString::from(\"Mon\");\n"]);
}

#[test]
fn a_primitive_local_returned_at_the_end_is_the_value() {
    let twice = translate("class A { static int twice(int n) { int r = n + n; return r; } }");
    assert!(!twice.contains("let r") && !twice.contains("return"), "{twice}");
    let text = translate("class A { static String text() { String t = \"a\"; return t; } }");
    assert_contains(&text, &["    let t = java::JString::from(\"a\");\n"]);
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
        &[
            "fn greet(name: &java::Wtf8) -> java::JString",
            "name.len() as i32",
            "greet(&n)",
            "greet(java::wtf8(\"Bob\"))",
        ],
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
    assert_contains(
        &rust,
        &[
            "let mut s = java::JString::new();",
            "java::append!(s, format_args!(\"{i}\"));",
            "s.push_str(\",\");",
        ],
    );
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
    assert_contains(&rust, &["fn twice(s: &java::Wtf8) -> java::JString"]);
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
            "fn find(n: i32) -> Option<java::JString>",
            "let s: Option<java::JString> = find(3);",
            "s.as_deref().unwrap_or(java::wtf8(\"null\"))",
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
    assert_contains(&rust, &["let s: Option<java::JString> = None;", "if s.is_none() {"]);
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
    assert_contains(&rust, &["fn size(s: Option<&java::Wtf8>) -> i32", "java::non_null(s).len() as i32"]);
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
fn guarded_indexes_of_shared_arrays_in_variables_are_not_checked() {
    let rust = translate(
        "class A {
            static int[] kept = new int[4];
            static int sum(int[] values) {
                int sum = 0;
                for (int i = 0; i < values.length; i++) { sum += values[i]; }
                return sum;
            }
            static int zero(int[] values) {
                int i = 0;
                while (i < values.length && values[i] != 0) { i++; }
                return i;
            }
            static int past(int[] values) {
                int sum = 0;
                for (int i = 0; i <= values.length; i++) { sum += values[i]; }
                return sum;
            }
            public static void main(String[] args) {
                int[] a = kept;
                for (int i = 0; i < a.length; i++) { a[i] = i + 1; }
                System.out.println(sum(kept) + zero(kept) + past(kept));
            } }",
    );
    // A counting loop takes the array's view once; another guarded access checks the length
    // only against a wrong proof; an index past the end is checked as Java checks it.
    assert_contains(
        &rust,
        &[
            "let values_elements = java::view(&values);",
            "java::view_get(&values_elements, i)",
            "let a_elements = java::view(&a);",
            "java::view_set(&a_elements, i, ",
            "java::get_in_bounds(&values, i)",
            "java::get(&values, i)",
        ],
    );
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
fn an_array_borrowed_for_a_call_is_read_after_the_arguments_that_write_it() {
    let rust = translate(
        "class A {
            static int squeeze(char[] path) { path[0] = 'x'; return 1; }
            public static void main(String[] args) {
                char[] path = \"ab/./cd\".toCharArray();
                char[] alias = path;
                System.out.println(new String(path, 0, squeeze(path)));
                System.out.println(alias.length);
            } }",
    );
    assert_contains(
        &rust,
        &[
            "{ let array = path.clone(); let arg = { let value = squeeze(&mut path.borrow_mut()); value }; java::from_chars_range(&array.borrow(), 0, arg) }",
        ],
    );
    assert_lacks(&rust, &["from_chars_range(&path.borrow(), 0, "]);
}

#[test]
fn a_row_stored_with_a_value_that_cannot_throw_stays_plain() {
    // Java computes a stored value before it throws for the array, so a value that may throw
    // holds the row in a local first, and the row then has two names. `% 100` and a division of
    // doubles cannot throw.
    let rust = translate(
        "class A { public static void main(String[] args) {
            int[][] m = new int[3][3];
            double[][] d = new double[3][3];
            for (int i = 0; i < 3; i++) {
                for (int j = 0; j < 3; j++) { m[i][j] = (i * 7 + j) % 100; d[i][j] = i / 2.0; }
            }
            System.out.println(m[2][1] + \" \" + d[2][1]);
        } }",
    );
    assert_contains(&rust, &["Vec<Vec<i32>>", "Vec<Vec<f64>>"]);
    assert_lacks(&rust, &["java::Array", "receiver_"]);
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
fn loops_that_search_an_array_call_search() {
    let rust = translate(
        "class A {
            static Object[] palette = new Object[3];
            static int idFor(Object key) {
                for (int i = 0; i < palette.length; i++) { if (palette[i] == key) { return i; } }
                return -1;
            }
            static boolean has(int[] a, int x, int n) {
                for (int i = 1; i < n; i++) if (a[i] == x) return true;
                return false;
            }
            static int called(int[] a) {
                for (int i = 0; i < a.length; i++) { if (a[i] == f()) { return i; } }
                return -1;
            }
            static int f() { return 2; }
            public static void main(String[] args) {
                int[] a = { 1, 2, 3 };
                System.out.println(idFor(args) + \" \" + has(a, 3, 3) + \" \" + called(a));
            }
        }",
    );
    assert_contains(&rust, &["java::search_same(&A_PALETTE.cloned(), 0, None, Some(key), true, |item| "]);
    assert_contains(&rust, &["java::slice_search_eq(a, 1, Some(n), x);", "if found.is_some() {"]);
    // A condition that calls a method stays a loop.
    assert_contains(&rust, &["a[i as usize] == f()"]);
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
        &[
            "fn describe(s: Option<&java::Wtf8>) -> java::JString {",
            "let Some(s) = s else {",
            "describe(Some(java::wtf8(\"x\")))",
        ],
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
        &[
            "fn or_default(m: Option<&java::Wtf8>) -> java::JString {",
            "m.unwrap_or(java::wtf8(\"missing\")).to_owned()",
        ],
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
            "match (n.as_deref(), noisy(java::wtf8(\"y\"))) { (receiver_, other_) => java::non_null(receiver_) == other_ }",
        ],
    );
}

#[test]
fn a_shared_object_is_read_by_value_beside_a_call_that_changes_it() {
    // The object is shared with a thread; its fields are cells, so a read holds nothing that the
    // call beside it could conflict with.
    let rust = translate(
        "class P { int x; int bump() { x++; return x; } }
        class A {
            static void share(P q) {}
            static int f(int v) { return v + 1; }
            public static void main(String[] a) throws Exception {
                P p = new P();
                Thread t = new Thread(() -> share(p));
                t.start();
                t.join();
                System.out.println(p.x);
                System.out.println(f(p.x) + p.bump());
            }
        }",
    );
    assert_contains(&rust, &["java::println!(\"{}\", p.x.get());", "f(p.x.get()).wrapping_add(p.bump())"]);
}

#[test]
fn arguments_that_read_the_receiver_are_plain_calls() {
    // The receiver, shared with a thread, is not borrowed, so an argument that reads it needs no
    // binding before the call.
    let rust = translate(
        "class P { int x; void add(int n) { x += n; } int get() { return x; } }
        class A {
            static void share(P q) {}
            public static void main(String[] a) throws Exception {
                P p = new P();
                Thread t = new Thread(() -> share(p));
                t.start();
                t.join();
                P q = p;
                q.add(p.get());
                q.add(2);
            }
        }",
    );
    assert_contains(&rust, &["q.add(p.get());", "q.add(2);"]);
}

#[test]
fn to_string_is_a_method_that_printing_and_concatenation_call() {
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
            "fn to_string(&self) -> java::JString {\n        java::concat!(format_args!(\"P{}\", self.x))\n    }",
            "java::println!(\"{}\", p.to_string());",
            "s.push_wtf8(&p.to_string());",
            "java::println!(\"{}\", java::concat!(s, p.to_string()));",
        ],
    );
    assert_lacks(&rust, &["impl fmt::Display for P"]);
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
            "struct ShapeFields {\n    name: java::Field<java::JString>,\n    moves: java::Field<i32>,\n}",
            "struct Circle {\n    base: ShapeFields,\n    radius: java::Field<f64>,\n}",
        ],
    );
    // A value of the superclass's type is the one class's object that it can be.
    assert_contains(&rust, &["let s = c.clone();", "Arc::ptr_eq(&s, &c)"]);
    assert_lacks(&rust, &["struct Shape {", "dyn "]);
}

#[test]
fn inherited_field_methods_live_on_the_fields_struct() {
    let rust = translate(SHAPES);
    assert_contains(
        &rust,
        &["impl ShapeFields {", "fn name(&self) -> java::JString {", "fn move_(&self) {", "s.base.move_();"],
    );
    // `describe` calls `area`, which a subclass implements, so it runs on the subclass's object.
    assert_contains(
        &rust,
        &["impl Circle {", "fn describe(&self) -> java::JString {", "self.base.name", "self.area()"],
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
        &[
            "fn ShapeFields__new(name: &java::Wtf8) -> ShapeFields {",
            "Circle { base: ShapeFields__new(java::wtf8(\"circle\")), radius: java::Field::new(radius) }",
        ],
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
            "fn withdraw_in_account(&self, amount: i64) -> bool {",
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
            "let mut this = Impl { base: BaseFields__new() };",
            "this.construct_base();",
            "fn construct_base(&self) {",
            "self.base.x.set(self.size());",
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
    assert_contains(
        &rust,
        &["fn init_sub() {", "    init_base();\n    SUB_OTHER.set(make(java::wtf8(\"sub\")));"],
    );
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
            "enum BaseView {\n    Circle(Arc<Circle>) = 2,\n    Rect(Arc<Rect>) = 3,\n}",
            // A method of the enum is a function of its module, which takes the value.
            "fn Base__area(this: Base) -> f64 {\n    match this.0.tag() {\n        2 => { let o: &Arc<Circle> = &unsafe { this.0.to_pointer() }; o.area() }\n        3 => { let o: &Arc<Rect> = &unsafe { this.0.to_pointer() }; o.area() }\n        tag => java::bad_tag(tag),\n    }\n}",
            // Every class keeps the fields struct at the start of its object, where `base()`
            // reads it whatever the class.
            "#[repr(C)]\nstruct Circle {\n    base: BaseFields,",
            "#[repr(C)]\nstruct Rect {\n    base: BaseFields,",
            "    const BASE_AT_START: () = {\n        assert!(std::mem::offset_of!(Circle, base) == 0);\n        assert!(std::mem::offset_of!(Rect, base) == 0);\n    };\n\n    fn base(&self) -> &BaseFields {\n        let () = Self::BASE_AT_START;\n        // Every variant is an object that starts with the fields struct (above).\n        unsafe { self.0.as_prefix() }\n    }",
            // A method that only touches the fields runs on the fields struct that `base()` lends.
            "fn Base__get_name(this: Base) -> java::JString {\n    this.base().get_name()\n}",
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
            "fn Base__to_string_in_base(this: Base) -> java::JString {",
            "java::concat!((*this.base().name.fixed()).clone(), format_args!(\":{}\", java::Double(Base__area(this))))",
            // `super.toString()` wraps the subclass's object in the enum to call the body there.
            "Base__to_string_in_base(Base::Circle(this.clone()))",
            "impl fmt::Display for Base {",
            "3 => fmt::Display::fmt(&Base__to_string_in_base(*self), f),",
        ],
    );
}

#[test]
fn a_dispatcher_over_many_classes_runs_each_call_in_a_function_of_its_own() {
    // With a profile, LLVM inlines the hot arms of a `match` over thousands of classes into the
    // dispatcher, whose frame then holds all their stack slots; a function of its own per arm
    // keeps the dispatcher's frame small.
    let shapes = |n: usize| {
        let classes: String = (0..n)
            .map(|k| format!("class C{k} implements Shape {{ int k; C{k}(int k) {{ this.k = k; }} public int area() {{ return k * {k}; }} }}\n"))
            .collect();
        let made: Vec<String> = (0..n).map(|k| format!("new C{k}(a.length + {k})")).collect();
        translate(&format!(
            "interface Shape {{ int area(); }}
            {classes}
            class A {{
                public static void main(String[] a) {{
                    Shape[] shapes = {{ {} }};
                    int total = 0;
                    for (Shape s : shapes) total += s.area();
                    System.out.println(total);
                }}
            }}",
            made.join(", ")
        ))
    };
    assert_contains(&shapes(70), &["#[inline(never)] fn arm(o: &", "arm(o)"]);
    assert_lacks(&shapes(60), &["fn arm("]);
}

#[test]
fn upcasts_wrap_the_handle() {
    let rust = translate(TREE);
    assert_contains(
        &rust,
        &["let b = Base::Circle(c.clone());", "let r = Base::Rect(Arc::new(Rect__new(2.0, 3.0)));"],
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
            // Every variant is an object compared by identity: the same bits, class and address.
            "impl PartialEq for Base {",
            "self.0 == other.0",
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
            "enum NView {\n    Leaf(Arc<Leaf>) = 2,\n    Twice(Arc<Twice>) = 3,\n}",
            "fn base(&self) -> &NFields {",
            "2 => { let o: &Arc<Leaf> = &unsafe { this.0.to_pointer() }; o.eval() }",
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
    assert_contains(&rust, &["let mut t = Tri__new();", "t.base.move_();", "t.base.moves"]);
    // With one class left, the type is that class: no enum.
    assert_contains(&rust, &["let s = slot_1.put(RefCell::new(Square__new()));"]);
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
            "enum ShapeView {\n    Square(Arc<Square>) = 2,\n    Tri(Arc<Tri>) = 3,\n}",
            "let s = slot_1.put(Square__new());",
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
            "fn P__new(x: i32) -> P {",
            "let mut this = P { x: 0, y: 0 };",
            "this.field_inits();",
            "this.x = x;",
            "fn sum(&self) -> i32 {",
            "let p = slot_1.put(P__new(3));",
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
            "fn P__new(x: i32) -> P {\n    P { x: java::Field::new(x) }\n}",
            "fn set(&self, v: i32) {\n        self.x.set(v);\n    }",
            "fn get(&self) -> i32 {",
            "let p = slot_1.put(P__new(1));",
            "p.set(2);",
            "java::println!(\"{}\", p.get());",
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
            "fn P__register(this: &Arc<P>) {\n    share(this);",
            // `o` may be `this`, which the method then must not hold borrowed.
            "fn P__copy(this: &Arc<P>, o: &Arc<P>) {\n    this.x.set(o.x.get());",
            "P__register(&p);",
            "P__copy(&q, &p);",
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
            "fn P__new(x: i32) -> P {\n    P { x }\n}",
            "fn set(&mut self, v: i32) {",
            "fn get(&self) -> i32 {",
            "let mut p = P__new(1);",
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
        &["let c = next();\n    let a = next();\n    let this = P { b: x, c, a };", "    this\n"],
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
        &[
            "let mut p = P__new();",
            "java::println!(\"{}\", p.to_string());",
            "java::println!(\"p={}\", p.to_string());",
        ],
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
    assert_contains(&rust, &["let p = P__new(1);"]);
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
    assert_contains(
        &rust,
        &["i as i8", "java::double_to_int(d) as u16", "java::float_to_int(f)", "\"-56 A 4464\""],
    );
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
            "static A_COUNT: java::Global<Cell<i32>> = java::Global::new(Cell::new(0));",
            "A_COUNT.get()",
            "A_COUNT.set(value);",
            "static A_NAME: java::LazyGlobal<RefCell<",
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
    assert_contains(&plain, &["let args: Vec<java::JString> = java::main_args();"]);
    let shared = translate(
        "class A {
            static String[] kept;
            public static void main(String[] args) { kept = args; System.out.println(kept.length); }
        }",
    );
    assert_contains(&shared, &["let args: java::Array<java::JString> = java::from_vec(java::main_args());"]);
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
            "fn log(f: &java::Wtf8, xs: &[Object]) {",
            "log(java::wtf8(\"x\"), &[Object::Integer(1), Object::String(java::JString::from(\"two\"))]);",
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
            "use crate::com::example::app;\n\nfn main() {\n    app::main();\n}",
            "mod com {\n    pub(crate) mod example {\n        pub(crate) mod app {",
            "        pub(crate) mod util {\n            use java_runtime as java;",
            // Imported, statically imported and fully qualified names are paths through the `use`
            // line of their module.
            "            use crate::com::example::util;\n",
            "util::Counter__new(java::wtf8(\"clicks\"))",
            "util::square(7)",
            "util::NUMBERS_LIMIT - util::square(3)",
            "util::cube(3)",
            // Names in the same package are not.
            "line(java::wtf8(\"total\"), clicks.total())",
            "x * square(x)",
            "pub(crate) struct Counter {\n                pub(crate) name: java::JString,",
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
    assert_contains(&rust, &["fn run(", "fn area(", "java::concat!(\"task \", (*self.name.fixed()))"]);
    // `audit` and `describe` are never called, nor is `never`, whose anonymous class uses `extra`.
    assert_lacks(&rust, &["extra", "Audit", "audit", "describe", "fn never", "\"never\""]);
}

#[test]
fn classes_with_one_name_in_two_packages() {
    let rust = translate_project("statics", "com.shop.Main");
    assert_contains(
        &rust,
        &[
            // The packages inside `com.shop` are in scope in its module without a `use` line.
            "model::Item__new_string_i32(java::wtf8(\"apple\"), 3)",
            "legacy::Item__new(7)",
            "model::RegistryEntry__new(4)",
            "model::REGISTRY_CREATED.get()",
            "pub(crate) static COUNTER_HITS: java::Global<Cell<i32>>",
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
            "Arc::new(Circle__new());",
            "java::throw(\"java.lang.ClassNotFoundException\", &format!(\"{}, which is not in the translation\"",
        ],
    );
    // No code names `Square`, so no name can create one; and no reflection is needed.
    assert_lacks(&rust, &["Square", "square", "Constructor"]);
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
        &[
            "use crate::lib;",
            "lib::shout(java::wtf8(\"hi\"))",
            "mod lib {",
            "pub(crate) fn shout(s: &java::Wtf8) -> java::JString",
        ],
    );
    assert_lacks(&rust, &["unused", "Broken"]);
}

/// A program whose source root is a module that requires a library's module, both with a
/// `module-info.java` (without the JDK's sources, so both are compiled in the unnamed module).
fn module_sources(dir: &str) -> std::path::PathBuf {
    write_sources(
        dir,
        &[
            (
                "app/module-info.java",
                "/** The application. */\nmodule com.example.app {\n    requires org.lib;\n}\n",
            ),
            (
                "app/com/example/app/Main.java",
                "package com.example.app;\nimport org.lib.text.Shout;\npublic class Main {\n    public static void main(String[] a) { System.out.println(Shout.shout(\"hi\")); }\n}\n",
            ),
            ("lib/module-info.java", "module org.lib {\n    exports org.lib.text;\n}\n"),
            (
                "lib/org/lib/text/Shout.java",
                "package org.lib.text;\npublic class Shout {\n    public static String shout(String s) { return s + \"!\"; }\n}\n",
            ),
        ],
    )
}

#[test]
fn library_roots_that_are_modules_are_read_as_sources() {
    let dir = module_sources("module-roots");
    let program = java_to_rust::Program {
        main: "com.example.app.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &[
            "use crate::org::lib::text;",
            "text::shout(java::wtf8(\"hi\"))",
            "pub(crate) fn shout(s: &java::Wtf8) -> java::JString",
        ],
    );
    let report = survey(&program).report;
    assert_contains(&report, &["  Library: 1 pieces; 1 (100.0%) reach nothing outside the sources."]);
}

#[test]
fn a_required_module_that_is_not_given_is_named() {
    let dir = module_sources("module-missing");
    let program = java_to_rust::Program {
        main: "com.example.app.Main".into(),
        sources: vec![dir.join("app")],
        ..Default::default()
    };
    let error = java_to_rust::translate_program(&program).expect_err("org.lib is not given");
    assert!(
        error.to_string().ends_with(
            "Main.java:2:20: package org.lib.text does not exist; module com.example.app requires module org.lib, \
             whose sources are not given: pass the root of its sources as --library"
        ),
        "{error}"
    );
    // A package of a module that javac has only as class files, here the JDK's `java.base`, whose
    // `jdk.internal.misc` is not exported.
    let dir = write_sources(
        "module-not-visible",
        &[(
            "app/p/Main.java",
            "package p;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(jdk.internal.misc.VM.isBooted());\n    }\n}\n",
        )],
    );
    let program =
        java_to_rust::Program { main: "p.Main".into(), sources: vec![dir.join("app")], ..Default::default() };
    let error = java_to_rust::translate_program(&program).expect_err("java.base is not given");
    assert!(
        error.to_string().ends_with(
            "Main.java:4:40: package jdk.internal.misc is in module java.base, whose sources are not given: \
             pass the root of its sources as --library"
        ),
        "{error}"
    );
}

#[test]
fn annotation_types_used_only_as_annotations_are_left_out_of_a_whole_program() {
    let dir = write_sources(
        "annotation-types",
        &[
            (
                "app/Tag.java",
                "package app;\nimport java.lang.annotation.*;\n@Retention(RetentionPolicy.RUNTIME)\npublic @interface Tag {\n    String value() default \"\";\n}\n",
            ),
            (
                "app/Main.java",
                "package app;\n@Tag(\"main\")\npublic class Main {\n    @interface Note { int level(); }\n    @Note(level = 2) @Tag static int twice(@Tag int x) { return 2 * x; }\n    public static void main(String[] a) { System.out.println(twice(4)); }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "app.Main".into(),
        sources: vec![dir.join("app")],
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(&rust, &["fn twice(x: i32) -> i32", "twice(4)"]);
    assert_lacks(&rust, &["Tag", "Note", "level", "Retention"]);
}

#[test]
fn annotation_types_used_as_values_are_rejected_where_the_code_uses_them() {
    let rejected = |name: &str, main: &str| {
        let tag = "package app;\npublic @interface Tag {\n    String value() default \"\";\n    StringBuilder NAMES = new StringBuilder();\n}\n";
        let dir = write_sources(name, &[("app/Tag.java", tag), ("app/Main.java", main)]);
        let program =
            java_to_rust::Program { main: "app.Main".into(), sources: vec![dir], ..Default::default() };
        java_to_rust::translate_program(&program).expect_err("a value of an annotation type").to_string()
    };
    let error = rejected(
        "annotation-variable",
        "package app;\npublic class Main {\n    public static void main(String[] a) {\n        Tag t = Main.class.getAnnotation(Tag.class);\n        System.out.println(t == null);\n    }\n}\n",
    );
    assert!(
        error.ends_with("app/Main.java:4:9: using the annotation type `app.Tag` other than as an annotation not supported yet"),
        "{error}"
    );
    let error = rejected(
        "annotation-field",
        "package app;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(Tag.NAMES.length());\n    }\n}\n",
    );
    assert!(
        error.ends_with("app/Main.java:4:28: a field of the annotation type `app.Tag` not supported yet"),
        "{error}"
    );
    let error = rejected(
        "annotation-implemented",
        "package app;\npublic class Main implements Tag {\n    public String value() { return \"v\"; }\n    public Class<? extends java.lang.annotation.Annotation> annotationType() { return Tag.class; }\n    public static void main(String[] a) { System.out.println(new Main().value()); }\n}\n",
    );
    assert!(error.ends_with("app/Main.java:2:30: implementing `app.Tag` not supported yet"), "{error}");
}

#[test]
fn library_code_compares_strings_by_value_and_the_programs_own_code_by_their_sources() {
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
    assert_contains(&rust, &["tag == java::wtf8(\"\")", "!(a == b)"]);
    // The same comparison in the program's own code is decided by where its strings come from:
    // an argument of `main` is a new string, never the literal.
    let own = "package main;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(a.length > 0 && a[0] == \"x\");\n    }\n}\n";
    let dir = write_sources("program-string-equality", &[("app/main/Main.java", own)]);
    let program = java_to_rust::Program {
        main: "main.Main".into(),
        sources: vec![dir.join("app")],
        ..Default::default()
    };
    java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
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
fn a_new_array_whose_inner_rows_are_object_arrays_makes_each_level() {
    // `aisle` makes `String[][]` an `Object[]`, so the rows of `new String[2][2][3]` hold
    // `Object`s, each an `Object[]` of class `String[][]` holding the third level's `String[]`s.
    let dir = write_sources(
        "deep_object_rows",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    public static void main(String[] a) {\n        String[][][] words = new String[2][2][3];\n        Object[] aisle = words[1];\n        System.out.println(aisle.length);\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert!(rust.contains("check_sizes"), "{rust}");
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
            "java::address_hash(Arc::as_ptr(&x)) == java::address_hash(Arc::as_ptr(&x))",
            "java::write_byte(65);",
            "java::flush();",
            "java::available_processors()",
        ],
    );
    // `System.getenv(name)` reads the environment the program runs in; the whole environment as a
    // map is the JDK's `ProcessEnvironment`, which only a whole program translates.
    let out = java_to_rust::translate(
        "class A { public static void main(String[] a) { System.out.println(System.getenv(\"HOME\")); } }",
    )
    .unwrap();
    assert!(out.contains("java::getenv(java::wtf8(\"HOME\"))"), "{out}");
    let err = java_to_rust::translate(
        "class A { public static void main(String[] a) { Object env = System.getenv(); } }",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("`java.lang.System.getenv` outside a whole program not supported yet"),
        "{err}"
    );
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
    assert_contains(&rust, &["const A_LIMIT: i32 = 10;", "const A_NAME: &java::Wtf8 = java::wtf8(\"a\");"]);
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
            "static HELPER__INITIALIZED: java::Global<Cell<bool>> = java::Global::new(Cell::new(false));",
            "fn init_helper() {",
            "if HELPER__INITIALIZED.get() {",
            "fn make() -> i32 {\n    init_helper();",
        ],
    );
}

#[test]
fn a_class_initialized_by_an_earlier_statement_is_not_checked_again() {
    let rust = translate(
        "class Helper {
            static int made;
            static { made = 10; System.out.println(\"ready\"); }
        }
        public class A {
            public static void main(String[] args) {
                if (args.length > 5 && Helper.made > 0) { System.out.println(1); }
                int a = Helper.made;
                int b = Helper.made + 1;
                for (int i = 0; i < 3; i++) { b += Helper.made; }
                Runnable r = () -> System.out.println(Helper.made);
                r.run();
                System.out.println(a + b);
            }
        }",
    );
    // The use past `&&` may not run, so the next one checks; after it, none does, but the
    // lambda's body, which may run on another thread, checks for itself.
    assert_contains(
        &rust,
        &[
            "if args.len() as i32 > 5 && ({ init_helper(); HELPER_MADE.get() }) > 0 {",
            "let a = { init_helper(); HELPER_MADE.get() };",
            "let mut b = HELPER_MADE.get().wrapping_add(1);",
            "b = b.wrapping_add(HELPER_MADE.get());",
            "let r = || { java::println!(\"{}\", { init_helper(); HELPER_MADE.get() }); };",
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
            "fn enum_name(self) -> java::JString {",
            "AColor::VeryBig => \"VERY_BIG\",",
            "fn enum_value_of(name: &java::Wtf8) -> AColor {",
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
    assert_contains(&rust, &["fn APaint__new(color: AColor) -> APaint {\n    APaint { color }\n}"]);
}

#[test]
fn an_enum_field_that_a_constructor_reads_before_setting_it_is_an_option() {
    let program = |extra: &str| {
        format!(
            "public class A {{
                enum Color {{ RED, BLUE }}
                static class Paint {{
                    private Color color;
                    Paint(Color color) {{ System.out.println(this.color); this.color = color; }}
                    {extra}
                }}
                public static void main(String[] args) {{ new Paint(Color.BLUE); }}
            }}"
        )
    };
    let rust = translate(&program(""));
    assert_contains(&rust, &["color: Option<AColor>", "APaint { color: None }", "this.color = Some(color);"]);
    // Used as an enum value, the field is unwrapped, which throws for null.
    let rust = translate(&program("int order() { return color.ordinal(); }"));
    assert_contains(&rust, &["java::non_null(self.color) as i32"]);
}

#[test]
fn an_enum_element_that_may_be_null_is_an_option_unwrapped_where_used_as_a_value() {
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
    let uses = [
        ("kinds[0].ordinal()", "java::non_null(kinds[0]) as i32"),
        ("switch (kinds[0]) { default -> 0; }", "match java::non_null(kinds[0])"),
        ("take(kinds[0])", "fn take(_k: Option<AKind>) -> i32"),
    ];
    for (use_, expected) in uses {
        let rust = translate(&format!(
            "public class A {{
                enum Kind {{ TOP, DEAD }}
                static int take(Kind k) {{ return 1; }}
                public static void main(String[] args) {{
                    Kind[] kinds = new Kind[2];
                    System.out.println({use_});
                }}
            }}"
        ));
        assert_contains(&rust, &[expected]);
    }
}

#[test]
fn a_catch_right_around_the_text_of_an_object_whose_to_string_throws_catches_it() {
    // The generator makes the text with a call of the `toString`, whose exception the `catch` sees.
    let rust = translate(
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
    );
    assert_contains(
        &rust,
        &[
            "fn to_string(&self) -> Result<java::JString, Throwable> {",
            "java::println!(\"item {}\", match Arc::new(AItem__new()).to_string() { Ok(v) => v, Err(e) => break 'attempt Err(e) });",
        ],
    );
}

#[test]
fn a_catch_around_the_text_of_an_enum_constant_whose_to_string_throws_sees_it() {
    // Where the generator makes the text, it calls the enum's `toString`, whose exception goes on
    // to the `catch`; `printf` makes its text through the enum's `Display`, which cannot.
    let program = |print: &str| {
        format!(
            "public class A {{
                enum Color {{
                    RED;
                    public String toString() {{ throw new IllegalStateException(\"no text\"); }}
                }}
                public static void main(String[] args) {{
                    try {{
                        {print}
                    }} catch (IllegalStateException e) {{
                        System.out.println(\"caught\");
                    }}
                }}
            }}"
        )
    };
    let rust = java_to_rust::translate(&program("System.out.println(\"color \" + Color.RED);")).unwrap();
    assert_contains(
        &rust,
        &[
            "java::println!(\"color {}\", match AColor::to_string(AColor::Red) { Ok(v) => v, Err(e) => break 'attempt Err(e) });",
        ],
    );
    let error = java_to_rust::translate(&program("System.out.printf(\"color %s%n\", Color.RED);"))
        .expect_err("an enum's toString whose exception a catch around a printf sees");
    assert!(
        error.to_string().ends_with("catching an exception that this toString may throw not supported yet"),
        "{error}"
    );
}

#[test]
fn a_catch_around_a_printf_of_an_object_whose_to_string_throws_is_rejected() {
    // `printf` makes its whole text before it prints, where Java prints `item ` first.
    let error = java_to_rust::translate(
        "public class A {
            static class Item {
                public String toString() { throw new IllegalStateException(\"no text\"); }
            }
            public static void main(String[] args) {
                try {
                    System.out.printf(\"item %s%n\", new Item());
                } catch (IllegalStateException e) {
                    System.out.println(\"caught\");
                }
            }
        }",
    )
    .expect_err("a toString that printf runs, whose exception a catch around it sees");
    assert!(
        error.to_string().ends_with("catching an exception that this toString may throw not supported yet"),
        "{error}"
    );
}

#[test]
fn a_hash_code_that_the_default_text_of_an_object_shows_passes_its_exception_on() {
    // `Object`'s `toString` shows the `hashCode`, which may throw what the `catch` catches: the
    // generator makes the text with a call of it, whose exception leaves the `try` block.
    let rust = translate(
        "public class A {
            static class Item {
                public int hashCode() { throw new IllegalStateException(\"no hash\"); }
            }
            public static void main(String[] args) {
                try {
                    System.out.println(\"item \" + new Item());
                } catch (IllegalStateException e) {
                    System.out.println(\"caught\");
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::default_text(\"A$Item\", match Arc::new(AItem__new()).hash_code() { Ok(v) => v, Err(e) => break 'attempt Err(e) })",
        ],
    );
}

#[test]
fn the_default_text_of_an_object_whose_hash_code_may_throw_ends_the_program_on_it() {
    let rust = translate(
        "public class A {
            static class Key {
                final int n;
                Key(int n) { this.n = n; }
                public int hashCode() { if (n < 0) throw new IllegalStateException(\"negative\"); return n; }
            }
            public static void main(String[] args) {
                try { new Key(-1).hashCode(); } catch (IllegalStateException e) { System.out.println(\"caught\"); }
                System.out.println(new Key(3));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn hash_code(&self) -> Result<i32, Throwable> {",
            "java::default_text(\"A$Key\", ",
            ".hash_code().unwrap_or_else(java::uncaught))",
        ],
    );
}

#[test]
fn a_catch_on_the_way_to_the_text_of_an_object_whose_to_string_throws_catches_it() {
    // The `catch` is around a call of the method that makes the text, which passes the exception on.
    let rust = translate(
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
    );
    assert_contains(
        &rust,
        &[
            "fn show(item: &Arc<AItem>) -> Result<(), Throwable> {",
            "java::println!(\"item {}\", match item.to_string() { Ok(v) => v, Err(e) => return Err(e) });",
            "match show(&Arc::new(AItem__new())) { Ok(v) => v, Err(e) => break 'attempt Err(e) };",
        ],
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
            "match kind.as_bytes() {",
            "b\"log\" => {",
            "match s.as_bytes() {",
            "match java::non_null(missing.as_deref()).as_bytes() {",
        ],
    );
    assert_lacks(&rust, &["'sw", "let mut case"]);
}

#[test]
fn a_string_switch_with_many_labels_matches_the_number_of_its_case() {
    // A match on bytes with thousands of string labels makes LLVM's register coalescing run for
    // most of an hour, so above a few dozen labels the label is looked up in a sorted table.
    let cases: String = (0..70).map(|i| format!("case \"k{i}\" -> {i};\n")).collect();
    let rust = translate(&format!(
        "class A {{
            static int code(String s) {{
                return switch (s) {{
                    {cases}
                    default -> -1;
                }};
            }}
            public static void main(String[] a) {{
                System.out.println(code(a.length == 0 ? \"k7\" : a[0]));
            }}
        }}"
    ));
    assert_contains(
        &rust,
        &[
            "java::string_case(",
            "const CASES: &[(&[u8], usize)] = &[(b\"k0\", 0), (b\"k1\", 1), (b\"k10\", 10),",
        ],
    );
    assert_lacks(&rust, &["b\"k7\" =>"]);
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
        &[
            "'sw1: {",
            "break 'sw1 java::JString::from(\"big\");",
            "break 'sw1 java::JString::from(\"negative\");",
        ],
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
            "next: Option<Arc<Node>>,",
            "while n.is_some() {",
            "s = s.wrapping_add(java::non_null(n.as_ref()).value);",
            "n = java::non_null(n.as_ref()).next.clone();",
            "fn sum(head: &Arc<Node>) -> i32 {",
            "let mut n: Option<Arc<Node>> = Some(head.clone());",
            "sum(&slot_1.put(Node__new(1, Some(Arc::new(Node__new(2, None))))))",
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
    assert_contains(&rust, &["from: Arc<Node>,", "to: Arc<Node>,", "Edge { from, to }"]);
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
            "java::println!(\"{}\", Arc::ptr_eq(&p, &q));",
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
            // Two strings may be the halves of a pair, so the line is made before it is printed.
            "java::println!(\"{}\", java::concat!(java::or_null(p.as_ref().map(|o| o.to_string())), \" \", java::or_null(q.as_ref().map(|o| o.to_string()))));",
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
            "let mut words: Vec<Option<java::JString>> = vec![None; 3];",
            "words[1] = Some(java::JString::from(\"mid\"));",
            "java::println!(\"{}\", java::concat!(words[0].as_deref().unwrap_or(java::wtf8(\"null\")), words[1].as_deref().unwrap_or(java::wtf8(\"null\"))));",
            "let rows: java::Array<Option<java::Array<i32>>> = java::filled(2, None);",
            "java::println!(\"{}\", java::get_in_bounds(&rows, 1).is_none());",
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
    assert_contains(&rust, &["fn twice(b: &Arc<Box_>) {", "twice(&x);", "fn keep(b: Arc<Box_>)"]);
}

#[test]
fn a_string_literal_with_an_unpaired_surrogate_is_its_wtf8_bytes() {
    let rust = translate(
        "class A { public static void main(String[] args) { System.out.println(\"a\\uD800b\".length()); } }",
    );
    assert_contains(&rust, &["const { java::wtf8_bytes(b\"a\\xed\\xa0\\x80b\") }"]);
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
    // type `Tag` is reached with the class it is on, for reflection to read.
    assert_contains(&report, &["Reachable from Main: 7 classes (3 instantiated), 7 methods"]);
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

/// A program in package `p` whose `main` reaches three methods that the translator rejects, each
/// for its own reason, and others that it translates.
fn census_sources(dir: &str) -> std::path::PathBuf {
    write_sources(
        dir,
        &[
            (
                "p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] a) {\n        Other.process();\n        Other.split(a);\n        Other.compare(new Name());\n        Other.fine();\n    }\n}\n",
            ),
            (
                "p/Other.java",
                "package p;\nclass Other {\n    static void process() {\n        ProcessBuilder t = null;\n    }\n    static void split(String[] a) {\n        try { System.out.println(\"a,b\".split(a[0]).length); }\n        catch (RuntimeException e) { System.out.println(\"bad pattern\"); }\n    }\n    static void compare(Comparable<String> c) {\n        System.out.println(c.compareTo(\"x\"));\n    }\n    static void fine() {\n        System.out.println(\"fine\");\n    }\n}\nclass Name implements Comparable<String> {\n    public int compareTo(String s) {\n        return s == \"x\" ? 0 : 1;\n    }\n}\n",
            ),
        ],
    )
}

/// The census of a program, which must not fail.
fn census(program: &java_to_rust::Program) -> java_to_rust::program::Census {
    java_to_rust::census_program(program).unwrap_or_else(|e| panic!("census failed: {e}"))
}

#[test]
fn the_census_reports_every_rejection_not_only_the_first() {
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![census_sources("census")],
        ..Default::default()
    };
    // The translation stops at the first rejection.
    let error = java_to_rust::translate_program(&program).expect_err("ProcessBuilder is not supported");
    assert!(error.to_string().ends_with("type `java.lang.ProcessBuilder` not supported yet"), "{error}");
    let census = census(&program);
    let report = &census.report;
    // Lowering and two analyses each reject one method; `main`, `compare`, `fine` and `Name`'s
    // constructor translate.
    assert_lacks(report, &["Incomplete"]);
    assert_contains(
        report,
        &[
            "  Program: 7 pieces; 4 (57.1%) translate, 3 rejected.",
            "type `…` not supported yet\n          at p/Other.java:4:9",
            "catching the `PatternSyntaxException` that this call of the library may throw not supported yet\n          at p/Other.java:7",
            "`==` on strings, where a string may come from where the translation does not follow it: a parameter of a method that the runtime calls\n          at p/Other.java:19",
        ],
    );
    let pieces = census.json["pieces"].as_array().expect("pieces");
    let errors = |name: &str| {
        let piece =
            pieces.iter().find(|p| p["name"] == name).unwrap_or_else(|| panic!("no {name} in {pieces:?}"));
        piece["errors"].as_array().expect("errors").len()
    };
    assert_eq!(
        (errors("process"), errors("split"), errors("compareTo"), errors("fine"), errors("main")),
        (1, 1, 1, 0, 0)
    );
    assert_eq!(census.json["complete"], true);
    assert_eq!(census.json["totals"]["program"]["rejected"], 3);
}

#[test]
fn a_census_that_writes_its_translation_ends_the_program_in_rejected_code() {
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![census_sources("census_written")],
        ends_rejected: true,
        ..Default::default()
    };
    let census = census(&program);
    assert_eq!(census.json["totals"]["program"]["rejected"], 3);
    let rust = census.modules.expect("a translation").one_file();
    // The methods that lowering and two analyses reject each end the program with
    // the rejection; `fine` translates.
    assert_contains(
        &rust,
        &[
            "java::not_translated(\"p.Other.process: type `java.lang.ProcessBuilder` not supported yet (line 4)\")",
            "java::not_translated(\"p.Other.split: catching the `PatternSyntaxException`",
            "java::not_translated(\"p.Name.compareTo: `==` on strings, where a string may come from where the translation does not follow it: a parameter of a method that the runtime calls (line 19)\")",
            "java::println!(\"fine\")",
        ],
    );
    assert_lacks(&rust, &["unreachable!()"]);
}

#[test]
fn a_census_stubs_a_rejected_statement_and_the_rest_of_its_block_not_the_whole_method() {
    // `zone` is rejected for the `Optional` that its `StackWalker.walk` returns, behind a test that is false in this run, as
    // jtracy's `TracyClient.beginZone` is when the profiler is not loaded: the rest of `zone`
    // runs. `twice` is rejected for two statements, each cut with the rest of its block, and the
    // static initializer for one, after which `ready` is still incremented.
    let dir = write_sources(
        "census_cut_statements",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    static boolean loaded = Boolean.getBoolean(\"loaded\");\n    static int ready;\n    static {\n        ready = 1;\n        if (loaded) {\n            ProcessBuilder p = null;\n        }\n        ready += 1;\n    }\n    static void zone(String name, boolean capture) {\n        System.out.println(\"zone \" + name);\n        if (loaded && capture) {\n            System.out.println(\"walking\");\n            Object frame = StackWalker.getInstance().walk(s -> s.findFirst());\n            System.out.println(frame);\n        }\n        System.out.println(\"after \" + name);\n    }\n    static int twice(int n) {\n        if (n > 10) {\n            ProcessBuilder b = null;\n        }\n        if (n > 20) {\n            ProcessBuilder c = null;\n        }\n        return n * 2;\n    }\n    public static void main(String[] a) {\n        zone(\"a\", true);\n        System.out.println(twice(a.length) + ready);\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let census = census(&program);
    let pieces = census.json["pieces"].as_array().expect("pieces");
    let errors = |name: &str| {
        let piece =
            pieces.iter().find(|p| p["name"] == name).unwrap_or_else(|| panic!("no {name} in {pieces:?}"));
        piece["errors"].as_array().expect("errors").len()
    };
    assert_eq!((errors("zone"), errors("twice"), errors("main")), (1, 2, 0));
    let rust = census.modules.expect("a translation").one_file();
    assert_contains(
        &rust,
        &[
            "java::not_translated(\"p.Main.zone: type `java.util.Optional` not supported yet (line 16)\")",
            "java::println!(\"walking\");\n            java::not_translated(\"p.Main.zone",
            "java::println!(\"after {name}\")",
            "java::not_translated(\"p.Main.twice: type `java.lang.ProcessBuilder` not supported yet (line 23)\")",
            "java::not_translated(\"p.Main.twice: type `java.lang.ProcessBuilder` not supported yet (line 26)\")",
            "java::not_translated(\"p.Main: type `java.lang.ProcessBuilder` not supported yet (line 8)\")",
        ],
    );
}

#[test]
fn a_census_stubs_only_the_code_of_the_file_with_the_rejection() {
    // The generator rejects the local `tasks` of `A.drain` at line 4, column 9, where `B.fine`
    // declares a local and `Main.main` makes a call too.
    let dir = write_sources(
        "census_one_file",
        &[
            (
                "p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(A.drain(a.length > 5) != null);\n        B.fine();\n    }\n}\n",
            ),
            (
                "p/A.java",
                "package p;\nclass A {\n    static Object drain(boolean fail) {\n        Object tasks;\n        try {\n            switch (fail ? 1 : 0) {\n                case 0: if (fail) { tasks = \"early\"; break; } tasks = new Object(); break;\n                default:\n                    throw new IllegalStateException(\"no\");\n            }\n        } catch (IllegalStateException e) {\n            tasks = e;\n        }\n        return tasks;\n    }\n}\n",
            ),
            (
                "p/B.java",
                "package p;\nclass B {\n    static void fine() {\n        Object o = \"b\";\n        System.out.println(o);\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let census = census(&program);
    assert_eq!(census.json["totals"]["program"]["rejected"], 1);
    let rust = census.modules.expect("a translation").one_file();
    assert_contains(
        &rust,
        &["java::not_translated(\"p.A.drain: a local of type `Object` declared without a value"],
    );
    assert_lacks(&rust, &["java::not_translated(\"p.B.fine", "java::not_translated(\"p.Main.main"]);
}

#[test]
fn a_census_that_keeps_going_rejects_the_code_that_javac_reports_errors_in() {
    let dir = write_sources(
        "census_keep_going",
        &[
            (
                "p/Main.java",
                "package p;\npublic class Main {\n    static int count = Gone.value;\n    public static void main(String[] a) {\n        System.out.println(Other.fine() + count);\n        Other.bad();\n        Runnable r = () -> System.out.println(Other.fine());\n        r.run();\n        new Broken().run();\n    }\n}\n",
            ),
            (
                "p/Other.java",
                "package p;\nclass Other {\n    @Missing\n    static int fine() { return 4; }\n    static void bad() { System.out.println(missing()); }\n}\n",
            ),
            (
                "p/Broken.java",
                "package p;\nclass Broken extends Unknown {\n    void run() { System.out.println(1); }\n}\n",
            ),
        ],
    );
    let mut program =
        java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    // Without it, the first compile error stops the census.
    let error = java_to_rust::census_program(&program).err().expect("the sources have compile errors");
    assert!(error.to_string().contains("cannot find symbol"), "{error}");
    program.keep_going = true;
    let census = census(&program);
    // The method, the field's initializer and the class that have the errors are rejected; the
    // rest translates, `fine` too, whose annotation is missing: annotations are left out. `Gone`
    // and `Unknown` are not taken as missing classes: no import names them.
    assert_contains(
        &census.report,
        &[
            "javac reports a compile error (compiler.err.cant.resolve.location)",
            "at p/Main.java:3:",
            "at p/Other.java:5:",
            "at p/Broken.java:2:",
        ],
    );
    let pieces = census.json["pieces"].as_array().expect("pieces");
    let errors = |name: &str| {
        let piece =
            pieces.iter().find(|p| p["name"] == name).unwrap_or_else(|| panic!("no {name} in {pieces:?}"));
        piece["errors"].as_array().expect("errors").len()
    };
    assert_eq!((errors("fine"), errors("main"), errors("bad")), (0, 0, 1));
}

#[test]
fn a_missing_annotation_leaves_the_names_of_the_class_files_of_its_file() {
    // javac reports an error for `@Missing`, which leaves the code as it is: the message of the
    // `NullPointerException` still names what the class file names.
    let dir = write_sources(
        "missing_annotation_names",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    @Missing\n    static int kind = 1;\n    static String name(Main m) { return m.toString(); }\n    public static void main(String[] a) {\n        try {\n            name(null);\n        } catch (NullPointerException e) {\n            System.out.println(e.getMessage());\n        }\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        keep_going: true,
        ..Default::default()
    };
    let rust = java_to_rust::translate_program(&program).expect("only the annotation is missing");
    assert_contains(&rust, &["Cannot invoke \\\"Object.toString()\\\" because \\\"<parameter1>\\\" is null"]);
}

#[test]
fn a_superclass_with_errors_leaves_the_names_of_the_classes_that_extend_it() {
    // javac's code generator erases the types of `Base` before `Main`'s, and fails on its call
    // with too few arguments: `Main`'s names do not depend on it.
    let dir = write_sources(
        "superclass_errors_names",
        &[
            (
                "p/Base.java",
                "package p;\npublic class Base {\n    void f(int a, int b) {}\n    void g() { f(); }\n}\n",
            ),
            (
                "p/Main.java",
                "package p;\npublic class Main extends Base {\n    static String name(Main m) { return m.toString(); }\n    public static void main(String[] a) {\n        try {\n            name(null);\n        } catch (NullPointerException e) {\n            System.out.println(e.getMessage());\n        }\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        keep_going: true,
        ..Default::default()
    };
    let rust = java_to_rust::translate_program(&program).expect("Base's error is not reached");
    assert_contains(&rust, &["Cannot invoke \\\"Object.toString()\\\" because \\\"<parameter1>\\\" is null"]);
}

#[test]
fn the_census_lists_handle_calls_that_throw_where_the_translation_does_not_follow_them() {
    let java = "package p;\nimport java.lang.invoke.*;\npublic class Main {\n    static MethodHandle find() {\n        try {\n            return MethodHandles.publicLookup().findVirtual(Thread.class, \"isVirtual\", MethodType.methodType(boolean.class));\n        } catch (Throwable e) {\n            return null;\n        }\n    }\n    static boolean lengths(MethodHandle h) throws Throwable {\n        return (int) MethodHandles.dropArguments(h, 0, int.class).invokeExact(1) > 0;\n    }\n    public static void main(String[] a) throws Throwable {\n        System.out.println(find() == null);\n        if (a.length > 0) {\n            System.out.println(lengths(find()));\n        }\n    }\n}\n";
    let dir = write_sources("census_unfollowed_handles", &[("p/Main.java", java)]);
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    // The calls translate, into code that throws where Java runs them: none is a rejection, and
    // the census lists each, by what was not followed, with its place.
    let census = census(&program);
    let not_followed = "of a handle or arguments that the translation does not follow";
    assert_contains(
        &census.report,
        &[
            "Program: 3 pieces; 3 (100.0%) translate, 0 rejected.",
            "Program code: pieces that translate into code that throws UnsupportedOperationException for a \
             handle call that the translation does not follow, where Java runs it",
            &format!(
                "  throws UnsupportedOperationException: `Lookup.findVirtual` {not_followed}, or that may \
                 fail, 1 piece\n          at p/Main.java:6:20"
            ),
            &format!(
                "  throws UnsupportedOperationException: `MethodHandles.dropArguments` {not_followed}, 1 \
                 piece\n          at p/Main.java:12:22"
            ),
        ],
    );
    let rows = census.json["unfollowedHandles"].as_array().expect("unfollowedHandles");
    assert_eq!(rows.len(), 3, "{rows:?}");
    let find =
        rows.iter().find(|r| r["message"].as_str().is_some_and(|m| m.starts_with("`Lookup.findVirtual`")));
    let find = find.expect("findVirtual is listed");
    assert_eq!((&find["source"], &find["pieces"]), (&"program".into(), &1.into()), "{find}");
    assert_eq!(find["places"][0], "p/Main.java:6:20", "{find}");
    assert_eq!(census.json["rejections"].as_array().map(Vec::len), Some(0));
    let pieces = census.json["pieces"].as_array().expect("pieces");
    let piece = pieces.iter().find(|p| p["name"] == "find").expect("find is a piece");
    assert_eq!(piece["errors"].as_array().map(Vec::len), Some(0));
    assert_eq!(piece["unfollowedHandles"][0]["place"], "p/Main.java:6:20", "{piece}");
}

#[test]
fn the_census_lists_charset_names_known_only_at_run_time() {
    let java = "package p;\nimport java.nio.charset.Charset;\npublic class Main {\n    static boolean has(String name) {\n        return Charset.isSupported(name);\n    }\n    static byte[] bytes(String s, String charset) throws Exception {\n        return s.getBytes(charset);\n    }\n    public static void main(String[] a) throws Exception {\n        System.out.println(Charset.forName(\"windows-1252\").name() + \" \" + has(\"KOI8-R\"));\n        System.out.println(bytes(\"x\", \"UTF-8\").length + \" \" + new String(new byte[] {65}, \"cp1252\"));\n    }\n}\n";
    let dir = write_sources("census_charset_names", &[("p/Main.java", java)]);
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    // A computed name may name a charset of the JDK that the translation does not have, where
    // the program throws: no rejection, but listed with its place. A constant name is not.
    let census = census(&program);
    assert_contains(
        &census.report,
        &[
            "Program: 3 pieces; 3 (100.0%) translate, 0 rejected.",
            "Program code: pieces that translate into code that throws UnsupportedOperationException \
             where a charset name known only at run time names a charset of the JDK that the \
             translation does not have, where Java finds it",
            "  may throw UnsupportedOperationException: `Charset.isSupported` of a charset name known \
             only at run time, 1 piece\n          at p/Main.java:5:16",
            "  may throw UnsupportedOperationException: `String.getBytes` of a charset name known only \
             at run time, 1 piece\n          at p/Main.java:8:16",
        ],
    );
    let rows = census.json["charsetNames"].as_array().expect("charsetNames");
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(census.json["rejections"].as_array().map(Vec::len), Some(0));
    let pieces = census.json["pieces"].as_array().expect("pieces");
    let has = pieces.iter().find(|p| p["name"] == "has").expect("has is a piece");
    assert_eq!(has["charsetNames"][0]["place"], "p/Main.java:5:16", "{has}");
    let main = pieces.iter().find(|p| p["name"] == "main").expect("main is a piece");
    assert_eq!(main["charsetNames"].as_array().map(Vec::len), Some(0), "{main}");
}

#[test]
fn a_constant_name_of_a_jdk_charset_without_a_table_is_rejected_and_a_computed_one_throws() {
    for call in
        ["Charset.forName(\"Shift_JIS\")", "\"x\".getBytes(\"sjis\")", "new String(new byte[0], \"GBK\")"]
    {
        let java = format!(
            "import java.nio.charset.Charset;\npublic class Main {{\n    public static void main(String[] a) throws Exception {{\n        System.out.println({call});\n    }}\n}}\n"
        );
        let error = java_to_rust::translate(&java).expect_err("a charset of the JDK without a table");
        assert!(
            error
                .message
                .ends_with("a charset of the JDK that the translation does not have, not supported yet"),
            "{call}: {error}"
        );
    }
    // A computed name: the program carries the tables and the names of the JDK's other charsets.
    let rust = translate(
        "import java.nio.charset.Charset;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(Charset.forName(a[0]));\n    }\n}\n",
    );
    assert_contains(
        &rust,
        &[
            "java::ByteCharset::new(\"windows-1252\", \"Cp1252\", &[\"cp1252\"",
            "\"Shift_JIS\"",
            "java::register_charsets(&CHARSETS, &[",
        ],
    );
}

#[test]
fn code_that_names_a_missing_class_throws_no_class_def_found_error_and_the_census_lists_it() {
    let dir = write_sources(
        "missing_class",
        &[
            (
                "app/p/Main.java",
                "package p;\nimport lib.Codec;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(Codec.encode(a.length > 0));\n        System.out.println(Codec.broken());\n    }\n}\n",
            ),
            (
                "lib/lib/Codec.java",
                "package lib;\nimport fast.Native;\npublic class Codec {\n    public static String encode(boolean fast) {\n        if (fast) {\n            return Native.encode();\n        }\n        return \"plain\";\n    }\n    public static int broken() { return missing(); }\n}\n",
            ),
        ],
    );
    let mut program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    // Without going on, the first compile error stops the translation.
    let error = java_to_rust::translate_program(&program).expect_err("fast.Native is missing");
    assert!(error.to_string().contains("does not exist"), "{error}");
    program.keep_going = true;
    // The branch that names the missing class throws as on the JVM; `broken`, whose error is not
    // a missing class, is rejected.
    let census = census(&program);
    assert_contains(
        &census.report,
        &[
            "Library code: pieces that translate, where a class is missing, into code that throws",
            "  throws NoClassDefFoundError like Java: missing class fast/Native, 1 piece\n          at lib/Codec.java:4",
            "javac reports a compile error (compiler.err.cant.resolve.location.args)",
        ],
    );
    let missing = &census.json["missingClasses"];
    assert_eq!(missing[0]["class"], "fast/Native", "{missing}");
    assert_eq!(missing[0]["pieces"], 1, "{missing}");
    let pieces = census.json["pieces"].as_array().expect("pieces");
    let encode = pieces.iter().find(|p| p["name"] == "encode").expect("encode is a piece");
    assert_eq!(encode["missingClass"], "fast/Native");
    assert_eq!(encode["errors"].as_array().map(Vec::len), Some(0));
    // A translation that goes on stops only at the other error, and without it translates, with
    // the branch throwing.
    let error = java_to_rust::translate_program(&program).expect_err("broken is rejected");
    assert!(error.to_string().contains("lib/Codec.java:10:5: javac reports a compile error"), "{error}");
    let codec = dir.join("lib/lib/Codec.java");
    let java = std::fs::read_to_string(&codec).unwrap().replace("return missing();", "return 1;");
    std::fs::write(&codec, java).unwrap();
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(&rust, &["NoClassDefFoundError__new(java::wtf8(\"fast/Native\"))", "\"plain\""]);
}

#[test]
fn a_super_call_that_names_a_missing_class_throws_before_the_superclass_part_is_made() {
    // javac's rewrite replaces the `super(..)` with a throw, so the constructor has no superclass
    // part to put in its object: it throws before the object is made (netty's
    // `OpenSslServerContext`, whose `super(..)` reads a constant of the native library's `SSL`).
    let dir = write_sources(
        "missing_class_super",
        &[(
            "p/Main.java",
            "package p;\nimport missing.Native;\npublic class Main {\n    static class Context {\n        final int mode;\n        Context(int mode) { this.mode = mode; }\n    }\n    static final class Server extends Context {\n        final String name;\n        Server(String name) {\n            super(Native.MODE);\n            this.name = name;\n        }\n    }\n    public static void main(String[] a) {\n        System.out.println(new Context(a.length).mode);\n        System.out.println(new Server(\"s\").name);\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        keep_going: true,
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &["NoClassDefFoundError__new(java::wtf8(\"missing/Native\"))", "base: unreachable!()"],
    );
}

#[test]
fn a_missing_class_of_an_annotation_is_ignored_as_on_the_jvm_without_going_on() {
    let dir = write_sources(
        "missing_class_annotation",
        &[
            (
                "app/p/Main.java",
                "package p;\nimport lib.Codec;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(Codec.encode(\"x\"));\n    }\n}\n",
            ),
            (
                "lib/lib/Codec.java",
                "package lib;\nimport marks.Mark;\nimport marks.Unused;\nimport static marks.Level.HIGH;\n@Mark(level = HIGH)\npublic class Codec {\n    @Mark(level = marks.Level.LOW)\n    public static String encode(@Mark String s) {\n        @Mark String t = s + \"!\";\n        return t;\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    // The annotations' classes and their imports are missing, which the JVM ignores.
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(&rust, &["\"!\""]);
    // A missing class that code uses outside annotations is an error, and so is its import.
    let codec = dir.join("lib/lib/Codec.java");
    let java = std::fs::read_to_string(&codec).unwrap().replace("return t;", "return t + Mark.class;");
    std::fs::write(&codec, java).unwrap();
    let error = java_to_rust::translate_program(&program).expect_err("Mark is used as a class");
    assert!(error.to_string().contains("does not exist"), "{error}");
}

#[test]
fn the_census_rejects_a_class_whose_declaration_is_rejected_with_its_code_and_goes_on() {
    let dir = write_sources(
        "census_class",
        &[
            (
                "p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] a) {\n        System.out.println(new Loader().n() + Other.fine());\n        Number x = Integer.valueOf(5);\n        System.out.println(x.intValue());\n    }\n}\n",
            ),
            (
                "p/Loader.java",
                "package p;\nclass Loader extends java.security.SecureClassLoader {\n    int count = 3;\n    { count++; }\n    int n() { return count; }\n}\n",
            ),
            ("p/Other.java", "package p;\nclass Other {\n    static int fine() { return 4; }\n}\n"),
        ],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let error = java_to_rust::translate_program(&program).expect_err("SecureClassLoader is not translated");
    assert!(
        error
            .to_string()
            .ends_with("p/Loader.java:2:22: extending `java.security.SecureClassLoader` not supported yet"),
        "{error}"
    );
    // The class's constructor, field initializer, initializer block and method are rejected with
    // it, and `main`, which uses the class, and `fine` translate. (With `Number` in the program,
    // lowering asks for the superclass of every class with a method without parameters, which
    // once ended the census at this error.)
    let census = census(&program);
    assert_lacks(&census.report, &["Incomplete"]);
    assert_contains(
        &census.report,
        &[
            "  Program: 6 pieces; 2 (33.3%) translate, 4 rejected.",
            "       4  extending `…` not supported yet\n          at p/Loader.java:2:22\n",
        ],
    );
    assert_eq!(census.json["complete"], true);
}

#[test]
fn the_census_stubs_an_exception_that_prints_its_own_stack_trace_and_goes_on() {
    // `Loud` overrides `printStackTrace(PrintStream)`, which the translation would not run where
    // Java does. `main` is rejected (`PrintStream` needs the JDK's sources), so the override
    // becomes a stub that runs none of its code, and the census goes on instead of stopping at it.
    let dir = write_sources(
        "census_print_stack_trace",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    public static void main(String[] a) {\n        new Loud().printStackTrace((java.io.PrintStream) null);\n    }\n}\nclass Loud extends RuntimeException {\n    @Override\n    public void printStackTrace(java.io.PrintStream s) { throw new IllegalStateException(\"loud\"); }\n}\n",
        )],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let census = census(&program);
    assert_lacks(&census.report, &["Incomplete"]);
    assert_eq!(census.json["complete"], true);
}

#[test]
fn a_census_stubs_the_code_whose_null_pointer_message_is_not_certain() {
    // `len` reads its parameter in a `finally` block whose copies name it differently, one after
    // the write and one before; `main`'s message is certain.
    let dir = write_sources(
        "census_messages",
        &[(
            "Main.java",
            "public class Main {\n    static int len(String s, boolean b) {\n        try { if (b) s = null; } finally { System.out.print(s.length()); }\n        return 0;\n    }\n    public static void main(String[] args) {\n        String t = args.length > 3 ? \"a\" : null;\n        try { System.out.println(t.length() + len(t, false)); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let census = census(&program);
    assert_contains(
        &census.report,
        &[
            "  Program: 2 pieces; 1 (50.0%) translate, 1 rejected.",
            "       1  the message of the `NullPointerException` of this dereference not supported yet\n          at Main.java:3:61\n",
        ],
    );
    let rust = census.modules.expect("a translation").one_file();
    assert_contains(
        &rust,
        &[
            "java::not_translated(\"Main.len: the message of the `NullPointerException` of this dereference not supported yet (line 3)\")",
            "Cannot invoke \\\"String.length()\\\" because \\\"<local1>\\\" is null",
        ],
    );
}

#[test]
fn a_census_approximates_the_null_pointer_message_that_is_not_certain() {
    // As above, with `--approximate-messages`: `len`'s message is the JVM's words for what failed
    // alone, and the census lists it rather than rejecting `len`.
    let dir = write_sources(
        "census_approximated_messages",
        &[(
            "Main.java",
            "public class Main {\n    static int len(String s, boolean b) {\n        try { if (b) s = null; } finally { System.out.print(s.length()); }\n        return 0;\n    }\n    public static void main(String[] args) {\n        String t = args.length > 3 ? \"a\" : null;\n        try { System.out.println(t.length() + len(t, false)); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        approximate_messages: true,
        ..Default::default()
    };
    let census = census(&program);
    assert_contains(
        &census.report,
        &[
            "  Program: 2 pieces; 2 (100.0%) translate, 0 rejected.",
            "  approximates the message of the `NullPointerException` of this dereference, 1 piece\n          at Main.java:3:61\n",
        ],
    );
    assert_eq!(census.json["approximatedMessages"][0]["pieces"], 1);
    let rust = census.modules.expect("a translation").one_file();
    assert_contains(
        &rust,
        &[
            "match s.as_deref() { Some(v) => v, None => return Err(Throwable::j2r_null_pointer(\"Cannot invoke \\\"String.length()\\\"\")) }",
            "Cannot invoke \\\"String.length()\\\" because \\\"<local1>\\\" is null",
        ],
    );
}

#[test]
fn a_library_class_reached_only_by_name_has_null_pointer_messages() {
    // No code names `q.Hidden`, so javac would not compile it from the library's root: the
    // names of its class file come from compiling the files that the translation reached.
    let dir = write_sources(
        "messages_by_name",
        &[
            (
                "app/p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] a) throws Exception {\n        try { System.out.println(q.Factories.make().size(null)); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n    }\n}\n",
            ),
            ("lib/q/Factory.java", "package q;\npublic interface Factory {\n    int size(String s);\n}\n"),
            (
                "lib/q/Hidden.java",
                "package q;\nclass Hidden implements Factory {\n    public int size(String s) {\n        return s.length();\n    }\n}\n",
            ),
            (
                "lib/q/Factories.java",
                "package q;\npublic class Factories {\n    public static Factory make() throws Exception {\n        return (Factory) Class.forName(\"q.Hidden\").getDeclaredConstructor().newInstance();\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(&rust, &["Cannot invoke \\\"String.length()\\\" because \\\"<parameter1>\\\" is null"]);
}

#[test]
fn a_census_takes_a_native_that_the_runtime_lacks_to_take_null() {
    // A JNI native takes a null string as it is: the census lists the native, and its call
    // raises no `NullPointerException` whose message would have to be found.
    let dir = write_sources(
        "census_native_null",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    static native long leak(String name);\n    static boolean loaded = Boolean.getBoolean(\"x\");\n    static long plot(String name) {\n        return loaded ? leak(name) : 0;\n    }\n    public static void main(String[] a) {\n        try { System.out.println(plot(a.length > 0 ? a[0] : null)); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let census = census(&program);
    assert_contains(&census.report, &["p.Main.leak"]);
    assert_lacks(&census.report, &["NullPointerException"]);
}

#[test]
fn a_census_takes_the_arguments_of_a_native_that_the_runtime_lacks_as_code_that_never_runs() {
    // The call of a JNI native is written as code that never runs, without its arguments: the
    // dereference in an argument, inside a `try` that catches its `NullPointerException`, is no
    // dereference that the code has to check.
    let dir = write_sources(
        "census_native_argument",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    static native long leak(int size);\n    static boolean loaded = Boolean.getBoolean(\"x\");\n    static long plot(String name) {\n        return loaded ? leak(name.length()) : 0;\n    }\n    public static void main(String[] a) {\n        try { System.out.println(plot(a.length > 0 ? a[0] : null)); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let census = census(&program);
    assert_contains(&census.report, &["p.Main.leak"]);
    assert_lacks(&census.report, &["NullPointerException"]);
}

#[test]
fn a_census_gives_the_value_of_a_native_that_the_runtime_lacks_the_type_of_the_call() {
    // The value of a native of the JDK that the runtime lacks is code that never runs; a local
    // declared with it, a comparison and a borrow of it take the type of the call, as `!` can do
    // none of these. (A class outside the JDK's modules would be a library's, whose natives throw
    // `UnsatisfiedLinkError`.)
    let dir = write_sources(
        "census_native_type",
        &[
            ("module-info.java", "module jdk.census {}\n"),
            (
                "jdk/census/Main.java",
                "package jdk.census;\npublic class Main {\n    static native int level();\n    static native String name();\n    static boolean loaded = Boolean.getBoolean(\"x\");\n    static int size(String s) {\n        return s.length();\n    }\n    public static void main(String[] a) {\n        if (loaded) {\n            int l = level();\n            System.out.println(l >= 0 ? size(name()) : 0);\n        }\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "jdk.census.Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let rust = census(&program).modules.expect("a translation").one_file();
    assert_contains(
        &rust,
        &[
            "let never: i32 = unreachable!(\"the native method jdk.census.Main.level()int never runs\"); never }",
            "let never: java::JString = unreachable!(\"the native method jdk.census.Main.name()java.lang.String never runs\"); never }",
        ],
    );
}

#[test]
fn a_census_stubs_the_bridge_of_a_lambda_whose_parameter_type_is_not_supported() {
    // The lambda's own `accept(ProcessBuilder)` is a stub with an `int` for the type; its bridge,
    // the interface's `accept(Object)`, ends the program the same way rather than pass the object
    // on (`LogRecord`'s lambdas over `StackWalker.StackFrame`).
    let dir = write_sources(
        "census_stubbed_bridge",
        &[(
            "p/Main.java",
            "package p;\ninterface Sink<T> {\n    void accept(T t);\n}\npublic class Main {\n    static <T> void feed(Sink<T> s, T t) {\n        if (t != null) s.accept(t);\n    }\n    public static void main(String[] a) {\n        Sink<ProcessBuilder> print = b -> System.out.println(b);\n        feed(print, null);\n        System.out.println(print == print);\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let rust = census(&program).modules.expect("a translation").one_file();
    let stub = "type `java.lang.ProcessBuilder` not supported yet (line 10)\")";
    assert_eq!(rust.matches(stub).count(), 2, "{rust}");
    assert!(!rust.contains("accept_i32(t"), "{rust}");
}

#[test]
fn a_census_rejects_the_methods_that_a_rejected_enums_constant_bodies_override() {
    // Two constants' bodies declare a helper of one name, which lowering rejects with the enum:
    // `test`, which each body implements for the interface, has no switch over the constants
    // and no body of its own (Guava's `Predicates.ObjectPredicate.apply`), so it is a stub.
    let dir = write_sources(
        "census_enum_bodies",
        &[(
            "p/Main.java",
            "package p;\nimport java.util.function.Predicate;\npublic class Main {\n    enum Always implements Predicate<Object> {\n        YES { public boolean test(Object o) { return true; } void help() {} },\n        NO { public boolean test(Object o) { return false; } void help() {} };\n    }\n    public static void main(String[] a) {\n        Predicate<Object> p = Always.YES;\n        System.out.println(p.test(a));\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let rust = census(&program).modules.expect("a translation").one_file();
    assert_contains(
        &rust,
        &[
            "java::not_translated(\"p.Main.Always.test: methods of one name and signature that override nothing in two enum constants' bodies not supported yet (line 6)\")",
        ],
    );
}

#[test]
fn a_census_calls_a_native_that_the_runtime_lacks_through_the_interface_it_implements() {
    // `VMManagementImpl`'s natives implement `VMManagement`'s methods: a call through the
    // interface runs the native of the implementing class, which the census lists.
    let dir = write_sources(
        "census_native_dispatch",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    interface Management {\n        long total();\n    }\n    static class Impl implements Management {\n        public native long total();\n    }\n    static long count(Management m) {\n        return m.total();\n    }\n    public static void main(String[] a) {\n        System.out.println(a.length > 3 ? count(new Impl()) : 0);\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let census = census(&program);
    assert_contains(&census.report, &["p.Main.Impl.total"]);
    let rust = census.modules.expect("a translation").one_file();
    assert_contains(&rust, &["__total(this: &"]);
}

/// A program whose census takes a round of the analyses for each kind of check that rejects code
/// (`a_census_stubs_what_each_check_of_the_analyses_rejects_round_by_round`).
const ROUNDS: &str = r#"public class Main {
    static final String LOCK = "lock";
    static int n;

    static boolean blank(Object o) {
        return !(o instanceof String s) || s.isBlank();
    }

    static void count() {
        synchronized (LOCK) {
            n++;
        }
    }

    static int len(String s, boolean b) {
        try {
            if (b) s = null;
        } finally {
            System.out.print(s.length());
        }
        return 0;
    }

    static void compare(Comparable<String> c) {
        System.out.println(c.compareTo("x"));
    }

    static void level() {
        try {
            System.out.printf("level %s%n", Level.LOW);
        } catch (IllegalStateException e) {
            System.out.println("no level");
        }
    }

    public static void main(String[] args) throws InterruptedException {
        System.out.println(blank(args.length > 2 ? (Object) 1 : " "));
        Thread t = new Thread(Main::count);
        t.start();
        t.join();
        System.out.println(n);
        compare(new Name());
        level();
        String u = args.length > 3 ? "a" : null;
        try {
            System.out.println(u.length() + len(u, false));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}

class Name implements Comparable<String> {
    public int compareTo(String s) {
        return s == "x" ? 0 : 1;
    }
}

enum Level {
    LOW;

    public String toString() {
        throw new IllegalStateException("no text");
    }
}
"#;

/// A method whose local the generator rejects (an `Object` declared without a value, which a case
/// of a switch in a `try` sets before a `break` in the middle of the case).
const DRAIN: &str = r#"
class Drain {
    static Object drain(boolean fail) {
        Object tasks;
        try {
            switch (fail ? 1 : 0) {
                case 0:
                    if (fail) { tasks = "early"; break; }
                    tasks = new Object();
                    break;
                default:
                    throw new IllegalStateException("no");
            }
        } catch (IllegalStateException e) {
            tasks = e;
        }
        return tasks;
    }
}
"#;

#[test]
fn a_census_stubs_what_each_check_of_the_analyses_rejects() {
    // The checks of the analyses reject a pattern variable and `synchronized` on a string (the
    // checks that come before the range analysis and nullness), a `toString` whose exception a
    // `catch` around a `printf` sees (exceptions) and the message of a `NullPointerException` (after the analyses); the
    // census stubs the code of each.
    let dir = write_sources("census_rounds", &[("Main.java", ROUNDS)]);
    let program = java_to_rust::Program { main: "Main".into(), sources: vec![dir], ..Default::default() };
    let census = census(&program);
    assert_contains(
        &census.report,
        &[
            "  Program: 12 pieces; 7 (58.3%) translate, 5 rejected.",
            "a parameter of a method that the runtime calls\n          at Main.java:55:16\n",
            "`synchronized` on a string not supported yet\n          at Main.java:10:9\n",
            "a pattern variable used here not supported yet\n          at Main.java:6:18\n",
            "catching an exception that this toString may throw not supported yet\n          at Main.java:30:45\n",
            "the message of the `NullPointerException` of this dereference not supported yet\n          at Main.java:19:30\n",
        ],
    );
}

#[test]
fn a_census_stubs_what_the_checks_of_the_code_reject_before_the_analyses_run() {
    // `synchronized` on a string, in a program that starts a thread, is rejected by a check that
    // reads the code alone: the census stubs it before the analyses first run, which then run
    // once.
    let main = r#"public class Main {
    static final String LOCK = "lock";
    static int n;

    static void count() {
        synchronized (LOCK) {
            n++;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Thread t = new Thread(Main::count);
        t.start();
        t.join();
        System.out.println(n);
    }
}
"#;
    let dir = write_sources("census_code_checks", &[("Main.java", main)]);
    let program = java_to_rust::Program { main: "Main".into(), sources: vec![dir], ..Default::default() };
    let (census, rounds) = java_to_rust::program::rounds_of_analyses(|| census(&program));
    assert_contains(
        &census.report,
        &["`synchronized` on a string not supported yet\n          at Main.java:6:9\n"],
    );
    assert_eq!(rounds, 1, "{}", census.report);
}

#[test]
fn a_census_stubs_a_class_converted_to_an_interface_the_runtime_lacks_before_the_analyses_run() {
    // A class as an `AnnotatedElement` or a `TypeDescriptor.OfField`, which `java.lang.Class`
    // implements but the runtime's classes do not, is rejected by the generator; the census finds
    // it in the code before the analyses first run, which then run once.
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(std::path::PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this test");
        return;
    };
    let main = r#"import java.lang.invoke.TypeDescriptor;
import java.lang.reflect.AnnotatedElement;

public class Main {
    static String shown(AnnotatedElement e) {
        return String.valueOf(e);
    }

    static String descriptor(TypeDescriptor.OfField<?> d) {
        return d.descriptorString();
    }

    static void annotated() {
        System.out.println(shown(Main.class));
    }

    static void primitive() {
        System.out.println(descriptor(int.class));
    }

    public static void main(String[] args) {
        if (args.length > 0) {
            annotated();
            primitive();
        }
        System.out.println("done");
    }
}
"#;
    let dir = write_sources("census_class_interfaces", &[("app/Main.java", main)]);
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![java_base],
        ..Default::default()
    };
    let (census, rounds) = java_to_rust::program::rounds_of_analyses(|| census(&program));
    assert_contains(
        &census.report,
        &[
            "  Program: 5 pieces; 3 (60.0%) translate, 2 rejected.",
            "1  a `java.lang.Class` as a `java.lang.invoke.TypeDescriptor.OfField` not supported yet\n",
            "1  a `java.lang.Class` as a `java.lang.reflect.AnnotatedElement` not supported yet\n",
            "          at Main.java:14:34, Main.java:18:39\n",
        ],
    );
    assert_eq!(rounds, 1, "{}", census.report);
}

#[test]
fn the_generator_writes_the_same_on_several_threads() {
    // The generator can write the modules of a round on several threads, each with a generator
    // of its own (`codegen::on_threads`). Programs of several packages translate the same on one
    // thread and on three, and so does a census whose generator rejects code, with the JDK's
    // packages as modules.
    for (project, main) in
        [("zoo", "zoo.app.Main"), ("library", "library.app.Main"), ("statics", "com.shop.Main")]
    {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/projects").join(project);
        let program = java_to_rust::Program {
            main: main.into(),
            sources: vec![root.join("src")],
            ..Default::default()
        };
        let translate = |threads| {
            java_to_rust::codegen::on_threads(threads, || java_to_rust::translate_program_modules(&program))
                .unwrap_or_else(|e| panic!("translation failed: {e}"))
        };
        let (one, three) = (translate(1), translate(3));
        assert!(one.modules.len() > 2, "{project} has modules");
        assert_eq!(one.one_file(), three.one_file(), "{project}");
        assert_eq!(one.dependencies(), three.dependencies(), "{project}");
    }
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(std::path::PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run the census");
        return;
    };
    let main = r#"import java.lang.reflect.AnnotatedElement;
import java.util.*;

public class Main {
    static String shown(AnnotatedElement e) {
        return String.valueOf(e);
    }

    public static void main(String[] args) {
        if (args.length > 0) {
            System.out.println(shown(Main.class));
        }
        List<String> names = new ArrayList<>(List.of("b", "a", "c"));
        Collections.sort(names);
        Map<String, Integer> lengths = new TreeMap<>();
        for (String name : names) {
            lengths.merge(name, name.length(), Integer::sum);
        }
        System.out.println(names + " " + lengths);
    }
}
"#;
    let dir = write_sources("generator_threads", &[("app/Main.java", main)]);
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![java_base],
        ends_rejected: true,
        ..Default::default()
    };
    let one = java_to_rust::codegen::on_threads(1, || census(&program));
    let three = java_to_rust::codegen::on_threads(3, || census(&program));
    // The generator's rejection of the class as an `AnnotatedElement`.
    assert_contains(&one.report, &["not supported yet\n          at Main.java:11:38\n"]);
    assert_eq!(one.report, three.report);
    assert_eq!(one.json, three.json);
    let (one, three) = (one.modules.expect("a translation"), three.modules.expect("a translation"));
    assert_eq!(one.one_file(), three.one_file());
    assert_eq!(one.dependencies(), three.dependencies());
}

#[test]
fn a_census_stubs_every_cast_the_generator_misses_in_one_round() {
    // A cast whose `ClassCastException` a `catch` sees, in the argument of a native that the
    // runtime lacks, is not written (the call is code that never runs), so the generator rejects
    // it (`Gen::check_casts`). Each such cast is rejected, not only the first, so that one round
    // stubs the three methods, and the generator goes on with the stubs in the same run, on the
    // facts of the analyses, which run once. Those facts no longer hold the dereference of the
    // code a stub replaced (`name.length()`), which the generator would find not written.
    let peers = r#"package jdk.peers;

public class Peers {
    public interface Peer {
        int f();
    }

    public static class XPeer implements Peer {
        public int f() { return 1; }
        long window() { return 2; }
    }

    public static class YPeer implements Peer {
        public int f() { return 9; }
    }

    static native void focusNative(long window);

    static native boolean gray(XPeer peer);

    public static void setFocus(Peer peer) {
        focusNative(((XPeer) peer).window());
    }

    public static int create(Peer peer) {
        if (gray((XPeer) peer)) {
            return 1;
        }
        return 2;
    }

    public static int named(Peer peer, String name) {
        focusNative(((XPeer) peer).window());
        return name.length();
    }
}
"#;
    let main = r#"import jdk.peers.Peers;

public class Main {
    public static void main(String[] args) {
        Peers.Peer p = args.length > 7 ? null : new Peers.YPeer();
        try {
            Peers.setFocus(p);
        } catch (ClassCastException e) {
            System.out.println("cce 1");
        }
        try {
            System.out.println(Peers.create(p));
        } catch (ClassCastException e) {
            System.out.println("cce 2");
        }
        try {
            System.out.println(Peers.named(p, null));
        } catch (ClassCastException | NullPointerException e) {
            System.out.println("cce 3");
        }
    }
}
"#;
    let dir = write_sources(
        "census_missed_casts",
        &[
            ("src/Main.java", main),
            // A module of the JDK's, whose natives the runtime may lack (`Native::Missing`).
            ("lib/module-info.java", "module jdk.peers {\n    exports jdk.peers;\n}\n"),
            ("lib/jdk/peers/Peers.java", peers),
        ],
    );
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir.join("src")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    let ((taken, rounds), runs) = java_to_rust::program::runs_of_generator(|| {
        java_to_rust::program::rounds_of_analyses(|| census(&program))
    });
    assert_contains(
        &taken.report,
        &[
            "       3  catching the `ClassCastException` of this cast not supported yet\n          at jdk/peers/Peers.java:22:22, jdk/peers/Peers.java:26:18, jdk/peers/Peers.java:33:22\n",
        ],
    );
    assert_eq!(rounds, 1, "{}", taken.report);
    assert_eq!(runs, 1, "{}", taken.report);
    // The same as with a run of the generator for the stubs, the translation too.
    let program = java_to_rust::Program { ends_rejected: true, ..program };
    let (once, again) = (census(&program), java_to_rust::program::generator_again(|| census(&program)));
    assert_eq!(once.report, again.report);
    assert_eq!(once.json, again.json);
    let rust = once.modules.expect("a translation").one_file();
    assert_eq!(rust, again.modules.expect("a translation").one_file());
}

#[test]
fn a_census_finds_the_same_with_the_errors_of_every_check_in_one_round() {
    // A census takes the errors of every check of the analyses, and of the messages and the
    // generator after them, in one round (`analysis::analyze_collecting`); here the generator
    // rejects a local (`Drain`). Round by round, each ending at the first check that rejects
    // code as a translation's analyses end, it finds the same, its translation too.
    let source = ROUNDS.replace(
        "compare(new Name());",
        "compare(new Name());\n        System.out.println(Drain.drain(args.length > 4) != null);",
    );
    let dir = write_sources("census_one_round", &[("Main.java", &format!("{source}{DRAIN}"))]);
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let together = census(&program);
    let by_round = java_to_rust::program::round_by_round(|| census(&program));
    assert_eq!(together.report, by_round.report);
    assert_eq!(together.json, by_round.json);
    let rust = together.modules.expect("a translation").one_file();
    assert_eq!(rust, by_round.modules.expect("a translation").one_file());
}

#[test]
fn a_census_finds_the_same_with_the_messages_of_unchanged_code_kept_from_round_to_round() {
    // A census keeps the messages of `NullPointerException`s of each method from one round to
    // the next, finding them anew only for the code that the stubs changed
    // (`analysis::KeptMessages`). Over a census whose first round stubs the errors of the checks,
    // of the messages and of the generator, and whose second round finds the messages again, the
    // census is the same as with the messages of every method found anew in each round, its
    // translation too.
    let source = ROUNDS.replace(
        "compare(new Name());",
        "compare(new Name());\n        System.out.println(Drain.drain(args.length > 4) != null);",
    );
    let dir = write_sources("census_kept_messages", &[("Main.java", &format!("{source}{DRAIN}"))]);
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let kept = census(&program);
    let anew = java_to_rust::program::messages_found_anew(|| census(&program));
    assert_eq!(kept.report, anew.report);
    assert_eq!(kept.json, anew.json);
    let rust = kept.modules.expect("a translation").one_file();
    assert_eq!(rust, anew.modules.expect("a translation").one_file());
    assert_contains(
        &kept.report,
        &["the message of the `NullPointerException` of this dereference not supported yet"],
    );
}

#[test]
fn a_census_finds_the_same_with_the_steps_of_the_analyses_one_after_another() {
    // The analyses run the steps that read none of each other's facts alongside each other
    // (`analysis::analyze_from`). Over a census whose analyses reject a pattern variable,
    // `synchronized` on a string, a `toString` whose exception a `catch` around a `printf` sees and the message of a
    // `NullPointerException`, and then whose generator rejects a local, the census is the same as
    // with the steps run one after another, its translation too.
    let dir = write_sources(
        "census_alongside",
        &[(
            "Main.java",
            &format!(
                "{}{DRAIN}",
                ROUNDS.replace(
                    "compare(new Name());",
                    "compare(new Name());\n        System.out.println(Drain.drain(args.length > 4) != null);"
                )
            ),
        )],
    );
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir],
        ends_rejected: true,
        ..Default::default()
    };
    let alongside = census(&program);
    let one_after_another = java_to_rust::analysis::one_after_another(|| census(&program));
    assert_eq!(alongside.report, one_after_another.report);
    assert_eq!(alongside.json, one_after_another.json);
    let rust = alongside.modules.expect("a translation").one_file();
    assert_eq!(rust, one_after_another.modules.expect("a translation").one_file());
    assert_contains(
        &alongside.report,
        &[
            "`synchronized` on a string not supported yet",
            "a pattern variable used here not supported yet",
            "catching an exception that this toString may throw not supported yet",
            "the message of the `NullPointerException` of this dereference not supported yet",
            "a local of type `Object` declared without a value",
        ],
    );
}

/// Two source roots that are modules of the JDK, as javac's `jdk.compiler` and `java.compiler`
/// are: javac patches one module at most in a compilation, so the class files whose names the
/// messages of a `NullPointerException` show come from one compilation for each.
#[test]
fn messages_name_the_class_files_of_two_modules_of_the_jdk() {
    let dir = write_sources(
        "two-jdk-modules",
        &[
            ("prefs/module-info.java", "module java.prefs {\n}\n"),
            (
                "prefs/p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] args) {\n        String t = args.length > 3 ? \"a\" : null;\n        try { System.out.println(t.length()); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n    }\n}\n",
            ),
            ("logging/module-info.java", "module java.logging {\n}\n"),
            (
                "logging/q/Other.java",
                "package q;\npublic class Other {\n    public static String name() { return \"!\"; }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir.join("prefs"), dir.join("logging")],
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    // The JDK compiles its modules with `-g`: the message names the local.
    assert_contains(&rust, &["Cannot invoke \\\"String.length()\\\" because \\\"t\\\" is null"]);
}

/// A program whose class files name their locals (`Program::named_locals`), with a library whose
/// code is translated with it: the messages of a `NullPointerException` name the locals of both,
/// the library's from its classes compiled with the program's.
#[test]
fn messages_name_the_locals_of_class_files_compiled_with_their_names() {
    let dir = write_sources(
        "named-locals",
        &[
            (
                "src/p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] args) {\n        String t = args.length > 3 ? \"a\" : null;\n        try { System.out.println(t.length()); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n        try { System.out.println(q.Lib.size(null)); }\n        catch (NullPointerException e) { System.out.println(e.getMessage()); }\n    }\n}\n",
            ),
            (
                "lib/q/Lib.java",
                "package q;\npublic class Lib {\n    public static int size(String text) {\n        String copy = text;\n        return copy.length();\n    }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "p.Main".into(),
        sources: vec![dir.join("src")],
        libraries: vec![dir.join("lib")],
        named_locals: true,
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &[
            "Cannot invoke \\\"String.length()\\\" because \\\"t\\\" is null",
            "Cannot invoke \\\"String.length()\\\" because \\\"copy\\\" is null",
        ],
    );
}

#[test]
fn census_json_writes_the_census_to_the_file_given() {
    let dir = census_sources("census_cli");
    let json = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("census_cli.json");
    let _ = std::fs::remove_file(&json);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_java-to-rust"))
        .args(["--main", "p.Main", "--source"])
        .arg(&dir)
        .arg("--census-json")
        .arg(&json)
        .output()
        .expect("runs the translator");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Census of the code reachable from p.Main"), "{stdout}");
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(written["main"], "p.Main");
    let rejections = written["rejections"].as_array().expect("rejections");
    assert_eq!(rejections.len(), 3, "{written}");
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
    // `Enum`'s `toString()`, which the enum does not declare, becomes the enum's method.
    let rust = translate(
        "public class A {
            enum Op { PLUS { public String toString() { return \"+\"; } }, MINUS }
            public static void main(String[] args) { System.out.println(Op.PLUS); }
        }",
    );
    assert_contains(&rust, &["fn to_string(self) -> java::JString {\n        match self {"]);
}

#[test]
fn a_method_of_a_constant_body_that_overrides_nothing_is_a_method_of_the_enum() {
    let rust = translate(
        "public class A {
            enum Op {
                TWICE { int apply(int a) { return twice(a); } private int twice(int x) { return 2 * x; } },
                SAME { int apply(int a) { return a; } };
                abstract int apply(int a);
            }
            public static void main(String[] args) { System.out.println(Op.TWICE.apply(4)); }
        }",
    );
    assert_contains(
        &rust,
        &["AOp::Twice => {\n                self.twice(a)", "fn twice(self, x: i32) -> i32 {"],
    );
    // Two of one name and signature would be two methods that the calls cannot tell apart.
    let error = java_to_rust::translate(
        "public class A {
            enum Op {
                TWICE { int apply(int a) { return f(a); } private int f(int x) { return 2 * x; } },
                SAME { int apply(int a) { return f(a); } private int f(int x) { return x; } };
                abstract int apply(int a);
            }
            public static void main(String[] args) { System.out.println(Op.TWICE.apply(4)); }
        }",
    )
    .expect_err("two helpers of one name and signature");
    assert!(error.to_string().contains("methods of one name and signature that override nothing"), "{error}");
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
            "fmt::Display::fmt(&Self::to_string(*self), f)",
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
            "sb.append(1).append_char(',' as u16).append_str(java::wtf8(\"x\"));",
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
            "java::ascii_substring_from(&s, 1)",
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
        &["java::print_format(java::wtf8(\"%d %s %.2f%n\"), &[java::Arg::Long(big), \
           java::Arg::Str(java::wtf8(\"x\")), java::Arg::Double(1.5)]);"],
    );
}

#[test]
fn unsupported_regular_expressions_and_formats_are_rejected() {
    let cases = [
        (
            "\"axb\".split(\"(?<=x)\").length",
            "lookbehind, flags and named groups in the regular expression \"(?<=x)\" not supported yet",
        ),
        (
            "\"ab\".replaceAll(\"\\\\pLb\", \"c\").length()",
            "the escape \\p in the regular expression \"\\pLb\" not supported yet",
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
    assert_contains(&rust, &["C { n: 3 }", "let mut this = C__new();"]);
    // A field initializer written after a block runs after it, as an assignment.
    let rust = translate(
        "class C { int k = 1; { System.out.println(k); } int n = k + 2; }
        class A { public static void main(String[] a) { System.out.println(new C().n); } }",
    );
    assert_contains(
        &rust,
        &[
            "java::println!(\"{}\", this.k);",
            "let mut this = C { k: 1, n: 0 };",
            "this.n = this.k.wrapping_add(2);",
        ],
    );
}

#[test]
fn intern_is_the_string_itself_and_string_identity_follows_the_sources() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            String s = \"x\" + a.length;
            String t = s.intern();
            System.out.println(t + s.intern().length());
        } }",
    );
    assert_contains(&rust, &["let t = s.clone();", "s.len() as i32"]);
    // Two strings that Java interns are the same object exactly when their texts are equal.
    let rust = translate(
        "class A { public static void main(String[] a) { String s = \"x\"; System.out.println(s.intern() == \"x\"); } }",
    );
    assert_contains(&rust, &["s == java::wtf8(\"x\")"]);
    // A new string is never a literal: only two nulls would be the same.
    let rust = translate(
        "class A { public static void main(String[] a) { String s = new String(\"x\"); System.out.println(s == \"x\"); } }",
    );
    assert_contains(&rust, &["java::println!(\"{}\", false & false);"]);
    assert_lacks(&rust, &["java::JStr::", "java::same_str"]);
}

#[test]
fn a_string_static_compared_with_a_string_that_may_be_null_borrows_its_text() {
    // Swing's `BasicDesktopPaneUI.Actions` compares `getName()` with its action names, statics
    // that are not final: the static's text and the name's are both `Option<&Wtf8>`.
    let rust = translate(
        "class A {
            static String CLOSE = \"close\";
            public static void main(String[] a) {
                String key = a.length > 0 ? \"close\" : null;
                System.out.println(CLOSE == key);
                if (a.length > 5) { CLOSE = \"shut\"; }
            }
        }",
    );
    assert_contains(&rust, &["Some::<&java::Wtf8>(&"]);
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
            // The translator's patch replaces it (java/patches/jdk/internal/misc/Unsafe.java), so
            // the stand-in has it too.
            public native Object allocateInstance(Class<?> cls) throws InstantiationException;
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
            public final void putReference(Object o, long offset, Object x) { putReferenceVolatile(o, offset, x); }
            public native int getInt(Object o, long offset);
            public native void putIntVolatile(Object o, long offset, int x);
            public final native boolean compareAndSetReference(Object o, long offset, Object expected, Object x);
            public final native int getAndBitwiseOrInt(Object o, long offset, int mask);
            public final native int getAndAddInt(Object o, long offset, int delta);
            public native long staticFieldOffset(java.lang.reflect.Field f);
            public native Object staticFieldBase(java.lang.reflect.Field f);
            public final native boolean compareAndSetInt(Object o, long offset, int expected, int x);
            public final native boolean weakCompareAndSetLong(Object o, long offset, long expected, long x);
            public final native long getLongVolatile(Object o, long offset);
            public final boolean compareAndSetFloat(Object o, long offset, float expected, float x) {
                return compareAndSetInt(o, offset, Float.floatToRawIntBits(expected), Float.floatToRawIntBits(x));
            }
            public final double getAndAddDouble(Object o, long offset, double delta) {
                long expectedBits;
                double v;
                do {
                    expectedBits = getLongVolatile(o, offset);
                    v = Double.longBitsToDouble(expectedBits);
                } while (!weakCompareAndSetLong(o, offset, expectedBits, Double.doubleToRawLongBits(v + delta)));
                return v;
            }
            public final byte compareAndExchangeByte(Object o, long offset, byte expected, byte x) {
                long wordOffset = offset & ~3;
                int shift = (int) (offset & 3) << 3;
                int mask = 0xFF << shift;
                return (byte) ((getInt(o, wordOffset) & mask) >> shift);
            }
        }",
    ),
    (
        "lib/jdk/internal/misc/Floats.java",
        "package jdk.internal.misc;
        public final class Floats {
            private static final Unsafe U = Unsafe.getUnsafe();
            private volatile float level;
            private byte state;
            private final double[] sums = new double[2];
            private static final long LEVEL = U.objectFieldOffset(Floats.class, \"level\");
            private static final long STATE = U.objectFieldOffset(Floats.class, \"state\");
            private static final long BASE = U.arrayBaseOffset(double[].class);
            private static final int SCALE = U.arrayIndexScale(double[].class);
            public boolean raise(float from, float to) { return U.compareAndSetFloat(this, LEVEL, from, to); }
            public double add(int i, double x) { return U.getAndAddDouble(sums, BASE + (long) i * SCALE, x); }
            public byte swap(byte from, byte to) { return U.compareAndExchangeByte(this, STATE, from, to); }
        }",
    ),
    (
        "lib/jdk/internal/misc/Slots.java",
        "package jdk.internal.misc;
        public final class Slots {
            private static final Unsafe U = Unsafe.getUnsafe();
            public static final class Cell { final String name; public Cell(String n) { name = n; } public String toString() { return name; } }
            private static final long BASE = U.arrayBaseOffset(Cell[].class);
            private static final int SHIFT = 31 - Integer.numberOfLeadingZeros(U.arrayIndexScale(Cell[].class));
            private static final int SCALE = U.arrayIndexScale(Cell[].class);
            private final Cell[] slots = new Cell[4];
            public boolean claim(int i, Cell o) {
                return U.compareAndSetReference(slots, ((long) i << SHIFT) + BASE, null, o);
            }
            public Cell at(int i) { return (Cell) U.getReferenceAcquire(slots, BASE + (long) i * SCALE); }
            public void set(int i, Cell o) { U.putReferenceRelease(slots, ((long) i << SHIFT) + BASE, o); }
            public Cell stray(int i) { return (Cell) U.getReferenceAcquire(slots, BASE + i); }
            static long slotOffset(int i) { return ((long) i << SHIFT) + BASE; }
            public Cell take(int i) {
                long k;
                Cell c = (Cell) U.getReferenceAcquire(slots, k = slotOffset(i));
                return c != null && U.compareAndSetReference(slots, k, c, null) ? c : null;
            }
            public Cell first() { return (Cell) U.getReferenceAcquire(slots, slotOffset(0)); }
            private int flags;
            private static final long FLAGS = U.objectFieldOffset(Slots.class, \"flags\");
            public int mark(int bit) { return U.getAndBitwiseOrInt(this, FLAGS, bit); }
            private static int made;
            private static final Object MADE_BASE;
            private static final long MADE;
            static {
                Class<Slots> klass = Slots.class;
                try {
                    java.lang.reflect.Field f = klass.getDeclaredField(\"made\");
                    MADE_BASE = U.staticFieldBase(f);
                    MADE = U.staticFieldOffset(f);
                } catch (NoSuchFieldException e) {
                    throw new Error(e);
                }
            }
            public static int count() { return U.getAndAddInt(MADE_BASE, MADE, 1); }
        }",
    ),
    (
        "lib/jdk/internal/misc/Names.java",
        "package jdk.internal.misc;
        public final class Names {
            private static final Unsafe U = Unsafe.getUnsafe();
            public static final class Tag { final String n; public Tag(String n) { this.n = n; } public String toString() { return n; } }
            private static final long BASE = U.arrayBaseOffset(Tag[].class);
            private static final int SCALE = U.arrayIndexScale(Tag[].class);
            private final Tag[] tags = new Tag[3];
            private static long offsetFor(long index) { return BASE + SCALE * index; }
            private Tag read(long offset) { return (Tag) U.getReferenceAcquire(tags, offset); }
            private boolean claim(long offset, Tag t) { return U.compareAndSetReference(tags, offset, null, t); }
            public Tag take(int i, Tag t) {
                final long offset = offsetFor(i);
                return claim(offset, t) ? t : read(offsetFor(i));
            }
            private Tag peek(long offset) { return (Tag) U.getReferenceAcquire(tags, offset); }
            public Tag stray(int i) { return peek(BASE + i); }
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
            // The translator's patch replaces it (java/patches/jdk/internal/misc/VM.java), so the
            // stand-in has it too.
            public static native String[] getRuntimeArguments();
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
    (
        "lib/jdk/internal/misc/Erasable.java",
        "package jdk.internal.misc;
        public class Erasable {
            Object locals;
            public Object locals() { return locals; }
            public void set(Object o) { locals = o; }
        }",
    ),
    (
        "lib/jdk/internal/misc/Erasing.java",
        "package jdk.internal.misc;
        public final class Erasing extends Erasable {
            private static final Unsafe U = Unsafe.getUnsafe();
            private static final long LOCALS = U.objectFieldOffset(Erasable.class, \"locals\");
            public void erase() { U.putReference(this, LOCALS, null); }
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
fn a_native_that_the_runtime_implements_runs_through_the_interface_it_implements() {
    // As `VMManagementImpl`'s natives implement `VMManagement`'s methods: the call through the
    // interface dispatches to a method whose body is the runtime's native (here `initIDs`, which
    // does nothing), where it once found no method to run.
    let dir = write_sources(
        "native-through-interface",
        &[(
            "p/Main.java",
            "package p;\npublic class Main {\n    interface Natives {\n        void initIDs();\n    }\n    static class Impl implements Natives {\n        public native void initIDs();\n    }\n    static void init(Natives n) {\n        n.initIDs();\n    }\n    public static void main(String[] a) {\n        init(new Impl());\n        System.out.println(\"ok\");\n    }\n}\n",
        )],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(&rust, &["__init_i_ds("]);
    assert!(!rust.contains("no object runs") && !rust.contains("never runs"), "{rust}");
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
            "use crate::jdk::internal::misc;",
            "{ misc::init_unsafe_(); misc::UNSAFE_ARRAY_INT_BASE_OFFSET.get() }",
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
fn a_write_through_the_offset_of_a_superclass_field_is_the_field() {
    // `InnocuousThread` erases the thread-locals of its `Thread` this way: the offset names a
    // field of the superclass, and the object written is the subclass's.
    let rust = translate_with_natives(
        "jdk-superclass-handles",
        "Erasing e = new Erasing(); e.set(\"set\"); e.erase(); System.out.println(e.locals());",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(&rust, &["self.base.locals = None;"]);
    assert_lacks(&rust, &["LOCALS", "field_offset", "fn put_reference"]);
}

#[test]
fn sun_misc_unsafe_accesses_warn_and_are_the_field() {
    // `sun.misc.Unsafe`'s methods forward to `jdk.internal.misc.Unsafe`'s after their warning,
    // which names the class whose code calls them.
    // The class is in a JDK module other than `java.base`, `jdk.unsupported`, which the
    // translation reads with it from its sources next to those of `java.base`.
    let Some(java_base) = std::env::var_os("JAVA_BASE_SOURCES").map(std::path::PathBuf::from) else {
        println!("skipped: set JAVA_BASE_SOURCES to the source root of java.base to run this test");
        return;
    };
    let unsupported = java_base.parent().unwrap().join("jdk.unsupported");
    if !unsupported.is_dir() {
        println!("skipped: no sources of jdk.unsupported next to those of java.base");
        return;
    }
    let main = "import sun.misc.Unsafe;
        public class Main {
            static Unsafe U;
            private volatile int count;
            private static final long COUNT;
            static {
                try {
                    COUNT = U.objectFieldOffset(Main.class.getDeclaredField(\"count\"));
                } catch (NoSuchFieldException e) {
                    throw new Error(e);
                }
            }
            int bump() { return U.getAndAddInt(this, COUNT, 1); }
            boolean swap(int from, int to) { return U.compareAndSwapInt(this, COUNT, from, to); }
            public static void main(String[] a) {
                Main m = new Main();
                m.bump();
                System.out.println(m.swap(1, 5) + \" \" + U.getIntVolatile(m, COUNT));
            }
        }";
    let dir = write_sources("sun-misc-unsafe", &[("app/Main.java", main)]);
    let program = java_to_rust::Program {
        main: "Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![java_base, unsupported],
        ..Default::default()
    };
    let rust =
        java_to_rust::translate_program(&program).unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &[
            "java::memory_access_warning(",
            "\"objectFieldOffset\"",
            "\"getAndAddInt\"",
            "\"compareAndSwapInt\"",
        ],
    );
    assert_lacks(&rust, &["MAIN_COUNT", "field_offset", "fn get_and_add_int", "fn compare_and_swap_int"]);
}

#[test]
fn an_access_at_an_element_offset_is_the_element() {
    let rust = translate_with_natives(
        "jdk-element-handles",
        "Slots s = new Slots(); s.set(1, new Slots.Cell(\"b\"));
        System.out.println(s.claim(0, new Slots.Cell(\"a\")) + \" \" + s.claim(0, new Slots.Cell(\"c\")) + \" \" + s.at(0) + s.at(1));",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    // The update reads, compares and writes the element in one runtime call, which holds the
    // array for all of it; reads and writes are of the element.
    assert_contains(
        &rust,
        &[
            "java::compare_and_set_element(&self.slots, i, None, Some(o), ",
            "java::set(&self.slots, i, Some(o));",
            "java::get(&self.slots, i)",
        ],
    );
    assert_lacks(&rust, &["fn compare_and_set_reference", "fn get_reference_acquire"]);
}

#[test]
fn offsets_computed_elsewhere_and_static_fields_are_handles_too() {
    let rust = translate_with_natives(
        "jdk-computed-handles",
        "Slots s = new Slots(); s.set(0, new Slots.Cell(\"a\"));
        System.out.println(s.first() + \" \" + s.take(0) + s.take(0) + s.mark(2) + s.mark(1) + Slots.count() + Slots.count());",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &[
            // A call of a method that returns an element offset is the element of its argument.
            "java::get(&self.slots, 0)",
            // A local that holds an element offset is Java's value: the index is computed back.
            "java::compare_and_set_element(&array, java::div(k.wrapping_sub(java::array_base_offset()), \
             java::array_index_scale(\"java.lang.Object\") as i64) as i32, ",
            // A bitwise update of a field, and an update of a static field under the class's lock.
            "fn get_and_bitwise_or_flags(",
            "fn get_and_add_made(",
        ],
    );
    // The reflection that names the static field is left out with its handles.
    assert_lacks(&rust, &["get_declared_field", "MADE_BASE", "fn get_and_add_int"]);
}

#[test]
fn unsafe_updates_written_over_other_kinds_are_updates_of_the_field() {
    let rust = translate_with_natives(
        "jdk-wrapped-updates",
        "Floats f = new Floats();
        System.out.println(f.raise(0.0f, 1.5f) + \" \" + f.raise(Float.NaN, 2f) + \" \" + f.add(1, 0.5) + \" \" + f.swap((byte) 0, (byte) 3));",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    assert_contains(
        &rust,
        &[
            // The float's compare-and-set compares raw bits, as `compareAndSetInt` of them does.
            "fn compare_and_set_level(",
            ".to_bits() as i32 == ",
            "java::get_and_add_element(&self.sums, i, x, |a, b| a + b)",
            "fn compare_and_exchange_state(",
        ],
    );
    // The wrappers themselves, with their offset arithmetic, are not translated.
    assert_lacks(
        &rust,
        &["fn compare_and_set_float", "fn get_and_add_double", "fn compare_and_exchange_byte"],
    );
}

#[test]
fn an_element_offset_passed_to_a_private_method_is_the_element() {
    let rust = translate_with_natives(
        "jdk-parameter-handles",
        "Names n = new Names(); System.out.println(n.take(1, new Names.Tag(\"a\")) + \" \" + n.take(1, new Names.Tag(\"b\")));",
    )
    .unwrap_or_else(|e| panic!("translation failed: {e}"));
    // The parameter holds Java's offset, so the index is computed back from it.
    assert_contains(
        &rust,
        &[
            "java::compare_and_set_element(&",
            "java::div(offset.wrapping_sub(java::array_base_offset()), \
             java::array_index_scale(\"java.lang.Object\") as i64) as i32",
        ],
    );
    assert_lacks(&rust, &["fn compare_and_set_reference", "fn get_reference_acquire"]);
    // A call that passes an offset of no element leaves the parameter no element offset: the
    // access would throw `UnsupportedOperationException`, which this program does not have.
    let error = translate_with_natives("jdk-parameter-stray", "System.out.println(new Names().stray(1));")
        .expect_err("must be rejected");
    assert!(
        error.ends_with(
            "`Unsafe.getReferenceAcquire` at an offset that the translation does not resolve not supported yet"
        ),
        "{error}"
    );
}

#[test]
fn offset_arithmetic_that_is_no_element_offset_is_rejected_without_the_exception_to_throw() {
    let error = translate_with_natives("jdk-element-stray", "System.out.println(new Slots().stray(1));")
        .expect_err("must be rejected");
    assert!(
        error.ends_with(
            "`Unsafe.getReferenceAcquire` at an offset that the translation does not resolve not supported yet"
        ),
        "{error}"
    );
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
            "enum ShapeView<'a> {\n    Circle(Arc<Circle>) = 1,\n    Rect(Arc<RectFields>) = 2,\n    Square(Arc<Square>) = 3,\n    Color(&'a Color) = 6,\n}",
            "        3 => { let o: &Arc<Square> = &unsafe { this.0.to_pointer() }; o.base.area() }\n        6 => { let o: &Color = unsafe { this.0.as_value() }; (*o).area() }",
            "Shape::Circle(slot_2.put(Circle__new(1.0))), Shape::Rect(r.clone())",
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
            "        1 => { let o: &Arc<Circle> = &unsafe { this.0.to_pointer() }; o.kind() }\n        2..=6 => Shape__kind_in_shape(this),",
            "fn Shape__kind_in_shape(this: Shape) -> java::JString {\n    java::JString::from(\"shape\")\n}",
            "fn Shape__describe(this: Shape) -> java::JString {\n    java::concat!(Shape__kind(this), format_args!(\" {}\", java::Double(Shape__area(this))))\n}",
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
            "    Color(&'a Color) = 6,",
            "(ShapeView::Color(a), ShapeView::Color(b)) => a == b,",
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
        &["let n = slot_4.put(Person__new());", "    fn hello(&self, _times: i32) -> java::JString {"],
    );
    assert_lacks(&rust, &["enum Named"]);
}

#[test]
fn a_class_whose_subclasses_never_become_its_values_is_not_an_enum() {
    let rust = translate(INTERFACES);
    // A `Square` is a `Shape` but never a `Rect`, so a `Rect` is always a `Rect` object.
    assert_contains(&rust, &["let r = slot_1.put(RectFields__new(2.0, 3.0));", "java::Double(r.area())"]);
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
    assert_contains(&rust, &["fn compare_to(&self, o: &Arc<V>) -> i32 {"]);
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
            "Err({ let x = x; java::cold(|| Throwable::Missing(Arc::new(",
            "fn twice(xs: &[i32], x: i32) -> Result<i32, Throwable> {",
            "match find(xs, x) { Ok(v) => v, Err(e) => return Err(e) }",
            "Err(caught) => {",
            "let e = match caught.view() { ThrowableView::Missing(ref o) => o.clone(), _ => unreachable!(\"the arm matches its class\") };",
            "let attempt: Result<(), Throwable> = 'attempt: {",
            "match twice(&xs, 5) { Ok(v) => v, Err(e) => break 'attempt Err(e) }",
        ],
    );
}

#[test]
fn a_thrown_exception_whose_message_can_throw_is_made_out_of_line() {
    let rust = translate(
        "class Refused extends Exception { Refused(String m) { super(m); } }
        class A {
            static String describe(int code) throws Refused {
                if (code < 0) throw new Refused(\"can't describe \" + code);
                return \"code \" + code;
            }
            static int checked(int code) throws Refused {
                if (code > 2) throw new Refused(\"can't take \" + describe(code));
                return code;
            }
            static int caught(int code) {
                try {
                    if (code > 1) throw new Refused(\"'\" + describe(code) + \"' in a try\");
                    return code;
                } catch (Refused e) {
                    return -1;
                }
            }
            public static void main(String[] a) {
                try {
                    System.out.println(checked(1) + checked(3) + caught(2));
                } catch (Refused e) {
                    System.out.println(e.getMessage());
                }
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            // A message with a quote in a literal, and no call that throws. The closure borrows a
            // copy of `code` made where it runs, so `code` itself stays in a register.
            "Err({ let code = code; java::cold(|| Throwable::Refused(",
            // The call in the message throws out of the closure, as out of the method.
            "Err({ let code = code; java::cold_or_raised(|| -> Result<Throwable, Throwable> { Ok(Throwable::Refused(",
            "match describe(code) { Ok(v) => v, Err(e) => return Err(e) }",
        ],
    );
    // Inside a `try` the call's exception goes to the `catch`, so the message stays in place.
    assert_contains(&rust, &["match describe(code) { Ok(v) => v, Err(e) => break 'attempt Err(e) }"]);
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
        &[
            "fn check(x: i32) -> i32 {",
            "java::throw_uncaught({ let x = x; java::cold(|| Throwable::IllegalArgumentException(",
        ],
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
            "Err(caught) if matches!(caught.0.tag(), 6) => {",
            "Err(e) if matches!(e.0.tag(), 5..=8) => {",
            "Err(_) => unreachable!(\"every class of exception is caught\"),",
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
            "break 'attempt Err(java::cold(|| Throwable::IllegalStateException(",
            "match attempt { Ok(v) => v, Err(e) => return Err(e) };",
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
            "match java::try_div(a, b) { Ok(v) => v, Err(e) => return Err(Throwable::j2r_raised(e)) }",
            "match java::try_put(&mut v, argument_0, argument_1) { Ok(v) => v, Err(e) => break 'attempt Err(Throwable::j2r_raised(e)) }",
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
            "(match java::try_non_null(a.next.as_ref()) { Ok(v) => v, Err(e) => break 'attempt Err(Throwable::j2r_raised(e)) }).get()",
            "\"java.lang.NullPointerException\" => ",
        ],
    );
    // A dereference that no handler sees keeps the plain check.
    assert_lacks(&rust, &["try_non_null(a.as_ref())"]);
}

#[test]
fn the_message_of_a_null_dereference_is_javas_or_rejected_and_a_created_ones_is_its_own() {
    let program = |second: &str, message: &str| {
        format!(
            "class A {{
                A next;
                int v;
                static int second(A a) {{
                    try {{
                        if (a.v > 0) {{ throw new NullPointerException(\"given\"); }}
                        {second}
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
    let rust = java_to_rust::translate(&program("return a.next.v;", "e.getMessage()")).unwrap();
    assert!(rust.contains(r#"Cannot read field \"v\" because \"<parameter1>.next\" is null"#), "{rust}");
    // A parameter that its method assigns is a local once the bytecode before it wrote it.
    let rust = java_to_rust::translate(&program("a = a.next; return a.v;", "e.getMessage()")).unwrap();
    assert!(rust.contains(r#"Cannot read field \"v\" because \"<local0>\" is null"#), "{rust}");
    // A `finally` block is copied to the end of its `try`, after the write, and to the handler of
    // any exception, before it: the copies name the parameter differently.
    let in_finally = "try { a = a.next; } finally { System.out.print(a.v); } return 0;";
    let err = java_to_rust::translate(&program(in_finally, "e.getMessage()")).unwrap_err().to_string();
    assert!(
        err.contains("the message of the `NullPointerException` of this dereference not supported yet"),
        "{err}"
    );
    let rust = java_to_rust::translate(&program(in_finally, "\"caught\""))
        .expect("a catch that does not read the message");
    assert!(!rust.contains("try_non_null_message"), "{rust}");
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
            "static A_EMPTY_INSTANCE: java::Global<RefCell<Option<Arc<AEmpty>>>> = java::Global::new(RefCell::new(None));",
            "A_EMPTY_INSTANCE.set(Some(Arc::new(AEmpty__new())));",
            "java::non_null(A_EMPTY_INSTANCE.cloned())",
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
    assert_contains(
        &rust,
        &[
            "match java::catching(|| java::new_vec(a.len() as i32 - 1, 0)) { Ok(v) => v, Err(e) => break 'attempt Err(Throwable::j2r_raised(e)) }",
        ],
    );
    // So does a replacement's, of the classes the runtime raises for it.
    let rust = translate(
        "class A { public static void main(String[] a) {
            try { System.out.println(\"a,b\".replaceAll(\",\", a[0])); }
            catch (RuntimeException e) { System.out.println(\"bad replacement\"); }
        } }",
    );
    assert_contains(&rust, &["java::catching(|| "]);
    // So does a format's that is not a literal: one of the `IllegalFormatException`s.
    let rust = translate(
        "class A { public static void main(String[] a) {
            try { System.out.println(String.format(a[0], 1)); }
            catch (RuntimeException e) { System.out.println(\"bad format\"); }
        } }",
    );
    assert_contains(&rust, &["java::catching(|| ", "\"java.util.MissingFormatArgumentException\""]);
    // So does a charset's name, which may be unsupported or illegal.
    let rust = translate(
        "class A { public static void main(String[] a) {
            try { System.out.println(java.nio.charset.Charset.forName(a[0])); }
            catch (RuntimeException e) { System.out.println(\"bad charset\"); }
        } }",
    );
    assert_contains(&rust, &["java::catching(|| ", "\"java.nio.charset.UnsupportedCharsetException\""]);
    // Nor is a `PatternSyntaxException`, which is not one of the standard classes; a `catch` that
    // cannot see it is fine.
    let err = reject(
        "class A { public static void main(String[] a) {
            try { System.out.println(\"a,b\".split(a[0]).length); }
            catch (RuntimeException e) { System.out.println(\"bad pattern\"); }
        } }",
    );
    assert!(
        err.contains(
            "catching the `PatternSyntaxException` that this call of the library may throw not supported yet"
        ),
        "{err}"
    );
    translate(
        "class A { public static void main(String[] a) {
            try { System.out.println(Integer.parseInt(\"1,2\".replaceAll(a[0], \"\"))); }
            catch (NumberFormatException e) { System.out.println(\"not a number\"); }
        } }",
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
        &[
            "(match java::try_non_null(s(a.len() as i32).as_deref()) { Ok(v) => v, Err(e) => break 'attempt Err(Throwable::j2r_raised(e)) }).len()",
        ],
    );
    // A program's own exception constructor that throws passes its failure on from the `new`
    // (only the JDK's are taken to throw for bugs alone, `URISyntaxException`'s tests of its
    // arguments).
    let rust = translate(
        "class Bad extends RuntimeException {
            Bad(int x) { super(\"bad\"); if (x < 0) throw new IllegalStateException(\"negative\"); }
        }
        class A { public static void main(String[] a) {
            try { throw new Bad(-1); } catch (IllegalStateException e) { System.out.println(\"caught\"); }
        } }",
    );
    assert_contains(&rust, &["-> Result<"]);
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
            "fn ARange__new_i32_i32(low: i32, high: i32) -> Result<ARange, Throwable> {",
            "return Err(java::cold(|| Throwable::IllegalArgumentException(",
            // A constructor that only delegates returns the other's `Result`.
            "fn ARange__new_i32(high: i32) -> Result<ARange, Throwable> {\n    ARange__new_i32_i32(0, high)\n}",
            "match ARange__new_i32(args.len() as i32 - 1) { Ok(v) => v, Err(e) => break 'attempt Err(e) }",
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
    assert_contains(&rust, &["fn round(s: &Shape) -> bool {\n    matches!(s.0.tag(), 1)\n}"]);
}

#[test]
fn a_test_of_a_class_with_subclasses_is_one_range_of_class_numbers() {
    // The classes are numbered each before its subclasses, so the classes at or below `Dog` are
    // `Dog`'s number to `Puppy`'s, and `Cat`, numbered after them, is outside.
    let rust = translate(
        "abstract class Animal {}
        class Dog extends Animal {}
        class Cat extends Animal {}
        class Puppy extends Dog {}
        class Main {
            static boolean dog(Animal a) { return a instanceof Dog; }
            static boolean cat(Animal a) { return a instanceof Cat; }
            public static void main(String[] args) {
                Animal[] all = { new Dog(), new Cat(), new Puppy() };
                for (Animal a : all) System.out.println(dog(a) + \" \" + cat(a));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn dog(a: &Animal) -> bool {\n    matches!(a.0.tag(), 2..=3)\n}",
            "fn cat(a: &Animal) -> bool {\n    matches!(a.0.tag(), 4)\n}",
        ],
    );
}

#[test]
fn a_pattern_binds_the_object_with_if_let() {
    let rust = translate(PATTERNS);
    assert_contains(
        &rust,
        &[
            "    fn as_rect(&self) -> Option<Arc<Rect>> {\n        match self.0.tag() {\n            2 => { let o: &Arc<Rect> = &unsafe { self.0.to_pointer() }; Some(o.clone()) }\n            _ => None,\n        }\n    }",
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
        &[
            "    let Some(r) = s.as_rect() else {\n",
            "    java::concat!(format_args!(\"rect {}\", java::Double(r.w)))",
        ],
    );
}

#[test]
fn downcasts_are_checked() {
    let rust = translate(PATTERNS);
    assert_contains(
        &rust,
        &[
            "let r = java::cast(&all[1], |o| o.as_rect(), &(\"Rect\", java::APP));",
            "java::cast(&s, |o| o.as_unit(), &(\"Unit\", java::APP)) as i32",
            "    fn as_unit(&self) -> Option<Unit> {\n        match self.0.tag() {\n            4 => { let o: &Unit = unsafe { self.0.as_value() }; Some(*o) }",
        ],
    );
}

#[test]
fn a_negated_pattern_in_an_or_chain_binds_its_variable_for_the_rest_of_the_block() {
    // The `if` is turned around, with the rest of the block as its then branch
    // (`lower/negated_patterns.rs`).
    let rust = java_to_rust::translate(
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
    .unwrap();
    assert!(rust.contains("if let Some(a) = s.as_a() && !(a.v == 0) {"), "{rust}");
}

#[test]
fn a_cast_of_a_value_that_may_be_null_to_an_enum_is_an_option_of_the_enum() {
    let rust = java_to_rust::translate(
        "interface S {}
        enum E implements S { X }
        class M {
            static S pick(int i) { return i == 0 ? null : E.X; }
            public static void main(String[] x) { E e = (E) pick(1); System.out.println(e.ordinal()); }
        }",
    )
    .unwrap();
    assert!(rust.contains("let e: Option<E> = pick(1).as_ref().map("), "{rust}");
    assert!(rust.contains("java::non_null(e) as i32"), "{rust}");
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
            "enum ObjectView<'a> {\n    Key(Arc<Key>) = 1,\n    Plain(Arc<Plain>) = 2,\n    String(&'a java::JString) = 4,\n    Integer(&'a i32) = 5,\n}",
            "let s = Object::String(java::JString::from(\"text\"));",
            "let n = Object::Integer(5);",
            "let back = java::cast(&n, |o| o.as_integer(), &(\"java.lang.Integer\", java::BASE));",
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
            "fn Object__equals(this: Object, other: &Object) -> bool {\n    match this.0.tag() {\n        1 => { let o: &Arc<Key> = &unsafe { this.0.to_pointer() }; o.equals(other) }\n        2 => other.0 == this.0,\n        4 => { let o: &java::JString = unsafe { this.0.as_value() }; matches!(other.view(), ObjectView::String(b) if o == b) }",
            "2 => java::address_hash(this.0.address()),",
            "4 => { let o: &java::JString = unsafe { this.0.as_value() }; java::string_hash(o) }",
            "2 => { let o: &Arc<Plain> = &unsafe { self.0.to_pointer() }; fmt::Display::fmt(&java::default_text(\"Plain\", java::address_hash(Arc::as_ptr(o))), f) }",
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
fn identity_of_object_values_that_may_be_strings_is_checked_at_run_time() {
    // Different values are different objects; equal ones end the program, as the translation
    // does not keep their identity.
    let rust = translate(
        "class M { public static void main(String[] x) { Object a = \"s\"; Object b = \"s\"; System.out.println(a == b); } }",
    );
    assert_contains(
        &rust,
        &[
            "java::same_object(Some(&a), Some(&b))",
            "impl java::SameObject for Object {",
            "(ObjectView::String(a), ObjectView::String(b)) => a == b && java::unkept_identity(),",
        ],
    );
}

#[test]
fn identity_of_object_values_follows_what_flows_into_them() {
    // Only program objects reach `same`'s parameters, so `==` is plain identity; once a box may
    // reach both sides, it is checked at run time.
    let program = |arg: &str| {
        format!(
            "class M {{ static boolean same(Object a, Object b) {{ return a == b; }}
                public static void main(String[] x) {{ Object m = {arg}; System.out.println(same(m, {arg})); }} }}"
        )
    };
    assert!(!translate(&program("new M()")).contains("same_object"));
    assert!(translate(&program("1")).contains("java::same_object("));
}

#[test]
fn an_array_as_an_object_prints_as_its_jvm_name_and_hash() {
    let rust = translate(
        "class M { public static void main(String[] x) { Object a = new int[2]; System.out.println(a); } }",
    );
    assert_contains(
        &rust,
        &[
            "11 => { let o: &java::Array<i32> = &unsafe { self.0.to_pointer() }; fmt::Display::fmt(&java::default_text(\"[I\", java::identity_hash(o)), f) }",
        ],
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
            "struct Box_ {\n    value: java::Field<Object>,\n}",
            "fn get(&self) -> Object {",
            "fn set(&self, v: Object) {",
            "Box___new(Object::Integer(3))",
            "b.set(Object::Integer(x));",
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
        &[
            "let x = java::cast(&b.get(), |o| o.as_integer(), &(\"java.lang.Integer\", java::BASE)).wrapping_add(1);",
        ],
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
            "enum ComparableView<'a> {\n    Point(Arc<Point>) = 5,\n    String(&'a java::JString) = 7,\n}",
            "5 => { let o: &Arc<Point> = &unsafe { this.0.to_pointer() }; o.compare_to(other) }",
            "7 => { let o: &java::JString = unsafe { this.0.as_value() }; match (&other).view() { ComparableView::String(b) => java::compare_to(o, b), _ => { let b = &other; java::cast_failed_on(&b, &(\"java.lang.String\", java::BASE)) } } }",
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
            // The argument, a cast that may fail, is computed before the receiver's check.
            "let receiver_0 = java::cast(&a, |o| o.as_comparable(), &(\"java.lang.Comparable\", java::BASE));",
            "Comparable__compare_to(receiver_0, ",
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
            "IntArray(java::Array<i32>) = 11,",
            "let held = Object::IntArray(cell.clone());",
            "java::cast(&held, |o| o.as_int_array(), &(\"[I\", java::BASE))",
            "fn eq(&self, other: &Object) -> bool {",
            "self.0 == other.0",
            "11 => { let o: &java::Array<i32> = &unsafe { this.0.to_pointer() }; java::identity_hash(o) }",
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
            "fn fixed(a: java::Array<Object>) -> Arc<MainFixed> {",
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
    // `String[]` meets `Integer[]` at `T[]`, so it is an `Object[]` of strings, which carries
    // its class; the runtime's string builtins take and give copies as `String[]`.
    assert_contains(
        &rust,
        &[
            "String(&'a java::JString) = 2,",
            "fn first(a: &[Object]) -> Object {",
            "let words: java::Array<Object> = java::tagged(java::from_vec((&java::from_vec(java::split(java::wtf8(\"b a\"), java::wtf8(\" \"), 0)).borrow()).iter().map(|s| Object::String(s.clone())).collect::<Vec<_>>()), Some(java::ElementClass::<Object>::new(|| &const { java::Class::array(\"[Ljava.lang.String;\", \"String[]\", || &java::Class::STRING) }, \"java.lang.String\", java::BASE, |v| Some(v.runtime_type_names()), &[\"[Ljava.lang.String;\", \"[Ljava.io.Serializable;\", \"[Ljava.lang.Comparable;\", \"[Ljava.lang.CharSequence;\", \"[Ljava.lang.constant.Constable;\", \"[Ljava.lang.constant.ConstantDesc;\", \"[Ljava.lang.Object;\", \"java.lang.Cloneable\", \"java.io.Serializable\", \"java.lang.Object\"])));",
            "java::join(java::wtf8(\",\"), &java::from_vec((&words.borrow()).iter().map(|s| java::cast(&s, |o| o.as_string(), &(\"java.lang.String\", java::BASE))).collect::<Vec<_>>()).borrow())",
        ],
    );
}

#[test]
fn an_enum_array_passed_for_a_type_variables_array_is_an_object_array_of_its_constants() {
    let rust = translate(
        "public class Main {
            enum Day { MON, TUE }
            static <T> T last(T[] a) { return a[a.length - 1]; }
            public static void main(String[] args) {
                Day[] days = Day.values();
                Day[] some = new Day[2];
                System.out.println(last(days).ordinal() + \" \" + (some[0] == null) + \" \" + last(new Integer[] {1}));
            }
        }",
    );
    // `Day[]` meets `Integer[]` at `T[]`: its constants are `Object`s, cast back where read, and
    // a null test of an element tests the checked cast.
    assert_contains(
        &rust,
        &[
            "let days: Vec<Option<Object>> = vec![Some(Object::Day(MainDay::Mon)), Some(Object::Day(MainDay::Tue))];",
            "java::non_null(last(&days).as_ref().map(|o| java::cast(o, Object::as_day, &(\"Main$Day\", java::APP)))) as i32",
            "some[0].as_ref().map(|o| java::cast(o, Object::as_day, &(\"Main$Day\", java::APP))).is_none()",
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
        &[
            "String(&'a java::JString) = 2,",
            "main_args().into_iter().map(|s| Object::String(s)).collect::<Vec<_>>()",
        ],
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
            "StringBuilder(java::StringBuilder) = 11,",
            "let held = Object::StringBuilder(sb.clone());",
            "11 => { let o: &java::StringBuilder = &unsafe { this.0.to_pointer() }; matches!(other.view(), ObjectView::StringBuilder(ref b) if o.same(b)) }",
            "11 => { let o: &java::StringBuilder = &unsafe { self.0.to_pointer() }; fmt::Display::fmt(o, f) }",
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
            "let (src_pos, dest_pos, length_2) = (0, 0, length); match (&src, &dest) { (src, dest) => match (src.view(), dest.view()) {",
            "(ObjectView::IntArray(ref src), ObjectView::IntArray(ref dest)) => java::arraycopy(src, src_pos, dest, dest_pos, length_2),",
            "(ObjectView::LongArray(ref src), ObjectView::LongArray(ref dest)) => java::arraycopy(src, src_pos, dest, dest_pos, length_2),",
            "_ => java::try_arraycopy_across::<Object>(src, src_pos, dest, dest_pos, length_2).unwrap_or_else(|e| java::throw_exception(e)),",
            "11 => java::CopyKind::Primitive(\"int\"),",
        ],
    );
    // One arm takes every pair of two classes, so the arms grow with the array classes.
    assert_lacks(&rust, &["(ObjectView::IntArray(ref src), ObjectView::LongArray(ref dest))"]);
}

#[test]
fn an_arraycopy_between_objects_reads_and_stores_each_array_class_once() {
    let rust = translate(
        "interface Shape {}
        enum Color { RED }
        record Circle(int r) implements Shape {}
        public class Main {
            static void copy(Object src, Object dest) {
                System.arraycopy(src, 0, dest, 0, 1);
            }
            public static void main(String[] args) {
                Shape[] shapes = new Shape[1];
                copy(new Circle[] {new Circle(1)}, shapes);
                Object[] objects = new Object[1];
                copy(new Color[] {Color.RED}, objects);
                copy(new String[][] {{\"s\"}}, objects);
                copy(objects, new String[1]);
                System.out.println(shapes[0] + \" \" + objects[0]);
            }
        }",
    );
    // Each array class has an arm that reads its elements as `Object`s, and one that stores
    // `Object`s, which the runtime has checked against the class of its elements. The elements of
    // every array that is an `Object` are `Object`s, as the copy may read them.
    assert_contains(
        &rust,
        &[
            "impl java::CopyAcross for Object {",
            "fn copy_out(&self, range: std::ops::Range<usize>) -> Vec<Option<Self>> {",
            "java::Elements::reading(o, range, |items| items.iter().map(|e| { let x = e.clone(); Some(Object::Color(x)) }).collect())",
            "java::Elements::reading(o, range, |items| items.iter().map(|e| { let x = e.clone(); Some(Object::StringArray(x)) }).collect())",
            "fn copy_in(&self, at: usize, items: &[Option<Self>]) {",
            "java::Elements::store_from(o, at, &items.iter().map(|e| e.as_ref().map(|o| o.as_string().unwrap_or_else(",
        ],
    );
    assert_lacks(
        &rust,
        &["not supported yet", "(ObjectView::ColorArray(ref src), ObjectView::ObjectArray(ref dest))"],
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
            "let vs: Vec<Object> = vec![Object::V(Arc::new(V__new(1)))];",
            "count(&vs)",
            "java::cast(&vs[0], |o| o.as_v(), &(\"V\", java::APP)).n",
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
            "fn next_integer(&self) -> i32 {",
            "fn next(&self) -> Object {\n        Object::Integer(self.next_integer())\n    }",
            "fn compare_to_point(&self, o: &Arc<Point>) -> i32 {",
            "fn compare_to(&self, other: Comparable) -> i32 {\n        self.compare_to_point(&java::cast(&other, |o| o.as_point(), &(\"Point\", java::APP)))\n    }",
        ],
    );
}

#[test]
fn wildcards_and_raw_types_erase() {
    let rust = translate(GENERICS);
    // `? extends Shape` is `Shape`, and a raw `Box` is the class itself.
    assert_contains(
        &rust,
        &["java::cast(&b.get(), |o| o.as_shape(), &(\"Shape\", java::APP)).sides()", "let raw = b;"],
    );
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
            "let s: Vec<Object> = vec![Object::String(java::JString::from(\"a\"))];",
            "java::cast(&first(&s), |o| o.as_string(), &(\"java.lang.String\", java::BASE))",
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
    assert_contains(&rust, &["struct Holder<T> {\n    held: T,\n}", "Holder::new(Arc::new(T__new()))"]);
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
            "struct Stack<T> {\n    head: Option<Arc<Node<T>>>,\n}",
            "impl<T: Clone> Stack<T> {",
            "fn push(&mut self, value: T) {",
            "fn pop(&mut self) -> T {",
            "struct Pair<A, B> {\n    first: A,\n    second: B,\n}",
            "fn swap(&self) -> Arc<Pair<B, A>> {",
            "words.push(java::JString::from(\"a\"));",
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
    assert_contains(&rust, &["struct Box_ {\n    v: Object,\n}", "struct Bag {"]);
    assert!(!rust.contains("struct Box_<T>") && !rust.contains("struct Bag<T>"), "{rust}");
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
    // The old head's `next` is copied out of its cell, so nothing borrowed outlives the read
    // when the field is written.
    assert_contains(&rust, &["self.head = java::non_null(self.head.as_ref()).next.get();"]);
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
        &["struct PlainObject {", "let o = Arc::new(PlainObject__new());", "java.lang.Object"],
    );
}

#[test]
fn an_element_that_may_be_null_is_passed_as_the_receiver_that_it_is() {
    // `start` uses `this` as a value, so it takes the element's `&Arc<..>`, which the null check
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
    assert_contains(&rust, &["PBox__start(java::non_null(boxes[i as usize].as_ref()));"]);
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
            "AShapeView::Shape(_) => match java::try_clone_refused(\"A$Shape\") { Ok(v) => v, Err(e) => return Err(Throwable::j2r_raised(e)) },",
            "AShapeView::Circle(ref o) => { AShape::Circle(Arc::new(ACircle { base: AShapeFields { x: java::Field::new(o.base.x.get()) }, r: java::Field::new(o.r.get()) })) }",
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

#[test]
fn uncaught_exceptions_with_causes_are_printed_with_their_stack_trace() {
    let with_cause = translate(
        "class A { public static void main(String[] a) {
            throw new IllegalStateException(\"outer\", new RuntimeException(\"inner\"));
        } }",
    );
    assert_contains(&with_cause, &["java::set_trace_hook(", "fn Throwable__stack_trace_text("]);
    // An exception that has nothing more than its `toString()` is printed by it.
    let plain = translate(
        "class A { public static void main(String[] a) {
            throw new IllegalStateException(\"plain\");
        } }",
    );
    assert_lacks(&plain, &["set_trace_hook", "stack_trace_text"]);
}

#[test]
fn stack_trace_overrides_that_java_runs_unseen_are_rejected() {
    let reject = |java: &str| java_to_rust::translate(java).unwrap_err().to_string();
    let filling = reject(
        "class A { static class E extends RuntimeException {
            public Throwable fillInStackTrace() { System.out.println(\"filled\"); return this; }
        }
        public static void main(String[] a) { new E(); } }",
    );
    assert!(filling.contains("a `fillInStackTrace` that does more than return the exception"), "{filling}");
    let printing = reject(
        "class A { static class E extends RuntimeException {
            public void printStackTrace(java.io.PrintStream s) {}
        }
        public static void main(String[] a) { new E().printStackTrace(); } }",
    );
    assert!(printing.contains("overriding `Throwable.printStackTrace` of a stream or writer"), "{printing}");
}

#[test]
fn a_static_method_asking_for_its_caller_gets_the_calling_class() {
    let rust = translate(
        "class A {
            static String caller() {
                return StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE).getCallerClass().getName();
            }
            static class B { static String name() { return caller(); } }
            public static void main(String[] a) { System.out.println(caller() + B.name()); }
        }",
    );
    assert_contains(&rust, &["fn caller(caller_class: &'static java::Class)", "caller(&crate::CLASS__A)"]);
    let reject = |java: &str| java_to_rust::translate(java).unwrap_err().to_string();
    let instance = reject(
        "class A {
            String caller() { return StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE).getCallerClass().getName(); }
            public static void main(String[] a) { System.out.println(new A().caller()); }
        }",
    );
    assert!(
        instance.contains("asking for the caller (`getCallerClass`) in a method that is not static"),
        "{instance}"
    );
    // A walk's frames are a `Stream`, which a program without the JDK's sources does not have.
    let walked = reject(
        "class A {
            static long depth() { return StackWalker.getInstance().walk(frames -> frames.count()); }
            public static void main(String[] a) { System.out.println(depth()); }
        }",
    );
    assert!(walked.contains("type `java.util.stream.Stream` not supported yet"), "{walked}");
}

#[test]
fn scalb_is_the_runtime_port_in_both_widths() {
    let rust = translate(
        "public class Main {
            public static void main(String[] a) {
                System.out.println(Math.scalb(1.5, 10) + StrictMath.scalb(2.0, 3));
                System.out.println(Math.scalb(1.5f, -3));
            }
        }",
    );
    assert_contains(&rust, &["java::scalb(1.5, 10)", "java::scalb(2.0, 3)", "java::scalb_f32(1.5, -3)"]);
}

#[test]
fn seeds_come_from_the_operating_system() {
    let rust = translate(
        "import java.security.SecureRandom;
        public class Main {
            public static void main(String[] a) { System.out.println(SecureRandom.getSeed(16).length); }
        }",
    );
    assert_contains(&rust, &["or_throw(java::try_secure_seed(16))"]);
    assert_lacks(&rust, &["SecureRandom"]);
}

#[test]
fn a_builder_made_from_a_char_sequence_copies_its_text_or_reads_a_program_class_unit_by_unit() {
    let rust = translate(
        "public class Main {
            public static void main(String[] a) {
                CharSequence text = \"abc\";
                CharSequence nothing = null;
                System.out.println(new StringBuilder(text).append(new StringBuffer(nothing)));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::StringBuilder::from_text(&text.java_string())",
            "java::StringBuilder::from_text(&java::non_null(nothing.as_ref()).java_string())",
        ],
    );
    let program = "package app;\npublic class Main implements CharSequence {\n    public int length() { return 0; }\n    public char charAt(int i) { return 'x'; }\n    public CharSequence subSequence(int s, int e) { return this; }\n    public String toString() { return \"\"; }\n    public static void main(String[] a) {\n        CharSequence s = new Main();\n        System.out.println(new StringBuilder(s));\n    }\n}\n";
    let dir = write_sources("builder-from-program-class", &[("app/Main.java", program)]);
    let program = java_to_rust::Program { main: "app.Main".into(), sources: vec![dir], ..Default::default() };
    let rust = java_to_rust::translate_program(&program).expect("a program class as a CharSequence");
    // Its `length()` sizes the builder, which appends it with its `length()` and `charAt`.
    assert_contains(&rust, &["java::StringBuilder::with_capacity(", "append_seq_range::<"]);
}

#[test]
fn a_class_that_is_a_char_sequence_through_an_interface_gets_the_default_get_chars() {
    // As javac's names are, through `javax.lang.model.element.Name`.
    let rust = translate(
        "public class Main {
            interface Named extends CharSequence {}
            static class Word implements Named {
                public int length() { return 2; }
                public char charAt(int i) { return 'w'; }
                public CharSequence subSequence(int s, int e) { return \"w\"; }
            }
            public static void main(String[] a) {
                CharSequence s = new Word();
                char[] dst = new char[2];
                s.getChars(0, 2, dst, 0);
                System.out.println(new String(dst));
            }
        }",
    );
    assert_contains(&rust, &["java::check_from_to_index(", "java::check_index("]);
}

#[test]
fn an_abstract_class_copy_of_get_chars_calls_the_length_and_char_at_of_its_subclass() {
    let rust = translate(
        "public class Main {
            interface Named extends CharSequence {}
            static abstract class Shell implements Named {
                public CharSequence subSequence(int s, int e) { return \"w\"; }
            }
            static class Word extends Shell {
                public int length() { return 2; }
                public char charAt(int i) { return 'w'; }
            }
            public static void main(String[] a) {
                CharSequence s = new Word();
                char[] dst = new char[2];
                s.getChars(0, 2, dst, 0);
                System.out.println(new String(dst));
            }
        }",
    );
    assert_contains(&rust, &["self.length()", "self.char_at("]);
    assert!(!rust.contains("::length(self)"), "{rust}");
}

/// Two files with code at the same line and column, of which the analyses reject one: the error
/// names its own file, and the census rejects that file's method alone.
#[test]
fn an_analysis_error_names_its_file_where_another_file_has_code_at_its_position() {
    let class = |name: &str, pattern: &str| {
        format!(
            "package p;\nclass {name} {{\n    static void split(String[] a) {{\n        try {{ System.out.println(\"a,b\".split({pattern}).length); }}\n        catch (RuntimeException e) {{ System.out.println(\"bad pattern\"); }}\n    }}\n}}\n"
        )
    };
    let dir = write_sources(
        "census_same_place",
        &[
            (
                "p/Main.java",
                "package p;\npublic class Main {\n    public static void main(String[] a) {\n        Fine.split(a);\n        Rank.split(a);\n    }\n}\n",
            ),
            // The same text but for the pattern, a constant that compiles, which cannot throw.
            ("p/Fine.java", &class("Fine", "\"[,]\"")),
            ("p/Rank.java", &class("Rank", "a[0]")),
        ],
    );
    let program = java_to_rust::Program { main: "p.Main".into(), sources: vec![dir], ..Default::default() };
    let error = java_to_rust::translate_program(&program).expect_err("a dynamic pattern's failure is caught");
    assert!(error.to_string().contains("p/Rank.java:4:"), "{error}");
    let census = census(&program);
    assert_contains(
        &census.report,
        &["  Program: 3 pieces; 2 (66.7%) translate, 1 rejected.", "at p/Rank.java:4:"],
    );
    let pieces = census.json["pieces"].as_array().expect("pieces");
    let errors = |class: &str| {
        let piece = pieces
            .iter()
            .find(|p| p["class"] == class && p["name"] == "split")
            .unwrap_or_else(|| panic!("no {class}.split in {pieces:?}"));
        piece["errors"].as_array().expect("errors").len()
    };
    assert_eq!((errors("p.Fine"), errors("p.Rank")), (0, 1));
}

#[test]
fn a_string_constant_compared_with_what_a_method_returns_is_compared_as_an_object() {
    // `direct` returns the constant itself or a new string: only the objects decide, so every
    // string of the program is shared, the literal interned.
    let java = "public class A {
        static final String ALL = \".*\";
        static String direct(String s) { if (s.isEmpty()) { return ALL; } return s + \"!\"; }
        public static void main(String[] args) {
            String p = direct(args.length > 0 ? args[0] : \"\");
            System.out.println(p == ALL);
            System.out.println(direct(\"x\") != ALL);
        }
    }";
    let (rust, notes) = java_to_rust::translate_with_notes(java).expect("translates");
    assert_contains(
        &rust,
        &[
            "java::println!(\"{}\", java::same_str(&p, &",
            "java::interned_literal(&LITERAL, A_ALL)",
            "!java::same_str(&direct(",
        ],
    );
    assert_eq!(notes.len(), 0);
}

#[test]
fn static_fields_of_stub_classes_are_rejected() {
    // A stub declares no fields (`program::reach::stubs`), so a static field read from the
    // JDK's untranslated class is rejected, not written as a cell that is never declared. The
    // stub is declared because reachable code calls its instance method.
    let dir = write_sources(
        "stub-static-field",
        &[
            (
                "app/Main.java",
                "package app;\nimport com.sun.crypto.provider.ChaCha20Cipher;\npublic class Main {\n    static ChaCha20Cipher held;\n    public static void main(String[] a) { System.out.println(ChaCha20Cipher.LIMIT + (held.isOk() ? 1 : 0)); }\n}\n",
            ),
            (
                "lib/com/sun/crypto/provider/ChaCha20Cipher.java",
                "package com.sun.crypto.provider;\npublic class ChaCha20Cipher {\n    public static final int LIMIT = Integer.getInteger(\"limit\", 16);\n    public boolean isOk() { return true; }\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "app.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    let error = java_to_rust::translate_program(&program).expect_err("a static field of a stub");
    assert!(
        error.to_string().contains(
            "a static field of `com.sun.crypto.provider.ChaCha20Cipher` (security) not supported yet"
        ),
        "{error}"
    );
}

#[test]
fn a_resource_of_an_untranslated_class_is_null_and_not_closed() {
    // No object of a class that the translation leaves out can be made (`program::reach::stubs`),
    // so a try-with-resources over one has a null resource, which Java does not close: the JDK's
    // key store reads a sealed key so (`SealedObjectForKeyProtector.getKey`).
    let dir = write_sources(
        "untranslated-resource",
        &[
            (
                "app/Main.java",
                "package app;\nimport java.io.ObjectInputStream;\npublic class Main {\n    static ObjectInputStream held;\n    public static void main(String[] a) throws Exception {\n        try (ObjectInputStream in = held) { System.out.println(in == null); }\n    }\n}\n",
            ),
            (
                "lib/java/io/ObjectInputStream.java",
                "package java.io;\npublic class ObjectInputStream implements AutoCloseable {\n    public void close() {}\n}\n",
            ),
        ],
    );
    let program = java_to_rust::Program {
        main: "app.Main".into(),
        sources: vec![dir.join("app")],
        libraries: vec![dir.join("lib")],
        ..Default::default()
    };
    let rust = java_to_rust::translate_program(&program).expect("a resource that is null");
    assert!(!rust.contains("close"), "{rust}");
}

#[test]
fn a_copy_from_a_subclass_array_into_an_object_array_stores_each_element_as_it_is() {
    let rust = translate(
        "public class Main {
            static class Box {}
            public static void main(String[] args) {
                Box[] boxes = {new Box(), new Box()};
                Object[] objects = new Object[3];
                System.arraycopy(boxes, 0, objects, 1, 2);
                System.out.println(objects[2]);
            }
        }",
    );
    // Every element is a `Box`, so the copy has no check: each one is converted and stored.
    assert_contains(
        &rust,
        &[
            "java::try_copy_refs(&boxes, src_pos, &mut objects, dest_pos, length, |e: &Arc<MainBox>| -> Result<Option<Arc<MainBox>>, &'static str> { match e { x => Ok(Some(x.clone())), } })",
        ],
    );
    assert!(!rust.contains("arraycopy: type mismatch"), "{rust}");
}

#[test]
fn an_object_array_with_a_foreign_element_copied_into_a_box_array_stops_at_it() {
    let rust = translate(
        "public class Main {
            static class Box {}
            public static void main(String[] args) {
                Object[] mixed = {new Box(), \"text\", new Box()};
                Box[] partial = new Box[3];
                try {
                    System.arraycopy(mixed, 0, partial, 0, 3);
                } catch (ArrayStoreException e) {
                    System.out.println(e.getMessage());
                }
            }
        }",
    );
    // The check is per element: a `Box` or null passes, and the first other element throws after
    // the ones before it are stored. The message names the source's static element type.
    assert_contains(
        &rust,
        &[
            "ObjectView::Box(ref x) => Ok(Some(x.clone())), ",
            "_ => Err(\"arraycopy: element type mismatch: can not cast one of the elements of java.lang.Object[] to the type of the destination array, Main$Box\"), ",
        ],
    );
}

#[test]
fn a_null_copied_into_an_array_of_another_class_is_stored_there() {
    let rust = translate(
        "public class Main {
            static class Box {}
            static class Plain {}
            public static void main(String[] args) {
                Box[] holes = new Box[2];
                Plain[] filled = {new Plain()};
                System.arraycopy(holes, 0, filled, 0, 1);
                System.out.println(filled[0] == null);
            }
        }",
    );
    // The source holds nulls, so the destination's elements may be null too: it is an `Option`
    // array, and a null passes the check.
    assert_contains(
        &rust,
        &[
            "let mut filled: Vec<Option<Arc<MainPlain>>>",
            "|e: &Option<Arc<MainBox>>| -> Result<Option<Arc<MainPlain>>, &'static str> { match e { None => Ok(None), _ => Err(",
        ],
    );
}

#[test]
fn a_copy_between_unrelated_classes_names_the_type_mismatch() {
    let rust = translate(
        "public class Main {
            static class Box {}
            static class Plain {}
            public static void main(String[] args) {
                Box[] boxes = {null, new Box()};
                Plain[] plains = new Plain[2];
                try {
                    System.arraycopy(boxes, 0, plains, 0, 2);
                } catch (ArrayStoreException e) {
                    System.out.println(e.getMessage());
                }
            }
        }",
    );
    // An unrelated pair stores nulls and fails on the first other element, with HotSpot's text for
    // a pair that has no common subclass.
    assert_contains(
        &rust,
        &["_ => Err(\"arraycopy: type mismatch: can not copy Main$Box[] into Main$Plain[]\"), "],
    );
}

#[test]
fn a_range_loop_bound_that_reads_a_shared_field_is_bound_before_the_loop() {
    // The start reads `this`'s field, a cell shared with a thread, by value: no borrow outlives
    // the read, so the bound stays in the range, which Rust reads once before the loop, as Java
    // reads it.
    let rust = translate(
        "public class Main {\n    static Main last;\n    int next = 2;\n    void end(int i) {\n        next++;\n        last = this;\n    }\n    void ends(int limit) {\n        for (int i = next; i < limit; i++) end(i);\n    }\n    public static void main(String[] a) throws Exception {\n        Main m = new Main();\n        Thread t = new Thread(() -> m.ends(6));\n        t.start();\n        t.join();\n    }\n}\n",
    );
    assert_contains(&rust, &["for i in this.next.get()..limit {"]);
    assert_lacks(&rust, &["this.borrow()"]);
}

#[test]
fn a_for_loop_whose_bound_is_a_field_the_body_writes_stays_a_while_loop() {
    // Java re-reads `count` on every iteration, and the body grows it, so the loop must not be a
    // Rust range, which would fix the end once.
    let rust = translate(
        "public class Main {\n    int count = 2;\n    void grow() {\n        count++;\n    }\n    int run() {\n        int seen = 0;\n        for (int i = 0; i < count; i++) {\n            if (count < 5) grow();\n            seen++;\n        }\n        return seen;\n    }\n    public static void main(String[] a) {\n        new Main().run();\n    }\n}\n",
    );
    assert_contains(&rust, &["while i < self.count.get() {"]);
    assert_lacks(&rust, &["for i in"]);
}

#[test]
fn a_shared_string_is_given_itself_to_the_methods_that_take_an_index() {
    let rust = translate(
        "class A {
            static int walk(String s) {
                int n = 0;
                for (int i = s.indexOf(' '); i >= 0; i = s.indexOf(' ', i + 1)) {
                    n += s.substring(0, i).length() + s.charAt(i);
                }
                return n;
            }
            public static void main(String[] a) {
                String s = a.length > 0 ? a[0] : \"a b c\";
                System.out.println(walk(s) + \" \" + (s == \"a b c\"));
            }
        }",
    );
    assert_contains(
        &rust,
        &["java::index_of_char(s.as_text(), ' ' as i32, 0)", "java::char_at(s.as_text(), i)"],
    );
}

#[test]
fn a_string_static_is_read_in_place_where_the_statement_cannot_change_it() {
    let rust = translate(
        "class A {
            static String text = \"a b c\";
            static String line;
            static int change() {
                text = \"changed\";
                return 1;
            }
            public static void main(String[] a) {
                int n = 0;
                for (int i = text.indexOf(' '); i >= 0; i = text.indexOf(' ', i + 1)) {
                    n += text.substring(0, i).length() + text.length();
                }
                int m = text.length() + change() + text.length();
                text = text.substring(1);
                line = a.length > 0 ? a[0] : null;
                n += line.indexOf('x', 1);
                System.out.println(n + \" \" + m + \" \" + text);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "java::index_of_char(&(*std::cell::RefCell::borrow(&A_TEXT)), ' ' as i32, 0)",
            "java::ascii_substring(&(*std::cell::RefCell::borrow(&A_TEXT)), 0, i)",
            "((*std::cell::RefCell::borrow(&A_TEXT)).len() as i32)",
            "(A_TEXT.cloned().len() as i32).wrapping_add(change())",
            "A_TEXT.set(java::ascii_substring_from(&A_TEXT.cloned(), 1))",
            "java::index_of_char(java::non_null((*std::cell::RefCell::borrow(&A_LINE)).as_ref()), 'x' as i32, 1)",
        ],
    );
}

#[test]
fn a_string_parameter_read_by_index_is_given_with_whether_it_is_ascii() {
    let rust = translate(
        "class A {
            static int next(String s, int from) { return s.indexOf(' ', from); }
            static int first(String s) { return s == null ? -1 : s.charAt(0); }
            static int walk(String s) {
                int n = 0;
                for (int i = next(s, 0); i >= 0; i = next(s, i + 1)) {
                    n += s.charAt(i) + first(s);
                }
                return n;
            }
            public static void main(String[] a) {
                String built = a.length > 0 ? a[0] : \"x y z\";
                System.out.println(walk(built) + walk(\"a b\") + walk(\"é f\") + first(a.length > 1 ? a[1] : null));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn next(s: java::Str<'_>, from: i32) -> i32",
            "java::index_of_char(s.as_text(), ' ' as i32, from)",
            "fn first(s: Option<java::Str<'_>>) -> i32",
            "fn walk(s: java::Str<'_>) -> i32",
            "next(s, 0)",
            "first(Some(s))",
            "walk(java::Str::of(&built))",
            "walk(java::Str::known(java::wtf8(\"a b\"), true))",
            "walk(java::Str::known(java::wtf8(\"é f\"), false))",
        ],
    );
}

#[test]
fn a_small_default_body_is_copied_for_each_class_whose_arm_calls_it() {
    // The dispatcher's arm for each class that runs `Reader.minY()` calls a copy of it for the
    // class, where `dimension()` is the class's own method rather than a dispatch.
    let rust = translate(
        r#"
        public class Heights {
            record Dimension(int minY) { }

            interface Accessor { int minY(); }

            interface Reader extends Accessor {
                Dimension dimension();
                default int minY() { return dimension().minY(); }
            }

            static class Level implements Reader {
                final Dimension d;
                Level(Dimension d) { this.d = d; }
                public Dimension dimension() { return d; }
            }

            static class Other implements Reader {
                public Dimension dimension() { return new Dimension(1); }
            }

            static class Region implements Reader {
                public Dimension dimension() { return new Dimension(2); }
                public int minY() { return 7; }
            }

            static class Fixed implements Accessor {
                public int minY() { return 3; }
            }

            public static void main(String[] args) {
                Accessor[] all = { new Level(new Dimension(-64)), new Other(), new Region(), new Fixed() };
                int sum = 0;
                for (Accessor a : all) {
                    sum += a.minY();
                }
                System.out.println(sum);
            }
        }
    "#,
    );
    assert_contains(
        &rust,
        &[
            "HeightsReader__min_y_in_reader__on_heights_level(HeightsReader::Level(o.clone()))",
            "fn HeightsReader__min_y_in_reader__on_heights_level(this: HeightsReader) -> i32 {\n    ({ let o: &Arc<HeightsLevel> = &unsafe { this.0.to_pointer() }; o.dimension() }).min_y()",
            "fn HeightsReader__min_y_in_reader__on_heights_other(",
        ],
    );
    assert_lacks(&rust, &["__on_heights_region", "__on_heights_fixed"]);
}

#[test]
fn a_field_that_a_loop_only_reads_is_read_once_before_it() {
    let rust = translate(
        "class A {
            double x, vx;
            static void step(A[] all, int n) {
                for (int i = 0; i < n; i++) {
                    A a = all[i];
                    for (int j = i + 1; j < n; j++) {
                        A b = all[j];
                        a.vx -= a.x - b.x;
                    }
                }
            }
            static double called(A a, int n) {
                double s = 0;
                for (int i = 0; i < n; i++) { s += a.x; a.move(); }
                return s;
            }
            void move() { x += 1; }
            public static void main(String[] args) {
                A[] all = {new A(), new A()};
                all[1].x = 2;
                step(all, 2);
                System.out.println(all[0].vx + called(all[0], 3));
            }
        }",
    );
    // `a.x` is read once before the inner loop, `b.x` at every turn; the loop with a call reads
    // `a.x` at every turn.
    assert_contains(&rust, &["let a_x = a.x.get();", "a_x - b.x.get()", "s += a.x.get();"]);
}

#[test]
fn a_catch_all_far_from_the_throw_catches_by_unwinding_and_its_callees_return_plain_values() {
    let rust = translate(
        "class A {
            static int leaf(int n) { if (n > 3) throw new IllegalStateException(\"big\"); return n; }
            static int middle(int n) { return leaf(n) * 2; }
            public static void main(String[] a) {
                try { System.out.println(middle(a.length + 4)); }
                catch (Exception e) { System.out.println(\"caught \" + e.getMessage()); }
            }
        }",
    );
    assert_contains(&rust, &["java::unwinding(|| ", "fn middle(n: i32) -> i32 {"]);
}

#[test]
fn a_failure_of_a_class_the_program_lacks_that_unwinds_to_a_catch_all_is_rejected() {
    // A `PatternSyntaxException` is not one of the standard classes, so it could not reach the
    // `catch` as an object: rejected as where a `catch` observes the call that fails.
    let error = java_to_rust::translate(
        "class A {
            static int parts(String s, String p) { return s.split(p).length; }
            public static void main(String[] a) {
                try { System.out.println(parts(\"a,b\", a[0])); }
                catch (RuntimeException e) { System.out.println(\"bad pattern\"); }
            }
        }",
    )
    .expect_err("a PatternSyntaxException that unwinds to a catch of RuntimeException");
    assert!(
        error
            .to_string()
            .contains("catching the `PatternSyntaxException` that this call may throw not supported yet"),
        "{error}"
    );
}
