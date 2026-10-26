// `==` between a class that implements CharSequence and a CharSequence (netty's
// `AsciiString.compareTo`: `this == string`), which compares the class's object as a
// CharSequence.
public class CharSequenceIdentity {
    static final class Ascii implements CharSequence, Comparable<CharSequence> {
        private final String text;

        Ascii(String text) {
            this.text = text;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int index) {
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            return new Ascii(text.substring(start, end));
        }

        public int compareTo(CharSequence other) {
            if (this == other) {
                return 0;
            }
            return text.compareTo(other.toString());
        }

        boolean same(CharSequence other) {
            return other == this;
        }

        public String toString() {
            return text;
        }
    }

    public static void main(String[] args) {
        Ascii a = new Ascii("abc");
        CharSequence b = args.length > 0 ? null : a;
        System.out.println(b.length() + " " + b.charAt(0));
        System.out.println(a.compareTo(b) + " " + a.compareTo("abd") + " " + a.same(b) + " " + a.same(new Ascii("abc")));
    }
}
