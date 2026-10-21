import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

public class CharsetValuesUnknown {
    public static void main(String[] args) {
        Charset cs = Charset.forName("UTF-8");
        System.out.println(cs.name() + " " + Charset.isSupported("bogus-" + args.length));
        String name = "no-such-" + args.length;
        System.out.println("looking up " + name);
        Charset missing = Charset.forName(name);
        System.out.println("not reached " + missing + " " + StandardCharsets.UTF_8);
    }
}
