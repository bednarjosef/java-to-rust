/**
 * A builder made from the `subSequence` of a builder, which is a string, copies it as a string,
 * and so does appending it: no `toString` of the program's own `CharSequence`s runs, though one
 * of them may throw what a `catch` around it would catch. A range outside the builder throws
 * `StringIndexOutOfBoundsException`, which the `catch` sees.
 */
public class BuilderFromSubSequence {
    static final class Letters implements CharSequence {
        private final String text;

        Letters(String text) {
            this.text = text;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int i) {
            return text.charAt(i);
        }

        public CharSequence subSequence(int start, int end) {
            return new Letters(text.substring(start, end));
        }

        public String toString() {
            if (text.isEmpty()) {
                throw new IllegalStateException("no letters");
            }
            return text;
        }
    }

    static String tail(StringBuilder first, int from) {
        StringBuilder middle = new StringBuilder(first.subSequence(from, first.length()));
        middle.append(first.subSequence(0, from));
        return middle.toString();
    }

    public static void main(String[] args) {
        CharSequence[] all = {new Letters("abc"), new Letters(""), "text", "a"};
        for (CharSequence s : all) {
            try {
                System.out.println(tail(new StringBuilder(s.toString()), s.length() > 1 ? 1 : 2));
            } catch (IllegalStateException e) {
                System.out.println("caught " + e.getMessage());
            } catch (IndexOutOfBoundsException e) {
                System.out.println("out of range: " + e.getClass().getSimpleName());
            }
        }
    }
}
