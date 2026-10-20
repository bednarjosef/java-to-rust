//! The shape of the generated Rust for the unsigned methods: each operand is read as the unsigned
//! type of its width, and the runtime does the arithmetic. (Whether the output matches Java is
//! checked by the corpus test.)

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
fn unsigned_division_reads_the_operands_as_unsigned() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            int x = -1;
            long y = -2L;
            System.out.println(Integer.divideUnsigned(x, 7) + Long.remainderUnsigned(y, 3L));
        } }",
    );
    assert_contains(
        &rust,
        &["divide_unsigned(x as u32 as u64, 7", "remainder_unsigned(y as u64, 3"],
    );
}

#[test]
fn unsigned_parsing_passes_the_width_of_the_type() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            System.out.println(Integer.parseUnsignedInt(\"ff\", 16) + Long.parseUnsignedLong(\"7\"));
        } }",
    );
    assert_contains(&rust, &["parse_unsigned(Some(\"ff\"), 16, 32) as i32", "parse_unsigned(Some(\"7\"), 10, 64) as i64"]);
}

#[test]
fn widening_an_unsigned_byte_masks_its_bits() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            byte b = -1;
            System.out.println(Byte.toUnsignedInt(b));
        } }",
    );
    assert_contains(&rust, &["b as u8 as i32"]);
}
