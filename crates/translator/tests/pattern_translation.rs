//! The shape of records, pattern switches and the members that javac leaves out of its tree:
//! records are plain structs with the methods Java gives them, and a switch over a sealed
//! hierarchy is an `if let` chain over the enum of its classes.

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
fn a_record_is_an_immutable_struct_with_java_text_and_hash() {
    let rust = translate(
        "public class A {
            record Point(int x, int y) {
                Point(int both) { this(both, both); }
            }
            public static void main(String[] args) {
                Point p = new Point(3, -4);
                System.out.println(p + \" \" + p.x() + \" \" + p.hashCode() + \" \" + new Point(2));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct APoint {\n    x: i32,\n    y: i32,\n}",
            "fn x(&self) -> i32 {\n        self.x\n    }",
            "fn hash_code(&self) -> i32 {\n        31i32.wrapping_mul(self.x).wrapping_add(self.y)\n    }",
            "write!(f, \"Point[x={}, y={}]\", self.x, self.y)",
            "let p = Rc::new(APoint::new_i32_i32(3, -4));",
        ],
    );
    // Nothing calls `y()` or `equals`, and the record never changes.
    assert_lacks(&rust, &["fn y(", "fn equals", "RefCell"]);
}

#[test]
fn a_record_equals_compares_its_components_as_java_does() {
    let rust = translate(
        "public class A {
            record Named(String name, double weight) {}
            public static void main(String[] args) {
                Object o = new Named(\"a\", 1.5);
                System.out.println(new Named(\"a\", 1.5).equals(o));
            }
        }",
    );
    assert_contains(
        &rust,
        &["self.name == that.name && java::double_compare(self.weight, that.weight) == 0"],
    );
}

#[test]
fn a_switch_over_a_sealed_interface_is_an_if_let_chain() {
    let rust = translate(
        "public class A {
            sealed interface Shape permits Circle, Square {}
            record Circle(double r) implements Shape {}
            record Square(double side) implements Shape {}
            static double area(Shape s) {
                return switch (s) {
                    case Circle c when c.r() > 10 -> 0;
                    case Circle c -> 3 * c.r() * c.r();
                    case Square(double side) -> side * side;
                };
            }
            public static void main(String[] args) {
                System.out.println(area(new Circle(1)) + area(new Square(2)));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "if let Some(c) = s.as_circle() && c.r() > 10.0 {",
            "} else if let Some(square) = s.as_square() {\n        square.side * square.side\n    } else {",
            "unreachable!(",
        ],
    );
    assert_lacks(&rust, &["non_null", "match 0"]);
}

#[test]
fn case_null_is_tested_first_and_a_null_selector_otherwise_throws() {
    let rust = translate(
        "public class A {
            static String f(Object o) {
                return switch (o) {
                    case Integer i -> \"int \" + i;
                    case null -> \"null\";
                    default -> \"other\";
                };
            }
            static String g(Object o) {
                return switch (o) {
                    case String s -> s;
                    default -> \"other\";
                };
            }
            public static void main(String[] args) {
                System.out.println(f(null) + f(1) + g(null));
            }
        }",
    );
    assert_contains(
        &rust,
        &["if let Some(o) = o.as_ref() {", "{ java::non_null(o.as_ref()); String::from(\"other\") }"],
    );
}

#[test]
fn a_break_out_of_a_pattern_case_leaves_a_labeled_block() {
    let rust = translate(
        "public class A {
            sealed interface T permits X, Y {}
            record X(int v) implements T {}
            record Y(int v) implements T {}
            public static void main(String[] args) {
                T t = new X(3);
                switch (t) {
                    case X x -> {
                        if (x.v() > 2) {
                            System.out.println(\"big\");
                            break;
                        }
                        System.out.println(\"small\");
                    }
                    case Y y -> System.out.println(\"y\");
                }
            }
        }",
    );
    assert_contains(&rust, &["'sw1: {\n", "break 'sw1;"]);
}

#[test]
fn a_caught_value_of_failure_uses_the_checked_form() {
    let rust = translate(
        "public class A {
            enum Size { SMALL, LARGE }
            static Size parse(String s) {
                try {
                    return Size.valueOf(s);
                } catch (IllegalArgumentException e) {
                    return Size.SMALL;
                }
            }
            public static void main(String[] args) { System.out.println(parse(\"HUGE\")); }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn try_enum_value_of(name: &str) -> Result<ASize, java::Exception> {",
            "No enum constant A.Size.{name}",
            "ASize::try_enum_value_of(",
        ],
    );
}

#[test]
fn comparing_classes_matches_the_variant_of_the_class() {
    let rust = translate(
        "public class A {
            static class Animal {}
            static class Dog extends Animal {}
            static final class Money {
                final long cents;
                Money(long cents) { this.cents = cents; }
                @Override public boolean equals(Object o) {
                    if (o == null || getClass() != o.getClass()) return false;
                    return cents == ((Money) o).cents;
                }
                @Override public int hashCode() { return (int) cents; }
            }
            public static void main(String[] args) {
                Animal a = args.length > 0 ? new Animal() : new Dog();
                System.out.println(a.getClass() == Dog.class);
                Object o = args.length > 1 ? null : new Money(1);
                System.out.println(new Money(1).equals(o) + Dog.class.getSimpleName() + new Money(2).equals(a));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "matches!(a, AAnimal::Dog(_))",
            "!matches!(java::non_null(o.as_ref()), Object::Money(_))",
            "String::from(\"Dog\")",
        ],
    );
    assert_lacks(&rust, &["\"A$Dog\" =="]);
}
