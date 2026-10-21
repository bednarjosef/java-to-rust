// Arrays of arrays that no other name sees: made, filled, read and returned as plain values.
public class ArrayGrids {
    static int[][] identity(int n) {
        int[][] m = new int[n][n];
        for (int i = 0; i < n; i++) {
            m[i][i] = 1;
        }
        return m;
    }

    static int[][] multiply(int[][] a, int[][] b) {
        int n = a.length;
        int[][] c = new int[n][n];
        for (int i = 0; i < n; i++) {
            for (int k = 0; k < n; k++) {
                int aik = a[i][k];
                for (int j = 0; j < n; j++) {
                    c[i][j] += aik * b[k][j];
                }
            }
        }
        return c;
    }

    static void scale(int[][] m, int factor) {
        for (int i = 0; i < m.length; i++) {
            for (int j = 0; j < m[i].length; j++) {
                m[i][j] *= factor;
            }
        }
    }

    static long total(int[][] m) {
        long sum = 0;
        for (int i = 0; i < m.length; i++) {
            for (int j = 0; j < m[i].length; j++) {
                sum += m[i][j];
            }
        }
        return sum;
    }

    static int rowSum(int[] row) {
        int s = 0;
        for (int x : row) {
            s += x;
        }
        return s;
    }

    static void bumpRow(int[] row) {
        row[0]++;
    }

    public static void main(String[] args) {
        int[][] a = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}};
        int[][] id = identity(3);
        int[][] p = multiply(a, id);
        System.out.println(total(p) + " " + p[2][1] + " " + p.length + " " + p[0].length);
        scale(p, 2);
        System.out.println(total(p) + " " + p[1][1]);

        // Rows lent to methods.
        System.out.println(rowSum(a[1]) + " " + rowSum(p[2]));
        bumpRow(a[0]);
        bumpRow(a[0]);
        System.out.println(a[0][0] + " " + rowSum(a[0]));

        // A jagged literal, rows replaced by new arrays, and their lengths.
        int[][] jagged = {{1}, {2, 3}, {4, 5, 6}};
        jagged[0] = new int[4];
        jagged[0][3] = 9;
        System.out.println(jagged[0].length + " " + jagged[0][3] + " " + jagged[2].length + " " + total(jagged));

        // Dynamic programming: each cell from its neighbours, in place.
        int n = 6;
        long[][] paths = new long[n][n];
        for (int i = 0; i < n; i++) {
            paths[i][0] = 1;
            paths[0][i] = 1;
        }
        for (int i = 1; i < n; i++) {
            for (int j = 1; j < n; j++) {
                paths[i][j] = paths[i - 1][j] + paths[i][j - 1];
            }
        }
        System.out.println(paths[n - 1][n - 1]);
        for (int i = 1; i < n; i++) {
            for (int j = 1; j < n; j++) {
                paths[i][j] += paths[i - 1][j - 1];
            }
        }
        System.out.println(paths[n - 1][n - 1]);

        // Three dimensions, chars, doubles, booleans and strings.
        int[][][] cube = new int[2][3][4];
        for (int x = 0; x < 2; x++) {
            for (int y = 0; y < 3; y++) {
                for (int z = 0; z < 4; z++) {
                    cube[x][y][z] = x * 100 + y * 10 + z;
                }
            }
        }
        cube[1][2][3]++;
        cube[0][1][2] -= 5;
        System.out.println(cube[1][2][3] + " " + cube[0][1][2] + " " + cube[1].length + " " + cube[1][2].length);
        char[][] board = new char[3][3];
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                board[i][j] = (i + j) % 2 == 0 ? 'x' : 'o';
            }
        }
        String line = "";
        for (int i = 0; i < 3; i++) {
            line += new String(board[i]) + "|";
        }
        System.out.println(line);
        double[][] weights = {{0.5, 1.5}, {2.5, 3.5}};
        weights[1][0] /= 2;
        System.out.println(weights[1][0] + weights[0][1]);
        boolean[][] seen = new boolean[2][2];
        seen[1][1] = !seen[1][1];
        System.out.println(seen[1][1] + " " + seen[0][1]);
        String[][] names = {{"ada", "bob"}, {"cy"}};
        names[1][0] += "!";
        System.out.println(names[0][1] + names[1][0] + names[1].length);

        // An index that writes the same array, evaluated before the element is read.
        int[][] counters = new int[2][3];
        counters[1][counters[0][0]++] = 7;
        counters[1][counters[0][0]++] += counters[1][0] + 1;
        System.out.println(counters[0][0] + " " + counters[1][0] + " " + counters[1][1]);
    }
}
