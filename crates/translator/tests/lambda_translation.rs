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
            "OpLambdaInMain(&'a Arc<OpLambdaInMain>),",
            "4 => { let o: &Arc<OpLambdaInMain> = unsafe { self.0.as_pointer() }; o.apply(a, b) }",
            "AOp::OpLambdaInMain(Arc::new(OpLambdaInMain::new()))",
        ],
    );
}

#[test]
fn a_method_reference_is_a_closure_of_the_call_it_makes() {
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
            "let length = |a: &java::Wtf8| -> i32 { a.len() as i32 };",
            "let doubled = |a: &java::Wtf8| -> i32 { twice(a) };",
            "let make = |a: i32| -> Arc<APoint> { Arc::new(APoint::new(a)) };",
            "length(java::wtf8(\"abc\"))",
        ],
    );
    assert_lacks(&rust, &["struct MeasureLambdaInMain", "enum AMeasure", "type AMeasure"]);
}

#[test]
fn a_generic_interfaces_closure_converts_its_arguments_and_its_result() {
    let rust = translate(
        "import java.util.function.Consumer;
        import java.util.function.Function;
        class A {
            public static void main(String[] args) {
                Function<Integer, Integer> square = x -> x * x;
                System.out.println(square.apply(7));
                Consumer<String> show = s -> System.out.println(s.length());
                show.accept(\"abc\");
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "let square = |t: Object| -> Object {\n        let x: i32 = java::cast(&t, |o| o.as_integer(), (\"java.lang.Integer\", java::BASE));\n        Object::Integer(x.wrapping_mul(x))\n    };",
            "square(Object::Integer(7))",
            "let show = |t: Object| {\n        let s: &java::Wtf8 = &java::cast(&t, |o| o.as_string(), (\"java.lang.String\", java::BASE));\n        java::println!(\"{}\", java::string_length(s));\n    };",
        ],
    );
    assert_lacks(&rust, &["fn apply_integer", "struct FunctionLambdaInMain"]);
}

#[test]
fn a_lambda_that_returns_early_behind_a_bridge_is_a_closure_called_at_once() {
    let rust = translate(
        "import java.util.function.Function;
        class A {
            public static void main(String[] args) {
                Function<Integer, Integer> f = x -> {
                    if (x > 4) {
                        return x - 4;
                    }
                    return 4 - x;
                };
                System.out.println(f.apply(1));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "Object::Integer((|x: i32| -> i32 {",
            "return x.wrapping_sub(4);",
            "})(java::cast(&t, |o| o.as_integer(), (\"java.lang.Integer\", java::BASE))))",
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
    assert_contains(&rust, &["let op = Rc::new(|a: i32| -> i32 { a + 1 }) as AOp;"]);
    assert_lacks(&rust, &["Serializable"]);
}

#[test]
fn a_lambda_moves_the_locals_it_captures_into_its_closure() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a); }
            static int run(Op op) { return op.apply(1); }
            public static void main(String[] args) {
                int k = 3;
                String name = \"n\";
                Op op = a -> a + k + name.length();
                System.out.println(run(op));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "type AOp = Rc<dyn Fn(i32) -> i32>;",
            "let op = Rc::new(move |a: i32| -> i32 { a.wrapping_add(k).wrapping_add(name.len() as i32) }) as AOp;",
            "op(1)",
        ],
    );
}

#[test]
fn a_local_that_only_calls_its_lambda_is_a_closure_that_borrows_its_captures() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a); }
            int base = 2;
            int run(int x) { Op scaled = a -> a * base; base++; return scaled.apply(x); }
            public static void main(String[] args) {
                String name = \"n\";
                Op op = a -> a + name.length();
                System.out.println(op.apply(1) + new A().run(3) + name);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "let scaled = |a: i32| -> i32 { a.wrapping_mul(this.base.get()) };",
            "let op = |a: i32| -> i32 { a.wrapping_add(name.len() as i32) };",
            "op(1)",
        ],
    );
    assert_lacks(&rust, &["type AOp", "Rc::new(|", "move |"]);
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
            "fn adder(this: &Arc<A>) -> AOp {\n        let outer = this.clone();\n        Rc::new(move |a: i32| -> i32 { a.wrapping_add(outer.base.get()) })\n    }",
            "let outer = this.clone();\n        Rc::new(move |a: i32| -> i32 { outer.scaled(a) })\n",
            "A::adder(&o)(1)",
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
    assert_contains(&rust, &["let inner = |z: i32| -> i32 { z.wrapping_add(x).wrapping_add(k) };"]);
}

#[test]
fn a_method_reference_on_a_computed_receiver_evaluates_it_once() {
    let rust = translate(
        "import java.util.function.Supplier;
        class A {
            public static void main(String[] args) {
                Supplier<String> s = new StringBuilder(\"ab\")::toString;
                System.out.println(s.get());
            }
        }",
    );
    // The receiver is made once, before the closure, which captures it.
    assert!(rust.contains("let receiver = java::StringBuilder::from_text(java::wtf8(\"ab\"));"), "{rust}");
    assert!(rust.contains("let receiver = receiver.clone(); Rc::new(move ||"), "{rust}");
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

#[test]
fn a_closure_read_from_a_field_is_taken_out_before_the_call() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a); }
            Op op = a -> a * 2;
            int total;
            void run() { total += op.apply(total + 1); }
            void reset() { op = a -> a; }
            static int twice(A a) { return a.op.apply(3); }
            public static void main(String[] args) {
                A a = new A();
                A shared = a;
                a.run();
                System.out.println(shared.total + twice(a));
                a.reset();
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "op: java::Field<AOp>,",
            "type AOp = Rc<dyn Fn(i32) -> i32>;",
            "(self.op.get())(self.total.get().wrapping_add(1))",
            "(a.op.get())(3)",
        ],
    );
}

#[test]
fn a_closure_in_a_field_only_constructors_write_is_called_in_place() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a); }
            final Op op = a -> a * 2;
            int total;
            void run() { total += op.apply(total + 1); }
            static int twice(A a) { return a.op.apply(3); }
            public static void main(String[] args) {
                A a = new A();
                A shared = a;
                a.run();
                System.out.println(shared.total + twice(a));
            }
        }",
    );
    assert_contains(&rust, &["(a.op.get())(3)"]);
}

#[test]
fn a_functional_interface_whose_values_are_seen_as_objects_stays_an_enum() {
    let rust = translate(
        "class A {
            interface Op { int apply(int a); }
            public static void main(String[] args) {
                Op op = a -> a + 1;
                Object o = op;
                System.out.println(op.apply(1) + \" \" + (o == op));
            }
        }",
    );
    assert_contains(&rust, &["struct OpLambdaInMain {"]);
    assert_lacks(&rust, &["type AOp ="]);
}

#[test]
fn the_default_methods_of_a_closure_type_are_its_extension_trait() {
    let rust = translate(
        "class A {
            interface Check {
                boolean test(int x);
                default Check negate() { return x -> !test(x); }
                default String describe(int x) { return test(x) ? \"yes\" : \"no\"; }
            }
            public static void main(String[] args) {
                Check even = x -> x % 2 == 0;
                System.out.println(even.negate().test(3) + even.describe(2));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "type ACheck = Rc<dyn Fn(i32) -> bool>;",
            "trait ACheckMethods {\n    fn negate(&self) -> ACheck;\n    fn describe(&self, x: i32) -> java::JString;\n}",
            "impl ACheckMethods for ACheck {\n    fn negate(&self) -> ACheck {",
            "let outer = self.clone();\n        Rc::new(move |x: i32| -> bool { !outer(x) })\n",
            "if self(x) {",
            "even.negate()(3)",
            "even.describe(2)",
        ],
    );
}

#[test]
fn an_interface_that_extends_a_closure_type_names_its_closures() {
    let rust = translate(
        "import java.util.function.Function;
        import java.util.function.UnaryOperator;
        class A {
            static String run(Function<String, String> f) { return f.apply(\"x\"); }
            public static void main(String[] args) {
                UnaryOperator<String> twice = s -> s + s;
                System.out.println(run(twice) + run(s -> s));
            }
        }",
    );
    assert_contains(
        &rust,
        &["type Function = Rc<dyn Fn(Object) -> Object>;", "type UnaryOperator = Function;", "run(&twice)"],
    );
    assert_lacks(&rust, &["enum Function", "struct UnaryOperatorLambdaInMain"]);
}
