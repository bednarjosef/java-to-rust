package sun.util.logging;

import java.util.Objects;

/**
 * The translator's patch of the JDK's {@code PlatformLogger} (see docs/jdk-overlays.md), the
 * logger of the JDK's own classes. Java's logger finds its backend through
 * {@code jdk.internal.logger}'s lazy loggers and the {@code System.LoggerFinder} services, which
 * a translated program does not have; nothing configures logging there, so every logger behaves
 * as the default backend does, which {@code TranslatedConsoleLogger} is. A logger is cached by its
 * name, as in Java, so that {@code getLogger} gives the same logger for the same name; Java holds
 * it weakly, and a logger that nothing holds any more cannot be told apart from a new one. Each
 * level is without the {@code System.Logger.Level} it maps to, a type of the runtime's.
 */
@Edit(from = "ALL(System.Logger.Level.ALL)", to = "OFF(System.Logger.Level.OFF);",
        replace = "ALL, FINEST, FINER, FINE, CONFIG, INFO, WARNING, SEVERE, OFF;")
class PlatformLogger {
    @Patch
    public static enum Level {
        // An enum's body starts with its constants; the patch names none.
        ;

        @Remove
        final System.Logger.Level systemLevel;

        @Remove
        Level(System.Logger.Level systemLevel);

        @Replace
        public System.Logger.Level systemLevel() {
            return switch (this) {
                case ALL -> System.Logger.Level.ALL;
                case FINEST, FINER -> System.Logger.Level.TRACE;
                case FINE, CONFIG -> System.Logger.Level.DEBUG;
                case INFO -> System.Logger.Level.INFO;
                case WARNING -> System.Logger.Level.WARNING;
                case SEVERE -> System.Logger.Level.ERROR;
                case OFF -> System.Logger.Level.OFF;
            };
        }
    }

    @Replace
    private static final Map<String, PlatformLogger> loggers = new HashMap<>();

    @Replace
    public static synchronized PlatformLogger getLogger(String name) {
        // Java's default backend refuses a null name, without a message.
        Objects.requireNonNull(name);
        PlatformLogger log = loggers.get(name);
        if (log == null) {
            log = new PlatformLogger(new TranslatedConsoleLogger(name));
            loggers.put(name, log);
        }
        return log;
    }

    /**
     * Setting a level changes which messages the backend prints in ways that depend on the
     * backend's configuration and on the logger's parents, so it stays a native the translation
     * does not have, rejected where it can run.
     */
    @Replace
    @Deprecated
    public void setLevel(Level newLevel) {
        setLevel0(newLevel);
    }

    @Add
    private static native void setLevel0(Level newLevel);
}
