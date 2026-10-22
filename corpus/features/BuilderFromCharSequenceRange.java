public class BuilderFromCharSequenceRange {
    static class Chars implements CharSequence {
        final String text;

        Chars(String text) {
            this.text = text;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int index) {
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            return new Chars(text.substring(start, end));
        }

        public String toString() {
            return text;
        }
    }

    static String prefixOf(CharSequence src, int spanLength) {
        if (spanLength != 0) {
            StringBuilder sb = new StringBuilder(src.length()).append(src, 0, spanLength);
            return sb.toString();
        }
        return src.toString();
    }

    static String bracketed(CharSequence src) {
        return new StringBuilder(4).append(src, 1, 3).append("!").toString();
    }

    static String nullableRange(CharSequence s) {
        StringBuilder sb = new StringBuilder();
        sb.append(s, 0, 2);
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.println(prefixOf("hello", 2));
        System.out.println(prefixOf(new Chars("world"), 3));
        System.out.println(prefixOf(new Chars("world"), 0));
        System.out.println(bracketed(new Chars("world")));
        System.out.println(bracketed("hello"));
        System.out.println(nullableRange(new Chars("abc")));
        System.out.println(nullableRange(null));
        try {
            System.out.println(prefixOf(new Chars("ab"), 5));
        } catch (IndexOutOfBoundsException e) {
            System.out.println("out of range");
        }
    }
}
