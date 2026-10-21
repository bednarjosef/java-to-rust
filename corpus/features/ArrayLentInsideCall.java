import java.util.Arrays;

// A shared array lent to a method whose result is an argument of a call that reads an alias of
// the array: the loan ends when the inner call returns, before the outer call reads it.
public class ArrayLentInsideCall {
    static int shift(int[] values, int k) {
        values[0] += k;
        return values[0];
    }

    public static void main(String[] args) {
        int[] a = {1, 5, 9};
        int[] b = a;
        System.out.println("found " + Arrays.binarySearch(a, shift(b, 4)));
        System.out.println(Arrays.toString(a) + " " + (shift(b, 1) + a[0]));
        int total = 0;
        for (int i = 0; i < 2; i++) {
            total += Math.max(a[1], shift(b, i));
        }
        System.out.println(total + " " + a[0]);
    }
}
