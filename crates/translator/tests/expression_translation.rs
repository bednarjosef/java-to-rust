//! The shape of the generated Rust for expressions where Rust's grammar, type inference, borrows
//! or order of evaluation differ from Java's: blocks before operators, literals under unary
//! operators, expressions of literals whose type inference would choose, an array borrowed while
//! its index is computed, and a variable read before a later operand assigns it. (Whether the
//! output matches Java is checked by the corpus test.)

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
fn a_block_before_an_operator_or_a_cast_is_parenthesised() {
    let rust = translate(
        "class A {
            static char c = 'A';
            static int n = 7;
            static double widened() { return (c++); }
            static boolean same() { return (n -= 2) == 5; }
            static long absolute() { return Math.abs(n += 1); }
            public static void main(String[] a) {
                System.out.println(widened() + \" \" + same() + \" \" + absolute());
            }
        }",
    );
    assert_contains(&rust, &["old }) as f64", "value }) == 5", "value }).wrapping_abs()"]);
    assert_lacks(&rust, &["} as f64", "} == 5", "}.wrapping_abs()"]);
}

#[test]
fn a_float_static_steps_by_one_without_wrapping() {
    let rust = translate(
        "class A {
            static float f = 1.5f;
            public static void main(String[] a) { System.out.println(++f); }
        }",
    );
    assert_contains(&rust, &["let value = old + 1.0;"]);
    assert_lacks(&rust, &["wrapping"]);
}

#[test]
fn a_unary_operator_on_a_literal_gives_a_literal() {
    let rust = translate(
        "class A {
            static int n = 1;
            static byte b = 3;
            public static void main(String[] a) {
                n *= (char) (-(3));
                long big = -(4294967296L) + n;
                int shifted = n >>> ~9223372036854775807L;
                System.out.println(n + \" \" + big + \" \" + shifted + \" \" + ((-32) + b));
            }
        }",
    );
    assert_contains(&rust, &["wrapping_mul(65533)", "-4294967296i64", "(-32i32).wrapping_add("]);
    assert_lacks(&rust, &["-3 as u16", "!9223372036854775807", "(-32).wrapping_add("]);
}

#[test]
fn float_literal_arithmetic_is_written_as_float() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            System.out.println((double) (2.0f / 3));
            System.out.println(0.1f + 0.2f == 0.3f);
        } }",
    );
    assert_contains(&rust, &["(2.0f32 / 3.0) as f64", "0.1f32 + 0.2 == 0.3"]);
}

#[test]
fn a_conditional_of_literals_has_its_type_written_where_inference_would_choose() {
    let rust = translate(
        "class A {
            static boolean flag = true;
            public static void main(String[] a) {
                boolean b = flag;
                System.out.println((short) (b ? 64 : 1234567890123L));
                System.out.println(b ? 3000000000L : 1L);
                System.out.println(Math.abs(b ? -2.5 : 3.0) + \" \" + (b ? 1 : 2));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "(if b { 64i64 } else { 1234567890123i64 }) as i16",
            "if b { 3000000000i64 } else { 1i64 }",
            "(if b { -2.5f64 } else { 3.0f64 }).abs()",
            "if b { 1 } else { 2 }",
        ],
    );
}

#[test]
fn an_index_that_reads_the_array_is_bound_before_the_array_is_borrowed() {
    let rust = translate(
        "class A {
            static int k = 7;
            public static void main(String[] a) {
                float[] w = new float[5];
                w[Math.floorMod(k, w.length)] += 1.5f;
                w[Math.floorMod(k, w.length)]++;
                w[k - 7] *= 2;
                System.out.println(w[2]);
            }
        }",
    );
    assert_contains(&rust, &["let index = java::floor_mod(", "java::put(&mut w, index, "]);
    assert_lacks(&rust, &["java::modify(&mut w, java::floor_mod(", "java::put(&mut w, java::floor_mod("]);
}

#[test]
fn a_cast_variable_in_a_concatenation_is_read_before_a_later_operand_assigns_it() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            long count = 5L;
            System.out.println(((long) count) + \" \" + (--count));
        } }",
    );
    assert_contains(&rust, &["{ count }, { count -= 1; count }"]);
}

#[test]
fn a_long_literal_appended_to_a_builder_has_its_type_written() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            StringBuilder sb = new StringBuilder();
            sb.insert(0, 4294967296L).append(7);
            System.out.println(sb);
        } }",
    );
    assert_contains(&rust, &["sb.insert(0, 4294967296i64).append(7);"]);
}
