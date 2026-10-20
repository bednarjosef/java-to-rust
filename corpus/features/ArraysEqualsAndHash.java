import java.util.Arrays;

public class ArraysEqualsAndHash {
    public static void main(String[] args) {
        int[] a = { 1, 2, 3 };
        int[] b = { 1, 2, 3 };
        int[] c = { 3, 2, 1 };
        int[] none = null;
        System.out.println(Arrays.equals(a, b) + " " + Arrays.equals(a, c));
        System.out.println(Arrays.equals(a, none) + " " + Arrays.equals(a, null));
        System.out.println(Arrays.hashCode(a) + " " + Arrays.hashCode(c) + " " + Arrays.hashCode(none));

        long[] longs = { 1L << 40, -7L };
        System.out.println(Arrays.equals(longs, new long[] { 1L << 40, -7L }));
        System.out.println(Arrays.hashCode(longs));

        double[] d = { 0.0, Double.NaN };
        System.out.println(Arrays.equals(d, new double[] { -0.0, Double.NaN }));
        System.out.println(Arrays.hashCode(d));

        char[] chars = { 'x', 'y' };
        System.out.println(Arrays.equals(chars, new char[] { 'x', 'y' }) + " " + Arrays.hashCode(chars));

        boolean[] flags = { true, false };
        System.out.println(Arrays.hashCode(flags) + " " + Arrays.equals(flags, new boolean[] { true, false }));

        byte[] bytes = { -1, 5 };
        short[] shorts = { 300, -300 };
        float[] floats = { 1.5f, Float.NaN };
        System.out.println(Arrays.hashCode(bytes) + " " + Arrays.hashCode(shorts) + " " + Arrays.hashCode(floats));

        String[] words = { "a", "bc" };
        String[] withNull = { "a", null };
        System.out.println(Arrays.equals(words, new String[] { "a", "bc" }) + " " + Arrays.hashCode(words));
        System.out.println(Arrays.equals(withNull, new String[] { "a", null }) + " " + Arrays.hashCode(withNull));
        System.out.println(Arrays.equals(withNull, words));
    }
}
