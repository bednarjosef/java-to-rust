package store;

import engine.Engine;
import engine.Event;

/** Finds out in its static initializer whether the engine is there, as libraries do. */
public class Probe {
    private static final Throwable CAUSE;
    private static Engine instance;

    static {
        Throwable cause = null;
        try {
            System.out.println("loading " + Engine.version());
        } catch (Throwable t) {
            cause = t;
        }
        CAUSE = cause;
        if (cause == null) {
            instance = Engine.make();
            System.out.println("loaded " + instance.name());
        }
    }

    public static boolean available() {
        return CAUSE == null;
    }

    public static String cause() {
        return String.valueOf(CAUSE);
    }

    public static String describe(boolean use) {
        String prefix = "described";
        if (use) {
            Engine engine = Engine.make();
            return prefix + " " + engine.name();
        }
        return prefix + " without it";
    }

    public static int count(Engine engine) {
        if (engine == null) {
            return -1;
        }
        return engine.name().length();
    }

    public static Runnable later() {
        return () -> System.out.println(Engine.version());
    }

    public static Object event() {
        Event e = () -> System.out.println("fired");
        return e;
    }
}
