import java.util.Arrays;

public class FloydWarshall {
    static final int INF = 1_000_000;

    static void relax(int[][] dist, int[][] next) {
        int n = dist.length;
        for (int k = 0; k < n; k++) {
            for (int i = 0; i < n; i++) {
                for (int j = 0; j < n; j++) {
                    if (dist[i][k] + dist[k][j] < dist[i][j]) {
                        dist[i][j] = dist[i][k] + dist[k][j];
                        next[i][j] = next[i][k];
                    }
                }
            }
        }
    }

    static String route(int[][] next, int from, int to) {
        if (next[from][to] == -1) {
            return "none";
        }
        StringBuilderLike sb = new StringBuilderLike();
        sb.add(from);
        int at = from;
        while (at != to) {
            at = next[at][to];
            sb.add(at);
        }
        return sb.text();
    }

    static boolean hasNegativeCycle(int[][] dist) {
        for (int i = 0; i < dist.length; i++) {
            if (dist[i][i] < 0) {
                return true;
            }
        }
        return false;
    }

    static int diameter(int[][] dist) {
        int best = 0;
        for (int[] row : dist) {
            for (int d : row) {
                if (d < INF && d > best) {
                    best = d;
                }
            }
        }
        return best;
    }

    public static void main(String[] args) {
        int n = 5;
        int[][] dist = new int[n][n];
        int[][] next = new int[n][n];
        for (int i = 0; i < n; i++) {
            Arrays.fill(dist[i], INF);
            Arrays.fill(next[i], -1);
            dist[i][i] = 0;
        }
        int[][] edges = {{0, 1, 3}, {1, 2, -2}, {0, 2, 8}, {2, 3, 4}, {3, 1, 1}, {3, 4, 2}, {4, 0, -1}};
        for (int[] e : edges) {
            dist[e[0]][e[1]] = e[2];
            next[e[0]][e[1]] = e[1];
        }
        relax(dist, next);
        System.out.println("negative cycle: " + hasNegativeCycle(dist));
        for (int i = 0; i < n; i++) {
            StringBuilderLike row = new StringBuilderLike();
            for (int j = 0; j < n; j++) {
                row.addPadded(dist[i][j] >= INF ? "-" : "" + dist[i][j], 4);
            }
            System.out.println(row.text());
        }
        System.out.println("route 0 to 4: " + route(next, 0, 4));
        System.out.println("route 4 to 2: " + route(next, 4, 2));

        int size = 60;
        int[][] grid = new int[size][size];
        long s = 17;
        for (int i = 0; i < size; i++) {
            for (int j = 0; j < size; j++) {
                s = (s * 1103515245 + 12345) & 0x7fffffff;
                grid[i][j] = (i == j) ? 0 : (s % 4 == 0 ? INF : (int) (s % 97) + 1);
            }
        }
        int[][] nextBig = new int[size][size];
        for (int i = 0; i < size; i++) {
            for (int j = 0; j < size; j++) {
                nextBig[i][j] = grid[i][j] < INF ? j : -1;
            }
        }
        relax(grid, nextBig);
        System.out.println("60-node diameter: " + diameter(grid));
        System.out.println("corner distance: " + grid[0][size - 1] + " " + grid[size - 1][0]);
    }

    /** Accumulates numbers and short texts without a library builder. */
    static class StringBuilderLike {
        private String out = "";

        void add(int v) {
            out = out + (out.length() == 0 ? "" : " ") + v;
        }

        void addPadded(String v, int width) {
            String pad = "";
            for (int i = v.length(); i < width; i++) {
                pad = pad + " ";
            }
            out = out + pad + v;
        }

        String text() {
            return out;
        }
    }
}
