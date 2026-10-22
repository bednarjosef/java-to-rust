// Charsets that may be null, as javac's BaseFileManager keeps the one its caller gives it (null
// for the default): a field given null, compared with null and replaced by a default, a static
// that starts out null, a local set when null, `equals(null)`, printing a null charset, and the
// NullPointerException of using one, with Java's message where a method of the charset is called.
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

public class NullableCharsets {
    static class FileManager {
        protected Charset charset;

        FileManager(Charset charset) {
            this.charset = charset;
        }

        String charsetName(String encName) {
            return charset == null ? encName : charset.name();
        }

        Charset decoderCharset(String encodingName) {
            Charset cs = (this.charset == null) ? Charset.forName(encodingName) : this.charset;
            return cs;
        }
    }

    static Charset unset;

    static Charset orDefault(Charset cs) {
        if (cs == null) {
            cs = StandardCharsets.UTF_8;
        }
        return cs;
    }

    static String describe(Charset cs) {
        return "charset " + cs;
    }

    public static void main(String[] args) {
        FileManager plain = new FileManager(null);
        FileManager utf8 = new FileManager(StandardCharsets.UTF_8);
        System.out.println(plain.charsetName("latin1") + " " + utf8.charsetName("latin1"));
        System.out.println(plain.decoderCharset("ISO-8859-1") + " " + utf8.decoderCharset("ISO-8859-1"));
        System.out.println(unset == null);
        System.out.println(describe(unset) + ", " + describe(StandardCharsets.US_ASCII));
        unset = plain.decoderCharset("US-ASCII");
        System.out.println(unset != null ? unset.name() : "none");
        System.out.println((plain.charset == utf8.charset) + " " + (plain.charset == null) + " " + (utf8.charset != null));
        System.out.println(orDefault(null) + " " + orDefault(StandardCharsets.UTF_16BE));
        System.out.println(StandardCharsets.UTF_8.equals(plain.charset) + " " + StandardCharsets.UTF_8.equals(utf8.charset));
        System.out.println(new String(new byte[] {104, 105}, utf8.decoderCharset("x")));
        try {
            System.out.println(plain.charset.name());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println("text".getBytes(plain.charset).length);
        } catch (NullPointerException e) {
            System.out.println("no charset to encode with");
        }
    }
}
