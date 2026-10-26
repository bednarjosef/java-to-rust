import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Properties;
import java.util.TreeMap;

// Properties written as XML, where a catch prints what it catches: the JDK's XML writer names the
// charset it was given (`_charset.name()`), a call of a charset that may be null.
public class PropertiesXmlCaught {
    public static void main(String[] args) {
        try {
            Properties p = new Properties();
            p.setProperty("name", "<value> & more");
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            p.storeToXML(out, "a comment");
            String xml = out.toString(StandardCharsets.UTF_8);
            System.out.println(xml.contains("<entry key=\"name\">&lt;value&gt; &amp; more</entry>"));
            out.reset();
            p.storeToXML(out, null, StandardCharsets.ISO_8859_1);
            System.out.println(out.toString(StandardCharsets.ISO_8859_1).startsWith("<?xml version=\"1.0\" encoding=\"ISO-8859-1\""));
            Properties back = new Properties();
            back.loadFromXML(new ByteArrayInputStream(xml.getBytes(StandardCharsets.UTF_8)));
            System.out.println(new TreeMap<>(back));
        } catch (Exception e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
    }
}
