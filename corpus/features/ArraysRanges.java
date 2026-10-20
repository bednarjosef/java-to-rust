import java.util.Arrays;

public class ArraysRanges {
    public static void main(String[] args) {
        int[] a = { 9, 8, 7, 6, 5, 4 };
        Arrays.sort(a, 1, 4);
        System.out.println(Arrays.toString(a));

        String[] words = { "d", "c", "b", "a" };
        Arrays.sort(words, 0, 2);
        System.out.println(Arrays.toString(words));

        double[] ds = { 3.0, 2.0, 1.0 };
        Arrays.sort(ds, 1, 3);
        System.out.println(Arrays.toString(ds));

        Arrays.fill(a, 2, 5, -1);
        System.out.println(Arrays.toString(a));

        char[] cs = new char[4];
        Arrays.fill(cs, 'x');
        Arrays.fill(cs, 1, 3, 'y');
        System.out.println(Arrays.toString(cs));

        int[] src = { 1, 2, 3, 4, 5 };
        System.out.println(Arrays.toString(Arrays.copyOfRange(src, 1, 4)));
        System.out.println(Arrays.toString(Arrays.copyOfRange(src, 3, 7)));
        System.out.println(Arrays.toString(Arrays.copyOfRange(src, 2, 2)));

        int[] sorted = { 1, 3, 5, 7, 9 };
        System.out.println(Arrays.binarySearch(sorted, 7) + " " + Arrays.binarySearch(sorted, 4));
        System.out.println(Arrays.binarySearch(sorted, 0) + " " + Arrays.binarySearch(sorted, 10));
        System.out.println(Arrays.binarySearch(sorted, 1, 4, 5) + " " + Arrays.binarySearch(sorted, 0, 2, 9));

        long[] longs = { 10L, 20L, 30L };
        System.out.println(Arrays.binarySearch(longs, 20));

        String[] names = { "ann", "bob", "cy" };
        System.out.println(Arrays.binarySearch(names, "bob") + " " + Arrays.binarySearch(names, "zed"));

        double[] sortedDoubles = { 1.0, 2.0 };
        System.out.println(Arrays.binarySearch(sortedDoubles, 1.5));

        Arrays.sort(a, 4, 2);
        System.out.println("not reached");
    }
}
