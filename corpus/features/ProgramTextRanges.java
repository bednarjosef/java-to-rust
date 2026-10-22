// A program class that implements `CharSequence`, appended as a range by `StringBuilder.append(s,
// start, end)`: Java asks `s` for its length once, checks the range against it, then asks `charAt`
// for each index in order. A bad range, or a `charAt` that throws, leaves the builder unchanged.
public class ProgramTextRanges {
    static class Spelled implements CharSequence {
        final String text;

        Spelled(String text) {
            this.text = text;
        }

        public int length() {
            System.out.println("length " + text.length());
            return text.length();
        }

        public char charAt(int index) {
            System.out.println("charAt " + index);
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            return new Spelled(text.substring(start, end));
        }

        @Override
        public String toString() {
            return text;
        }
    }

    static class Fragile implements CharSequence {
        final String text;
        final int failAt;

        Fragile(String text, int failAt) {
            this.text = text;
            this.failAt = failAt;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int index) {
            if (index == failAt) {
                throw new IllegalStateException("no character " + index);
            }
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            return new Fragile(text.substring(start, end), -1);
        }

        @Override
        public String toString() {
            return text;
        }
    }

    static class Overlong implements CharSequence {
        final String text;

        Overlong(String text) {
            this.text = text;
        }

        public int length() {
            return text.length() + 2;
        }

        public char charAt(int index) {
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            return new Overlong(text.substring(start, end));
        }

        @Override
        public String toString() {
            return text;
        }
    }

    static void appendSpelled(StringBuilder sb, CharSequence s, int start, int end) {
        sb.append(s, start, end);
    }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder("<");
        Spelled word = new Spelled("program");
        sb.append(word, 0, 3);
        sb.append(word, 4, 7);
        System.out.println(sb);
        sb.append(word, 2, 2);
        System.out.println(sb);
        try {
            sb.append(word, 3, 9);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            appendSpelled(sb, word, -1, 2);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            appendSpelled(sb, word, 5, 4);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println(sb);

        Fragile fragile = new Fragile("abcdef", 3);
        try {
            System.out.println("direct " + fragile.charAt(3));
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        Overlong overlong = new Overlong("abc");
        try {
            System.out.println("direct " + overlong.charAt(3));
        } catch (StringIndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            sb.append(overlong, 1, 5);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println(sb);

        CharSequence mixed = new Spelled("xyz");
        StringBuilder other = new StringBuilder("!?");
        sb.append(mixed, 1, 3).append(other, 0, 1);
        System.out.println(sb + ">");
    }
}
