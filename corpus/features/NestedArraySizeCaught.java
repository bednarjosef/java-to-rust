// Arrays of arrays whose length may be negative, inside a `try` that catches the
// `NegativeArraySizeException`, as the JDK's CaseFolding builds its hash table.
public class NestedArraySizeCaught {
    static int[][] table(int n) {
        return new int[n << 1][3];
    }

    static long[][][] cube(int a, int b) {
        return new long[a][b][2];
    }

    static String make(int n) {
        try {
            return "rows " + table(n).length;
        } catch (NegativeArraySizeException e) {
            return "negative: " + e.getMessage();
        }
    }

    static String build(int a, int b) {
        try {
            long[][][] c = cube(a, b);
            return "cube " + c.length + " " + (c.length > 0 ? c[0].length : -1);
        } catch (NegativeArraySizeException e) {
            return "negative: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(make(2));
        System.out.println(make(-3));
        System.out.println(build(2, 3));
        System.out.println(build(2, -1));
        System.out.println(build(-4, 1));
        System.out.println(build(0, -1));
    }
}
