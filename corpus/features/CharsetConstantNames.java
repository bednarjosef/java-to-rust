import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

// `Charset.forName` of a name that is a constant the runtime knows is that charset itself.
public class CharsetConstantNames {
    public static void main(String[] args) {
        Charset utf8 = Charset.forName("UTF-8");
        System.out.println(utf8 == StandardCharsets.UTF_8);
        System.out.println(Charset.forName("latin1").name() + " " + Charset.forName("ascii").displayName());
        byte[] bytes = "naïve".getBytes(Charset.forName("UTF-16BE"));
        System.out.println(bytes.length + " " + new String(bytes, Charset.forName("utf-16be")));
    }
}
