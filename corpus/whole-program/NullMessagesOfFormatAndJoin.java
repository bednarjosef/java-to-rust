import java.util.Iterator;
import java.util.List;

// The messages of a null format of `String.format`, a null receiver of `formatted`, null
// elements of `String.join` and a null iterator of them, with a format that a field
// initializer builds from an earlier field, as javac's `Option.HELP_LINT` does.
public class NullMessagesOfFormatAndJoin {
    final String indent = "  ";
    final String keyFormat = indent + "%-" + (28 - "    ".length()) + "s %s";
    String missing;

    String line(String key) {
        return String.format(keyFormat, key, "desc");
    }

    String broken(String key) {
        return String.format(missing, key, "desc");
    }

    String viaFormatted(String key) {
        return missing.formatted(key);
    }

    static String joined(List<String> values) {
        return String.join(", ", values);
    }

    static class NoIterator implements Iterable<String> {
        public Iterator<String> iterator() {
            return null;
        }
    }

    static String joinedAll(Iterable<String> values) {
        return String.join("+", values);
    }

    static void run(Runnable r) {
        try {
            r.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        NullMessagesOfFormatAndJoin f = new NullMessagesOfFormatAndJoin();
        run(() -> System.out.println(f.line("all")));
        run(() -> System.out.println(f.broken("all")));
        run(() -> System.out.println(f.viaFormatted("all")));
        run(() -> System.out.println(joined(List.of("x", "y"))));
        run(() -> System.out.println(joined(null)));
        run(() -> System.out.println(joinedAll(List.of("x", "y"))));
        run(() -> System.out.println(joinedAll(new NoIterator())));
    }
}
