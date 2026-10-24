import sun.util.logging.PlatformLogger;

// Loggers as values, as the JDK's classes hold them besides static fields: created lazily in a
// field that starts null, handed out by a method, passed as a parameter, compared, and logged to
// with parameters and a Throwable. What the loggers print goes to standard error, which is
// compared by hand; standard output says what the program sees of them.
public class PlatformLoggerValues {
    private static PlatformLogger lazy;
    private final PlatformLogger own = PlatformLogger.getLogger("corpus.values.own");
    private PlatformLogger unset;

    static PlatformLogger lazyLog() {
        if (lazy == null) {
            lazy = PlatformLogger.getLogger("corpus.values.lazy");
        }
        return lazy;
    }

    static void report(PlatformLogger log, String what) {
        if (log != null && log.isLoggable(PlatformLogger.Level.INFO)) {
            log.info("report " + what);
        }
    }

    static int calls = 0;

    static String counted() {
        calls++;
        return "counted " + calls;
    }

    public static void main(String[] args) {
        System.out.println(lazy == null);
        PlatformLogger first = lazyLog();
        System.out.println(lazy != null);
        System.out.println(first == lazyLog());
        System.out.println(first == PlatformLogger.getLogger("corpus.values.lazy"));
        System.out.println(first == PlatformLogger.getLogger("corpus.values.other"));
        System.out.println(first.getName());
        System.out.println(first.isEnabled());
        System.out.println(first.level());
        report(first, "passed");
        report(null, "never");
        PlatformLoggerValues v = new PlatformLoggerValues();
        System.out.println(v.unset == null);
        v.own.warning("own warning");
        v.own.fine(counted());
        v.own.info(counted());
        System.out.println(calls);
        lazyLog().severe("with a {0} and {1}", "parameter", 2);
        lazyLog().info("no pattern", "unused");
        lazyLog().finest("hidden {0}", counted());
        System.out.println(calls);
        lazyLog().warning("with a throwable", new IllegalStateException("bad state"));
        lazyLog().warning("with no throwable", (Throwable) null);
        for (PlatformLogger.Level level : PlatformLogger.Level.values()) {
            System.out.println(level + " " + first.isLoggable(level) + " " + level.intValue());
        }
        try {
            v.unset.info("on null");
        } catch (NullPointerException e) {
            System.out.println("null logger");
        }
        try {
            PlatformLogger.getLogger(null);
        } catch (NullPointerException e) {
            System.out.println("null name");
        }
    }
}
