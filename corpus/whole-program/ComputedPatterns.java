import java.util.Arrays;
import java.util.regex.PatternSyntaxException;

// Regular expressions whose pattern is computed run on the JDK's own `Pattern`, which throws, with
// Java's messages, for a malformed pattern or replacement, as javac's `Resolve` catches them.
public class ComputedPatterns {
    final String regex;

    ComputedPatterns(String regex) {
        this.regex = regex;
    }

    // `Resolve.DiagnosticRewriter.Template.matches`'s shape: a field's pattern in a `try`.
    boolean matches(String code) {
        try {
            return code.matches(regex);
        } catch (PatternSyntaxException e) {
            System.out.println("bad pattern: " + e.getMessage());
            return false;
        }
    }

    static String noisy(String s) {
        System.out.println("computed " + s);
        return s;
    }

    public static void main(String[] args) {
        String[] patterns = {"compiler\\.err\\..*", "a+b", "(unclosed", "x{2,1}", "[z-a]", "*"};
        for (String p : patterns) {
            ComputedPatterns c = new ComputedPatterns(p);
            System.out.println(p + ": " + c.matches("compiler.err.cant.resolve") + " " + c.matches("aab"));
        }
        String sep = args.length > 0 ? args[0] : ",";
        System.out.println(Arrays.toString("a,b,,c,,".split(sep)));
        System.out.println(Arrays.toString("a,b,,c,,".split(sep, -1)));
        System.out.println(Arrays.toString("a,b,,c,,".split(sep + "+", 2)));
        System.out.println(Arrays.toString("one  two three".split(noisy("\\s+"))));
        String word = noisy("o");
        System.out.println("foo boo".replaceAll(word, "0") + " " + "foo boo".replaceFirst(word + "+", "[$0]"));
        String[] replacements = {"<$1>", "$2", "\\", "${name}", "\\$"};
        for (String r : replacements) {
            try {
                System.out.println("ab".replaceAll(noisy("(a)"), r));
            } catch (IllegalArgumentException | IndexOutOfBoundsException e) {
                System.out.println(e.getClass().getName() + ": " + e.getMessage());
            }
        }
        // A null receiver throws once the arguments are computed, before the pattern compiles.
        String none = args.length > 3 ? "x" : null;
        try {
            System.out.println(none.matches(noisy("(bad")));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println(Arrays.toString(none.split(noisy(","), 3)));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println("text".replaceAll(noisy("(bad"), "x"));
        } catch (PatternSyntaxException e) {
            System.out.println("caught " + e.getDescription() + " at " + e.getIndex() + " in " + e.getPattern());
        }
    }
}
