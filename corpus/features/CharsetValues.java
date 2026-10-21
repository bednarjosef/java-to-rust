import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

public class CharsetValues {
    static final Charset DEFAULT = StandardCharsets.UTF_8;
    static Charset chosen = StandardCharsets.ISO_8859_1;

    static Charset pick(boolean latin) {
        return latin ? StandardCharsets.ISO_8859_1 : StandardCharsets.UTF_16LE;
    }

    static String describe(Charset cs) {
        return cs.name() + " " + cs.displayName() + " " + cs;
    }

    static boolean same(Charset a, Charset b) {
        return a == b;
    }

    public static void main(String[] args) {
        Charset utf = StandardCharsets.UTF_8;
        Charset ascii = StandardCharsets.US_ASCII;
        System.out.println(utf + " " + ascii.name());
        System.out.println(utf == DEFAULT);
        System.out.println(utf != ascii);
        System.out.println(utf.equals(StandardCharsets.UTF_8));
        System.out.println(utf.equals(ascii));
        System.out.println(utf.hashCode() == StandardCharsets.UTF_8.hashCode());
        System.out.println(describe(chosen));
        System.out.println(describe(pick(false)));
        System.out.println(same(pick(true), StandardCharsets.ISO_8859_1));
        System.out.println(same(utf, ascii));
        chosen = StandardCharsets.UTF_16BE;
        System.out.println(describe(chosen));
        System.out.println(StandardCharsets.UTF_16.name() + " " + StandardCharsets.UTF_16.displayName());
        System.out.println(StandardCharsets.UTF_16BE.hashCode() + " " + StandardCharsets.UTF_16LE.name());
        Charset copy = chosen;
        chosen = StandardCharsets.US_ASCII;
        System.out.println(copy.name() + " " + chosen.name());
    }
}
