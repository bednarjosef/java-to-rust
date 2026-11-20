import java.util.regex.PatternSyntaxException;

// A malformed pattern of split, matches, replaceAll and replaceFirst, caught as a
// PatternSyntaxException: its message (the description, the index, the pattern and a caret
// under the index, with a tab under each tab before it), and its description, index and pattern
// one by one. Each kind of error the runtime finds, some patterns only known when the program
// runs, and a quote, whose escapes count in the index.
public class PatternSyntaxCaught {
    static void show(PatternSyntaxException e) {
        System.out.println("[" + e.getMessage() + "]");
        System.out.println(e.getDescription() + " | " + e.getIndex() + " | " + e.getPattern());
    }

    static String opened(int depth) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < depth; i++) {
            sb.append("(a");
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        String[] patterns = {
            "(ab", "*a", "a|+b", "a{", "a{2,1}", "a{1,x}", "a{99999999999}", "\\x4", "\\u12g4", "ab\\",
            "[\\R]", "\\y", "[abc", "[]", "[z-a]", "[a-\\d]", "ab)c", ")", "(?z)", "\\k",
            "\ta{x}", "a\t\tb{x}", "\\Qa.b\\E)", "[\\Qab]", opened(3), "x" + opened(1) + "\\"
        };
        for (String p : patterns) {
            try {
                System.out.println("abc".split(p).length);
            } catch (PatternSyntaxException e) {
                show(e);
            }
        }
        try {
            System.out.println("abc".matches("a(b"));
        } catch (PatternSyntaxException e) {
            show(e);
        }
        try {
            System.out.println("abc".replaceAll("b{", "x"));
        } catch (PatternSyntaxException e) {
            show(e);
        }
        try {
            System.out.println("abc".replaceFirst("[b-a]", "x"));
        } catch (PatternSyntaxException e) {
            show(e);
        }
    }
}
