import java.util.Arrays;
import java.util.Objects;

public class ArrayCopyAfterNullCheck {
    // The parameter is checked for null, then passed to System.arraycopy.
    static void copy(byte[] b, int off, int len, byte[] dst) {
        if (b == null) throw new NullPointerException();
        System.arraycopy(b, off, dst, 0, len);
    }

    // The same check through Objects, as a call that the translation reads as `b == null`.
    static void copyRequired(byte[] b, int off, int len, byte[] dst) {
        if (Objects.isNull(b)) throw new NullPointerException("no source");
        System.arraycopy(b, off, dst, 0, len);
    }

    // The check is on the destination, and the source is only read.
    static int[] copyInts(int[] src, int[] dst) {
        if (dst == null) throw new IllegalArgumentException("no destination");
        System.arraycopy(src, 0, dst, 0, src.length);
        return dst;
    }

    static char[] copyChars(char[] src, int from, char[] dst) {
        if (src == null) {
            throw new NullPointerException("no source");
        }
        System.arraycopy(src, from, dst, 0, src.length - from);
        return dst;
    }

    // The parameter is used again after the copy, so it must stay usable.
    static int copyThenSum(int[] src, int[] dst) {
        if (src == null) throw new NullPointerException();
        System.arraycopy(src, 0, dst, 0, src.length);
        int s = 0;
        for (int i = 0; i < src.length; i++) {
            s += src[i] + dst[i];
        }
        return s;
    }

    // Read after the copy only, so the destination may be borrowed as a slice.
    static int copyFirst(byte[] b, byte[] dst) {
        if (b == null) throw new NullPointerException();
        System.arraycopy(b, 0, dst, 0, 1);
        return dst[0];
    }

    // The checks are conditions, not throws.
    static void copyIfPresent(long[] a, long[] dst) {
        if (a != null && dst != null) System.arraycopy(a, 0, dst, 0, a.length);
    }

    // Both parameters are checked, then copied in place.
    static void shiftInPlace(char[] a, int by) {
        if (a == null) throw new NullPointerException();
        if (by < 0 || by >= a.length) return;
        System.arraycopy(a, 0, a, by, a.length - by);
    }

    static String describe(Throwable t) {
        return t.getClass().getName();
    }

    public static void main(String[] args) {
        byte[] src = {1, 2, 3, 4, 5};
        byte[] dst = new byte[4];
        copy(src, 1, 3, dst);
        System.out.println(Arrays.toString(dst));
        copyRequired(src, 0, 4, dst);
        System.out.println(Arrays.toString(dst));

        try {
            copy(null, 0, 2, dst);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + describe(e));
        }
        try {
            copyRequired(null, 0, 2, dst);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + describe(e));
        }
        System.out.println(Arrays.toString(dst));

        int[] from = {10, 20, 30};
        int[] to = new int[5];
        System.out.println(Arrays.toString(copyInts(from, to)));
        try {
            copyInts(from, null);
            System.out.println("no exception");
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage());
        }

        char[] word = "abcdef".toCharArray();
        char[] tail = copyChars(word, 2, new char[6]);
        System.out.println(new String(tail, 0, 4));
        try {
            copyChars(null, 0, new char[2]);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }

        int[] left = {1, 2, 3};
        int[] right = new int[3];
        System.out.println(copyThenSum(left, right));
        System.out.println(Arrays.toString(right));
        try {
            copyThenSum(null, right);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + describe(e));
        }

        System.out.println(copyFirst(src, new byte[2]));
        long[] longs = new long[3];
        copyIfPresent(new long[] {4L, 5L}, longs);
        copyIfPresent(null, longs);
        System.out.println(Arrays.toString(longs));

        char[] shifted = {'x', 'y', 'z', 'w'};
        shiftInPlace(shifted, 1);
        System.out.println(new String(shifted));
        shiftInPlace(shifted, 9);
        System.out.println(new String(shifted));
        try {
            shiftInPlace(null, 1);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println("caught " + describe(e));
        }

        // An uncaught NullPointerException ends the program with exit code 1.
        copy(null, 0, 1, dst);
        System.out.println("unreachable");
    }
}
