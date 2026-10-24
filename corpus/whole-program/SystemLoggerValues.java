import java.util.ListResourceBundle;
import java.util.ResourceBundle;

// System.Logger values, as the JDK's classes hold them: in fields, created lazily, handed out
// by a method, passed as a parameter and compared, and logged to at every level with each of the
// interface's methods. What the loggers print goes to standard error, which is compared by hand;
// standard output says what the program sees of them.
public class SystemLoggerValues {
    private static final System.Logger LOG = System.getLogger("corpus.system.static");
    private static System.Logger lazy;
    private final System.Logger own = System.getLogger("corpus.system.own");
    private System.Logger unset;

    static System.Logger lazyLog() {
        if (lazy == null) {
            lazy = System.getLogger("corpus.system.lazy");
        }
        return lazy;
    }

    static void report(System.Logger log, String what) {
        if (log != null && log.isLoggable(System.Logger.Level.INFO)) {
            log.log(System.Logger.Level.INFO, "report " + what);
        }
    }

    static int calls = 0;

    static String counted() {
        calls++;
        return "counted " + calls;
    }

    public static class Messages extends ListResourceBundle {
        @Override
        protected Object[][] getContents() {
            return new Object[][] {{"greeting", "hello {0}"}, {"plain", "localized plain"}};
        }
    }

    public static void main(String[] args) {
        System.out.println(LOG.getName());
        System.out.println(lazy == null);
        System.Logger first = lazyLog();
        System.out.println(first == lazyLog());
        System.out.println(first.equals(System.getLogger("corpus.system.lazy")));
        System.out.println(first.equals(System.getLogger("corpus.system.other")));
        System.out.println(first.hashCode() == System.getLogger("corpus.system.lazy").hashCode());
        report(first, "passed");
        report(null, "never");
        SystemLoggerValues v = new SystemLoggerValues();
        System.out.println(v.unset == null);
        for (System.Logger.Level level : System.Logger.Level.values()) {
            System.out.println(level + " " + level.getName() + " " + level.getSeverity() + " " + v.own.isLoggable(level));
            v.own.log(level, "at " + level);
        }
        v.own.log(System.Logger.Level.DEBUG, counted());
        v.own.log(System.Logger.Level.INFO, counted());
        v.own.log(System.Logger.Level.TRACE, () -> counted());
        v.own.log(System.Logger.Level.WARNING, () -> counted());
        v.own.log(System.Logger.Level.DEBUG, (Object) 41);
        v.own.log(System.Logger.Level.ERROR, (Object) 42);
        System.out.println(calls);
        LOG.log(System.Logger.Level.ERROR, "with a {0} and {1}", "parameter", 2);
        LOG.log(System.Logger.Level.INFO, "no pattern", "unused");
        LOG.log(System.Logger.Level.INFO, "a bad {0 pattern", "unused");
        LOG.log(System.Logger.Level.DEBUG, "hidden {0}", counted());
        System.out.println(calls);
        LOG.log(System.Logger.Level.WARNING, "with a throwable", new IllegalStateException("bad state"));
        LOG.log(System.Logger.Level.WARNING, "with no throwable", (Throwable) null);
        LOG.log(System.Logger.Level.ERROR, () -> "supplied", new IllegalArgumentException("bad argument"));
        LOG.log(System.Logger.Level.INFO, (String) null);
        ResourceBundle bundle = new Messages();
        LOG.log(System.Logger.Level.INFO, bundle, "greeting", "world");
        LOG.log(System.Logger.Level.INFO, bundle, "plain", (Throwable) null);
        LOG.log(System.Logger.Level.INFO, bundle, "missing", (Throwable) null);
        LOG.log(System.Logger.Level.INFO, (ResourceBundle) null, "no bundle {0}", 7);
        try {
            v.unset.log(System.Logger.Level.INFO, "on null");
        } catch (NullPointerException e) {
            System.out.println("null logger");
        }
        try {
            LOG.isLoggable(null);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            LOG.log(System.Logger.Level.TRACE, (java.util.function.Supplier<String>) null);
        } catch (NullPointerException e) {
            System.out.println("null supplier " + e.getMessage());
        }
        try {
            LOG.log(System.Logger.Level.TRACE, (Object) null);
        } catch (NullPointerException e) {
            System.out.println("null object " + e.getMessage());
        }
        try {
            System.getLogger(null);
        } catch (NullPointerException e) {
            System.out.println("null name " + e.getMessage());
        }
    }
}
