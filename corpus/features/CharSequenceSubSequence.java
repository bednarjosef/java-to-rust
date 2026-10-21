public class CharSequenceSubSequence {
    /// A name that keeps its text in a string and gives its parts as strings, as javac's does.
    static class Name implements CharSequence {
        private final String text;

        Name(String text) {
            this.text = text;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int index) {
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            return toString().subSequence(start, end);
        }

        @Override
        public String toString() {
            return text;
        }
    }

    /// A view of part of a `char[]`, whose parts are views too.
    static class Slice implements CharSequence {
        private final char[] chars;
        private final int offset;
        private final int count;

        Slice(char[] chars, int offset, int count) {
            this.chars = chars;
            this.offset = offset;
            this.count = count;
        }

        public int length() {
            return count;
        }

        public char charAt(int index) {
            if (index < 0 || index >= count) {
                throw new IndexOutOfBoundsException("index " + index);
            }
            return chars[offset + index];
        }

        public Slice subSequence(int start, int end) {
            if (start < 0 || end > count || start > end) {
                throw new IndexOutOfBoundsException("start " + start + ", end " + end);
            }
            return new Slice(chars, offset + start, end - start);
        }

        @Override
        public String toString() {
            return new String(chars, offset, count);
        }
    }

    static String describe(CharSequence cs) {
        CharSequence part = cs.subSequence(1, cs.length() - 1);
        return part + " (" + part.length() + ", " + part.charAt(0) + ")";
    }

    public static void main(String[] args) {
        CharSequence[] all = {
            "[string]",
            new StringBuilder("<builder>"),
            new Name("(name)"),
            new Slice("..{slice}..".toCharArray(), 2, 7),
        };
        for (CharSequence cs : all) {
            System.out.println(describe(cs));
        }
        CharSequence nested = all[3].subSequence(1, 6).subSequence(1, 4);
        System.out.println(nested + " " + nested.length());
        Slice slice = new Slice("abcdef".toCharArray(), 1, 4);
        Slice sub = slice.subSequence(1, 3);
        System.out.println(sub + " " + sub.charAt(1));
        try {
            CharSequence bad = all[3].subSequence(3, 99);
            System.out.println("no failure " + bad.length());
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
