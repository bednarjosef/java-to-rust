import java.util.Iterator;
import java.util.List;

/**
 * A builder made from, or appending, a `CharSequence` of the program's own class reads it as
 * `AbstractStringBuilder` does: by its `length()` and `charAt`, never its `toString()`, which
 * here throws for a long text. A null one appends "null"; a null one made into a builder throws
 * with the constructor's message; a length that changes between its calls fails the range check;
 * a `charAt` that throws leaves the builder as it was; a negative length fails the constructor.
 */
public class BuildersFromSequences {
    static int lengths;

    static final class Ascii implements CharSequence {
        private final byte[] bytes;

        Ascii(String s) {
            bytes = s.getBytes();
        }

        public int length() {
            lengths++;
            return bytes.length;
        }

        public char charAt(int i) {
            if (bytes[i] == '!') {
                throw new IllegalArgumentException("no ! at " + i);
            }
            return (char) bytes[i];
        }

        public CharSequence subSequence(int start, int end) {
            return new Ascii(new String(bytes, start, end - start));
        }

        public String toString() {
            if (bytes.length > 20) {
                throw new IllegalStateException("too long");
            }
            return "Ascii(" + new String(bytes) + ")";
        }
    }

    /** A sequence whose length is the next of the given ones at each call. */
    static final class Shrinking implements CharSequence {
        private final int[] lengths;
        private int calls;

        Shrinking(int... lengths) {
            this.lengths = lengths;
        }

        public int length() {
            return lengths[Math.min(calls++, lengths.length - 1)];
        }

        public char charAt(int i) {
            return (char) ('a' + i);
        }

        public CharSequence subSequence(int start, int end) {
            return "";
        }
    }

    static CharSequence join(CharSequence separator, Iterable<? extends CharSequence> elements) {
        Iterator<? extends CharSequence> iterator = elements.iterator();
        if (!iterator.hasNext()) {
            return "";
        }
        CharSequence firstElement = iterator.next();
        if (!iterator.hasNext()) {
            return firstElement;
        }
        StringBuilder builder = new StringBuilder(firstElement);
        do {
            builder.append(separator).append(iterator.next());
        } while (iterator.hasNext());
        return builder;
    }

    public static void main(String[] args) {
        List<CharSequence> parts = List.of(new Ascii("a"), "b", new StringBuilder("c"), new Ascii("d"));
        try {
            System.out.println(join(new Ascii(", "), parts));
            System.out.println(join("-", List.of(new Ascii("x".repeat(30)), "y")));
            System.out.println(new Ascii("x".repeat(30)));
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println("lengths asked: " + lengths);

        StringBuilder sb = new StringBuilder("<");
        CharSequence none = null;
        sb.append(none).append(new Ascii("ok")).append((CharSequence) "s");
        System.out.println(sb);
        System.out.println(new StringBuilder(new Ascii("abcd")).capacity());
        try {
            System.out.println(new StringBuilder(none));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            sb.append(new Ascii("ab!cd"));
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage() + ", builder " + sb);
        }
        try {
            sb.append(new Shrinking(5, 2));
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getClass().getName() + ": " + e.getMessage());
        }
        System.out.println(sb.append(new Shrinking(2, 3)));
        try {
            System.out.println(new StringBuilder(new Shrinking(-1)));
        } catch (NegativeArraySizeException e) {
            System.out.println("caught " + e.getClass().getName() + ": " + e.getMessage());
        }
        StringBuilder missing = args.length > 5 ? sb : null;
        lengths = 0;
        try {
            missing.append(new Ascii("never"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage() + " after " + lengths + " lengths");
        }
    }
}
