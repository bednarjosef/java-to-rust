// The overloads of `insert` that take a range, of a `char[]` (an offset and a length) and of a
// `CharSequence` (a start and an end), as log4j's `FormattingInfo` pads a field; and the messages
// of a builder's offset and range checks, as Java 26 writes them.
public class BuilderInsertRanges {
    static final char[] SPACES = {' ', ' ', ' ', ' '};

    static int calls;

    static int next(int value) {
        calls++;
        return value;
    }

    static void pad(StringBuilder buffer, int fieldStart, int width) {
        int padLength = width - (buffer.length() - fieldStart);
        for (; padLength > SPACES.length; padLength -= SPACES.length) {
            buffer.insert(fieldStart, SPACES);
        }
        buffer.insert(fieldStart, SPACES, 0, padLength);
    }

    static void attempt(String name, Runnable r) {
        try {
            r.run();
            System.out.println(name + ": done");
        } catch (StringIndexOutOfBoundsException e) {
            System.out.println(name + ": " + e.getClass().getName() + ": " + e.getMessage());
        } catch (IndexOutOfBoundsException e) {
            System.out.println(name + ": " + e.getClass().getName() + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        StringBuilder line = new StringBuilder("level=");
        line.append("warn");
        pad(line, 6, 11);
        System.out.println("[" + line + "]");

        StringBuilder sb = new StringBuilder("ab");
        sb.insert(1, "xyz", 1, 3).insert(0, new char[] {'p', 'q', 'r'}, 1, 2);
        System.out.println(sb);
        CharSequence seq = args.length > 0 ? new StringBuilder("never") : "sequence";
        sb.insert(sb.length(), seq, 3, 6);
        System.out.println(sb);
        String none = args.length > 0 ? "x" : null;
        sb.insert(0, none, 1, 3);
        System.out.println(sb);
        sb.insert(2, sb, 0, 2);
        System.out.println(sb);

        StringBuffer buffer = new StringBuffer("12345");
        buffer.insert(next(2), new char[] {'a', 'b', 'c'}, next(0), next(3)).insert(5, "uvw", 0, 1);
        System.out.println(buffer + " after " + calls + " calls");

        attempt("offset past the end", () -> new StringBuilder("ab").insert(5, new char[] {'x'}, 0, 1));
        attempt("length past the array", () -> new StringBuilder("ab").insert(1, new char[] {'x'}, 0, 2));
        attempt("negative start", () -> new StringBuilder("ab").insert(1, new char[] {'x'}, -1, 1));
        attempt("start after end", () -> new StringBuilder("ab").insert(1, "xyz", 2, 1));
        attempt("offset before the range", () -> new StringBuilder("ab").insert(5, "xyz", 2, 1));
        attempt("range of null", () -> new StringBuilder("ab").insert(1, (CharSequence) null, 0, 5));
        attempt("plain insert", () -> new StringBuilder("ab").insert(-1, 'c'));
        attempt("delete", () -> new StringBuilder("ab").delete(2, 1));
        attempt("replace", () -> new StringBuilder("ab").replace(3, 5, "x"));
    }
}
