// A negated pattern in an `||` chain: its variable is in scope in the later operands of the
// chain and in the else branch, where every operand was false and so the pattern matched.

public class NegatedPatternsInDisjunctions {
    static int complaints = 0;

    // As javac's Gen.checkStringConstant: the later operand reads the variable.
    static void check(Object constant) {
        if (complaints != 0
                || constant == null
                || !(constant instanceof String text)
                || text.length() < 5) {
            return;
        }
        complaints++;
        System.out.println("too long: " + constant);
    }

    // As javac's Annotate.attributeTypeAnnotation: the else branch reads the variable.
    static String describe(Object cached, boolean fresh) {
        if (fresh || cached == null || !(cached instanceof StringBuilder builder)) {
            return "made anew";
        } else {
            return "reused " + builder.length() + " chars";
        }
    }

    // Both, with a negated operand that is not a pattern.
    static String size(Object value, boolean known) {
        if (!known || !(value instanceof Integer n) || n < 0) {
            return "unknown";
        } else {
            return "size " + (n + 1);
        }
    }

    public static void main(String[] args) {
        check(42);
        check("abc");
        check(null);
        check("abcdefgh");
        check("abcdefghij");
        System.out.println("complaints: " + complaints);
        System.out.println(describe(new StringBuilder("hello"), false));
        System.out.println(describe(new StringBuilder("hello"), true));
        System.out.println(describe("hello", false));
        System.out.println(describe(null, false));
        System.out.println(size(7, true));
        System.out.println(size(7, false));
        System.out.println(size(-7, true));
        System.out.println(size("7", true));
    }
}
