import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.util.Objects;

// `Objects.requireNonNull` of a charset: the charset, or `NullPointerException` with the given
// message for null.
public class RequireNonNullCharset {
    static Charset pick(int k) { return k > 0 ? StandardCharsets.UTF_8 : null; }

    static String open(int k) {
        Charset cs = Objects.requireNonNull(pick(k), "charset");
        return cs.name();
    }

    public static void main(String[] args) {
        System.out.println(open(1));
        try {
            System.out.println(open(0));
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            Objects.requireNonNull(pick(0), "second");
            System.out.println("not reached");
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
    }
}
