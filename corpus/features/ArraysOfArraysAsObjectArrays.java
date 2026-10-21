// An `int[][]` returned as an `Object[]` is an `Object[]` throughout the translation, so
// `new int[n][m]` makes its rows as `int[]`s that become `Object`s, and `(int[]) row` gets each
// back, the same array: a write through the matrix shows through the `Object[]`.
public class ArraysOfArraysAsObjectArrays {
    static class Distances {
        private final int[][] matrix;

        Distances(int n) {
            matrix = new int[n + 1][n + 1];
            for (int i = 0; i <= n; i++) {
                for (int j = 0; j <= n; j++) {
                    matrix[i][j] = i == j ? 0 : i * 10 + j;
                }
            }
        }

        void shorten(int i, int j, int d) {
            if (d < matrix[i][j]) {
                matrix[i][j] = d;
            }
        }

        Object[] rows() {
            return matrix;
        }
    }

    static long[][][] cube = new long[2][3][2];

    static Object[] layers() {
        return cube;
    }

    public static void main(String[] args) {
        Distances d = new Distances(3);
        Object[] rows = d.rows();
        d.shorten(1, 3, 2);
        for (Object row : rows) {
            int[] r = (int[]) row;
            StringBuilder b = new StringBuilder();
            for (int v : r) {
                b.append(v).append(' ');
            }
            System.out.println(b.toString().trim());
        }
        System.out.println(rows.length + " " + ((int[]) rows[2]).length);

        cube[1][2][1] = 7;
        Object[] ls = layers();
        long[][] last = (long[][]) ls[1];
        System.out.println(ls.length + " " + last.length + " " + last[2][1] + " " + last[0][0]);
    }
}
