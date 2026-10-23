import java.io.File;
import java.util.Objects;
import java.util.Optional;
import java.util.StringJoiner;

// `==` on strings that the JDK's own code returns: the string it was given (a message, a thread's
// name, a normal path, `Objects.toString`, `Optional.orElse`, a field set and read back), a
// literal of its own, or a new string.
public class LibraryStringIdentity {
    public static void main(String[] args) {
        String s = new String("a/b");
        String fallback = new String("fallback");
        System.out.println(System.getProperty("no.such.property", fallback) == fallback);
        System.out.println(new RuntimeException(s).getMessage() == s);
        System.out.println(new RuntimeException().getMessage() == null);
        Thread thread = new Thread(() -> {}, s);
        System.out.println(thread.getName() == s);
        System.out.println(new File(s).getPath() == s);
        System.out.println(Objects.toString(s) == s);
        System.out.println(Objects.toString(null) == "null");
        System.out.println(Objects.toString(null, fallback) == fallback);
        System.out.println(String.valueOf(s) == s);
        System.out.println(String.valueOf((Object) null) == "null");
        System.out.println(Optional.of(s).orElse(fallback) == s);
        System.out.println(Optional.<String>empty().orElse(fallback) == fallback);
        System.out.println(String.join(",", s) == s);
        System.out.println(String.join(",", s).equals(s));
        StringJoiner joiner = new StringJoiner(",");
        joiner.setEmptyValue(s);
        System.out.println(joiner.toString() == s);
        joiner.add("x");
        System.out.println(joiner.toString() == "x");
    }
}
