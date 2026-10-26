import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.Charset;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.StandardCharsets;

// Readers and writers over byte streams made with a charset, a decoder or an encoder that may be
// null, as the server's code makes them where a catch prints what it catches: Java's constructors
// check the charset first, and `getEncoding` names it while the stream is open.
public class CharsetNullChecks {
    static Charset charset(int n) {
        return n > 0 ? null : StandardCharsets.UTF_8;
    }

    static void make(int n) {
        try {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            OutputStreamWriter w = new OutputStreamWriter(bytes, charset(n));
            System.out.println("writer " + w.getEncoding());
            w.write("héllo");
            w.close();
            System.out.println("closed " + w.getEncoding() + " " + bytes.size());
            InputStreamReader r = new InputStreamReader(new ByteArrayInputStream(bytes.toByteArray()), charset(n));
            System.out.println("reader " + r.getEncoding() + " " + (char) r.read());
            r.close();
            System.out.println("closed " + r.getEncoding());
        } catch (Exception e) {
            System.out.println(e);
        }
    }

    static void coders(int n) {
        CharsetDecoder decoder = n > 0 ? null : StandardCharsets.ISO_8859_1.newDecoder();
        CharsetEncoder encoder = n > 0 ? null : StandardCharsets.ISO_8859_1.newEncoder();
        try {
            System.out.println(new InputStreamReader(new ByteArrayInputStream(new byte[] {65}), decoder).getEncoding());
        } catch (Exception e) {
            System.out.println(e);
        }
        try {
            System.out.println(new OutputStreamWriter(new ByteArrayOutputStream(), encoder).getEncoding());
        } catch (Exception e) {
            System.out.println(e);
        }
    }

    public static void main(String[] args) {
        make(args.length);
        make(args.length + 1);
        coders(args.length);
        coders(args.length + 1);
    }
}
