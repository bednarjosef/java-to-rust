//! The shape of the generated Rust for the `Character` code point methods: each one is a runtime
//! function on the code point, and `Character.toString(int)` is not taken for the boxed `char`
//! overload. (Whether the output matches Java is checked by the corpus test.)

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
fn code_point_methods_call_the_runtime_on_the_code_point() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            char[] pair = Character.toChars(0x1F600);
            System.out.println(pair.length + Character.charCount(0x1F600));
            System.out.println(Character.toCodePoint('a', 'b') + Character.highSurrogate(0x1F600));
            System.out.println(Character.isSupplementaryCodePoint(0x10000));
        } }",
    );
    assert_contains(
        &rust,
        &[
            "to_chars(128512)",
            "char_count(128512)",
            "to_code_point(",
            "high_surrogate(128512)",
            "is_supplementary_code_point(65536)",
        ],
    );
}

#[test]
fn to_string_of_an_int_is_a_code_point_and_of_a_char_is_boxed() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            System.out.println(Character.toString(66));
            System.out.println(Character.toString('A'));
        } }",
    );
    assert_contains(&rust, &["code_point_string(66)"]);
    assert!(!rust.contains("code_point_string('A'"), "a char must not take the code point path:\n{rust}");
}
