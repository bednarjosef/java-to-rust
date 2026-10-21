import java.util.Arrays;

// Failures that have no checked form of their own, seen by a catch or a finally: a negative array
// size, a negative length for Arrays.copyOf, and a System.arraycopy out of bounds.
public class ArrayCreationFailuresCaught {
    static int[] make(int n) {
        try {
            return new int[n];
        } catch (NegativeArraySizeException e) {
            System.out.println("negative size " + e.getMessage());
            return new int[0];
        }
    }

    static String[] grown(String[] names, int length) {
        try {
            return Arrays.copyOf(names, length);
        } catch (RuntimeException e) {
            System.out.println("cannot copy: " + e.getClass().getName());
            return names;
        }
    }

    static void copy(int[] from, int[] to, int count) {
        try {
            System.arraycopy(from, 0, to, 1, count);
        } finally {
            System.out.println("copied " + count + ": " + Arrays.toString(to));
        }
    }

    public static void main(String[] args) {
        System.out.println(make(3).length + " " + make(-2).length);
        System.out.println(grown(new String[] {"a", "b"}, 3).length + " " + grown(new String[] {"c"}, -1).length);
        int[] to = new int[4];
        copy(new int[] {1, 2, 3}, to, 2);
        try {
            copy(new int[] {4, 5, 6}, to, 3);
            copy(new int[] {7, 8, 9}, to, 4);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("out of bounds");
        }
    }
}
