package sun.util.logging;

import java.util.MissingResourceException;
import java.util.ResourceBundle;
import java.util.function.Supplier;
import sun.util.logging.PlatformLogger.Level;

/**
 * The backend of every {@code PlatformLogger} of a translated program (see the patch of
 * {@code PlatformLogger}): Java's default one, where nothing configures logging. It prints the
 * messages of level {@code INFO} and above to {@code System.err} as {@code LEVEL: message}, with
 * the stack trace of a {@code Throwable} logged with one after a line break and before an empty
 * line, and hides the others. Java's backend prints a line with the time and the caller before
 * each message, which a translated program has no stack to find the caller in: that line is left
 * out. A message with parameters is a {@code MessageFormat} pattern where a {@code '{'} is
 * followed by a digit, and is printed as it is where formatting it fails, as in Java. The source
 * class and method that {@code logp} names are only in that line.
 */
final class TranslatedConsoleLogger implements PlatformLogger.Bridge {
    private final String name;

    TranslatedConsoleLogger(String name) {
        this.name = name;
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public boolean isLoggable(Level level) {
        return level != Level.OFF && level.ordinal() >= Level.INFO.ordinal();
    }

    @Override
    public boolean isEnabled() {
        return true;
    }

    @Override
    public void log(Level level, String msg) {
        if (isLoggable(level)) {
            print(level, msg, null);
        }
    }

    @Override
    public void log(Level level, String msg, Throwable thrown) {
        if (isLoggable(level)) {
            print(level, msg, thrown);
        }
    }

    @Override
    public void log(Level level, String msg, Object... params) {
        if (isLoggable(level)) {
            print(level, format(msg, params), null);
        }
    }

    @Override
    public void log(Level level, Supplier<String> msgSupplier) {
        if (isLoggable(level)) {
            print(level, msgSupplier.get(), null);
        }
    }

    @Override
    public void log(Level level, Throwable thrown, Supplier<String> msgSupplier) {
        if (isLoggable(level)) {
            print(level, msgSupplier.get(), thrown);
        }
    }

    @Override
    public void logp(Level level, String sourceClass, String sourceMethod, String msg) {
        log(level, msg);
    }

    @Override
    public void logp(Level level, String sourceClass, String sourceMethod, Supplier<String> msgSupplier) {
        log(level, msgSupplier);
    }

    @Override
    public void logp(Level level, String sourceClass, String sourceMethod, String msg, Object... params) {
        log(level, msg, params);
    }

    @Override
    public void logp(Level level, String sourceClass, String sourceMethod, String msg, Throwable thrown) {
        log(level, msg, thrown);
    }

    @Override
    public void logp(Level level, String sourceClass, String sourceMethod, Throwable thrown,
            Supplier<String> msgSupplier) {
        log(level, thrown, msgSupplier);
    }

    @Override
    public void logrb(Level level, String sourceClass, String sourceMethod, ResourceBundle bundle, String msg,
            Object... params) {
        logrb(level, bundle, msg, params);
    }

    @Override
    public void logrb(Level level, String sourceClass, String sourceMethod, ResourceBundle bundle, String msg,
            Throwable thrown) {
        logrb(level, bundle, msg, thrown);
    }

    @Override
    public void logrb(Level level, ResourceBundle bundle, String msg, Object... params) {
        if (isLoggable(level)) {
            print(level, format(localized(bundle, msg), params), null);
        }
    }

    @Override
    public void logrb(Level level, ResourceBundle bundle, String msg, Throwable thrown) {
        if (isLoggable(level)) {
            print(level, localized(bundle, msg), thrown);
        }
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
        System.err.println(level.name() + ": " + msg);
        if (thrown != null) {
            thrown.printStackTrace();
            System.err.println();
        }
    }
}
