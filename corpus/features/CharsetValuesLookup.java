import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

public class CharsetValuesLookup {
    static Charset lookup(String name) {
        return Charset.forName(name);
    }

    public static void main(String[] args) {
        String[] names = {"utf8", "UTF8", "latin1", "ASCII", "utf-16be", "Unicode", "ISO8859_1", "us-ascii"};
        for (String name : names) {
            Charset cs = Charset.forName(name);
            System.out.println(name + " -> " + cs.name() + " " + cs.displayName());
        }
        System.out.println(Charset.isSupported("latin1") + " " + Charset.isSupported("nope"));
        System.out.println(Charset.isSupported("UTF8") + " " + Charset.isSupported("UTF-16LE"));
        Charset dflt = Charset.defaultCharset();
        System.out.println(dflt.name() + " " + (dflt == StandardCharsets.UTF_8));
        System.out.println(lookup("ascii") == StandardCharsets.US_ASCII);
        System.out.println(lookup("UTF-8").equals(StandardCharsets.UTF_8));
        System.out.println(lookup("utf-16").hashCode() == StandardCharsets.UTF_16.hashCode());
        byte[] latin = "café".getBytes(lookup("latin1"));
        System.out.println(latin.length + " " + new String(latin, lookup("ISO-8859-1")));
        String back = new String("hi".getBytes(lookup("UTF-16LE")), lookup("utf-16le"));
        System.out.println(back + " " + back.length());
    }
}
