public class Knapsack {
    static final int MAX_ITEMS = 12;

    static int bestValue(int[] weight, int[] value, int capacity) {
        int n = weight.length;
        int[][] best = new int[n + 1][capacity + 1];
        for (int i = 1; i <= n; i++) {
            for (int w = 0; w <= capacity; w++) {
                best[i][w] = best[i - 1][w];
                if (weight[i - 1] <= w) {
                    int withItem = best[i - 1][w - weight[i - 1]] + value[i - 1];
                    if (withItem > best[i][w]) {
                        best[i][w] = withItem;
                    }
                }
            }
        }
        return best[n][capacity];
    }

    static boolean[] chosenItems(int[] weight, int[] value, int capacity) {
        int n = weight.length;
        int[][] best = new int[n + 1][capacity + 1];
        for (int i = 1; i <= n; i++) {
            for (int w = 0; w <= capacity; w++) {
                best[i][w] = best[i - 1][w];
                if (weight[i - 1] <= w) {
                    best[i][w] = Math.max(best[i][w], best[i - 1][w - weight[i - 1]] + value[i - 1]);
                }
            }
        }
        boolean[] taken = new boolean[n];
        int w = capacity;
        for (int i = n; i >= 1; i--) {
            if (best[i][w] != best[i - 1][w]) {
                taken[i - 1] = true;
                w -= weight[i - 1];
            }
        }
        return taken;
    }

    /** Unbounded knapsack: any number of copies of each item. */
    static long unboundedValue(int[] weight, int[] value, int capacity) {
        long[] best = new long[capacity + 1];
        for (int w = 1; w <= capacity; w++) {
            best[w] = best[w - 1];
            for (int i = 0; i < weight.length; i++) {
                if (weight[i] <= w && best[w - weight[i]] + value[i] > best[w]) {
                    best[w] = best[w - weight[i]] + value[i];
                }
            }
        }
        return best[capacity];
    }

    /** Counts the subsets whose weights add up to exactly target. */
    static long countSubsets(int[] weight, int target) {
        long[] ways = new long[target + 1];
        ways[0] = 1;
        for (int i = 0; i < weight.length; i++) {
            for (int s = target; s >= weight[i]; s--) {
                ways[s] += ways[s - weight[i]];
            }
        }
        return ways[target];
    }

    public static void main(String[] args) {
        int[] weight = new int[MAX_ITEMS];
        int[] value = new int[MAX_ITEMS];
        long seed = 12345;
        for (int i = 0; i < MAX_ITEMS; i++) {
            seed = (seed * 1103515245L + 12345L) & 0x7fffffffL;
            weight[i] = (int) (seed % 23) + 1;
            seed = (seed * 1103515245L + 12345L) & 0x7fffffffL;
            value[i] = (int) (seed % 90) + 10;
        }
        System.out.print("weights:");
        for (int w : weight) {
            System.out.print(" " + w);
        }
        System.out.println();
        System.out.print("values: ");
        for (int v : value) {
            System.out.print(" " + v);
        }
        System.out.println();

        int[] capacities = {10, 25, 50, 100};
        for (int cap : capacities) {
            int best = bestValue(weight, value, cap);
            boolean[] taken = chosenItems(weight, value, cap);
            int used = 0;
            int total = 0;
            for (int i = 0; i < taken.length; i++) {
                if (taken[i]) {
                    used += weight[i];
                    total += value[i];
                }
            }
            System.out.println("capacity " + cap + ": best " + best + ", reconstructed " + total
                    + " using weight " + used + (total == best ? " ok" : " MISMATCH"));
        }

        System.out.println("unbounded 50: " + unboundedValue(weight, value, 50));
        System.out.println("subsets summing to 30: " + countSubsets(weight, 30));
        System.out.println("subsets summing to 60: " + countSubsets(weight, 60));

        int checksum = 0;
        for (int cap = 0; cap <= 200; cap += 7) {
            checksum = checksum * 31 + bestValue(weight, value, cap);
        }
        System.out.println("checksum " + checksum);
    }
}
