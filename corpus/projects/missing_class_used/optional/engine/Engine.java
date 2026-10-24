package engine;

/** An engine that the library uses when it is installed; the program runs without it. */
public class Engine {
    public static final int LIMIT = 4;
    public static int count = 3;
    public int size = 2;

    public static String version() {
        return "1.0";
    }

    public static Engine make() {
        return new Engine();
    }

    public String name() {
        return "engine";
    }

    public static class Mode {
        public static int value() {
            return 1;
        }
    }

    public static class Failure extends RuntimeException {
    }

    public interface Listener {
        void heard(String s);
    }

    public abstract static class Base {
        public abstract String base();
    }
}
