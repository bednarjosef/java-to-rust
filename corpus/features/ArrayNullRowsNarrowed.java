// Rows that may be null, used after a null check: walked by for-each, measured, passed on and
// written, in a loop over the rows and in a plain `if`.
public class ArrayNullRowsNarrowed {
    static int sum(int[] row) {
        int s = 0;
        for (int x : row) {
            s += x;
        }
        return s;
    }

    public static void main(String[] args) {
        short[][] g = new short[3][];
        g[1] = new short[] {4, 5};
        g[2] = new short[0];
        String s = "";
        for (short[] r : g) {
            if (r == null) {
                s += "null;";
                continue;
            }
            for (short x : r) {
                s += x + ",";
            }
            s += r.length + ";";
        }
        System.out.println(s);

        int[][] m = new int[2][];
        m[0] = new int[] {1, 2, 3};
        for (int[] row : m) {
            if (row != null) {
                row[0] += 10;
                System.out.println(sum(row) + " " + row.length);
            } else {
                System.out.println("no row");
            }
        }
        int[] first = m[0];
        if (first != null) {
            int[] same = first;
            same[1] = 7;
            for (int x : first) {
                System.out.print(x + " ");
            }
            System.out.println();
        }

        String[][] words = new String[2][];
        words[1] = new String[] {"a", null};
        for (String[] w : words) {
            if (w == null) {
                System.out.println("none");
                continue;
            }
            for (String x : w) {
                System.out.println(x == null ? "null word" : x + x.length());
            }
        }
    }
}
