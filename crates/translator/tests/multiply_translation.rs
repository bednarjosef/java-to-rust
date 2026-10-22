//! The shape of the generated Rust for the products of `Math` and `StrictMath`: the runtime
//! computes the 128-bit product, and `multiplyFull` takes `int` operands whatever their width.
//! (Whether the output matches Java is checked by the corpus test.)

fn translate(java: &str) -> String {
    java_to_rust::translate(java).unwrap_or_else(|e| panic!("translation failed: {e}"))
}

#[track_caller]
fn assert_contains(rust: &str, expected: &[&str]) {
    for e in expected {
        assert!(rust.contains(e), "expected `{e}` in:\n{rust}");
    }
}

#[test]
fn high_and_full_products_are_runtime_calls() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            long x = -3L;
            long y = 1L << 40;
            System.out.println(Math.multiplyHigh(x, y) + StrictMath.unsignedMultiplyHigh(x, 7L));
            System.out.println(Math.multiplyFull(-7, 3) + StrictMath.multiplyFull((short) 9, 'a'));
        } }",
    );
    assert_contains(
        &rust,
        &[
            "java::multiply_high(x, y)",
            "java::unsigned_multiply_high(x, 7i64)",
            "java::multiply_full(-7i32, 3i32)",
            "java::multiply_full(9i32, 97i32)",
        ],
    );
}
