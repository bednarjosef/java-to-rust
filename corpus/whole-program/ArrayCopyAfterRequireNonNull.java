import java.util.Arrays;
import java.util.Objects;

public class ArrayCopyAfterRequireNonNull {
    // `Objects.requireNonNull` is the JDK's own method, translated from its source, which
    // throws the exception that the copy would have thrown.
    static void copy(byte[] b, int off, int len, byte[] dst) {
        Objects.requireNonNull(b);
        System.arraycopy(b, off, dst, 0, len);
    }

    static char[] copyChars(char[] src, char[] dst) {
        Objects.requireNonNull(src, "no source");
        System.arraycopy(src, 1, dst, 0, src.length - 1);
        return dst;
    }

    static int copyThenSum(int[] src, int[] dst) {
        Objects.requireNonNull(src);
        System.arraycopy(src, 0, dst, 0, src.length);
        int s = 0;
        for (int i = 0; i < src.length; i++) {
            s += src[i] * dst[i];
        }
        return s;
    }

    public static void main(String[] args) {
        byte[] src = {1, 2, 3, 4, 5};
        byte[] dst = new byte[4];
        copy(src, 1, 3, dst);
        System.out.println(Arrays.toString(dst));
        System.out.println(new String(copyChars("hello".toCharArray(), new char[5])));
        int[] left = {1, 2, 3};
        System.out.println(copyThenSum(left, new int[3]));

        try {
            copy(null, 0, 2, dst);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getClass().getName());
        }
        try {
            copyChars(null, new char[2]);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            copyThenSum(null, new int[1]);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getClass().getName());
        }
        copy(null, 0, 1, dst);
        System.out.println("unreachable");
    }
}
