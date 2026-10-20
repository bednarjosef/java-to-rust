// Sorts arrays of a million pseudo-random ints, once with a hand-written quicksort and once with
// java.util.Arrays.sort, and checks that both give the same sorted array.
public class SortMillion {
    static int[] randomInts(int n, int seed) {
        int[] a = new int[n];
        int x = seed;
        for (int i = 0; i < n; i++) {
            x ^= x << 13;
            x ^= x >>> 17;
            x ^= x << 5;
            a[i] = x;
        }
        return a;
    }

    static void quickSort(int[] a, int lo, int hi) {
        while (lo < hi) {
            int pivot = a[(lo + hi) >>> 1];
            int i = lo;
            int j = hi;
            while (i <= j) {
                while (a[i] < pivot) {
                    i++;
                }
                while (a[j] > pivot) {
                    j--;
                }
                if (i <= j) {
                    int t = a[i];
                    a[i] = a[j];
                    a[j] = t;
                    i++;
                    j--;
                }
            }
            if (j - lo < hi - i) {
                quickSort(a, lo, j);
                lo = i;
            } else {
                quickSort(a, i, hi);
                hi = j;
            }
        }
    }

    static long checksum(int[] a) {
        long sum = 0;
        for (int i = 0; i < a.length; i += 997) {
            sum = sum * 31 + a[i];
        }
        return sum;
    }

    static long run(int n, int rounds) {
        long total = 0;
        for (int round = 0; round < rounds; round++) {
            int[] mine = randomInts(n, 42 + round);
            int[] library = new int[n];
            System.arraycopy(mine, 0, library, 0, n);
            quickSort(mine, 0, n - 1);
            java.util.Arrays.sort(library);
            int differences = 0;
            for (int i = 0; i < n; i++) {
                if (mine[i] != library[i]) {
                    differences++;
                }
            }
            total = total * 31 + checksum(mine) + differences;
        }
        return total;
    }

    public static void main(String[] args) {
        int n = 1000000;
        int rounds = 8;
        for (int warm = 0; warm < 2; warm++) {
            run(n, rounds);
        }
        long start = System.nanoTime();
        long total = run(n, rounds);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("sorted " + rounds + " arrays of " + n + ", checksum " + total);
        System.err.println("kernel-ms " + elapsed);
    }
}
