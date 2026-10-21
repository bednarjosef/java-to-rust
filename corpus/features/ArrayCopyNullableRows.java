// System.arraycopy between rows taken from arrays whose rows may be null: within one row, from
// a row into a local array and back, and between rows of characters.
public class ArrayCopyNullableRows {
    public static void main(String[] args) {
        double[][] g = new double[3][];
        g[0] = new double[] {1.5, 2.5, 3.5};
        g[1] = new double[] {9.0};
        double[] row = g[0];
        System.arraycopy(row, 0, row, 1, 2);
        System.out.println(row[0] + " " + row[1] + " " + row[2]);

        double[] local = new double[2];
        double[] from = g[1];
        System.arraycopy(from, 0, local, 1, 1);
        System.arraycopy(local, 0, row, 0, 2);
        System.out.println(local[1] + " " + row[0] + " " + row[1] + " " + g[0][1]);

        char[][] c = new char[3][];
        c[1] = "hey".toCharArray();
        c[2] = "abc".toCharArray();
        char[] dst = c[1];
        char[] src = c[2];
        System.arraycopy(src, 0, dst, 0, 1);
        System.arraycopy(dst, 1, dst, 0, 2);
        System.out.println(new String(dst) + " " + (c[0] == null));
    }
}
