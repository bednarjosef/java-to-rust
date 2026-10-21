// The message of a NullPointerException that code creates, as Objects.requireNonNull does with
// the message it is given or that its supplier returns.
import java.util.Objects;
import java.util.function.Supplier;

public class NullMessages {
    static String name(String value) {
        return Objects.requireNonNull(value, "name is null");
    }

    static String lazy(String value, String what) {
        Supplier<String> message = () -> what + " is missing";
        return Objects.requireNonNull(value, message);
    }

    public static void main(String[] args) {
        System.out.println(name("ann") + " " + lazy("bob", "b"));
        try {
            name(null);
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            lazy(null, "the value");
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            String missing = null;
            Objects.requireNonNull(missing);
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            throw new NullPointerException("thrown");
        } catch (RuntimeException e) {
            System.out.println("caught: " + e.getMessage());
        }
    }
}
