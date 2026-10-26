import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.ToIntFunction;

// The class the JVM makes for a method reference unboxes its arguments and calls the method on its
// receiver; a null there throws a `NullPointerException` with no message.
public class MethodReferenceNullArguments {
    record Pair(int a, int b) {}

    static int twice(int x) {
        return 2 * x;
    }

    String name() {
        return "named";
    }

    static void show(String what, Runnable run) {
        try {
            run.run();
            System.out.println(what + ": no exception");
        } catch (NullPointerException e) {
            System.out.println(what + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        BiFunction<Integer, Integer, Pair> pair = Pair::new;
        Function<Pair, Integer> first = Pair::a;
        Function<Integer, Integer> twice = MethodReferenceNullArguments::twice;
        ToIntFunction<Integer> twiceInt = MethodReferenceNullArguments::twice;
        Function<MethodReferenceNullArguments, String> name = MethodReferenceNullArguments::name;
        show("pair", () -> System.out.println(pair.apply(1, 2)));
        show("pair first null", () -> pair.apply(null, 2));
        show("pair second null", () -> pair.apply(1, null));
        show("accessor", () -> System.out.println(first.apply(new Pair(3, 4))));
        show("accessor null", () -> first.apply(null));
        show("static", () -> System.out.println(twice.apply(5)));
        show("static null", () -> twice.apply(null));
        show("to int null", () -> twiceInt.applyAsInt(null));
        show("instance", () -> System.out.println(name.apply(new MethodReferenceNullArguments())));
        show("instance null", () -> name.apply(null));
    }
}
