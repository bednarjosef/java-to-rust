package store;

import engine.Engine;

/** The ways code can name a missing class: each throws when it runs, and only then. */
public class Uses {
    static int calls = 0;
    private Engine current;
    private Box<Engine> engines = new Box<>();
    private Engine[] array;

    public String engineName() {
        if (current == null) {
            return "no engine " + engines.size() + " " + (array == null);
        }
        return current.name();
    }

    public static int qualified() {
        return engine.Engine.count + calls;
    }

    public static int nested(boolean go) {
        int before = calls;
        if (go) {
            return Engine.Mode.value();
        }
        return before;
    }

    public static boolean test(Object o) {
        return o instanceof Engine;
    }

    public static String select(int k) {
        return switch (k) {
            case 1 -> Engine.make().name();
            default -> "other " + k;
        };
    }

    public static Engine.Listener listener() {
        return s -> System.out.println("heard " + s);
    }

    public static int sizeOf(Engine e) {
        return e == null ? 0 : e.size;
    }
}
