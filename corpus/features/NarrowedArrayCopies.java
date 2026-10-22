import java.util.Arrays;

// An array that may be null, tested by an early exit and then copied: the rest of the method
// sees the array itself.
public class NarrowedArrayCopies {
    static byte[] trimmed(byte[] result, int total) {
        if (result == null) {
            return new byte[0];
        }
        return result.length == total ? result : Arrays.copyOf(result, total);
    }

    public static void main(String[] args) {
        byte[] bytes = {1, 2, 3, 4};
        System.out.println(Arrays.toString(trimmed(bytes, 2)) + " " + trimmed(bytes, 4).length);
        System.out.println(trimmed(null, 3).length);
        try {
            trimmed(bytes, -1);
        } catch (NegativeArraySizeException e) {
            System.out.println("negative: " + e.getMessage());
        }
    }
}
