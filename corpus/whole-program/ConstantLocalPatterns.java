/**
 * A regular expression call whose pattern is a local constant (a `final` local initialized with a
 * literal) compiles and so never throws `PatternSyntaxException`, as a literal pattern does: a
 * `catch` around it sees only what the rest of the code throws.
 */
public class ConstantLocalPatterns {
    static String part(String line) {
        final String marker = "Part:";
        return line.split(marker)[1].trim();
    }

    static boolean dotted(String line) {
        final String dots = "[.]+";
        final String replacement = "-";
        return !line.replaceAll(dots, replacement).equals(line) || line.matches(dots);
    }

    public static void main(String[] args) {
        for (String line : new String[] {"Part: 80P4315", "Serial Number"}) {
            try {
                System.out.println("part " + part(line));
            } catch (RuntimeException e) {
                System.out.println("caught " + e.getClass().getName());
            }
            try {
                System.out.println("dotted " + dotted(line));
            } catch (IllegalArgumentException e) {
                System.out.println("caught " + e.getClass().getName());
            }
        }
    }
}
