//! The shape of the generated Rust for inner, local and anonymous classes: each is a struct
//! whose synthetic fields hold its enclosing instance (`outer`) and the locals it captures, set
//! by its constructors from parameters that each `new` passes; and for-each over an `Iterable`,
//! which calls their iterators. (Whether the output matches Java is checked by the corpus test.)

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
fn a_local_class_holds_its_captures_as_fields() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            int scale = a.length + 2;
            String unit = \"kg\";
            class Labeler {
                int calls;
                String label(int n) { calls++; return n * scale + unit; }
            }
            Labeler l = new Labeler();
            System.out.println(l.label(3) + l.calls);
        } }",
    );
    assert_contains(
        &rust,
        &[
            "struct ALabeler {\n    scale: i32,\n    unit: String,\n    calls: i32,\n}",
            "fn new(scale: i32, unit: &str) -> ALabeler {",
            "ALabeler::new(scale, &unit)",
            "self.scale",
        ],
    );
}

#[test]
fn a_captured_array_is_shared() {
    let rust = translate(
        "class A { public static void main(String[] a) {
            int[] seen = new int[2];
            class Counter { void see(int i) { seen[i % 2]++; } }
            Counter c = new Counter();
            c.see(1);
            c.see(3);
            System.out.println(seen[1]);
        } }",
    );
    assert_contains(
        &rust,
        &["seen: java::Array<i32>,", "ACounter::new(seen.clone())", "update(&self.seen, i % 2"],
    );
}

#[test]
fn a_method_whose_this_is_captured_keeps_the_handle() {
    let rust = translate(
        "class A {
            int factor = 3;
            String scaled(int v) {
                class Scaler { int apply() { return v * factor; } }
                Scaler s = new Scaler();
                factor++;
                return \"\" + s.apply();
            }
            public static void main(String[] a) {
                A o = new A();
                System.out.println(o.scaled(7));
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "outer: Rc<RefCell<A>>,",
            "fn scaled(this: &Rc<RefCell<A>>, v: i32) -> String {",
            "AScaler::new(this.clone(), v)",
            "self.outer.borrow().factor",
        ],
    );
    assert_lacks(&rust, &["fn scaled(&self"]);
}

#[test]
fn an_inner_class_holds_the_outer_handle() {
    let rust = translate(
        "class A {
            int size = 2;
            class Cursor {
                int next;
                boolean more() { return next < size; }
            }
            Cursor cursor() { return new Cursor(); }
            public static void main(String[] a) {
                A list = new A();
                A.Cursor c = list.cursor();
                A.Cursor d = list.new Cursor();
                list.size = 3;
                System.out.println(c.more() + \" \" + d.more());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct ACursor {\n    outer: Rc<RefCell<A>>,\n    next: i32,\n}",
            "fn new(outer: Rc<RefCell<A>>) -> ACursor {",
            "ACursor::new(this.clone())",
            "ACursor::new(list.clone())",
            "self.next < self.outer.borrow().size",
        ],
    );
}

#[test]
fn outer_this_is_the_outer_field_chain() {
    let rust = translate(
        "class A {
            int width = 4;
            class Row {
                int width = 1;
                class Cell {
                    int span() { return width + A.this.width; }
                }
                Cell cell() { return new Cell(); }
            }
            public static void main(String[] a) {
                A grid = new A();
                System.out.println(grid.new Row().cell().span());
            }
        }",
    );
    assert_contains(&rust, &["self.outer.borrow().width.wrapping_add(self.outer.borrow().outer.width)"]);
}

#[test]
fn inner_subclasses_share_one_outer_field() {
    let rust = translate(
        "class A {
            int bias = 1;
            interface Step { int run(int x); }
            abstract class Op implements Step {
                public int run(int x) { return apply(x) + bias; }
                abstract int apply(int x);
            }
            class Twice extends Op { int apply(int x) { return 2 * x; } }
            class Square extends Op { int apply(int x) { return x * x; } }
            public static void main(String[] a) {
                A m = new A();
                Step[] steps = { m.new Twice(), m.new Square() };
                int x = 3;
                for (Step s : steps) x = s.run(x);
                System.out.println(x);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct AOpFields {\n    outer: Rc<A>,\n}",
            "struct ATwice {\n    base: AOpFields,\n}",
            "ATwice { base: AOpFields::new(outer) }",
        ],
    );
}

#[test]
fn an_anonymous_class_is_a_variant_of_its_interface() {
    let rust = translate(
        "class A {
            interface Order { int compare(String a, String b); }
            static int count(String[] items, Order order) {
                int n = 0;
                for (int i = 1; i < items.length; i++) if (order.compare(items[i - 1], items[i]) > 0) n++;
                return n;
            }
            public static void main(String[] a) {
                String[] words = { \"b\", \"a\", \"c\" };
                System.out.println(count(words, new Order() {
                    public int compare(String x, String y) { return x.compareTo(y); }
                }));
                Order byLength = new Order() {
                    public int compare(String x, String y) { return y.length() - x.length(); }
                };
                System.out.println(count(words, byLength) + byLength.getClass().getName());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct OrderInMain {\n}",
            "struct OrderInMain2 {\n}",
            "enum AOrder {\n    OrderInMain(Rc<OrderInMain>),\n    OrderInMain2(Rc<OrderInMain2>),\n}",
            "AOrder::OrderInMain(Rc::new(OrderInMain::new()))",
        ],
    );
}

#[test]
fn an_anonymous_subclass_passes_constructor_arguments_to_super() {
    let rust = translate(
        "class A {
            static abstract class Report {
                final String title;
                Report(String title) { this.title = title; }
                abstract String line();
                String render() { return title + \": \" + line(); }
            }
            public static void main(String[] a) {
                String sep = \"-\";
                Report r = new Report(\"squares\") {
                    String line() { return sep + sep; }
                };
                System.out.println(r.render() + r.getClass().getSimpleName().isEmpty());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct ReportInMain {\n    base: AReportFields,\n    sep: String,\n}",
            "fn new(title: &str, sep: &str) -> ReportInMain {",
            "ReportInMain { base: AReportFields::new(title), sep: sep.to_string() }",
            "ReportInMain::new(\"squares\", &sep)",
        ],
    );
}

#[test]
fn for_each_over_an_iterable_is_the_iterator_loop() {
    let rust = translate(
        "import java.util.Iterator;
        class A implements Iterable<String> {
            String[] items = { \"x\", \"yy\" };
            public Iterator<String> iterator() {
                return new Iterator<String>() {
                    int at;
                    public boolean hasNext() { return at < items.length; }
                    public String next() { return items[at++]; }
                };
            }
            public static void main(String[] args) {
                A a = new A();
                int n = 0;
                outer:
                for (String s : a) {
                    for (String t : a) {
                        if (t.length() == 2) continue outer;
                        n += s.length();
                    }
                }
                System.out.println(n);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "let it = A::iterator(&a);",
            "'outer: while it.borrow().has_next() {",
            "let s = java::cast(&(({ let value = it.borrow_mut().next(); value })), |o| o.as_string(), (\"java.lang.String\", java::BASE));",
            "continue 'outer;",
        ],
    );
}

#[test]
fn a_default_method_of_a_provided_interface_is_rejected() {
    let error = java_to_rust::translate(
        "import java.util.Iterator;
        class A {
            public static void main(String[] args) {
                Iterator<String> it = new Iterator<String>() {
                    public boolean hasNext() { return false; }
                    public String next() { return null; }
                };
                it.remove();
            }
        }",
    )
    .expect_err("Iterator.remove is a default method of the JDK's");
    assert!(
        error.to_string().contains("the default method `java.util.Iterator.remove` is the JDK's code"),
        "{error}"
    );
}

#[test]
fn an_inner_subclass_in_a_generic_class_embeds_its_superclass() {
    // javac names the superclass `A.Seq<E>.Itr`, an inner class of a parameterized type, as in
    // the JDK's `AbstractList.ListItr extends Itr`.
    let rust = translate(
        "class A {
            static class Seq<E> {
                Object[] items = { \"x\", \"y\" };
                class Itr {
                    int cursor;
                    boolean hasNext() { return cursor < items.length; }
                }
                class ListItr extends Itr {
                    ListItr(int index) { cursor = index; }
                    boolean hasPrevious() { return cursor > 0; }
                }
            }
            public static void main(String[] a) {
                Seq<String> s = new Seq<>();
                Seq<String>.ListItr it = s.new ListItr(1);
                System.out.println(it.hasNext() + \" \" + it.hasPrevious());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct ASeqListItr {\n    base: ASeqItrFields,\n}",
            "ASeqListItr { base: ASeqItrFields::new(outer) }",
        ],
    );
}

#[test]
fn an_inner_subclass_of_another_outers_inner_class_holds_both_outers() {
    let rust = translate(
        "class A {
            int total;
            class Step {
                final int by;
                Step(int by) { this.by = by; }
                int apply() { total += by; return total; }
            }
            class Batch {
                final int times = 3;
                class Repeated extends Step {
                    Repeated(int by) { super(by * times); }
                }
            }
            public static void main(String[] a) {
                A m = new A();
                A.Batch b = m.new Batch();
                System.out.println(b.new Repeated(2).apply());
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "struct ABatchRepeated {\n    base: AStepFields,\n    outer_: Rc<ABatch>,\n}",
            "ABatchRepeated { base: AStepFields::new(outer.outer.clone(), by.wrapping_mul(outer.times)), outer_: outer }",
        ],
    );
}

#[test]
fn an_anonymous_subclass_takes_its_superclasses_explicit_enclosing_instance() {
    let rust = translate(
        "class A {
            int total;
            class Step {
                final int by;
                Step(int by) { this.by = by; }
                int apply() { total += by; return total; }
            }
            public static void main(String[] a) {
                A m = new A();
                Step s = m.new Step(2) {
                    int apply() { return super.apply() * 10; }
                };
                System.out.println(s.apply());
            }
        }",
    );
    assert_contains(&rust, &["StepInMain { base: AStepFields::new(x0, by) }", "StepInMain::new(m, 2)"]);
}

#[test]
fn an_inner_superclass_whose_constructor_calls_an_override_gets_its_enclosing_instance_first() {
    // The object exists before the superclass's constructor runs (the second constructor form of
    // `analysis/whole_objects.rs`), and it is built with its enclosing instance, which javac
    // sets before `super(..)`, so the override reads it set.
    let rust = translate(
        "class A {
            int total = 5;
            class Base {
                final String label;
                Base() { label = describe(); }
                String describe() { return \"base \" + total; }
            }
            class Derived extends Base {
                String describe() { return \"derived \" + total; }
            }
            public static void main(String[] args) {
                System.out.println(new A().new Derived().label);
            }
        }",
    );
    assert_contains(
        &rust,
        &[
            "fn new(outer: Rc<A>) -> ABaseFields {",
            "ADerived { base: ABaseFields::new(outer.clone()) }",
            "format!(\"derived {}\", self.base.outer.total)",
        ],
    );
}

#[test]
fn an_enclosing_instance_built_in_super_for_a_superclass_built_whole_is_rejected() {
    // The zero object is built with the superclass's enclosing instance before `super(..)`
    // runs; one that `super(..)` computes rather than passes on as a parameter is not supported.
    let error = java_to_rust::translate(
        "class A {
            int total = 5;
            class Base {
                final String label;
                Base() { label = describe(); }
                String describe() { return \"base \" + total; }
            }
            static class Outside extends Base {
                Outside() { new A().super(); }
                String describe() { return \"outside\"; }
            }
            public static void main(String[] args) {
                System.out.println(new Outside().label);
            }
        }",
    )
    .expect_err("the enclosing instance is computed in `super(..)`");
    assert!(error.to_string().contains("that a constructor does not pass on as they are"), "{error}");
}
