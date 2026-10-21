// Matrix products computed by row bands on several threads: each worker reads the two inputs,
// which every worker shares, and writes the rows of its band of the result. The checksums are
// printed once every worker has been joined, and compared with a product on the main thread.
public class ParallelMatrixRows {
    static final int WORKERS = 4;

    static class Matrix {
        final int rows;
        final int cols;
        final double[][] cells;

        Matrix(int rows, int cols) {
            this.rows = rows;
            this.cols = cols;
            this.cells = new double[rows][cols];
        }

        static Matrix pattern(int rows, int cols, int seed) {
            Matrix m = new Matrix(rows, cols);
            long state = seed;
            for (int i = 0; i < rows; i++) {
                for (int j = 0; j < cols; j++) {
                    state = (state * 1103515245L + 12345L) & 0x7fffffffL;
                    m.cells[i][j] = (state % 2001 - 1000) / 100.0;
                }
            }
            return m;
        }

        double checksum() {
            double sum = 0;
            for (int i = 0; i < rows; i++) {
                for (int j = 0; j < cols; j++) {
                    sum += cells[i][j] * ((i + 2 * j) % 5 + 1);
                }
            }
            return sum;
        }
    }

    static class Band extends Thread {
        private final Matrix a;
        private final Matrix b;
        private final Matrix out;
        private final int from;
        private final int to;
        int rowsDone;

        Band(Matrix a, Matrix b, Matrix out, int from, int to) {
            super("band-" + from);
            this.a = a;
            this.b = b;
            this.out = out;
            this.from = from;
            this.to = to;
        }

        @Override
        public void run() {
            for (int i = from; i < to; i++) {
                multiplyRow(a, b, out, i);
                rowsDone++;
            }
        }
    }

    static void multiplyRow(Matrix a, Matrix b, Matrix out, int i) {
        double[] row = a.cells[i];
        double[] target = out.cells[i];
        for (int j = 0; j < b.cols; j++) {
            double sum = 0;
            for (int k = 0; k < a.cols; k++) {
                sum += row[k] * b.cells[k][j];
            }
            target[j] = sum;
        }
    }

    static Matrix parallel(Matrix a, Matrix b) throws InterruptedException {
        Matrix out = new Matrix(a.rows, b.cols);
        Band[] bands = new Band[WORKERS];
        int step = (a.rows + WORKERS - 1) / WORKERS;
        for (int w = 0; w < WORKERS; w++) {
            int from = Math.min(a.rows, w * step);
            int to = Math.min(a.rows, from + step);
            bands[w] = new Band(a, b, out, from, to);
            bands[w].start();
        }
        int rows = 0;
        for (Band band : bands) {
            band.join();
            rows += band.rowsDone;
        }
        System.out.println("rows computed: " + rows);
        return out;
    }

    static Matrix sequential(Matrix a, Matrix b) {
        Matrix out = new Matrix(a.rows, b.cols);
        for (int i = 0; i < a.rows; i++) {
            multiplyRow(a, b, out, i);
        }
        return out;
    }

    public static void main(String[] args) throws InterruptedException {
        int[][] shapes = {{12, 9, 7}, {30, 30, 30}, {5, 40, 3}};
        for (int[] shape : shapes) {
            Matrix a = Matrix.pattern(shape[0], shape[1], shape[0] * 7 + shape[1]);
            Matrix b = Matrix.pattern(shape[1], shape[2], shape[2] * 13 + 1);
            Matrix p = parallel(a, b);
            Matrix s = sequential(a, b);
            System.out.printf("%dx%d times %dx%d: %.4f%n", a.rows, a.cols, b.rows, b.cols, p.checksum());
            System.out.println("same as on one thread: " + (p.checksum() == s.checksum()));
        }
    }
}
