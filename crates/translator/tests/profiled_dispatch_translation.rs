//! Profiled calls (docs/native-speed.md): a translation that counts the classes of each call's
//! receivers, and one that tests first for the class a call mostly saw. One test in a binary of
//! its own, as both read the environment, which every test of a binary shares.

fn translate(java: &str) -> String {
    java_to_rust::translate(java).unwrap_or_else(|e| panic!("translation failed: {e}"))
}

#[track_caller]
fn assert_contains(rust: &str, expected: &[&str]) {
    for e in expected {
        assert!(rust.contains(e), "expected `{e}` in:\n{rust}");
    }
}

const SHAPES: &str = "public class Main {
    interface Shape { int area(); }
    static final class Square implements Shape {
        final int s; Square(int s) { this.s = s; } public int area() { return s * s; } }
    static final class Rect implements Shape {
        final int w; Rect(int w) { this.w = w; } public int area() { return w * 2; } }
    static int total(Shape[] shapes) {
        int sum = 0;
        for (Shape s : shapes) sum += s.area();
        return sum;
    }
    public static void main(String[] args) {
        Shape[] shapes = new Shape[30];
        for (int i = 0; i < 30; i++) shapes[i] = i % 10 == 0 ? new Rect(i) : new Square(i);
        System.out.println(total(shapes));
    }
}";

const BAGS: &str = "public class Main {
    interface Cursor { boolean more(); int next(); }
    interface Bag { Cursor cursor(); }
    static final class Arr implements Bag {
        final int[] xs; Arr(int[] xs) { this.xs = xs; }
        public Cursor cursor() { return new ArrCursor(this); } }
    static final class ArrCursor implements Cursor {
        final Arr a; int i; ArrCursor(Arr a) { this.a = a; }
        public boolean more() { return i < a.xs.length; }
        public int next() { return a.xs[i++]; } }
    static final class One implements Bag { public Cursor cursor() { return new OneCursor(); } }
    static final class OneCursor implements Cursor {
        boolean done; public boolean more() { return !done; } public int next() { done = true; return 1; } }
    static final class Never implements Bag { public Cursor cursor() { return null; } }
    static int total(Bag b) {
        int sum = 0;
        Cursor c = b.cursor();
        while (c.more()) sum += c.next();
        return sum;
    }
    public static void main(String[] args) {
        Bag[] bags = { new Arr(new int[] { 1, 2 }), new One() };
        int sum = 0;
        for (int i = 0; i < 30; i++) sum += total(bags[i % 10 == 0 ? 1 : 0]);
        System.out.println(sum + \" \" + new Never().cursor());
    }
}";

const PROFILER: &str = "public class Main {
    interface Name { String name(); }
    static final class Fixed implements Name { public String name() { return \"fixed\"; } }
    interface Filler { void push(Name name); void pop(); }
    static final class Inactive implements Filler { public void push(Name name) {} public void pop() {} }
    static final class Active implements Filler {
        final Name[] kept = new Name[8]; int n;
        public void push(Name name) { kept[n++ % 8] = name; }
        public void pop() {} }
    static final class Entity {
        final String type; int ticks; Entity(String type) { this.type = type; }
        String type() { return type; } }
    static Filler profiler = new Inactive();
    static void tick(Entity e) {
        Filler p = profiler;
        p.push(e::type);
        e.ticks++;
        p.pop();
    }
    public static void main(String[] args) {
        Entity[] es = { new Entity(\"a\"), new Entity(\"b\") };
        for (int i = 0; i < 30; i++) tick(es[i % 2]);
        Active active = new Active();
        profiler = active;
        tick(es[0]);
        active.push(new Fixed());
        System.out.println(es[0].ticks + es[1].ticks + active.kept[0].name() + active.kept[1].name());
    }
}";

/// The profile `JAVA_TO_RUST_DISPATCH_PROFILE` names while `translate` runs, as `text`.
fn translate_profiled(java: &str, text: &str) -> String {
    let profile = std::env::temp_dir().join(format!("profiled-dispatch-{}.txt", std::process::id()));
    std::fs::write(&profile, text).unwrap();
    // SAFETY (of `set_var`): this binary's only test, so no other thread reads the environment.
    unsafe { std::env::set_var("JAVA_TO_RUST_DISPATCH_PROFILE", &profile) };
    let rust = translate(java);
    unsafe { std::env::remove_var("JAVA_TO_RUST_DISPATCH_PROFILE") };
    std::fs::remove_file(&profile).unwrap();
    rust
}

/// `java` translated to count the classes of its calls' receivers.
fn counted_of(java: &str) -> String {
    // SAFETY (of `set_var`): this binary's only test, so no other thread reads the environment.
    unsafe { std::env::set_var("JAVA_TO_RUST_COUNT_DISPATCH", "1") };
    let counted = translate(java);
    unsafe { std::env::remove_var("JAVA_TO_RUST_COUNT_DISPATCH") };
    counted
}

/// The key of the count, in `counted`, of the call whose key ends in `end`.
fn count_key<'a>(counted: &'a str, end: &str) -> &'a str {
    let at = counted.find(&format!("{end}\")")).unwrap_or_else(|| panic!("a count of `{end}`"));
    let start = counted[..at].rfind('"').unwrap() + 1;
    &counted[start..at + end.len()]
}

/// The class number of the class whose struct is `name`, from its arm in a dispatcher of `rust`.
fn class_number(rust: &str, name: &str) -> u16 {
    let arm =
        rust.find(&format!(" => {{ let o: &Arc<{name}>")).unwrap_or_else(|| panic!("an arm for `{name}`"));
    rust[..arm].rsplit(' ').next().unwrap().parse().expect("a class number")
}

#[test]
fn a_call_counts_its_classes_and_then_tests_for_the_one_it_mostly_saw() {
    // SAFETY (of `set_var`): this binary's only test, so no other thread reads the environment.
    unsafe { std::env::set_var("JAVA_TO_RUST_COUNT_DISPATCH", "1") };
    let counted = translate(SHAPES);
    unsafe { std::env::remove_var("JAVA_TO_RUST_COUNT_DISPATCH") };
    let key = "Main.total/1@9:39#area";
    assert_contains(
        &counted,
        &[&format!("static COUNT: java::DispatchCount = java::DispatchCount::new({key:?});")],
    );
    assert_contains(
        &counted,
        &["java::count_dispatch(&COUNT, j2r_receiver.0.tag()); MainShape__area(j2r_receiver)"],
    );
    // The class number of `Square`, from its arm in the dispatcher.
    let arm = counted.find(" => { let o: &Arc<MainSquare>").expect("an arm for `Square`");
    let square: u16 = counted[..arm].rsplit(' ').next().unwrap().parse().expect("a class number");
    let profile = std::env::temp_dir().join(format!("profiled-dispatch-{}.txt", std::process::id()));
    std::fs::write(
        &profile,
        format!("{key}\t{square}:850 {}:50 *:0\nOther.x/0@1:1#y\t{square}:5 *:0\n", square + 1),
    )
    .unwrap();
    unsafe { std::env::set_var("JAVA_TO_RUST_DISPATCH_PROFILE", &profile) };
    let profiled = translate(SHAPES);
    unsafe { std::env::remove_var("JAVA_TO_RUST_DISPATCH_PROFILE") };
    std::fs::remove_file(&profile).unwrap();
    // 900 calls are fewer than a call must have made: no test.
    assert!(!profiled.contains("_among_"), "{profiled}");
    std::fs::write(&profile, format!("{key}\t{square}:1900 {}:100 *:0\n", square + 1)).unwrap();
    unsafe { std::env::set_var("JAVA_TO_RUST_DISPATCH_PROFILE", &profile) };
    let profiled = translate(SHAPES);
    unsafe { std::env::remove_var("JAVA_TO_RUST_DISPATCH_PROFILE") };
    std::fs::remove_file(&profile).unwrap();
    let among = profiled.find("fn MainShape__area_among_").expect("a dispatcher among the profiled classes");
    let end = among + profiled[among..].find("\n}").unwrap();
    assert_contains(
        &profiled[among..end],
        &[
            &format!(
                "{square} => {{ let o: &Arc<MainSquare> = &unsafe {{ this.0.to_pointer() }}; o.area() }}"
            ),
            "_ => MainShape__area(this),",
        ],
    );
    assert!(!profiled.contains("count_dispatch"), "{profiled}");

    // A call that mostly saw a class whose method is a factory makes its object on the stack, in
    // the test's arm for that class.
    let counted = counted_of(BAGS);
    assert!(!counted.contains("_on_stack"), "{counted}");
    let key = count_key(&counted, "#cursor");
    let (arr, one) = (class_number(&counted, "MainArr"), class_number(&counted, "MainOne"));
    let profiled = translate_profiled(BAGS, &format!("{key}\t{arr}:2700 {one}:300 *:0\n"));
    let total = profiled.find("fn total(").expect("`total`");
    let total = &profiled[total..total + profiled[total..].find("\n}").unwrap()];
    assert_contains(total, &["let slot_1 = java::Slot::<", "_on_stack_", ", &slot_1)"]);
    let among = profiled.find("fn MainBag__cursor_among_").expect("a dispatcher that passes the slot on");
    let end = among + profiled[among..].find("\n}").unwrap();
    assert_contains(&profiled[among..end], &["j2r_slot: &java::Slot<", "cursor_on_stack(o, j2r_slot)"]);

    // The same, where the factory is a body on the enum of a class and the one below it.
    let java = BAGS
        .replace("static final class Arr implements", "static class Arr implements")
        .replace(
            "    static final class One implements",
            "    static final class Pair extends Arr { Pair() { super(new int[] { 3 }); } }\n    static final class One implements",
        )
        .replace("new One() };", "new One(), new Pair() };")
        .replace("bags[i % 10 == 0 ? 1 : 0]", "bags[i % 10 == 0 ? 1 : i % 3 == 0 ? 2 : 0]");
    let counted = counted_of(&java);
    let key = count_key(&counted, "#cursor");
    let (arr, pair, one) = (
        class_number(&counted, "MainArrFields"),
        class_number(&counted, "MainPair"),
        class_number(&counted, "MainOne"),
    );
    let profiled = translate_profiled(&java, &format!("{key}\t{arr}:2850 {pair}:100 {one}:50 *:0\n"));
    let total = profiled.find("fn total(").expect("`total`");
    let total = &profiled[total..total + profiled[total..].find("\n}").unwrap()];
    assert_contains(total, &["let slot_1 = java::Slot::<", "_on_stack_", ", &slot_1)"]);
    let among = profiled.find("fn MainBag__cursor_among_").expect("a dispatcher that passes the slot on");
    let end = among + profiled[among..].find("\n}").unwrap();
    assert_contains(&profiled[among..end], &["MainArr__cursor_on_stack(MainArr::Arr(o.clone()), j2r_slot)"]);
    assert_contains(&profiled, &["fn MainArr__cursor_on_stack("]);

    // An object given to a call that mostly saw a class whose method keeps it nowhere starts in a
    // slot, and is moved to the heap only for a receiver of another class.
    let counted = counted_of(PROFILER);
    let key = count_key(&counted, "#push");
    let (inactive, active) = (class_number(&counted, "MainInactive"), class_number(&counted, "MainActive"));
    let profiled = translate_profiled(PROFILER, &format!("{key}\t{inactive}:2900 {active}:100 *:0\n"));
    let tick = profiled.find("fn tick(").expect("`tick`");
    let tick = &profiled[tick..tick + profiled[tick..].find("\n}").unwrap()];
    assert_contains(
        tick,
        &[
            "let slot_1 = java::Slot::<",
            "slot_1.put(",
            &format!(
                "if !matches!(p.0.tag(), {inactive}) {{ unsafe {{ slot_1.to_heap(&mut argument_0) }}; }}"
            ),
        ],
    );
}
