import java.io.UnsupportedEncodingException;

// Failures of library calls that the runtime implements, caught by their exact class: a
// builder's index out of range, a range to append outside the characters, an unsigned division
// by zero, an invalid code point, an unknown charset's name and a range of bytes outside the
// array. They are caught where they happen, by a superclass, and seen by a `finally` on their way
// to a caller.
public class LibraryFailuresCaught {
    static String removeAt(String text, int i) {
        StringBuilder b = new StringBuilder(text);
        try {
            b.deleteCharAt(i);
        } catch (StringIndexOutOfBoundsException e) {
            System.out.println("no character " + i + " in " + text);
        }
        return b.toString();
    }

    static String insertAt(String text, int i) {
        StringBuilder b = new StringBuilder(text);
        try {
            b.insert(i, "!");
        } catch (IndexOutOfBoundsException e) {
            System.out.println("cannot insert at " + i + ": " + e.getClass().getSimpleName());
        }
        return b.toString();
    }

    static String appendPart(char[] chars, int count) {
        StringBuilder b = new StringBuilder();
        try {
            b.append(chars, 1, count);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("cannot append " + count + " characters");
        }
        return b.toString();
    }

    static int ratio(int a, int b) {
        try {
            return Integer.divideUnsigned(a, b);
        } catch (ArithmeticException e) {
            System.out.println("no unsigned ratio of " + a + " and " + b);
            return 0;
        }
    }

    static long rest(long a, long b) {
        try {
            return Long.remainderUnsigned(a, b);
        } finally {
            System.out.println("remainder of " + a + " tried");
        }
    }

    static String letters(int[] codePoints) {
        try {
            return new String(codePoints, 0, codePoints.length);
        } catch (IllegalArgumentException e) {
            return "not letters: " + e.getClass().getSimpleName();
        }
    }

    static int encoded(String text, String charset) {
        try {
            return text.getBytes(charset).length;
        } catch (UnsupportedEncodingException e) {
            System.out.println("no charset " + charset);
            return -1;
        }
    }

    static String decoded(byte[] bytes, int count) {
        try {
            return new String(bytes, 0, count, "UTF-8");
        } catch (UnsupportedEncodingException e) {
            return "no charset";
        } catch (RuntimeException e) {
            return "out of range: " + e.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        System.out.println(removeAt("abc", 1));
        System.out.println(removeAt("abc", 5));
        System.out.println(insertAt("abc", 3));
        System.out.println(insertAt("abc", -1));
        char[] chars = {'w', 'x', 'y', 'z'};
        System.out.println(appendPart(chars, 2));
        System.out.println(appendPart(chars, 7));
        System.out.println(ratio(-1, 2));
        System.out.println(ratio(7, 0));
        System.out.println(rest(-1L, 10L));
        try {
            System.out.println(rest(5L, 0L));
        } catch (ArithmeticException e) {
            System.out.println("caught in main: " + e.getClass().getName());
        }
        System.out.println(letters(new int[] {72, 105, 0x1F600}));
        System.out.println(letters(new int[] {72, -5}));
        System.out.println(encoded("héllo", "UTF-8"));
        System.out.println(encoded("hello", "NO-SUCH-CHARSET"));
        byte[] bytes = {104, 105, 33};
        System.out.println(decoded(bytes, 2));
        System.out.println(decoded(bytes, 9));
    }
}
