import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

// Charset coding through the JDK's coders: UTF-8 decoding of valid bytes and of malformed ones,
// which is replaced rather than thrown, ISO-8859-1 encoding of text with characters outside the
// charset, and Charset.encode of a String. Each result prints its code points or its bytes.
public class CharsetCodings {
    public static void main(String[] args) {
        byte[] valid = "héllo €".getBytes(StandardCharsets.UTF_8);
        printChars("valid", StandardCharsets.UTF_8.decode(ByteBuffer.wrap(valid)));

        byte[] malformed = {0x61, (byte) 0xff, 0x62, (byte) 0xc3};
        printChars("malformed", StandardCharsets.UTF_8.decode(ByteBuffer.wrap(malformed)));

        printBytes("latin1", StandardCharsets.ISO_8859_1.encode("aé€中"));

        printBytes("utf8", Charset.forName("UTF-8").encode("x€"));
    }

    private static void printChars(String label, CharBuffer chars) {
        StringBuilder out = new StringBuilder(label + ":");
        while (chars.hasRemaining()) {
            out.append(' ').append(Integer.toHexString(chars.get()));
        }
        System.out.println(out);
    }

    private static void printBytes(String label, ByteBuffer bytes) {
        StringBuilder out = new StringBuilder(label + ":");
        while (bytes.hasRemaining()) {
            out.append(' ').append(bytes.get() & 0xff);
        }
        System.out.println(out);
    }
}
