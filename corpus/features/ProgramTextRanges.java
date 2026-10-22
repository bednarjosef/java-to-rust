// A program class that implements `CharSequence`, appended as a range by `StringBuilder.append(s,
// start, end)`: Java asks `s` for its length once, checks the range against it, then asks `charAt`
// for each index in order. A bad range leaves the builder unchanged, and the exception is caught.
// The classes here have a `length` and a `charAt` that cannot throw, the only ones translated.
public class ProgramTextRanges {
    static class Letters implements CharSequence {
        final int count;

        Letters(int count) {
            this.count = count;
        }

        public int length() {
            return count;
        }

        public char charAt(int index) {
            return (char) ('a' + index);
        }

        public CharSequence subSequence(int start, int end) {
            return new Letters(end - start);
        }

        @Override
        public String toString() {
            return "letters" + count;
        }
    }

    static class Shifted implements CharSequence {
        final int offset;

        Shifted(int offset) {
            this.offset = offset;
        }

        public int length() {
            return 4;
        }

        public char charAt(int index) {
            return (char) ('A' + index + offset);
        }

        public CharSequence subSequence(int start, int end) {
            return new Shifted(offset + start);
        }

        @Override
        public String toString() {
            return "shifted";
        }
    }

    static void appendLetters(StringBuilder sb, CharSequence s, int start, int end) {
        sb.append(s, start, end);
    }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder("<");
        Letters letters = new Letters(6);
        sb.append(letters, 0, 3);
        sb.append(letters, 4, 6);
        System.out.println(sb);
        sb.append(letters, 2, 2);
        System.out.println(sb);
        try {
            sb.append(letters, 3, 9);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            appendLetters(sb, letters, -1, 2);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            appendLetters(sb, letters, 5, 4);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println(sb);

        Shifted shifted = new Shifted(2);
        sb.append(shifted, 1, 4);
        System.out.println(sb);

        CharSequence mixed = letters;
        StringBuilder other = new StringBuilder("!?");
        sb.append(mixed, 0, 1).append(other, 0, 1);
        System.out.println(sb + ">");
    }
}
