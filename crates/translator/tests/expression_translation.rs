//! The shape of the generated Rust for expressions where Rust's grammar or type inference differs
//! from Java's: blocks before operators, literals under unary operators, and expressions of
//! literals whose type inference would choose. (Whether the output matches Java is checked by the
//! corpus test.)

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
