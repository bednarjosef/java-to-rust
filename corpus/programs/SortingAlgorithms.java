import java.util.Arrays;

public class SortingAlgorithms {
    static void insertionSort(int[] a) {
        for (int i = 1; i < a.length; i++) {
            int key = a[i];
            int j = i - 1;
            while (j >= 0 && a[j] > key) {
                a[j + 1] = a[j];
                j--;
            }
            a[j + 1] = key;
        }
    }

    static void mergeSort(int[] a, int[] scratch, int lo, int hi) {
        if (hi - lo < 2) {
            return;
        }
        int mid = (lo + hi) >>> 1;
        mergeSort(a, scratch, lo, mid);
        mergeSort(a, scratch, mid, hi);
        int i = lo;
        int j = mid;
        int k = lo;
        while (i < mid && j < hi) {
            scratch[k++] = a[i] <= a[j] ? a[i++] : a[j++];
        }
        while (i < mid) {
            scratch[k++] = a[i++];
        }
        while (j < hi) {
            scratch[k++] = a[j++];
        }
        for (k = lo; k < hi; k++) {
            a[k] = scratch[k];
        }
    }

    static void quickSort(int[] a, int lo, int hi) {
        if (lo >= hi) {
            return;
        }
        int pivot = a[(lo + hi) / 2];
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
        quickSort(a, lo, j);
        quickSort(a, i, hi);
    }

    static void siftDown(int[] a, int start, int end) {
        int root = start;
        while (2 * root + 1 <= end) {
            int child = 2 * root + 1;
            if (child + 1 <= end && a[child] < a[child + 1]) {
                child++;
            }
            if (a[root] >= a[child]) {
                return;
            }
            int t = a[root];
            a[root] = a[child];
            a[child] = t;
            root = child;
        }
    }

    static void heapSort(int[] a) {
        for (int start = a.length / 2 - 1; start >= 0; start--) {
            siftDown(a, start, a.length - 1);
        }
        for (int end = a.length - 1; end > 0; end--) {
            int t = a[0];
            a[0] = a[end];
            a[end] = t;
            siftDown(a, 0, end - 1);
        }
    }

    /** Counting sort for values in [0, maxValue]. */
    static int[] countingSort(int[] a, int maxValue) {
        int[] count = new int[maxValue + 1];
        for (int v : a) {
            count[v]++;
        }
        int[] out = new int[a.length];
        int k = 0;
        for (int v = 0; v <= maxValue; v++) {
            for (int c = 0; c < count[v]; c++) {
                out[k++] = v;
            }
        }
        return out;
    }

    /** LSD radix sort on non-negative ints, base 256. */
    static void radixSort(int[] a) {
        int[] buffer = new int[a.length];
        for (int shift = 0; shift < 32; shift += 8) {
            int[] count = new int[257];
            for (int v : a) {
                count[((v >>> shift) & 255) + 1]++;
            }
            for (int b = 0; b < 256; b++) {
                count[b + 1] += count[b];
            }
            for (int v : a) {
                buffer[count[(v >>> shift) & 255]++] = v;
            }
            for (int i = 0; i < a.length; i++) {
                a[i] = buffer[i];
            }
        }
    }

    static boolean isSorted(int[] a) {
        for (int i = 1; i < a.length; i++) {
            if (a[i - 1] > a[i]) {
                return false;
            }
        }
        return true;
    }

    static int[] copyOf(int[] a) {
        int[] out = new int[a.length];
        for (int i = 0; i < a.length; i++) {
            out[i] = a[i];
        }
        return out;
    }

    static boolean same(int[] a, int[] b) {
        if (a.length != b.length) {
            return false;
        }
        for (int i = 0; i < a.length; i++) {
            if (a[i] != b[i]) {
                return false;
            }
        }
        return true;
    }

    static String show(int[] a) {
        String out = "[";
        for (int i = 0; i < a.length; i++) {
            out = out + (i == 0 ? "" : ", ") + a[i];
        }
        return out + "]";
    }

    static long checksum(int[] a) {
        long h = 0;
        for (int i = 0; i < a.length; i++) {
            h = h * 1000003 + a[i];
        }
        return h;
    }

    static int[] generate(int n, long seed, int bound) {
        int[] a = new int[n];
        long s = seed;
        for (int i = 0; i < n; i++) {
            s = (s * 6364136223846793005L + 1442695040888963407L);
            a[i] = (int) ((s >>> 33) % bound);
        }
        return a;
    }

    public static void main(String[] args) {
        int[] small = {38, 27, 43, 3, 9, 82, 10, 3, -5, 0};
        int[] copy = copyOf(small);
        insertionSort(copy);
        System.out.println("insertion: " + isSorted(copy) + " first " + copy[0] + " last " + copy[copy.length - 1]);

        int[] sizes = {10, 1000, 100000};
        for (int n : sizes) {
            int[] base = generate(n, n, 1000000);
            int[] expected = copyOf(base);
            Arrays.sort(expected);

            int[] merged = copyOf(base);
            mergeSort(merged, new int[n], 0, n);
            int[] quick = copyOf(base);
            quickSort(quick, 0, n - 1);
            int[] heap = copyOf(base);
            heapSort(heap);
            int[] radix = copyOf(base);
            radixSort(radix);
            int[] counted = countingSort(base, 999999);

            System.out.println("n=" + n + " merge " + same(merged, expected)
                    + " quick " + same(quick, expected)
                    + " heap " + same(heap, expected)
                    + " radix " + same(radix, expected)
                    + " counting " + same(counted, expected));
            System.out.println("  checksum " + checksum(expected) + " median " + expected[n / 2]);
        }

        int[] data = generate(12, 5, 100);
        System.out.println("before: " + show(data));
        heapSort(data);
        System.out.println("after:  " + show(data));
    }
}
