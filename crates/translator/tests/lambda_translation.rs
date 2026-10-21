//! The shape of the translation of lambdas, method references and functional interfaces (see
//! docs/closures-and-nested-classes.md).

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

#[track_caller]
fn rejected(java: &str) -> String {
    match java_to_rust::translate(java) {
        Ok(rust) => panic!("expected a rejection, got:\n{rust}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_lambda_is_a_variant_of_its_interface() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a, int b); }
            static class Sub implements Op { public int apply(int a, int b) { return a - b; } }
            public static void main(String[] args) {
                Op[] ops = { (a, b) -> a + b, new Sub() };
                for (Op op : ops) System.out.println(op.apply(5, 3));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct OpLambdaInMain {",
            "fn apply(&self, a: i32, b: i32) -> i32 {\n        a + b\n    }",
            "OpLambdaInMain(Rc<OpLambdaInMain>),",
            "AOp::OpLambdaInMain(o) => o.apply(a, b),",
            "AOp::OpLambdaInMain(Rc::new(OpLambdaInMain::new()))",
        ],
    );
}

#[test]
fn a_method_reference_is_a_lambda_class() {
    let rust = translate(
        "class A {
            interface Measure { int of(String s); }
            interface Make { Point make(int x); }
            static class Point { int x; Point(int x) { this.x = x; } }
            static int twice(String s) { return 2 * s.length(); }
            public static void main(String[] args) {
                Measure length = String::length;
                Measure doubled = A::twice;
                Make make = Point::new;
                System.out.println(length.of(\"abc\") + doubled.of(\"ab\") + make.make(4).x);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn of(&self, a: &str) -> i32 {\n        a.len() as i32\n    }",
            "fn of(&self, a: &str) -> i32 {\n        twice(a)\n    }",
            "struct MakeLambdaInMain {",
            "AMeasure::MeasureLambdaInMain2(Rc::new(MeasureLambdaInMain2::new()))",
        ],
    );
}

#[test]
fn a_lambda_parameter_is_cast_from_the_erased_descriptor() {
    let rust = translate(
        "import java.util.function.Function;
        class A {
            public static void main(String[] args) {
                Function<Integer, Integer> square = x -> x * x;
                System.out.println(square.apply(7));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn apply_integer(&self, x: i32) -> i32 {",
            "fn apply(&self, t: Object) -> Object {",
            "Object::Integer(self.apply_integer(java::cast(t.as_integer(), \"java.lang.Integer\")))",
            "square.apply(Object::Integer(7))",
        ],
    );
}

#[test]
fn a_marker_interface_is_dropped_from_an_intersection_cast() {
    let rust = translate(
        "import java.io.Serializable;
        class A {
            interface Op { int apply(int a); }
            public static void main(String[] args) {
                Op op = (Op & Serializable) a -> a + 1;
                System.out.println(op.apply(1));
            }
        }",
    );
    assert_contains(&rust, &["fn apply(&self, a: i32) -> i32 {"]);
    assert_lacks(&rust, &["Serializable"]);
}

#[test]
fn a_capturing_lambda_is_rejected_until_captures_exist() {
    let message = rejected(
        "class A {
            interface Op { int apply(int a); }
            public static void main(String[] args) {
                int k = 3;
                Op op = a -> a + k;
                System.out.println(op.apply(1));
            }
        }",
    );
    assert!(message.contains("lambda captures not supported yet"), "{message}");
}
