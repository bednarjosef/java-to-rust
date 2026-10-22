import java.util.Arrays;

// Arrays.binarySearch over strings where an array may hold null, as the JDK's ZoneInfoFile's
// regions do (filled after they are made), with the NullPointerException a catch reads: of a
// null element compared, and of a null key.
public class BinarySearchNullElements {
    static String[] regions(int n) {
        String[] all = new String[n];
        for (int i = 0; i < n; i++) {
            all[i] = (char) ('a' + i) + "x";
        }
        return all;
    }

    static String key(int i) {
        return i < 0 ? null : (char) ('a' + i) + "x";
    }

    static String search(String[] a, int k) {
        try {
            return "found " + Arrays.binarySearch(a, key(k));
        } catch (NullPointerException e) {
            return "failed: " + e.getMessage();
        }
    }

    static String searchRange(String[] a, int from, int to, int k) {
        try {
            return "found " + Arrays.binarySearch(a, from, to, key(k));
        } catch (NullPointerException e) {
            return "failed: " + e.getMessage();
        } catch (IllegalArgumentException | ArrayIndexOutOfBoundsException e) {
            return "out of range: " + e.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        String[] full = regions(4);
        System.out.println(search(full, 2));
        System.out.println(search(full, 7));
        System.out.println(search(full, -1));
        System.out.println(search(regions(0), -1));
        System.out.println(search(null, 1));
        String[] holes = regions(5);
        holes[2] = null;
        System.out.println(search(holes, 0));
        System.out.println(search(holes, 4));
        System.out.println(searchRange(holes, 3, 5, 4));
        System.out.println(searchRange(holes, 0, 2, -1));
        System.out.println(searchRange(holes, 1, 1, -1));
        System.out.println(searchRange(holes, 0, 9, 1));
    }
}
