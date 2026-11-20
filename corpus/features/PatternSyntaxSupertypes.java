import java.util.regex.PatternSyntaxException;

// A PatternSyntaxException that the runtime raises, caught as an IllegalArgumentException, a
// RuntimeException and an Exception, with its class and toString; and one the program makes,
// whose message is built the same way, without an index or a pattern too.
public class PatternSyntaxSupertypes {
    static String[] split(String text, String regex) {
        return text.split(regex);
    }

    public static void main(String[] args) {
        try {
            split("a,b", "(,");
        } catch (IllegalArgumentException e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
        try {
            split("a,b", "[,");
        } catch (RuntimeException e) {
            System.out.println(e);
            if (e instanceof PatternSyntaxException p) {
                System.out.println(p.getIndex() + " " + p.getDescription());
            }
        }
        try {
            System.out.println("a,b".replaceAll("+", ";"));
        } catch (Exception e) {
            System.out.println(e);
        }
        System.out.println(String.join("|", split("a,b;c", "[,;]")));
        PatternSyntaxException made = new PatternSyntaxException("Made here", "a\tb[c", 4);
        System.out.println(made.getMessage());
        System.out.println(made);
        System.out.println(new PatternSyntaxException("No index", "abc", -1).getMessage());
        System.out.println(new PatternSyntaxException("Past the end", "abc", 3).getMessage());
        System.out.println(new PatternSyntaxException("No pattern", null, 0).getMessage());
        try {
            throw made;
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + ((PatternSyntaxException) e).getPattern());
        }
    }
}
