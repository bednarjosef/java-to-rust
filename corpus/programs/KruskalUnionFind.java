public class KruskalUnionFind {
    static int[] parent;
    static int[] rank;
    static int components;

    static void reset(int n) {
        parent = new int[n];
        rank = new int[n];
        components = n;
        for (int i = 0; i < n; i++) {
            parent[i] = i;
        }
    }

    static int find(int x) {
        while (parent[x] != x) {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        return x;
    }

    static boolean union(int a, int b) {
        int ra = find(a);
        int rb = find(b);
        if (ra == rb) {
            return false;
        }
        if (rank[ra] < rank[rb]) {
            int t = ra;
            ra = rb;
            rb = t;
        }
        parent[rb] = ra;
        if (rank[ra] == rank[rb]) {
            rank[ra]++;
        }
        components--;
        return true;
    }

    /** Sorts edges as rows {weight, u, v} by weight, then returns the total weight of a spanning forest. */
    static long kruskal(int[][] edges, int n, int[][] chosenOut) {
        for (int i = 1; i < edges.length; i++) {
            int[] key = edges[i];
            int j = i - 1;
            while (j >= 0 && edges[j][0] > key[0]) {
                edges[j + 1] = edges[j];
                j--;
            }
            edges[j + 1] = key;
        }
        reset(n);
        long total = 0;
        int count = 0;
        for (int[] e : edges) {
            if (union(e[1], e[2])) {
                total += e[0];
                if (count < chosenOut.length) {
                    chosenOut[count] = e;
                }
                count++;
            }
        }
        return total;
    }

    public static void main(String[] args) {
        int n = 9;
        int[][] edges = {
            {4, 0, 1}, {8, 0, 7}, {11, 1, 7}, {8, 1, 2}, {7, 7, 8}, {1, 7, 6},
            {2, 2, 8}, {6, 8, 6}, {4, 2, 5}, {2, 6, 5}, {7, 2, 3}, {14, 3, 5},
            {9, 3, 4}, {10, 4, 5},
        };
        int[][] chosen = new int[n - 1][];
        long total = kruskal(edges, n, chosen);
        System.out.println("mst weight " + total + " from " + n + " nodes");
        for (int[] e : chosen) {
            System.out.println("  edge " + e[1] + "-" + e[2] + " weight " + e[0]);
        }

        // Connectivity queries on a sparse random graph, counting components as edges arrive.
        int size = 2000;
        reset(size);
        long s = 99;
        int merges = 0;
        for (int i = 0; i < 1500; i++) {
            s = (s * 6364136223846793005L + 1442695040888963407L);
            int a = (int) ((s >>> 20) % size);
            s = (s * 6364136223846793005L + 1442695040888963407L);
            int b = (int) ((s >>> 20) % size);
            if (union(a, b)) {
                merges++;
            }
        }
        System.out.println("random: merges " + merges + ", components " + components);

        int[] sizeOf = new int[size];
        for (int i = 0; i < size; i++) {
            sizeOf[find(i)]++;
        }
        int largest = 0;
        for (int root = 0; root < size; root++) {
            largest = Math.max(largest, sizeOf[root]);
        }
        System.out.println("largest component " + largest);
        System.out.println("0 and 1 connected: " + (find(0) == find(1)));
    }
}
