import java.nio.charset.Charset;

// The JDK's single-byte charsets, which a translated program carries as tables: by constant and
// by computed name, through aliases, encoding what they can and cannot (a surrogate pair, a lone
// surrogate) and decoding bytes they have no char for.
public class CharsetTables {
    static String hex(byte[] bytes) {
        StringBuilder s = new StringBuilder();
        for (byte b : bytes) {
            s.append(String.format("%02x", b & 0xff));
        }
        return s.toString();
    }

    static String units(String text) {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < text.length(); i++) {
            s.append(Integer.toHexString(text.charAt(i))).append(' ');
        }
        return s.toString().trim();
    }

    public static void main(String[] args) throws Exception {
        Charset w1252 = Charset.forName("windows-1252");
        System.out.println(w1252 + " " + w1252.name() + " " + w1252.displayName() + " " + w1252.hashCode());
        System.out.println((w1252 == Charset.forName("CP1252")) + " " + w1252.equals(Charset.forName("cp1252")));
        String text = "Grüße € – “hi” 😀 \uD800 Ā";
        System.out.println(hex(text.getBytes(w1252)));
        byte[] high = new byte[64];
        for (int i = 0; i < high.length; i++) {
            high[i] = (byte) (0x80 + i);
        }
        System.out.println(units(new String(high, w1252)));
        System.out.println(units(new String(high, 8, 16, "ISO-8859-15")));
        System.out.println(hex("Kožušček".getBytes("ISO-8859-2")) + " " + hex("Привет".getBytes("KOI8-R")));
        Charset ebcdic = Charset.forName("IBM1047");
        System.out.println(hex("Hello? €".getBytes(ebcdic)) + " " + new String(new byte[] {(byte) 0xc8, (byte) 0x85}, ebcdic));
        String computed = "windows-" + (1250 + args.length);
        System.out.println(Charset.isSupported(computed) + " " + Charset.forName(computed).name() + " "
            + hex("Łódź".getBytes(computed)));
        System.out.println(Charset.isSupported("x-MacRoman") + " " + Charset.forName("MacRoman"));
    }
}
