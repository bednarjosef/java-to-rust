public class ArrayTypes {
    static long[] doubled(long[] values) {
        long[] out = new long[values.length];
        for (int i = 0; i < values.length; i++) {
            out[i] = values[i] * 2;
        }
        return out;
    }

    static int[][] grid(int n) {
        int[][] g = new int[n][n + 1];
        for (int r = 0; r < n; r++) {
            for (int c = 0; c < n + 1; c++) {
                g[r][c] = r * 10 + c;
            }
        }
        return g;
    }

    public static void main(String[] args) {
        long[] longs = doubled(new long[] {3L, 4000000000L});
        System.out.println(longs[0] + longs[1]);
        double[] ds = {1.5, 2.25};
        ds[0] *= 2;
        ds[1]++;
        System.out.println(ds[0] + " " + ds[1]);
        boolean[] flags = new boolean[3];
        flags[1] = true;
        System.out.println(flags[0] + " " + flags[1] + " " + flags.length);
        int[][] g = grid(2);
        g[1][2] += 100;
        System.out.println(g[1][2] + " " + g[0].length + " " + g.length);
        String[] names = {"a", "b"};
        System.out.println(names[0] + names[1] + names.length);
        for (int[] row : g) {
            System.out.println(row[0]);
        }
        System.out.println(args.length);
    }
}
