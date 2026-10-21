import java.util.function.Function;

// A closure over an erased interface whose string parameter may be null and is borrowed: the
// cast argument is bound before it is borrowed, and a borrowed optional string becomes an Object.
public class NullableClosureArguments {
    static String run(Function<String, String> f, String s) {
        return f.apply(s);
    }

    public static void main(String[] args) {
        Function<String, String> twice = l -> l == null ? "none" : l + l;
        System.out.println(run(twice, "ab") + " " + run(twice, null));
    }
}
