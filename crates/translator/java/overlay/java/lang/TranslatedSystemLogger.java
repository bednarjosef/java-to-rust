package java.lang;

import java.util.MissingResourceException;
import java.util.Objects;
import java.util.ResourceBundle;
import java.util.function.Supplier;

/**
 * The translator's {@code System.getLogger(name)}: a logger that behaves as the one a plain
 * {@code java Main} gives where nothing configures logging, which is backed by
 * {@code java.util.logging}. Its levels are those of {@code java.util.logging}, which
 * {@code ERROR} is {@code SEVERE} of, {@code DEBUG} {@code FINE} and {@code TRACE} {@code FINER};
 * its root logger and console handler pass {@code INFO} and above, and {@code OFF}, which they
 * compare as the highest level. A message passed is printed to {@code System.err} as
 * {@code LEVEL: message}, with the stack trace of a {@code Throwable} logged with it after a line
 * break and before an empty line. Java prints a line with the time and the caller before each
 * message, which a translated program has no stack to find the caller in: that line is left out.
 * A message with parameters is a {@code MessageFormat} pattern where a {@code '{'} is followed by
 * a digit, and is printed as it is where formatting it fails, as in Java. Every call gives a new
 * logger, equal to the others of its name, as Java's wrappers of the one logger of a name are.
 */
final class TranslatedSystemLogger implements System.Logger {
    private final String name;

    private TranslatedSystemLogger(String name) {
        this.name = name;
    }

    static System.Logger getLogger(String name) {
        if (name == null) {
            throw new NullPointerException();
        }
        return new TranslatedSystemLogger(name);
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public boolean isLoggable(Level level) {
        if (level == null) {
            throw new NullPointerException("Cannot invoke \"java.util.logging.Level.intValue()\" because \"level\" is null");
        }
        return level.getSeverity() >= Level.INFO.getSeverity();
    }

    @Override
    public void log(Level level, String msg) {
        if (isLoggable(level)) {
            print(level, msg, null);
        }
    }

    // The supplier and the object are checked whatever the level, as Java's wrapper checks them.
    @Override
    public void log(Level level, Supplier<String> msgSupplier) {
        Objects.requireNonNull(msgSupplier);
        if (isLoggable(level)) {
            print(level, msgSupplier.get(), null);
        }
    }

    @Override
    public void log(Level level, Object obj) {
        Objects.requireNonNull(obj);
        if (isLoggable(level)) {
            print(level, obj.toString(), null);
        }
    }

    @Override
    public void log(Level level, String msg, Throwable thrown) {
        if (isLoggable(level)) {
            print(level, msg, thrown);
        }
    }

    @Override
    public void log(Level level, Supplier<String> msgSupplier, Throwable thrown) {
        Objects.requireNonNull(msgSupplier);
        if (isLoggable(level)) {
            print(level, msgSupplier.get(), thrown);
        }
    }

    @Override
    public void log(Level level, String format, Object... params) {
        if (isLoggable(level)) {
            print(level, format(format, params), null);
        }
    }

    @Override
    public void log(Level level, ResourceBundle bundle, String msg, Throwable thrown) {
        if (isLoggable(level)) {
            print(level, localized(bundle, msg), thrown);
        }
    }

    @Override
    public void log(Level level, ResourceBundle bundle, String format, Object... params) {
        if (isLoggable(level)) {
            print(level, format(localized(bundle, format), params), null);
        }
    }

    @Override
    public boolean equals(Object obj) {
        return obj instanceof TranslatedSystemLogger other && other.name.equals(name);
    }

    @Override
    public int hashCode() {
        return name.hashCode();
    }

    /** The message that {@code bundle} has for the key {@code msg}, or the key where it has none. */
    private static String localized(ResourceBundle bundle, String msg) {
        if (bundle == null || msg == null) {
            return msg;
        }
        try {
            return bundle.getString(msg);
        } catch (MissingResourceException e) {
            return msg;
        }
    }

    /** {@code msg} with {@code params} put in, where it is a pattern of them. */
    private static String format(String msg, Object[] params) {
        if (params == null || params.length == 0 || msg == null) {
            return msg;
        }
        try {
            for (int i = msg.indexOf('{'); i >= 0 && i < msg.length() - 1; i = msg.indexOf('{', i + 1)) {
                char next = msg.charAt(i + 1);
                if (next >= '0' && next <= '9') {
                    return java.text.MessageFormat.format(msg, params);
                }
            }
            return msg;
        } catch (Exception e) {
            return msg;
        }
    }

    private static void print(Level level, String msg, Throwable thrown) {
        String name = level == Level.ERROR ? "SEVERE" : level.name();
        System.err.println(name + ": " + msg);
        if (thrown != null) {
            thrown.printStackTrace();
            System.err.println();
        }
    }
}
