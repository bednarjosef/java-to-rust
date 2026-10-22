// `Character.toChars` and `Character.toString` of a code point that is not valid throw
// `IllegalArgumentException`, which a `catch` sees with Java's message.
public class CodePointFailuresCaught {
    static String chars(int cp) {
        try {
            return new String(Character.toChars(cp));
        } catch (IllegalArgumentException e) {
            return "toChars: " + e.getMessage();
        }
    }

    static String text(int cp) {
        try {
            return Character.toString(cp);
        } catch (IllegalArgumentException e) {
            return "toString: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        int[] points = {0x41, 0x1F600, 0x110000, -1 - args.length};
        for (int cp : points) {
            System.out.println(chars(cp) + " | " + text(cp));
        }
    }
}
