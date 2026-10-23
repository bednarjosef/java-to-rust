// `CharSequence.getChars` copied into classes that are a `CharSequence` only through a
// sub-interface: the copy calls the `length()` and `charAt(int)` that Java calls on the object,
// the class's own, a default method of the sub-interface, or a superclass's.
public class GetCharsThroughSubInterface {
    interface Text extends CharSequence {
    }

    /** Declares `length` and `charAt` itself. */
    static final class Own implements Text {
        private final char[] chars;

        Own(String text) {
            chars = text.toCharArray();
        }

        @Override
        public int length() {
            return chars.length;
        }

        @Override
        public char charAt(int index) {
            return chars[index];
        }

        @Override
        public CharSequence subSequence(int start, int end) {
            throw new UnsupportedOperationException();
        }
    }

    interface Glyphs extends CharSequence {
        @Override
        default char charAt(int index) {
            return (char) ('a' + index);
        }
    }

    /** Gets `charAt` from its sub-interface, `length` from its own declaration. */
    static final class Alphabet implements Glyphs {
        private final int count;

        Alphabet(int count) {
            this.count = count;
        }

        @Override
        public int length() {
            return count;
        }

        @Override
        public CharSequence subSequence(int start, int end) {
            throw new UnsupportedOperationException();
        }
    }

    interface Sized extends CharSequence {
        @Override
        default int length() {
            return 3;
        }
    }

    /** Gets `length` from its sub-interface, `charAt` from its own declaration. */
    static final class Triple implements Sized {
        @Override
        public char charAt(int index) {
            return "xyz".charAt(index);
        }

        @Override
        public CharSequence subSequence(int start, int end) {
            throw new UnsupportedOperationException();
        }
    }

    static class Base {
        protected final String text;

        Base(String text) {
            this.text = text;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int index) {
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            throw new UnsupportedOperationException();
        }
    }

    /** Gets `length` and `charAt` from its superclass. */
    static final class Inherited extends Base implements Text {
        Inherited(String text) {
            super(text);
        }
    }

    /** Abstract, so `getChars` is copied here from `Text`, and `length` and `charAt` come below. */
    static abstract class Shell implements Text {
        @Override
        public CharSequence subSequence(int start, int end) {
            throw new UnsupportedOperationException();
        }
    }

    /** Declares `length` and `charAt` for `Shell`, whose copy of `getChars` calls them. */
    static final class Core extends Shell {
        private final String text;

        Core(String text) {
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
    }

    /** The copy of `cs` from `begin` to `end`, between `<` and `>`. */
    static String copy(CharSequence cs, int begin, int end) {
        char[] dst = new char[end - begin + 2];
        dst[0] = '<';
        cs.getChars(begin, end, dst, 1);
        dst[dst.length - 1] = '>';
        return new String(dst);
    }

    public static void main(String[] args) {
        Own own = new Own("hello");
        System.out.println(copy(own, 1, 4));
        char[] letters = new char[4];
        own.getChars(0, 4, letters, 0);
        System.out.println(new String(letters));

        Alphabet alphabet = new Alphabet(10);
        System.out.println(copy(alphabet, 2, 6));
        alphabet.getChars(5, 8, letters, 1);
        System.out.println(new String(letters));

        Triple triple = new Triple();
        System.out.println(copy(triple, 0, 3));
        triple.getChars(1, 3, letters, 0);
        System.out.println(new String(letters));

        Inherited inherited = new Inherited("java");
        System.out.println(copy(inherited, 0, 4));
        inherited.getChars(2, 4, letters, 2);
        System.out.println(new String(letters));

        Shell shell = new Core("abc");
        System.out.println(copy(shell, 0, 2));
        shell.getChars(1, 3, letters, 0);
        System.out.println(new String(letters));

        try {
            copy(triple, 1, 5);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
