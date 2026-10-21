// A for-each loop whose body replaces the array it walks: the loop goes on over the old array,
// and sees the writes made to it before it was replaced, but none made to the new one.
public class ArrayForEachReplacesItsArray {
    static int walk(int[] p) {
        int n = 0;
        for (int x : p) {
            n = n * 10 + x;
            p = new int[] {9};
        }
        return n + p[0];
    }

    public static void main(String[] args) {
        short[] a = {1, 2, 3};
        String s = "";
        for (short x : a) {
            s += x + ",";
            a = new short[2];
        }
        System.out.println(s + " " + a.length);

        int[] b = {1, 2, 3, 4};
        s = "";
        for (int x : b) {
            if (x == 1) {
                b[2] = 30;
                b = new int[] {7, 7, 7, 7};
            }
            b[3] = 40;
            s += x + ",";
        }
        System.out.println(s + " " + b[3]);

        String[] words = {"one", "two"};
        for (String w : words) {
            words = new String[] {w + "!"};
            System.out.println(w + " " + words[0]);
        }

        long[][] grid = {{1L, 2L}, {3L}};
        for (long[] row : grid) {
            row[0] += 100;
            grid = new long[][] {row};
            System.out.println(row[0] + " " + grid.length);
        }
        System.out.println(walk(new int[] {1, 2, 3}));
    }
}
