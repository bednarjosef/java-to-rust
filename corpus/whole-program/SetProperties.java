public class SetProperties {
    static String key(String suffix) {
        return "app." + suffix;
    }

    public static void main(String[] args) {
        System.out.println("before: " + System.getProperty("app.mode") + " " + System.getProperty("app.mode", "fallback"));
        System.out.println("previous: " + System.setProperty("app.mode", "server"));
        System.out.println("after: " + System.getProperty("app.mode") + " " + System.getProperty("app.mode", "fallback"));
        System.out.println("computed key: " + System.getProperty(key("mode")));
        System.out.println("previous: " + System.setProperty("line.separator", "|"));
        System.out.println("separator: " + System.getProperty("line.separator"));
        System.out.println("cleared: " + System.clearProperty("app.mode"));
        System.out.println("after clear: " + System.getProperty("app.mode", "fallback"));
        System.out.println("in all: " + System.getProperties().getProperty("line.separator"));
        try {
            System.setProperty("", "x");
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.setProperty(null, "x");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.setProperty("app.mode", null);
        } catch (NullPointerException e) {
            System.out.println("null value: " + e.getClass().getName());
        }
    }
}
