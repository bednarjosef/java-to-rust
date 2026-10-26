import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.Charset;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.CodingErrorAction;

// The coders, readers and writers of the JDK's single-byte charsets, which a translated program
// carries as tables: Charset.encode and decode, a decoder that reports a byte without a char, an
// encoder's canEncode, and a reader and a writer with their historical names.
public class CharsetTableCoders {
    public static void main(String[] args) throws Exception {
        Charset w1252 = Charset.forName("windows-1252");
        printBytes("encode", w1252.encode("€ ok Ā 😀 \uD800"));
        byte[] high = {(byte) 0x80, (byte) 0x81, (byte) 0x9f, 0x41};
        printChars("decode", w1252.decode(ByteBuffer.wrap(high)));
        CharsetDecoder strict = w1252.newDecoder().onUnmappableCharacter(CodingErrorAction.REPORT);
        try {
            strict.decode(ByteBuffer.wrap(high));
        } catch (CharacterCodingException e) {
            System.out.println("strict: " + e.getClass().getSimpleName() + " " + e.getMessage());
        }
        CharsetEncoder encoder = w1252.newEncoder();
        System.out.println(encoder.canEncode('€') + " " + encoder.canEncode('Ā') + " "
            + encoder.maxBytesPerChar() + " " + encoder.charset());
        Charset ebcdic = Charset.forName("IBM1047");
        printBytes("ebcdic", ebcdic.newEncoder().onUnmappableCharacter(CodingErrorAction.REPLACE)
            .encode(CharBuffer.wrap("Hi? €")));

        byte[] latin2 = "Žluťoučký kůň".getBytes("ISO-8859-2");
        InputStreamReader reader = new InputStreamReader(new ByteArrayInputStream(latin2), "ISO-8859-2");
        char[] read = new char[64];
        int n = reader.read(read);
        System.out.println(reader.getEncoding() + " " + new String(read, 0, n));

        ByteArrayOutputStream out = new ByteArrayOutputStream();
        OutputStreamWriter writer = new OutputStreamWriter(out, "KOI8-R");
        writer.write("Привет € 😀!");
        System.out.println(writer.getEncoding());
        writer.close();
        printBytes("koi8", ByteBuffer.wrap(out.toByteArray()));
        System.out.println(new OutputStreamWriter(new ByteArrayOutputStream(), w1252).getEncoding());
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
            out.append(' ').append(Integer.toHexString(bytes.get() & 0xff));
        }
        System.out.println(out);
    }
}
