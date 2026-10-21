// For-each loops over the rows of arrays of arrays: reading rows, writing through them, nested
// cubes, rows passed to methods, and loops whose rows escape or whose body uses the array.
public class ArrayRowLoops {
    static int sum(int[] row) {
        int s = 0;
        for (int x : row) {
            s += x;
        }
        return s;
    }

    static void bump(int[] row, int by) {
        for (int i = 0; i < row.length; i++) {
            row[i] += by;
        }
    }

    static int[][] grid(int rows, int columns) {
        int[][] g = new int[rows][columns];
        for (int i = 0; i < rows; i++) {
            for (int j = 0; j < columns; j++) {
                g[i][j] = i * columns + j;
            }
        }
        return g;
    }

    static int total(int[][] g) {
        int t = 0;
        for (int[] row : g) {
            t += sum(row);
        }
        return t;
    }

    static void doubleAll(int[][] g) {
        for (int[] row : g) {
            for (int j = 0; j < row.length; j++) {
                row[j] *= 2;
            }
        }
    }

    public static void main(String[] args) {
        int[][] g = grid(3, 4);
        for (int[] row : g) {
            StringBuilder line = new StringBuilder();
            for (int x : row) {
                line.append(x).append(' ');
            }
            System.out.println(line.toString().trim() + " | " + row.length);
        }
        System.out.println("total " + total(g));
        doubleAll(g);
        System.out.println("doubled " + total(g));
        for (int[] row : g) {
            bump(row, 1);
            row[0] = -row[0];
        }
        System.out.println("bumped " + total(g) + " " + g[1][0] + " " + g[2][3]);

        long[][][] cube = new long[2][3][2];
        long n = 1;
        for (long[][] plane : cube) {
            for (long[] cells : plane) {
                for (int k = 0; k < cells.length; k++) {
                    cells[k] = n * n;
                    n++;
                }
            }
        }
        long all = 0;
        for (long[][] plane : cube) {
            for (long[] cells : plane) {
                for (long v : cells) {
                    all += v;
                }
            }
        }
        System.out.println("cube " + all + " " + cube[1][2][1]);

        // A row kept after the loop is a second name for it: the writes show through.
        int[][] h = grid(2, 2);
        int[] last = null;
        for (int[] r : h) {
            last = r;
        }
        last[0] = 99;
        System.out.println("kept " + h[1][0]);

        // The body reads the array itself while it walks the rows.
        int[][] k = grid(3, 3);
        int i = 0;
        for (int[] r : k) {
            r[0] = k[(i + 1) % 3][1];
            i++;
        }
        System.out.println("self " + k[0][0] + " " + k[1][0] + " " + k[2][0]);

        // Jagged rows, one of them empty.
        int[][] jagged = {{1}, {}, {2, 3, 4}};
        for (int[] row : jagged) {
            System.out.println("jagged " + row.length + " " + sum(row));
        }

        String[][] words = {{"a", "b"}, {"c"}};
        for (String[] pair : words) {
            pair[0] = pair[0].toUpperCase();
        }
        System.out.println(words[0][0] + words[0][1] + words[1][0]);
    }
}
