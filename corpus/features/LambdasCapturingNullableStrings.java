import java.util.function.Supplier;

// Lambdas that outlive the method that makes them and capture its string parameter, which may be
// null and which the method itself only reads.
public class LambdasCapturingNullableStrings {
    @SuppressWarnings("unchecked")
    static final Supplier<String>[] KEPT = new Supplier[3];
    static int kept;

    static void keep(String name) {
        if (name != null && name.isEmpty()) {
            return;
        }
        KEPT[kept++] = () -> name == null ? "none" : name.toUpperCase();
    }

    static Supplier<Integer> length(String text) {
        System.out.println("length of " + text);
        return () -> text == null ? -1 : text.length();
    }

    public static void main(String[] args) {
        keep("abc");
        keep(null);
        keep("");
        for (int i = 0; i < kept; i++) {
            Supplier<String> s = KEPT[i];
            System.out.println(s.get());
        }
        System.out.println(length("four").get() + " " + length(null).get());
    }
}
