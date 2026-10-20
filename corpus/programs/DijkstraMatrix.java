public class DijkstraMatrix {
    static final int INF = Integer.MAX_VALUE / 2;

    /** Shortest distances from source on an adjacency matrix; INF marks a missing edge. */
    static int[] dijkstra(int[][] graph, int source, int[] previous) {
        int n = graph.length;
        int[] dist = new int[n];
        boolean[] done = new boolean[n];
        for (int i = 0; i < n; i++) {
            dist[i] = INF;
            previous[i] = -1;
        }
        dist[source] = 0;
        for (int round = 0; round < n; round++) {
            int u = -1;
            for (int i = 0; i < n; i++) {
                if (!done[i] && dist[i] < INF && (u == -1 || dist[i] < dist[u])) {
                    u = i;
                }
            }
            if (u == -1) {
                break;
            }
            done[u] = true;
            for (int v = 0; v < n; v++) {
                if (graph[u][v] < INF && dist[u] + graph[u][v] < dist[v]) {
                    dist[v] = dist[u] + graph[u][v];
                    previous[v] = u;
                }
            }
        }
        return dist;
    }

    static String path(int[] previous, int target) {
        if (previous[target] == -1) {
            return "" + target;
        }
        return path(previous, previous[target]) + " -> " + target;
    }

    static int[][] randomGraph(int n, long seed, int density) {
        int[][] g = new int[n][n];
        long s = seed;
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                if (i == j) {
                    g[i][j] = 0;
                    continue;
                }
                s = (s * 48271) % 2147483647;
                if (s % 100 < density) {
                    g[i][j] = (int) (s % 50) + 1;
                } else {
                    g[i][j] = INF;
                }
            }
        }
        return g;
    }

    public static void main(String[] args) {
        int[][] graph = {
            {0, 4, INF, INF, INF, INF, INF, 8, INF},
            {4, 0, 8, INF, INF, INF, INF, 11, INF},
            {INF, 8, 0, 7, INF, 4, INF, INF, 2},
            {INF, INF, 7, 0, 9, 14, INF, INF, INF},
            {INF, INF, INF, 9, 0, 10, INF, INF, INF},
            {INF, INF, 4, 14, 10, 0, 2, INF, INF},
            {INF, INF, INF, INF, INF, 2, 0, 1, 6},
            {8, 11, INF, INF, INF, INF, 1, 0, 7},
            {INF, INF, 2, INF, INF, INF, 6, 7, 0},
        };
        int[] previous = new int[graph.length];
        int[] dist = dijkstra(graph, 0, previous);
        for (int v = 0; v < dist.length; v++) {
            System.out.println("0 to " + v + ": " + dist[v] + " via " + path(previous, v));
        }

        int[][] big = randomGraph(120, 42, 30);
        long total = 0;
        int reachable = 0;
        int farthest = 0;
        for (int s = 0; s < big.length; s++) {
            int[] d = dijkstra(big, s, new int[big.length]);
            for (int v = 0; v < d.length; v++) {
                if (d[v] < INF) {
                    total += d[v];
                    reachable++;
                    farthest = Math.max(farthest, d[v]);
                }
            }
        }
        System.out.println("all pairs on 120 nodes: reachable " + reachable + ", total " + total
                + ", farthest " + farthest);
    }
}
