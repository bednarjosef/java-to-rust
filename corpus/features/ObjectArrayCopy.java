// `System.arraycopy` between arrays held as `Object`s, as the JDK's code that is generic over the
// kind of array writes it (`SpinedBuffer.OfPrimitive`'s `T_ARR`, an `int[]` or a `long[]`).
public class ObjectArrayCopy {
    static Object copy(Object src, int from, Object dest, int to, int n) {
        System.arraycopy(src, from, dest, to, n);
        return dest;
    }

    static <A> A grow(A array, A bigger, int n) {
        System.arraycopy(array, 0, bigger, 0, n);
        return bigger;
    }

    static void into(int[] src, Object dest) {
        System.arraycopy(src, 0, dest, 0, src.length);
    }

    static String show(Object array) {
        StringBuilder sb = new StringBuilder("[");
        if (array instanceof int[] ints) {
            for (int i = 0; i < ints.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(ints[i]);
            }
        } else if (array instanceof long[] longs) {
            for (int i = 0; i < longs.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(longs[i]);
            }
        }
        return sb.append("]").toString();
    }

    public static void main(String[] args) {
        int[] a = {1, 2, 3, 4};
        Object b = new int[6];
        System.out.println(show(copy(a, 1, b, 2, 3)));
        long[] l = {10L, 20L, 30L};
        System.out.println(show(copy(l, 0, new long[4], 1, 3)));
        // Within one array the ranges may overlap.
        Object same = a;
        System.out.println(show(copy(same, 0, same, 1, 3)));
        System.out.println(show(grow(new int[] {7, 8}, new int[3], 2)));
        Object c = new int[2];
        into(new int[] {5, 6}, c);
        System.out.println(show(c));
    }
}
