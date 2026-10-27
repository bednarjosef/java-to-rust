// Charsets held as objects: an argument of `Object...` (log4j's `PropertiesUtil` logs its default
// charset so), `Objects.requireNonNull`, elements of a list and keys of a map, `instanceof` and
// casts back. A charset's `equals`, `hashCode` and `toString` are its name's.
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

public class CharsetsAsObjects {
    static String describe(String format, Object... args) {
        StringBuilder sb = new StringBuilder(format);
        for (Object a : args) {
            sb.append(" [").append(a).append(']');
        }
        return sb.toString();
    }

    static Charset charsetProperty(String name, Charset defaultValue) {
        String value = System.getProperty(name);
        if (value == null) {
            return defaultValue;
        }
        if (Charset.isSupported(value)) {
            return Charset.forName(value);
        }
        System.out.println(describe("Unable to read charset", value, name, defaultValue));
        return defaultValue;
    }

    public static void main(String[] args) {
        Charset utf8 = Objects.requireNonNull(StandardCharsets.UTF_8, "charset");
        System.out.println(describe("charsets", utf8, StandardCharsets.ISO_8859_1));
        System.setProperty("my.charset", "no-such-charset");
        System.out.println(charsetProperty("my.charset", StandardCharsets.US_ASCII));
        System.out.println(charsetProperty("other.charset", StandardCharsets.UTF_16LE));
        Object o = StandardCharsets.UTF_16BE;
        System.out.println(o instanceof Charset);
        System.out.println(o instanceof String);
        Charset back = (Charset) o;
        System.out.println(back.name() + " " + back.equals(StandardCharsets.UTF_16BE) + " " + o.equals(Charset.forName("UTF-16BE")));
        System.out.println(o.hashCode() == "UTF-16BE".hashCode());
        System.out.println(o.toString());
        List<Object> things = new ArrayList<>();
        things.add("text");
        things.add(StandardCharsets.UTF_8);
        things.add(42);
        System.out.println(things + " " + things.contains(Charset.forName("utf8")));
        Map<Object, String> names = new HashMap<>();
        names.put(StandardCharsets.UTF_8, "eight");
        System.out.println(names.get(Charset.forName("UTF-8")));
        try {
            String s = (String) things.get(1);
            System.out.println(s);
        } catch (ClassCastException e) {
            System.out.println("not a string");
        }
    }
}
