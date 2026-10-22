// A format that is a `static final` constant, which fits its arguments, inside a `try` that
// catches what a call of the library may throw: it never throws, as a literal format does. As in
// javac's TransTypes.
public class ConstantFormatInTry {
    private static final String MESSAGE = "state [%s] of %s is %d";

    static String describe(String name, int n) {
        try {
            if (n < 0) {
                throw new IllegalStateException(String.format(MESSAGE, name, "x", n));
            }
            return String.format(MESSAGE, name, "y", n);
        } catch (RuntimeException e) {
            return "caught " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(describe("a", 1));
        System.out.println(describe("b", -1));
    }
}
