import java.util.Arrays;

// Arrays.copyOfRange of an array that may be null, whose exception a catch reads, as the JDK's
// MutableBigInteger copies another's digits: the JDK's code reads a null array's length (after
// checking the range, unless from is 0), or asks an array of objects for its class first.
public class CopyOfRangeNullArrays {
    static int[] digits(int n) {
        if (n < 0) {
            return null;
        }
        int[] d = new int[n];
        for (int i = 0; i < n; i++) {
            d[i] = i * 3;
        }
        return d;
    }

    static String[] words(int n) {
        return n < 0 ? null : new String[] {"a", "b", "c"};
    }

    static String ints(int n, int from, int to) {
        try {
            return Arrays.toString(Arrays.copyOfRange(digits(n), from, to));
        } catch (RuntimeException e) {
            return "failed: " + e;
        }
    }

    static String strings(int n, int from, int to) {
        try {
            return Arrays.toString(Arrays.copyOfRange(words(n), from, to));
        } catch (RuntimeException e) {
            return "failed: " + e;
        }
    }

    public static void main(String[] args) {
        System.out.println(ints(4, 1, 6));
        System.out.println(ints(-1, 0, 2));
        System.out.println(ints(-1, 1, 2));
        System.out.println(ints(-1, 2, 1));
        System.out.println(ints(-1, 0, -1));
        System.out.println(strings(3, 1, 2));
        System.out.println(strings(-1, 2, 1));
    }
}
