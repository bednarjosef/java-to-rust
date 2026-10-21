// Index failures of plain arrays (a `Vec` and a `Vec` of `Vec`s) and of shared ones caught as
// exceptions: an element read, written, incremented and updated with a value that itself reads
// out of bounds or divides by zero, a row out of bounds, and a counting loop whose accesses are
// checked once before it (a sliced loop) running past a row's end inside a `try`.
public class ArrayFailuresCaught {
    static int[] totals = new int[3];

    static void add(int i, int d) {
        try {
            totals[i] += 12 / d;
            System.out.println("totals[" + i + "] = " + totals[i]);
        } catch (ArithmeticException | ArrayIndexOutOfBoundsException e) {
            System.out.println("add " + i + " " + d + ": " + e);
        }
    }

    public static void main(String[] args) {
        int n = 4 + args.length;
        int[][] grid = new int[3][n];
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < n; j++) {
                grid[i][j] = i * 10 + j;
            }
        }
        int[] counts = new int[4];
        int[] data = {1, 3, 3, 7, 2};
        for (int d : data) {
            try {
                counts[d]++;
                counts[d] += grid[d][d % 2];
                grid[d][0] = counts[d];
            } catch (ArrayIndexOutOfBoundsException e) {
                System.out.println("skip " + d + ": " + e.getMessage());
            } finally {
                System.out.println("seen " + d);
            }
        }
        int[] row = new int[n];
        for (int r = 0; r < 5; r++) {
            try {
                for (int j = 0; j < n + r; j++) {
                    row[j] += grid[r][j];
                }
            } catch (ArrayIndexOutOfBoundsException e) {
                System.out.println("row " + r + ": " + e.getMessage());
            }
        }
        int total = 0;
        for (int v : row) {
            total += v;
        }
        System.out.println(total + " " + counts[1] + " " + counts[3] + " " + grid[1][0]);
        add(1, 4);
        add(1, 0);
        add(3, 1);
        System.out.println(totals[0] + totals[1] + totals[2]);
    }
}
