// A `new int[n][3]` whose `int[][]` is also an `Object[]`, inside a `try` that catches what it
// throws: a negative size, and a null array whose length gives the size. As the JDK's
// CaseFolding builds its hash table.
public class ObjectRowsInTry {
    static int[][] table(int[] keys, int shift) {
        var rows = new int[keys.length << shift][3];
        for (int i = 0; i < keys.length; i++) {
            rows[i][0] = keys[i];
        }
        return rows;
    }

    static int[] keys(int n) {
        return n < 0 ? null : new int[] {3, 5, 8, 13};
    }

    public static void main(String[] args) {
        Object[] all = table(keys(1), 1);
        System.out.println(all.length + " " + ((int[]) all[1])[0]);
        int[][] cases = {{1, 1}, {-1, 1}, {1, 29}};
        for (int[] c : cases) {
            try {
                System.out.println(table(keys(c[0]), c[1])[3][0]);
            } catch (NegativeArraySizeException e) {
                System.out.println("negative " + e.getMessage());
            } catch (Throwable e) {
                System.out.println("caught " + e.getClass().getName());
            }
        }
    }
}
