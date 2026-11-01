// String.indent, and String constructors whose int arguments are bytes, shorts or chars that
// widen to int (offsets read from a short[]), including the deprecated high-byte constructor.
public class StringIndentAndWidened {
    public static void main(String[] args) {
        String text = "first\n  second\r\nthird\r\tfourth";
        System.out.print(text.indent(2));
        System.out.print(text.indent(-1));
        System.out.print(text.indent(-8));
        System.out.print(text.indent(Integer.MIN_VALUE));
        System.out.print("a\n\nb\n".indent(0));
        System.out.println("[" + "".indent(3) + "]");
        System.out.println("[" + "x".indent(1) + "]");

        char[] table = "alphabetagamma".toCharArray();
        short[] ids = {0, 5, 9, 14};
        for (int i = 0; i + 1 < ids.length; i++) {
            System.out.println(new String(table, ids[i], ids[i + 1] - ids[i]));
        }
        byte[] bytes = "bytes here".getBytes();
        short len = 5;
        System.out.println(new String(bytes, 0, len));
        byte start = 6;
        System.out.println(new String(bytes, start, 4));
        @SuppressWarnings("deprecation")
        String high = new String(new byte[] {0x41, 0x42, 0x43}, 0);
        System.out.println(high);
        @SuppressWarnings("deprecation")
        String shifted = new String(new byte[] {0x41, 0x42}, 1);
        System.out.println((int) shifted.charAt(0) + " " + (int) shifted.charAt(1));
        StringBuilder sb = new StringBuilder('x');
        sb.append("capacity from a char");
        System.out.println(sb + " " + sb.length());
    }
}
