// A builder appends a range of a `CharSequence` whose `toString` may throw, inside a `try` that
// catches it: the append reads the range with `length` and `charAt` and never makes the text, as
// in the JDK's Matcher.appendReplacement on javac's names.
public class AppendedRangeOfThrowingText {
    // The first `count` letters of the alphabet.
    static class Name implements CharSequence {
        final int count;

        Name(int count) {
            this.count = count;
        }

        public int length() {
            return count;
        }

        public char charAt(int i) {
            return (char) ('a' + i);
        }

        public CharSequence subSequence(int start, int end) {
            return new Name(end - start);
        }

        @Override
        public String toString() {
            if (count > 3) {
                throw new IllegalStateException("too long to show");
            }
            return count == 2 ? "ab" : "a few letters";
        }
    }

    static String head(CharSequence s, int n) {
        StringBuilder sb = new StringBuilder("<");
        try {
            sb.append(s, 0, n);
            sb.append(" of ").append(s.length());
        } catch (IllegalStateException e) {
            sb.append("caught ").append(e.getMessage());
        } catch (IndexOutOfBoundsException e) {
            sb.append("out of range");
        }
        return sb.append('>').toString();
    }

    static String shown(CharSequence s) {
        try {
            return "shown " + s;
        } catch (IllegalStateException e) {
            return "shown caught " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        CharSequence[] all = {new Name(6), new Name(2), "plain text"};
        for (CharSequence s : all) {
            System.out.println(head(s, 2));
            System.out.println(head(s, 9));
            System.out.println(shown(s));
        }
    }
}
