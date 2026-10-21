// Arrays assigned inside larger expressions: the array and index of a store are fixed before its
// value replaces the array, a chain of stores into arrays whose strings may be null, and an
// array computed from itself by a method that borrows it.
public class ArrayAssignedInsideExpressions {
    static int[] reversed(int[] p) {
        int[] r = new int[p.length];
        for (int i = 0; i < p.length; i++) {
            r[i] = p[p.length - 1 - i];
        }
        return r;
    }

    static long[] doubled(long[] p) {
        long[] r = new long[p.length * 2];
        for (int i = 0; i < r.length; i++) {
            r[i] = p[i % p.length];
        }
        return r;
    }

    public static void main(String[] args) {
        // The store goes to the old array, the value comes from the new one.
        int[] a = {1, 2, 3};
        int[] b = {7, 8};
        a[a.length - 1] = (a = b)[0];
        System.out.println(a[0] + " " + a.length + " " + b[1]);
        char[] c = {'x', 'y'};
        char[] d = {'p', 'q', 'r'};
        for (int i = 0; i < 3; i++) {
            c[0] = (c = d)[i];
        }
        System.out.println(new String(c) + " " + new String(d));
        int[] n = {5};
        int[] m = {6, 6, 6};
        n[0] = (n = m).length;
        System.out.println(n[0] + " " + m[0] + " " + n.length);

        // Chains of stores where the elements may be null.
        String[] s = new String[3];
        s[0] = s[1] = "two";
        System.out.println(s[0] + " " + s[1] + " " + s[2]);
        String[] t = {"a", "b"};
        String[] u = new String[2];
        String got = u[1] = t[0] = "c";
        System.out.println(got + " " + t[0] + " " + u[0] + " " + u[1]);
        String[] shared = new String[2];
        String[] alias = shared;
        shared[0] = alias[1] = "both";
        System.out.println(alias[0] + " " + shared[1] + " " + (shared == alias));

        // An array replaced by what a borrowing method makes of it, while another name keeps it.
        int[] x = {1, 2, 3};
        int[] keep = x;
        x = reversed(x);
        x[0] += 100;
        System.out.println(x[0] + " " + keep[0] + " " + x.length);
        long[] y = {4L, 5L};
        long[] y2 = y;
        for (int i = 0; i < 2; i++) {
            y = doubled(y);
        }
        System.out.println(y.length + " " + y[7] + " " + y2.length);
    }
}
