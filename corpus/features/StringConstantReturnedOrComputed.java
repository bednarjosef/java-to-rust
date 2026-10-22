// `==` between a string constant and what a method returns that returns that constant on one
// path and computes the string on the others, as javac's MatchingUtils compares a pattern with
// `allMatchesString`: compared as text, on the assumption (which the translator notes) that no
// computed string has the constant's text.
public class StringConstantReturnedOrComputed {
    private static final String ALL = ".*";

    static String toPattern(String s) {
        if (s.equals("*")) {
            return ALL;
        } else {
            String p = s.replace(".", "\\.");
            if (p.endsWith("*")) {
                p = p.substring(0, p.length() - 1) + ".+";
            }
            return p;
        }
    }

    static String describe(String s) {
        String pattern = toPattern(s);
        if (pattern == ALL) {
            return s + ": everything";
        }
        return s + ": " + pattern + (toPattern(s) != ALL ? " (computed)" : "");
    }

    public static void main(String[] args) {
        for (String s : new String[] {"*", "java.util.*", "a.b", "x"}) {
            System.out.println(describe(s));
        }
    }
}
