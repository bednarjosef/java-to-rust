import java.util.Arrays;

public class ArrayCopy {
    static int[] grow(int[] a, int extra) {
        int[] bigger = new int[a.length + extra];
        System.arraycopy(a, 0, bigger, 0, a.length);
        return bigger;
    }

    static void shiftLeft(long[] a) {
        System.arraycopy(a, 1, a, 0, a.length - 1);
        a[a.length - 1] = 0;
    }

    static int sum(int[] a) {
        int s = 0;
        for (int i = 0; i < a.length; i++) {
            s += a[i];
        }
        return s;
    }

    public static void main(String[] args) {
        // Plain local arrays: copied between slices.
        int[] src = {1, 2, 3, 4, 5, 6, 7, 8};
        int[] dst = new int[10];
        System.arraycopy(src, 2, dst, 3, 4);
        for (int i = 0; i < dst.length; i++) {
            System.out.print(dst[i] + " ");
        }
        System.out.println();

        // Overlapping copies within one array, in both directions.
        int[] a = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
        System.arraycopy(a, 0, a, 2, 6);
        for (int i = 0; i < a.length; i++) {
            System.out.print(a[i] + " ");
        }
        System.out.println();
        System.arraycopy(a, 3, a, 1, a.length - 3);
        for (int i = 0; i < a.length; i++) {
            System.out.print(a[i] + " ");
        }
        System.out.println();
        System.arraycopy(a, 4, a, 4, 3);
        System.arraycopy(a, 0, a, 0, 0);
        System.arraycopy(a, a.length, a, 0, 0);
        System.out.println(sum(a));

        // Shared arrays: returned, printed with Arrays.toString.
        int[] grown = grow(new int[] {9, 8, 7}, 3);
        System.out.println(Arrays.toString(grown));
        long[] longs = {10L, 20L, 30L, 40L};
        shiftLeft(longs);
        shiftLeft(longs);
        System.out.println(Arrays.toString(longs));

        double[] ds = {0.5, -0.0, 1e300, Double.NaN};
        double[] dcopy = new double[6];
        System.arraycopy(ds, 0, dcopy, 1, 4);
        System.out.println(Arrays.toString(dcopy));

        boolean[] flags = {true, false, true};
        boolean[] more = new boolean[4];
        System.arraycopy(flags, 1, more, 2, 2);
        System.out.println(Arrays.toString(more));

        char[] cs = {'h', 'e', 'l', 'l', 'o'};
        char[] word = new char[5];
        System.arraycopy(cs, 0, word, 0, 5);
        System.arraycopy(cs, 1, cs, 0, 4);
        System.out.println(word);
        System.out.print(cs);
        System.out.println();

        byte[] bytes = {1, -2, 3};
        short[] shorts = {100, -200, 300};
        float[] floats = {1.5f, 2.5f, 3.5f};
        byte[] b2 = new byte[3];
        short[] s2 = new short[3];
        float[] f2 = new float[3];
        System.arraycopy(bytes, 0, b2, 0, 3);
        System.arraycopy(shorts, 1, s2, 0, 2);
        System.arraycopy(floats, 0, f2, 1, 2);
        System.out.println(Arrays.toString(b2) + Arrays.toString(s2) + Arrays.toString(f2));

        String[] names = {"ada", "grace", "barbara", "frances"};
        String[] shifted = {"-", "-", "-", "-", "-"};
        System.arraycopy(names, 0, shifted, 1, 4);
        System.arraycopy(names, 2, names, 0, 2);
        System.out.println(Arrays.toString(shifted) + " " + Arrays.toString(names));

        // Rows are references: the copy shares them.
        int[][] grid = {{1, 2}, {3, 4}, {5, 6}};
        int[][] rows = new int[3][2];
        System.arraycopy(grid, 0, rows, 0, 3);
        rows[1][0] = 33;
        System.out.println(grid[1][0] + " " + rows[2][1]);

        // Copying past the end throws before anything is copied.
        int[] small = new int[3];
        System.out.println("before");
        System.arraycopy(src, 6, small, 0, 3);
        System.out.println("not reached");
    }
}
