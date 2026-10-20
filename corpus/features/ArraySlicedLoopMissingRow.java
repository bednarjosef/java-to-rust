// A sliced loop over a row that does not exist throws at its first access, not before the loop:
// the loop prints a line before each access.
public class ArraySlicedLoopMissingRow {
    static int sum(int[][] g, int r, int n) {
        int s = 0;
        for (int j = 0; j < n; j++) {
            System.out.println("reading " + j);
            s += g[r][j];
        }
        return s;
    }

    public static void main(String[] args) {
        int[][] g = new int[2][3];
        g[1][2] = 7;
        System.out.println(sum(g, 1, 3));
        System.out.println(sum(g, 2, 3));
    }
}
