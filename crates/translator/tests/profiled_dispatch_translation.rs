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
}
