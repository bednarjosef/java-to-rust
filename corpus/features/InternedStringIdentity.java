// `==` on strings that Java interns, which is the same object exactly when the text is the same:
// literals, constant variables and their concatenations, and `intern()`, as javac's
// MatchingUtils compares a pattern with its constant.
public class InternedStringIdentity {
    static final String ALL = ".*";
    static final String PREFIX = "java.";
    static final String LANG = PREFIX + "lang";

    static String pattern(String s) {
        return (s.equals("*") ? ALL : s.replace(".", "\\.")).intern();
    }

    public static void main(String[] args) {
        System.out.println(ALL == ".*");
        System.out.println(LANG == "java.lang");
        System.out.println(LANG != "java.util");
        System.out.println(pattern("*") == ALL);
        System.out.println(pattern("a.b") == ALL);
        String built = new StringBuilder(".").append("*").toString();
        System.out.println(built.intern() == ALL);
        boolean wide = args.length == 0;
        System.out.println((wide ? ALL : "x") == ".*");
    }
}
