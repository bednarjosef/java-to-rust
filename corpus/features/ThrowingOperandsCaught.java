import java.util.Arrays;

// An operation whose failure a `catch` sees, with an operand that throws: the operand's exception
// goes on as Java's does, before the operation runs, as in the JDK's `ArrayList.grow`, which
// copies its array to `ArraysSupport.newLength(...)`, a call that may throw.
public class ThrowingOperandsCaught {
    static class TooLarge extends Exception {
        TooLarge(String message) {
            super(message);
        }
    }

    static int newLength(int n) throws TooLarge {
        if (n > 100) {
            throw new TooLarge("too large: " + n);
        }
        return n - 5;
    }

    static String grow(int[] data, int n) {
        try {
            int[] bigger = Arrays.copyOf(data, newLength(n));
            int[] fresh = new int[newLength(n)];
            return bigger.length + " " + fresh.length;
        } catch (NegativeArraySizeException e) {
            return "negative";
        } catch (TooLarge e) {
            return e.getMessage();
        }
    }

    static int sized(int n) throws TooLarge {
        int[] a = new int[newLength(n)];
        return a.length;
    }

    public static void main(String[] args) {
        int[] data = {1, 2, 3};
        System.out.println(grow(data, 8));
        System.out.println(grow(data, 2));
        System.out.println(grow(data, 200));
        try {
            System.out.println(sized(10));
            System.out.println(sized(1));
        } catch (NegativeArraySizeException | TooLarge e) {
            System.out.println("failed " + e.getClass().getSimpleName());
        }
    }
}
