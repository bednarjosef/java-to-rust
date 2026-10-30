import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

// `Charset.class`, as a proxy of `Arena` names it to look up `allocateFrom(String, Charset)`: the
// class of every charset.
public class CharsetClassLiteral {
    public static void main(String[] args) throws Exception {
        Class<?> c = Charset.class;
        System.out.println(c.getName());
        System.out.println(c.getSimpleName());
        System.out.println(c.isInstance(StandardCharsets.UTF_8));
        System.out.println(c.isInstance("text"));
        System.out.println(c.isInstance(Charset.forName("ISO-8859-1")));
        System.out.println(Charset.class.cast(StandardCharsets.US_ASCII).name());
    }
}
