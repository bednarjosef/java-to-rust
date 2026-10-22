/**
 * `CharSequence.getChars` on a string, a builder and a class that has only `charAt` and `length`,
 * which gets the interface's default, with ranges outside the sequence or the array and a null
 * array.
 */
public class SequenceChars {
    /** A sequence of the letters from `first`, `count` of them. */
    static class Letters implements CharSequence {
        private final char first;
        private final int count;

        Letters(char first, int count) {
            this.first = first;
            this.count = count;
        }

        public int length() {
            return count;
        }

        public char charAt(int index) {
            if (index < 0 || index >= count) {
                throw new IndexOutOfBoundsException("letter " + index);
            }
            return (char) (first + index);
        }

        public CharSequence subSequence(int start, int end) {
            return new Letters((char) (first + start), end - start);
        }

        public String toString() {
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < count; i++) {
                sb.append(charAt(i));
            }
            return sb.toString();
        }
    }

    static String copy(CharSequence s, int begin, int end, char[] dst, int at) {
        try {
            s.getChars(begin, end, dst, at);
        } catch (IndexOutOfBoundsException e) {
            return e.getClass().getSimpleName() + ": " + e.getMessage();
        } catch (NullPointerException e) {
            return "NullPointerException";
        }
        return dst == null ? "no array" : new String(dst).replace('\0', '_');
    }

    /** `charAt` and `subSequence` through `CharSequence`, whose failures are caught too. */
    static String parts(CharSequence s, int index, int begin, int end) {
        String at;
        try {
            at = "" + s.charAt(index);
        } catch (IndexOutOfBoundsException e) {
            at = e.getClass().getSimpleName() + ": " + e.getMessage();
        }
        try {
            return at + " / " + s.subSequence(begin, end).length();
        } catch (IndexOutOfBoundsException e) {
            return at + " / " + e.getClass().getSimpleName() + ": " + e.getMessage();
        }
    }

    /** The default `getChars` of `Letters`, whose `NullPointerException` names the array. */
    static String copyLetters(Letters s, int begin, int end, char[] dst) {
        try {
            s.getChars(begin, end, dst, 0);
        } catch (IndexOutOfBoundsException e) {
            return e.getClass().getSimpleName() + ": " + e.getMessage();
        } catch (NullPointerException e) {
            return "NullPointerException: " + e.getMessage();
        }
        return dst == null ? "no array" : new String(dst);
    }

    public static void main(String[] args) {
        CharSequence[] all = {"hello", new StringBuilder("world"), new Letters('a', 6)};
        for (CharSequence s : all) {
            System.out.println(copy(s, 0, s.length(), new char[s.length()], 0));
            System.out.println(copy(s, 1, 3, "........".toCharArray(), 4));
            System.out.println(copy(s, 2, 2, new char[1], 1));
            System.out.println(copy(s, 3, 2, new char[8], 0));
            System.out.println(copy(s, -1, 2, new char[8], 0));
            System.out.println(copy(s, 0, 9, new char[8], 0));
            System.out.println(copy(s, 0, 3, new char[2], 0));
            System.out.println(copy(s, 0, 3, new char[8], -1));
            System.out.println(copy(s, 0, 3, new char[8], 6));
            System.out.println(copy(s, 0, 3, null, 0));
            System.out.println(copy(s, 4, 2, null, 0));
            System.out.println(parts(s, 1, 1, 4));
            System.out.println(parts(s, 9, 3, 2));
        }
        Letters letters = new Letters('p', 4);
        System.out.println(copyLetters(letters, 1, 4, new char[3]));
        System.out.println(copyLetters(letters, 0, 2, null));
        System.out.println(copyLetters(letters, 3, 1, null));
    }
}
