// A split and a replacement by a pattern that `Pattern.quote` makes, inside a `try` that catches
// what the library may throw: the pattern always compiles, so neither throws
// `PatternSyntaxException`. As javac's Locations splits a search path.
import java.util.regex.Pattern;

public class QuotedPatternInTry {
    static int count(String path, String separator) {
        int n = 0;
        try {
            for (String s : path.split(Pattern.quote(separator), -1)) {
                if (!s.isEmpty()) {
                    n += Integer.parseInt(s);
                }
            }
        } catch (RuntimeException e) {
            return -1;
        }
        return n;
    }

    public static void main(String[] args) {
        System.out.println(count("1:2::3", ":"));
        System.out.println(count("1.2.x", "."));
        System.out.println(count("4*5", "*"));
        System.out.println("a+b+c".replaceAll(Pattern.quote("+"), "-"));
    }
}
