// `Math.floorMod` and `Math.floorDiv` in `try` blocks that catch the `ArithmeticException` of a
// zero divisor, directly and from a method, with `catch (RuntimeException)` and `finally`.
public class FloorModExceptions {
    static int[] values = {3, 4, 5};
    static int zero = 0;

    static int wrap(int k, int n) {
        return Math.floorMod(k, n);
    }

    static int cell(int[] a, int k) {
        return a[Math.floorMod(k, a.length)];
    }

    public static void main(String[] args) {
        try {
            System.out.println("mod " + Math.floorMod(-7, 3) + " " + Math.floorDiv(-7, 2));
            System.out.println("cell " + cell(values, -4) + " " + Math.abs(-3) + " " + Math.max(2, 5));
            System.out.println("wrap " + wrap(7, zero));
            System.out.println("not reached");
        } catch (ArithmeticException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println("div " + Math.floorDiv(9L, (long) zero));
        } catch (RuntimeException e) {
            System.out.println("runtime " + e.getMessage());
        } finally {
            System.out.println("finally");
        }
        try {
            System.out.println("empty " + cell(new int[0], 2));
        } catch (ArithmeticException e) {
            System.out.println("empty array " + e.getMessage());
        }
        System.out.println(wrap(-9, 4) + " " + Math.floorMod(10, 4));
    }
}
