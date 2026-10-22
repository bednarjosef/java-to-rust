// A replacement that is not a constant may name a group the pattern lacks
// (`IndexOutOfBoundsException`) or end in a dangling escape or a malformed reference
// (`IllegalArgumentException`); a `catch` of either sees Java's message.
public class ReplacementFailuresCaught {
    static String replace(String text, String replacement, boolean first) {
        try {
            return first ? text.replaceFirst("(a)", replacement) : text.replaceAll("(a)", replacement);
        } catch (IndexOutOfBoundsException e) {
            return "index: " + e.getMessage();
        } catch (IllegalArgumentException e) {
            return e.getClass().getName() + ": " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        String[] replacements = {"<$1>", "$2", "x\\", "$x", "\\$", "$1".repeat(args.length + 2)};
        for (String r : replacements) {
            System.out.println(replace("abcab", r, false));
            System.out.println(replace("abcab", r, true));
        }
        try {
            System.out.println("q".replaceAll("q", "$3"));
        } catch (RuntimeException e) {
            System.out.println("runtime " + e.getMessage());
        }
    }
}
