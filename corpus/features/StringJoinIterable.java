import java.util.Iterator;

// String.join over an Iterable of strings: the collection's own iterator runs, nulls join as
// "null", an empty collection joins to "", and the delimiter is evaluated before the elements.
public class StringJoinIterable {
    static class Words implements Iterable<String> {
        private final String[] items;

        Words(String... items) {
            this.items = items;
        }

        public Iterator<String> iterator() {
            return new Iterator<String>() {
                int next = 0;

                public boolean hasNext() {
                    return next < items.length;
                }

                public String next() {
                    System.out.println("  next " + next);
                    return items[next++];
                }
            };
        }
    }

    static String delimiter(String d) {
        System.out.println("  delimiter");
        return d;
    }

    static Words words(String... items) {
        System.out.println("  words");
        return new Words(items);
    }

    public static void main(String[] args) {
        System.out.println(String.join(", ", new Words("a", "b", "c")));
        System.out.println("[" + String.join("-", new Words()) + "]");
        System.out.println(String.join("", new Words("x", null, "y")));
        System.out.println(String.join(delimiter(" / "), words("one", "two")));
        String joined = String.join("+", new Words("1")) + "=" + String.join("+", new Words("2", "3"));
        System.out.println(joined);
    }
}
