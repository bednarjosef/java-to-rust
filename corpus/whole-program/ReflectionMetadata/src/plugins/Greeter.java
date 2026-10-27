package plugins;

public class Greeter {
    private static int made;

    public Greeter() {
        made++;
    }

    public static int made() {
        return made;
    }

    public String greet(String who) {
        return "hello " + who;
    }
}
