// A sliced loop whose start is negative runs with its checks and throws at the first access.
public class ArraySlicedLoopNegativeStart {
    static long dot(long[] a, long[] b, int start, int end) {
        long s = 0;
        for (int j = start; j < end; j++) {
            s += a[j] * b[j];
        }
        return s;
    }

    public static void main(String[] args) {
        long[] a = {1, 2, 3, 4, 5};
        long[] b = {5, 4, 3, 2, 1};
        System.out.println("dot " + dot(a, b, 1, 5));
        System.out.println("dot " + dot(a, b, -1, 3));
    }
}
