// A `try` block whose result the translation holds in a local beside static methods named as
// that local would be, `attempt` and `attempt2`, which are called after it.
public class TryBesideSameNamedMethods {
    static int attempt(int x) {
        return x + 1;
    }

    static int attempt2(int x) {
        return x * 2;
    }

    static int parse(String s) {
        try {
            return Integer.parseInt(s);
        } catch (NumberFormatException e) {
            return -1;
        }
    }

    public static void main(String[] args) {
        try {
            System.out.println(Integer.parseInt("x"));
        } catch (NumberFormatException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println(parse("12"));
        } catch (RuntimeException e) {
            System.out.println("not reached");
        }
        System.out.println(attempt(1) + " " + attempt2(3) + " " + parse("q"));
    }
}
