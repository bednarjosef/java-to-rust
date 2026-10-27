import java.util.List;

// `String.join` of a `CharSequence[]` whose elements are not all strings, as Guava's `Joiner`
// joins its parts: each element's `String.valueOf`, a null one "null". A null delimiter throws
// once both arguments are evaluated: of an array, from its `toString()`, which the message
// names; of an `Iterable`, from `Objects.requireNonNull`, with no message.
public class JoinedCharSequences {
    static int evaluated;

    static String join(String delimiter, CharSequence[] parts) {
        try {
            return String.join(delimiter, parts);
        } catch (NullPointerException e) {
            return "NullPointerException: " + e.getMessage();
        }
    }

    static <T> T counted(T value) {
        evaluated++;
        return value;
    }

    public static void main(String[] args) {
        StringBuilder builder = new StringBuilder("built");
        CharSequence[] parts = {"one", builder, null, new StringBuilder()};
        System.out.println(join(", ", parts));
        builder.append('!');
        System.out.println(join("", parts));
        System.out.println("[" + join("-", new CharSequence[0]) + "]");
        System.out.println(join(null, parts));
        StringBuilder[] builders = {new StringBuilder("a"), new StringBuilder("b")};
        System.out.println(String.join("+", builders));
        String none = null;
        try {
            System.out.println(String.join(none, counted(builders)));
        } catch (NullPointerException e) {
            System.out.println("array: " + e.getMessage() + " after " + evaluated);
        }
        try {
            System.out.println(String.join(none, counted(List.of("x", "y"))));
        } catch (NullPointerException e) {
            System.out.println("iterable: " + e.getMessage() + " after " + evaluated);
        }
    }
}
