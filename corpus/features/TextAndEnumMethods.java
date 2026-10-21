import java.util.Arrays;

public class TextAndEnumMethods {
    enum Kind { ALPHA, BETA }

    static class Pair {
        final Kind kind;
        final int arg;

        Pair(Kind kind, int arg) {
            this.kind = kind;
            this.arg = arg;
        }

        @Override
        public int hashCode() {
            return kind.hashCode() * 17 + arg;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Pair p && kind.equals(p.kind) && arg == p.arg;
        }
    }

    public static void main(String[] args) {
        String s = "a😀bé";
        System.out.println(s.codePointAt(0) + " " + s.codePointAt(1) + " " + s.codePointAt(2) + " " + s.codePointAt(3));
        System.out.println(s.codePointBefore(3) + " " + s.codePointBefore(2) + " " + s.codePointCount(0, s.length()));
        System.out.println(s.codePointCount(2, 4) + " " + s.offsetByCodePoints(0, 2) + " " + s.offsetByCodePoints(5, -3));
        try {
            System.out.println(s.codePointAt(9));
        } catch (StringIndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println(s.offsetByCodePoints(0, 9));
        } catch (IndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }

        StringBuilder sb = new StringBuilder("x");
        sb.appendCodePoint(0x1F600).appendCodePoint('y').repeat('-', 3).repeat("ab", 2).repeat(0x1F601, 1);
        System.out.println(sb + " " + sb.length() + " " + sb.codePointAt(1) + " " + sb.codePointBefore(3));
        System.out.println(sb.codePointCount(0, sb.length()) + " " + sb.offsetByCodePoints(0, 3));
        StringBuilder other = new StringBuilder("<>");
        sb.repeat(other, 2).repeat((String) null, 1).repeat(sb, 0);
        System.out.println(sb);
        char[] dst = new char[6];
        Arrays.fill(dst, '.');
        sb.getChars(4, 8, dst, 1);
        System.out.println(new String(dst));
        try {
            sb.repeat('z', -1);
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage());
        }

        CharSequence part = "hello world".subSequence(6, 11);
        CharSequence built = new StringBuilder("builder").subSequence(0, 5);
        System.out.println(part + " " + built + " " + part.length() + " " + built.charAt(1));
        System.out.println("world".contentEquals(part) + " " + "build".contentEquals(built) + " "
                + "x".contentEquals(new StringBuilder("x")) + " " + "x".contentEquals("y"));

        System.out.println("Hello World".regionMatches(6, "World", 0, 5) + " "
                + "Hello World".regionMatches(6, "world", 0, 5) + " "
                + "Hello World".regionMatches(true, 6, "WORLD", 0, 5) + " "
                + "Hello".regionMatches(-1, "H", 0, 1) + " "
                + "Hello".regionMatches(3, "lo!", 0, 3) + " "
                + "Hello".regionMatches(true, 0, "x", 0, 0));

        String block = "    first\n      second  \n\n    third\n    ";
        System.out.println("[" + block.stripIndent().replace("\n", "|") + "]");
        System.out.println("[" + "  a\n    b\n".stripIndent().replace("\n", "|") + "]");
        System.out.println("[" + "\t x\n\t  y".stripIndent().replace("\n", "|") + "]");

        byte[] bytes = new byte[4];
        "ABŁC".getBytes(1, 4, bytes, 0);
        System.out.println(Arrays.toString(bytes));

        Kind k = args.length > 0 ? Kind.ALPHA : Kind.BETA;
        System.out.println(k.equals(Kind.BETA) + " " + k.equals(Kind.ALPHA) + " " + k.equals("BETA"));
        System.out.println(k.hashCode() == Kind.BETA.hashCode());
        System.out.println(new Pair(Kind.ALPHA, 1).equals(new Pair(Kind.ALPHA, 1)) + " "
                + (new Pair(Kind.ALPHA, 1).hashCode() == new Pair(Kind.ALPHA, 1).hashCode()) + " "
                + new Pair(Kind.ALPHA, 1).equals(new Pair(Kind.BETA, 1)));
    }
}
