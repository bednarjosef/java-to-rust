// Variables that may be null, assigned a value that is not inside a larger expression: the value
// of the assignment is that array or string, used as it is.
public class NullableAssignedInsideExpressions {
    static int[] pick(int k) {
        return k > 0 ? new int[k] : null;
    }

    public static void main(String[] args) {
        int[] a = pick(0);
        int[] b = {4, 5, 6};
        System.out.println((a = b).length + " " + a[2]);
        int[] c = pick(2);
        c[0] = (c = b).length;
        System.out.println(c[0] + " " + b[0] + " " + c.length);

        long[][] g = new long[3][];
        long[] row = g[0];
        long[] full = {1L, 2L};
        long total = (row = full)[1] + (row = new long[] {7L})[0];
        System.out.println(total + " " + row.length + " " + (g[1] = full).length);

        String s = args.length > 5 ? "unused" : null;
        System.out.println((s = "naïve").length() + " " + s);
        String t = null;
        String u = (t = "x" + s.length()) + "!";
        System.out.println(t + " " + u);
        String[] words = new String[2];
        String w = words[0];
        for (int i = 0; i < 2; i++) {
            System.out.println((w = "w" + i).charAt(1) + " " + w);
        }
    }
}
