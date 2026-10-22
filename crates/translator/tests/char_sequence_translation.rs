//! Appending a range of a program's `CharSequence`: where its `length` and `charAt` cannot throw,
//! the range is appended as Java appends it; where either may throw, the translation is rejected.

fn translate(java: &str) -> String {
    java_to_rust::translate(java).unwrap_or_else(|e| panic!("translation failed: {e}"))
}

#[track_caller]
fn rejected(java: &str) -> String {
    match java_to_rust::translate(java) {
        Ok(rust) => panic!("expected a rejection, got:\n{rust}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_range_of_a_quiet_program_sequence_is_appended_through_its_methods() {
    let rust = translate(
        "class A {
            static class Letters implements CharSequence {
                public int length() { return 3; }
                public char charAt(int i) { return (char) ('a' + i); }
                public CharSequence subSequence(int s, int e) { return this; }
                public String toString() { return \"abc\"; }
            }
            public static void main(String[] a) {
                StringBuilder sb = new StringBuilder();
                sb.append(new Letters(), 0, 2);
                System.out.println(sb);
            }
        }",
    );
    assert!(rust.contains("append_seq_range::<java::Exception>("), "{rust}");
}

#[test]
fn a_range_of_a_program_sequence_whose_char_at_may_throw_is_rejected() {
    let error = rejected(
        "class A {
            static class Sq implements CharSequence {
                public int length() { return 3; }
                public char charAt(int i) { return \"abc\".charAt(i); }
                public CharSequence subSequence(int s, int e) { return this; }
                public String toString() { return \"abc\"; }
            }
            public static void main(String[] a) {
                StringBuilder sb = new StringBuilder();
                sb.append(new Sq(), 0, 2);
                System.out.println(sb);
            }
        }",
    );
    assert!(
        error.ends_with("a range of a `CharSequence` whose `charAt` or `length` may throw not supported yet"),
        "{error}"
    );
}

#[test]
fn a_range_of_a_program_sequence_whose_length_may_throw_is_rejected() {
    let error = rejected(
        "class A {
            static class Sq implements CharSequence {
                public int length() { throw new IllegalStateException(\"no length\"); }
                public char charAt(int i) { return 'a'; }
                public CharSequence subSequence(int s, int e) { return this; }
                public String toString() { return \"abc\"; }
            }
            public static void main(String[] a) {
                StringBuilder sb = new StringBuilder();
                sb.append(new Sq(), 0, 2);
                System.out.println(sb);
            }
        }",
    );
    assert!(
        error.ends_with("a range of a `CharSequence` whose `charAt` or `length` may throw not supported yet"),
        "{error}"
    );
}
