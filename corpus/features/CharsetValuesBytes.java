import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

public class CharsetValuesBytes {
    static Charset wire = StandardCharsets.UTF_16BE;

    static byte[] encode(String text, Charset cs) {
        return text.getBytes(cs);
    }

    static String decode(byte[] data, Charset cs) {
        return new String(data, cs);
    }

    public static void main(String[] args) {
        String text = "Aé€";
        Charset[] all = {StandardCharsets.UTF_8, StandardCharsets.US_ASCII, StandardCharsets.ISO_8859_1,
                StandardCharsets.UTF_16, StandardCharsets.UTF_16BE, StandardCharsets.UTF_16LE};
        for (Charset cs : all) {
            byte[] bytes = encode(text, cs);
            System.out.println(cs.name() + " " + bytes.length + " " + decode(bytes, cs));
        }
        byte[] local = text.getBytes(wire);
        System.out.println(local.length + " " + new String(local, wire));
        Charset pick = args.length == 0 ? StandardCharsets.ISO_8859_1 : StandardCharsets.UTF_8;
        System.out.println(decode(text.getBytes(pick), pick));
        System.out.println(new String(text.getBytes(StandardCharsets.UTF_16LE), StandardCharsets.UTF_16LE).length());
        wire = StandardCharsets.US_ASCII;
        System.out.println(new String(text.getBytes(wire), wire));
    }
}
