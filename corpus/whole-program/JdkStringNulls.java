import java.util.Base64;
import java.util.Locale;
import java.util.StringTokenizer;
import java.util.UUID;

// The JVM's messages of the NullPointerExceptions that the JDK's code raises when it calls a
// method of a string that is null, which name the method by `String` and the parameters of its
// overload.
public class JdkStringNulls {
    interface Action {
        void run();
    }

    static void attempt(String what, Action action) {
        try {
            action.run();
            System.out.println(what + ": no exception");
        } catch (NullPointerException e) {
            System.out.println(what + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        attempt("uuid", () -> UUID.fromString(null));
        attempt("tag", () -> Locale.forLanguageTag(null));
        attempt("tokenizer", () -> new StringTokenizer(null));
        attempt("base64", () -> Base64.getDecoder().decode((String) null));
        System.out.println(UUID.fromString("123e4567-e89b-12d3-a456-426614174000").getLeastSignificantBits());
    }
}
