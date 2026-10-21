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
fn a_lambda_holds_the_locals_it_captures_in_fields() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a); }
            public static void main(String[] args) {
                int k = 3;
                String name = \"n\";
                Op op = a -> a + k + name.length();
                System.out.println(op.apply(1));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct OpLambdaInMain {\n    k: i32,\n    name: String,\n}",
            "fn new(k: i32, name: &str) -> OpLambdaInMain {",
            "OpLambdaInMain::new(k, &name)",
            "self.k",
        ],
    );
}

#[test]
fn a_lambda_reaches_the_enclosing_object_through_outer() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a); }
            int base = 10;
            int scaled(int x) { return x * base; }
            Op adder() { return a -> a + base; }
            Op scaler() { return this::scaled; }
            public static void main(String[] args) {
                A o = new A();
                System.out.println(o.adder().apply(1) + o.scaler().apply(2));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct OpLambdaInAdder {\n    outer: Rc<RefCell<A>>,\n}",
            "AOp::OpLambdaInAdder(Rc::new(OpLambdaInAdder::new(this.clone())))",
            "self.outer.borrow().base",
            "self.outer.borrow().scaled(a)",
        ],
    );
}

#[test]
fn a_lambda_in_a_lambda_takes_its_captures_from_the_outer_one() {
    let rust = translate(
        "import java.util.function.IntUnaryOperator;
        class A {
            public static void main(String[] args) {
                int k = 3;
                IntUnaryOperator outer = x -> {
                    IntUnaryOperator inner = z -> z + x + k;
                    return inner.applyAsInt(1);
                };
                System.out.println(outer.applyAsInt(5));
            }
        }",
    );
    assert_contains(&rust, &["IntUnaryOperatorLambdaInMain2::new(x, self.k)"]);
}

#[test]
fn a_method_reference_on_a_computed_receiver_is_rejected() {
    let message = rejected(
        "import java.util.function.Supplier;
        class A {
            public static void main(String[] args) {
                Supplier<String> s = new StringBuilder(\"ab\")::toString;
                System.out.println(s.get());
            }
        }",
    );
    assert!(message.contains("a method reference on a receiver that is not a local"), "{message}");
}

#[test]
fn a_default_or_static_method_of_a_jdk_interface_needs_the_jdks_sources() {
    let message = rejected(
        "import java.util.function.Function;
        class A {
            public static void main(String[] args) {
                Function<Integer, Integer> f = x -> x + 1;
                System.out.println(f.andThen(x -> x * 2).apply(3));
            }
        }",
    );
    assert!(
        message.contains("the default method `java.util.function.Function.andThen` is the JDK's code"),
        "{message}"
    );
    assert!(message.contains("`--library`"), "{message}");
    let message = rejected(
        "import java.util.Comparator;
        class A {
            public static void main(String[] args) {
                Comparator<String> c = Comparator.naturalOrder();
                System.out.println(c.compare(\"a\", \"b\"));
            }
        }",
    );
    assert!(
        message.contains("the static method `java.util.Comparator.naturalOrder` is the JDK's code"),
        "{message}"
    );
}
