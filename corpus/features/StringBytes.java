import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

public class StringBytes {
    // Bytes as hex, so the output does not depend on the terminal's encoding.
    static void dump(String label, byte[] bytes) {
        StringBuilder sb = new StringBuilder(label);
        sb.append(" [").append(bytes.length).append("]");
        for (int i = 0; i < bytes.length; i++) {
            sb.append(' ').append(Integer.toHexString(bytes[i] & 0xff));
        }
        System.out.println(sb.toString());
    }

    // Decoded text as UTF-16 code units, for the same reason.
    static void units(String label, String s) {
        StringBuilder sb = new StringBuilder(label);
        sb.append(" (").append(s.length()).append(")");
        for (int i = 0; i < s.length(); i++) {
            sb.append(' ').append(Integer.toHexString(s.charAt(i)));
        }
        System.out.println(sb.toString());
    }

    public static void main(String[] args) throws Exception {
        String ascii = "Hello";
        String accents = "café";
        String euro = "5€";
        String emoji = "hi 😀!";
        String mixed = "été 🌍";

        dump("ascii utf8", ascii.getBytes());
        dump("accents utf8", accents.getBytes());
        dump("euro utf8", euro.getBytes(StandardCharsets.UTF_8));
        dump("emoji utf8", emoji.getBytes(StandardCharsets.UTF_8));
        dump("mixed utf8", mixed.getBytes("UTF-8"));
        dump("accents latin1", accents.getBytes(StandardCharsets.ISO_8859_1));
        dump("euro latin1", euro.getBytes(StandardCharsets.ISO_8859_1));
        dump("emoji ascii", emoji.getBytes(StandardCharsets.US_ASCII));
        dump("accents utf16", accents.getBytes(StandardCharsets.UTF_16));
        dump("emoji utf16be", emoji.getBytes(StandardCharsets.UTF_16BE));
        dump("emoji utf16le", emoji.getBytes(Charset.forName("UTF-16LE")));
        dump("emoji named", emoji.getBytes("UTF-16BE"));
        dump("empty utf16", "".getBytes(StandardCharsets.UTF_16));

        byte[] invalid = {
            (byte) 0x61, (byte) 0xC3, (byte) 0x28, (byte) 0xE2, (byte) 0x82,
            (byte) 0xF0, (byte) 0x9F, (byte) 0x98, (byte) 0x80, (byte) 0xFF
        };
        units("invalid utf8", new String(invalid, StandardCharsets.UTF_8));
        units("invalid utf8 slice", new String(invalid, 1, 4, StandardCharsets.UTF_8));
        units("invalid default", new String(invalid));
        units("round trip emoji", new String(emoji.getBytes(StandardCharsets.UTF_8), StandardCharsets.UTF_8));
        units("round trip mixed", new String(mixed.getBytes("UTF-8"), "UTF-8"));
        units("utf16 bom", new String(accents.getBytes(StandardCharsets.UTF_16), StandardCharsets.UTF_16));
        units("utf16 be no bom", new String(new byte[] {0, 65, (byte) 0xD8, 0, 0, 66}, StandardCharsets.UTF_16BE));
        units("utf16 odd", new String(new byte[] {0, 65, 0}, StandardCharsets.UTF_16BE));
        units("latin1", new String(new byte[] {(byte) 0xE9, 0x41, (byte) 0x80}, StandardCharsets.ISO_8859_1));
        units("ascii", new String(new byte[] {0x41, (byte) 0xE9, 0x42}, "US-ASCII"));
        units("empty", new String(new byte[0], Charset.forName("UTF-8")));
    }
}
