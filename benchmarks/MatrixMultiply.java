// Repeated multiplication of two square int matrices: triple-nested loops over two-dimensional
// arrays, with int arithmetic that wraps on overflow.
public class MatrixMultiply {
    static int[][] create(int n, int seed) {
        int[][] m = new int[n][n];
        int x = seed;
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                x = x * 1103515245 + 12345;
                m[i][j] = (x >>> 16) % 100;
            }
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

    static long run(int n, int rounds) {
        int[][] a = create(n, 1);
        int[][] b = create(n, 2);
        long checksum = 0;
        for (int round = 0; round < rounds; round++) {
            int[][] c = multiply(a, b);
            for (int i = 0; i < n; i++) {
                for (int j = 0; j < n; j++) {
                    checksum = checksum * 31 + c[i][j];
                }
            }
            a = b;
            b = c;
        }
        return checksum;
    }

    public static void main(String[] args) {
        int n = 300;
        int rounds = 20;
        for (int warm = 0; warm < 2; warm++) {
            run(n, rounds);
        }
        long start = System.nanoTime();
        long checksum = run(n, rounds);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("multiplied " + rounds + " times, checksum " + checksum);
        System.err.println("kernel-ms " + elapsed);
    }
}
