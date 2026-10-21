// Classes of the program that implement `CharSequence`, as the JDK's `CharBuffer` does: their
// `length` and `charAt`, declared or inherited from a superclass, run through a `CharSequence`
// beside strings and builders, and so does their `toString`.
public class CharSequenceImplementers {
    static class Repeat implements CharSequence {
        final char c;
        final int n;

        Repeat(char c, int n) {
            this.c = c;
            this.n = n;
        }

        public int length() {
            return n;
        }

        public char charAt(int index) {
            if (index < 0 || index >= n) {
                throw new IndexOutOfBoundsException("index " + index);
            }
            return c;
        }

        public CharSequence subSequence(int start, int end) {
            return new Repeat(c, end - start);
        }

        @Override
        public String toString() {
            return String.valueOf(c).repeat(n);
        }
    }

    // Inherits `length` and `charAt`, and names the interface again.
    static final class Stars extends Repeat implements CharSequence {
        Stars(int n) {
            super('*', n);
        }
    }

    static String reversed(CharSequence s) {
        StringBuilder out = new StringBuilder();
        for (int i = s.length() - 1; i >= 0; i--) {
            out.append(s.charAt(i));
        }
        return out.toString();
    }

    public static void main(String[] args) {
        CharSequence[] all = {new Repeat('a', 3), new Stars(2), "text", new StringBuilder("built")};
        for (CharSequence s : all) {
            System.out.println(s + " " + s.length() + " " + reversed(s) + " " + s.isEmpty());
        }
        System.out.println(new Repeat('b', 4).subSequence(1, 3).length());
        try {
            all[0].charAt(5);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
