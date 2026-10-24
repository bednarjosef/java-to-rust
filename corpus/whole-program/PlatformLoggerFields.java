import sun.util.logging.PlatformLogger;

// Loggers in static fields, as the JDK's own classes keep them: read by name, selected from their
// class, from a nested class before the outer class is initialized, and logged to with
// parameters at levels the default logger does not print.
public class PlatformLoggerFields {
    private static final String NAME = "corpus.fields";
    private static final PlatformLogger log = PlatformLogger.getLogger(NAME);
    private static PlatformLogger quiet = PlatformLogger.getLogger(NAME + ".quiet");
    static int counter = 0;

    static int next() {
        counter++;
        System.out.println("next " + counter);
        return counter;
    }

    static class Outer {
        static final PlatformLogger events = PlatformLogger.getLogger("corpus.events");
        static {
            System.out.println("Outer initialized");
        }

        static class Nested {
            static boolean fine() {
                return events.isLoggable(PlatformLogger.Level.FINE);
            }

            static void warn() {
                events.warning("nested warning");
            }
        }
    }

    public static void main(String[] args) {
        System.out.println(log.isLoggable(PlatformLogger.Level.FINEST));
        System.out.println(log.isLoggable(PlatformLogger.Level.INFO));
        System.out.println(quiet.isLoggable(PlatformLogger.Level.SEVERE));
        log.finest("{0} and {1}", next(), next());
        quiet.fine("value " + next());
        log.info("shown");
        PlatformLoggerFields.log.warning("selected");
        System.out.println("before nested");
        System.out.println(Outer.Nested.fine());
        Outer.Nested.warn();
        System.out.println(counter);
    }
}
