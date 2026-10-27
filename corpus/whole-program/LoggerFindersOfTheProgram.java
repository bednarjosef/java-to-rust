import java.util.ResourceBundle;

// A program's own `System.LoggerFinder`, as the JDK's `jdk.internal.logger.DefaultLoggerFinder`
// extends it: its loggers, and the localized logger that `LoggerFinder` wraps around one.
public class LoggerFindersOfTheProgram {
    static class PrintingLogger implements System.Logger {
        final String name;

        PrintingLogger(String name) {
            this.name = name;
        }

        public String getName() {
            return name;
        }

        public boolean isLoggable(Level level) {
            return level.getSeverity() >= Level.INFO.getSeverity();
        }

        public void log(Level level, ResourceBundle bundle, String message, Throwable thrown) {
            if (isLoggable(level)) {
                String cause = thrown == null ? "" : " (" + thrown.getMessage() + ")";
                System.out.println(name + " " + level + ": " + message + cause);
            }
        }

        public void log(Level level, ResourceBundle bundle, String format, Object... params) {
            if (isLoggable(level)) {
                int count = params == null ? 0 : params.length;
                System.out.println(name + " " + level + ": " + format + " with " + count + " parameters");
            }
        }
    }

    static class Finder extends System.LoggerFinder {
        int made;

        @Override
        public System.Logger getLogger(String name, Module module) {
            made++;
            return new PrintingLogger(name);
        }
    }

    public static void main(String[] args) {
        Finder finder = new Finder();
        System.Logger logger = finder.getLogger("app", null);
        logger.log(System.Logger.Level.INFO, "started");
        logger.log(System.Logger.Level.DEBUG, "hidden");
        logger.log(System.Logger.Level.WARNING, "careful", new IllegalStateException("low disk"));
        System.Logger localized = finder.getLocalizedLogger("local", null, null);
        localized.log(System.Logger.Level.ERROR, "failed");
        System.out.println(localized.getName() + " " + localized.isLoggable(System.Logger.Level.TRACE));
        System.LoggerFinder any = finder;
        System.out.println(any instanceof Finder);
        System.out.println(finder.made + " loggers made");
    }
}
