//! Appending a range of a program's `CharSequence`: the range is appended as Java appends it,
//! through its `length` and `charAt`, whose exceptions go on as a call's do.

fn translate(java: &str) -> String {
    java_to_rust::translate(java).unwrap_or_else(|e| panic!("translation failed: {e}"))
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
fn a_range_of_a_program_sequence_whose_length_throws_passes_the_exception_on() {
    let rust = translate(
        "class A {
            static class Sq implements CharSequence {
                public int length() { throw new IllegalStateException(\"no length\"); }
                public char charAt(int i) { return 'a'; }
                public CharSequence subSequence(int s, int e) { return this; }
                public String toString() { return \"abc\"; }
            }
            public static void main(String[] a) {
                StringBuilder sb = new StringBuilder();
                try {
                    sb.append(new Sq(), 0, 2);
                } catch (IllegalStateException e) {
                    System.out.println(e.getMessage());
                }
                System.out.println(sb);
            }
        }",
    );
    assert!(rust.contains("try_append_seq_range::<Throwable>("), "{rust}");
}

#[test]
fn a_range_of_a_null_program_sequence_is_a_range_of_null() {
    let rust = translate(
        "class A {
            static class Letters implements CharSequence {
                public int length() { return 3; }
                public char charAt(int i) { return (char) ('a' + i); }
                public CharSequence subSequence(int s, int e) { return this; }
                public String toString() { return \"abc\"; }
            }
            public static void main(String[] a) {
                CharSequence s = a.length > 5 ? new Letters() : null;
                StringBuilder sb = new StringBuilder();
                sb.append(s, 1, 3);
                System.out.println(sb);
            }
        }",
    );
    assert!(rust.contains("None => match &sb { held_ => { let builder: &java::StringBuilder = held_; builder.append_range(java::wtf8(\"null\"), 1, 3); builder.clone() } }"), "{rust}");
}
