// `equalsIgnoreCase` whose receiver may throw and whose argument may be null, inside a `try`:
// the receiver is evaluated first, so its exception is thrown even where the argument is null.
public class EqualsIgnoreCaseThrowingReceiver {
    static int calls;

    static String name(int i) {
        calls++;
        if (i < 0) {
            throw new IllegalStateException("negative " + i);
        }
        return i == 0 ? "ABC" : "x";
    }

    static String other(int j) {
        return j > 5 ? null : "abc";
    }

    static String compare(int i, int j) {
        try {
            return String.valueOf(name(i).equalsIgnoreCase(other(j)));
        } catch (IllegalStateException e) {
            return "caught " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(compare(0, 0));
        System.out.println(compare(1, 0));
        System.out.println(compare(0, 9));
        System.out.println(compare(-1, 9));
        System.out.println(compare(-2, 0));
        System.out.println(calls);
    }
}
