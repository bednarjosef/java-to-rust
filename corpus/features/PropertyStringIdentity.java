// `==` on system properties: Java keeps one string for each key, so two reads of the same key
// are the same object, and a property is not the literal of the same text.
public class PropertyStringIdentity {
    public static void main(String[] args) {
        String a = System.getProperty("line.separator");
        String b = System.getProperty("line.separator");
        System.out.println(a == b);
        System.out.println(a == System.lineSeparator());
        String home = System.getProperty("java.home");
        System.out.println(home == System.getProperty("java.home"));
        System.out.println(System.getProperty("file.separator") == "/");
        String missing = System.getProperty("no.such.property");
        System.out.println(missing == System.getProperty("no.such.property"));
        String fallback = "fallback";
        System.out.println(System.getProperty("no.such.property", fallback) == fallback);
        System.out.println(System.getProperty("line.separator", fallback) == a);
    }
}
