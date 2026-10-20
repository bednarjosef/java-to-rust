// Loops whose conditions keep indexes in bounds, in the shapes a sieve, a scan and a matrix use.
public class ArrayBoundsGuards {
    static int primes(int limit) {
        boolean[] composite = new boolean[limit + 1];
        for (int i = 2; i * i <= limit; i++) {
            if (!composite[i]) {
                for (int j = i * i; j <= limit; j += i) {
                    composite[j] = true;
                }
            }
        }
        int count = 0;
        for (int i = 2; i <= limit; i++) {
            if (!composite[i]) {
                count++;
            }
        }
        return count;
    }

    static int firstZero(int[] a) {
        int i = 0;
        while (i < a.length && a[i] != 0) {
            i++;
        }
        return i;
    }

    static int pairs(int[] a) {
        int count = 0;
        for (int i = 0; i < a.length - 1; i++) {
            if (a[i] < a[i + 1]) {
                count++;
            }
        }
        return count;
    }

    static int trace(int n) {
        int[][] m = new int[n][n + 1];
        int t = 0;
        for (int i = 0; i < n; i++) {
            m[i][i + 1] = i;
            t += m[i][i] + m[i][i + 1];
        }
        return t;
    }

    static int below(int[] a, int k) {
        return k < a.length ? a[k] : -1;
    }

    public static void main(String[] args) {
        System.out.println(primes(100) + " " + primes(1000));
        System.out.println(firstZero(new int[] {3, 1, 0, 2}) + " " + firstZero(new int[] {1, 2}));
        System.out.println(pairs(new int[] {1, 2, 2, 3, 1, 5}));
        System.out.println(trace(5));
        System.out.println(below(new int[] {4, 5}, 1) + " " + below(new int[] {4, 5}, 2));
        int[] small = new int[3];
        int i = 0;
        while (i < small.length) {
            small[i] = i * 3;
            i++;
        }
        System.out.println(small[2]);
    }
}
