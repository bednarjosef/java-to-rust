package store;

import engine.Engine;

/** Catches an exception of the missing engine: the JVM cannot load the class at all. */
public class Catcher {
    static int count = 5;

    public static String caught() {
        try {
            System.out.println("in try");
            return "fine";
        } catch (Engine.Failure f) {
            return "failed";
        }
    }

    public static int count() {
        return count;
    }
}
