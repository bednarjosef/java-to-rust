import java.util.Arrays;

public class ArraysSortAllTypes {
    public static void main(String[] args) {
        long[] longs = { 5L, -3000000000L, 0L, 7L };
        Arrays.sort(longs);
        System.out.println(Arrays.toString(longs));

        char[] chars = { 'z', 'A', 'm' };
        Arrays.sort(chars);
        System.out.println(Arrays.toString(chars));

        byte[] bytes = { 3, -128, 127, 0 };
        Arrays.sort(bytes);
        System.out.println(Arrays.toString(bytes));

        short[] shorts = { 300, -2, 12 };
        Arrays.sort(shorts);
        System.out.println(Arrays.toString(shorts));

        double[] doubles = { 2.5, Double.NaN, -0.0, 0.0, -1.0, Double.NEGATIVE_INFINITY };
        Arrays.sort(doubles);
        System.out.println(Arrays.toString(doubles));

        float[] floats = { 1.5f, -0.0f, Float.NaN, 0.0f, -2.25f };
        Arrays.sort(floats);
        System.out.println(Arrays.toString(floats));

        String[] words = { "pear", "Apple", "apple", "fig", "", "apples" };
        Arrays.sort(words);
        System.out.println(Arrays.toString(words));

        String[] wide = { "～", "😀" };
        Arrays.sort(wide);
        System.out.println(wide[0].equals("😀"));

        String[] names = { "b", "a" };
        Arrays.sort(names);
        System.out.println(names[0] + names[1]);

        String[] nulls = { "x", null };
        Arrays.sort(nulls);
        System.out.println("not reached");
    }
}
