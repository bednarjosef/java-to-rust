public class MatrixOps {
    static final long MOD = 1_000_000_007L;

    static long[][] identity(int n) {
        long[][] m = new long[n][n];
        for (int i = 0; i < n; i++) {
            m[i][i] = 1;
        }
        return m;
    }

    static long[][] multiply(long[][] a, long[][] b) {
        int n = a.length;
        int m = b[0].length;
        int inner = b.length;
        long[][] c = new long[n][m];
        for (int i = 0; i < n; i++) {
            for (int k = 0; k < inner; k++) {
                if (a[i][k] == 0) {
                    continue;
                }
                for (int j = 0; j < m; j++) {
                    c[i][j] = (c[i][j] + a[i][k] * b[k][j]) % MOD;
                }
            }
        }
        return c;
    }

    static long[][] power(long[][] base, long exp) {
        long[][] result = identity(base.length);
        long[][] b = base;
        long e = exp;
        while (e > 0) {
            if ((e & 1) == 1) {
                result = multiply(result, b);
            }
            b = multiply(b, b);
            e >>= 1;
        }
        return result;
    }

    static double[][] transpose(double[][] a) {
        double[][] t = new double[a[0].length][a.length];
        for (int i = 0; i < a.length; i++) {
            for (int j = 0; j < a[0].length; j++) {
                t[j][i] = a[i][j];
            }
        }
        return t;
    }

    /** Determinant by Gaussian elimination with partial pivoting. */
    static double determinant(double[][] input) {
        int n = input.length;
        double[][] a = new double[n][n];
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                a[i][j] = input[i][j];
            }
        }
        double det = 1.0;
        for (int col = 0; col < n; col++) {
            int pivot = col;
            for (int r = col + 1; r < n; r++) {
                if (Math.abs(a[r][col]) > Math.abs(a[pivot][col])) {
                    pivot = r;
                }
            }
            if (a[pivot][col] == 0.0) {
                return 0.0;
            }
            if (pivot != col) {
                double[] tmp = a[pivot];
                a[pivot] = a[col];
                a[col] = tmp;
                det = -det;
            }
            det *= a[col][col];
            for (int r = col + 1; r < n; r++) {
                double factor = a[r][col] / a[col][col];
                for (int c = col; c < n; c++) {
                    a[r][c] -= factor * a[col][c];
                }
            }
        }
        return det;
    }

    /** Solves a x = b for a square system; returns x. */
    static double[] solve(double[][] input, double[] rhs) {
        int n = input.length;
        double[][] a = new double[n][n + 1];
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                a[i][j] = input[i][j];
            }
            a[i][n] = rhs[i];
        }
        for (int col = 0; col < n; col++) {
            for (int r = 0; r < n; r++) {
                if (r == col) {
                    continue;
                }
                double f = a[r][col] / a[col][col];
                for (int c = col; c <= n; c++) {
                    a[r][c] -= f * a[col][c];
                }
            }
        }
        double[] x = new double[n];
        for (int i = 0; i < n; i++) {
            x[i] = a[i][n] / a[i][i];
        }
        return x;
    }

    static void printMatrix(String title, long[][] m) {
        System.out.println(title + ":");
        for (long[] row : m) {
            String line = "";
            for (long v : row) {
                line = line + v + " ";
            }
            System.out.println("  " + line);
        }
    }

    public static void main(String[] args) {
        long[][] fib = {{1, 1}, {1, 0}};
        for (long n : new long[] {10, 50, 90, 1000000}) {
            long[][] p = power(fib, n);
            System.out.println("F(" + n + ") mod p = " + p[0][1]);
        }

        long[][] a = {{1, 2, 3}, {4, 5, 6}, {7, 8, 10}};
        long[][] b = {{2, 0, 1}, {1, 3, 0}, {0, 1, 4}};
        printMatrix("A*B", multiply(a, b));
        printMatrix("A^5", power(a, 5));

        double[][] d = {
            {2, -1, 0, 0},
            {-1, 2, -1, 0},
            {0, -1, 2, -1},
            {0, 0, -1, 2},
        };
        System.out.println("det = " + determinant(d));
        double[][] t = transpose(new double[][] {{1, 2, 3}, {4, 5, 6}});
        System.out.println("transpose rows " + t.length + " cols " + t[0].length + " t[2][1] = " + t[2][1]);

        double[][] sys = {{4, 1, 0}, {1, 3, 1}, {0, 1, 2}};
        double[] x = solve(sys, new double[] {1, 2, 3});
        System.out.println("x = " + x[0] + " " + x[1] + " " + x[2]);

        double[][] hilbert = new double[6][6];
        for (int i = 0; i < 6; i++) {
            for (int j = 0; j < 6; j++) {
                hilbert[i][j] = 1.0 / (i + j + 1);
            }
        }
        System.out.println("det hilbert 6 ~ " + determinant(hilbert));
        System.out.println("det of singular = " + determinant(new double[][] {{1, 2}, {2, 4}}));
    }
}
