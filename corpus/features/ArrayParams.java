public class ArrayParams {
    static int sum(int[] v) {
        int s = 0;
        for (int i = 0; i < v.length; i++) { s += v[i]; }
        return s;
    }

    static void fill(int[] v, int k) {
        for (int i = 0; i < v.length; i++) { v[i] = i * k; }
    }

    static void pass(int[] v) { fill(v, 3); }

    static void copy(int[] from, int[] to) {
        for (int i = 0; i < from.length; i++) { to[i] = from[i] + 1; }
    }

    static int sumOf(int[] x, int[] y) {
        int s = 0;
        for (int i = 0; i < x.length; i++) { s += x[i] * 10 + y[i]; }
        return s;
    }

    static void bump(int[] v) { v[0] += 100; }

    public static void main(String[] args) {
        int[] a = new int[4];
        fill(a, 2);
        pass(a);
        System.out.println(sum(a) + " " + a[3]);

        int[] b = new int[4];
        copy(a, b);
        System.out.println(sum(b));

        int[] shared = {1, 2, 3};
        System.out.println(sumOf(shared, shared));
        bump(shared);
        System.out.println(shared[0] + " " + sum(shared));

        int[] c = {5, 6};
        int[] alias = c;
        bump(c);
        System.out.println(alias[0] + " " + sum(alias));

        long[] longs = {4L, 5L};
        double[] doubles = {0.5, 1.5};
        boolean[] flags = new boolean[3];
        String[] words = {"x", "yy"};
        int total = 0;
        for (int i = 0; i < words.length; i++) { total += words[i].length(); }
        for (int i = 0; i < flags.length; i++) { flags[i] = i % 2 == 0; }
        System.out.println(total + " " + longs.length + " " + doubles.length + " " + flags[2]);
    }
}
