// `System.getProperty` of a local that holds a string literal and is never assigned again.
public class PropertyKeyInLocal {
    public static void main(String[] args) {
        String key = "file.separator";
        String missing = "java-to-rust.no.such.property";
        System.out.println(System.getProperty(key) + " " + System.getProperty(missing));
        System.out.println(System.getProperty(missing, "fallback"));
    }
}
