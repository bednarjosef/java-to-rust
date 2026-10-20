// Counting loops whose accesses are checked once before the loop, and loops where that check
// fails: a jagged array whose short row throws at the iteration where Java throws, after the
// output of the earlier ones.
public class ArraySlicedLoops {
    static int[][] grid(int rows, int columns) {
        int[][] g = new int[rows][columns];
        for (int i = 0; i < rows; i++) {
            for (int j = 0; j < columns; j++) {
                g[i][j] = i * 10 + j;
            }
        }
        return g;
    }

    static int rowSum(int[][] g, int r, int n) {
        int s = 0;
        for (int j = 0; j < n; j++) {
            s += g[r][j];
        }
        return s;
    }

    static void addRow(int[][] g, int to, int from, int n) {
        for (int j = 0; j < n; j++) {
            g[to][j] += g[from][j] * 2;
        }
    }

    static long dot(long[] a, long[] b, int start, int end) {
        long s = 0;
        for (int j = start; j < end; j++) {
            s += a[j] * b[j];
        }
        return s;
    }

    public static void main(String[] args) {
        int[][] g = grid(3, 4);
        System.out.println("rowSum " + rowSum(g, 1, 4) + " " + rowSum(g, 2, 0) + " " + rowSum(g, 3, 0));

        addRow(g, 0, 2, 4);
        System.out.println(g[0][0] + " " + g[0][3]);

        long[] a = {1, 2, 3, 4, 5};
        long[] b = {5, 4, 3, 2, 1};
        System.out.println("dot " + dot(a, b, 0, 5) + " " + dot(a, b, 2, 4) + " " + dot(a, b, 4, 2));

        // A jagged array: the second row is too short, so its loop runs checked and prints what
        // it wrote before it throws.
        int[][] jagged = {new int[4], new int[2], new int[4]};
        int n = 4;
        for (int r = 0; r < 3; r++) {
            for (int j = 0; j < n; j++) {
                jagged[r][j] = r + j;
                System.out.print(jagged[r][j] + " ");
            }
            System.out.println();
        }
    }
}
