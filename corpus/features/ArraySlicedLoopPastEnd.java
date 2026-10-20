// A sliced loop whose bound is past the end of its row runs with its checks: it prints every
// element in range before the access past the end throws.
public class ArraySlicedLoopPastEnd {
    static void show(int[][] g, int r, int n) {
        for (int j = 0; j < n; j++) {
            System.out.println("g[" + r + "][" + j + "] = " + g[r][j]);
        }
    }

    public static void main(String[] args) {
        int[][] g = {{1, 2, 3}, {4, 5, 6}};
        show(g, 0, 3);
        show(g, 1, 0);
        show(g, 2, 0);
        show(g, 1, 5);
    }
}
