package lib;

import java.util.ArrayList;
import java.util.List;

public class Greeter {
    private static int made;
    private static String banner = "never read";
    private String prefix = "";
    private double[] cache = new double[0];

    public Greeter(String prefix) {
        this.prefix = prefix;
        made++;
    }

    /** Never called: lists and generics are not supported yet, so translating it would fail. */
    public Greeter(List<String> parts) {
        this(String.join(" ", parts));
    }

    public String greet(String name) {
        return prefix + ", " + name;
    }

    public static int count() {
        return made;
    }

    /** Never called. */
    public List<String> history() {
        return new ArrayList<>();
    }

    /** A second entry point, which is not the program's. */
    public static void main(String[] args) {
        System.out.println(banner);
    }
}
