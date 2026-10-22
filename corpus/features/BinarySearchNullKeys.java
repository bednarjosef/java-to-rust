import java.util.Arrays;

// Arrays.binarySearch with a key or an array that may be null, whose NullPointerException a catch
// reads, as the JDK's ZoneInfoFile does: a null key fails only where it is compared with an
// element, in String.compareTo.
public class BinarySearchNullKeys {
    static String[] names(int n) {
        if (n < 0) {
            return null;
        }
        if (n == 0) {
            return new String[] {};
        }
        return new String[] {"ax", "bx", "cx", "dx"};
    }

    static String key(int i) {
        return i < 0 ? null : (char) ('a' + i) + "x";
    }

    static String search(int n, int k) {
        try {
            return "found " + Arrays.binarySearch(names(n), key(k));
        } catch (NullPointerException e) {
            return "failed: " + e.getMessage();
        }
    }

    static String searchRange(int n, int from, int to, int k) {
        try {
            return "found " + Arrays.binarySearch(names(n), from, to, key(k));
        } catch (NullPointerException e) {
            return "failed: " + e.getMessage();
        } catch (RuntimeException e) {
            return "out of range: " + e.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        System.out.println(search(4, 3));
        System.out.println(search(4, 2));
        System.out.println(search(4, -1));
        System.out.println(search(0, -1));
        System.out.println(search(-1, 3));
        System.out.println(searchRange(4, 1, 3, 3));
        System.out.println(searchRange(4, 2, 2, -1));
        System.out.println(searchRange(4, 1, 3, -1));
        System.out.println(searchRange(4, 3, 1, -1));
        System.out.println(searchRange(-1, 0, 0, 3));
    }
}
