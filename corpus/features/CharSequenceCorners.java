// Corners of `CharSequence`: `contentEquals` of a sequence and of null, and a class of the program
// whose `subSequence` returns a string, as javac's names have it.
public class CharSequenceCorners {
    /** Text kept as a string, as javac's names keep theirs. */
    static final class Name implements CharSequence {
        private final String text;

        Name(String text) {
            this.text = text;
        }

        @Override
        public int length() {
            return text.length();
        }

        @Override
        public char charAt(int index) {
            return text.charAt(index);
        }

        @Override
        public String subSequence(int start, int end) {
            return text.substring(start, end);
        }

        @Override
        public String toString() {
            return text;
        }

        boolean contentEquals(CharSequence cs) {
            return text.contentEquals(cs);
        }
    }

    static String copied(CharSequence cs) {
        String s = cs.toString();
        char[] buf = new char[s.length() + 2];
        buf[0] = '<';
        s.getChars(0, s.length(), buf, 1);
        buf[buf.length - 1] = '>';
        return new String(buf);
    }

    static boolean same(String s, CharSequence cs) {
        return s.contentEquals(cs);
    }

    public static void main(String[] args) {
        Name name = new Name("java.lang");
        System.out.println(name.subSequence(0, 4) + " " + name.subSequence(5, 9).length());
        CharSequence cs = name;
        System.out.println(cs.subSequence(5, 9));
        System.out.println(copied(name) + copied("abc") + copied(new StringBuilder("sb")));
        System.out.println(same("java.lang", name) + " " + same("java", name) + " " + same("sb", new StringBuilder("sb")));
        System.out.println(name.contentEquals("java.lang") + " " + name.contentEquals(new StringBuilder("x")));
        try {
            System.out.println(same("text", null));
        } catch (NullPointerException e) {
            System.out.println("null sequence");
        }
        try {
            System.out.println("text".contentEquals((CharSequence) null));
        } catch (NullPointerException e) {
            System.out.println("null literal");
        }
        try {
            System.out.println("text".contentEquals(null));
        } catch (NullPointerException e) {
            System.out.println("null buffer");
        }
        try {
            copied(name).getChars(0, 20, new char[3], 0);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("out of range");
        }
    }
}
