// A builder's append(s, start, end) of a CharSequence that the program implements asks it for its
// length once and its charAt of each index; what they throw goes on as a call's would, and leaves
// the builder as it was. A null sequence appends a range of "null".
public class CharSequenceRangeFailures {
    static class Letters implements CharSequence {
        final String text;
        int asked;

        Letters(String text) {
            this.text = text;
        }

        public int length() {
            asked++;
            return text.length();
        }

        public char charAt(int i) {
            if (text.charAt(i) == '!') {
                throw new IllegalArgumentException("no ! at " + i);
            }
            return text.charAt(i);
        }

        public CharSequence subSequence(int start, int end) {
            return new Letters(text.substring(start, end));
        }

        @Override
        public String toString() {
            return text;
        }
    }

    static class Broken implements CharSequence {
        public int length() {
            throw new IllegalStateException("no length");
        }

        public char charAt(int i) {
            return 'x';
        }

        public CharSequence subSequence(int start, int end) {
            return this;
        }
    }

    static String append(CharSequence s, int start, int end) {
        StringBuilder sb = new StringBuilder("[");
        try {
            sb.append(s, start, end);
        } catch (IllegalArgumentException | IllegalStateException e) {
            sb.append(e.getClass().getSimpleName()).append(": ").append(e.getMessage());
        } catch (IndexOutOfBoundsException e) {
            sb.append("out of bounds: ").append(e.getMessage());
        }
        return sb.append("]").toString();
    }

    static CharSequence pick(int k) {
        return k > 0 ? new Letters("abc") : null;
    }

    public static void main(String[] args) {
        Letters letters = new Letters("hello!world");
        System.out.println(append(letters, 0, 5));
        System.out.println(append(letters, 2, 9));
        System.out.println(append(letters, 4, 20));
        System.out.println("length asked " + letters.asked + " times");
        System.out.println(append(new Broken(), 0, 1));
        System.out.println(append(pick(0), 1, 3));
        System.out.println(append(pick(0), 2, 7));
        System.out.println(append(pick(1), 1, 3));
    }
}
