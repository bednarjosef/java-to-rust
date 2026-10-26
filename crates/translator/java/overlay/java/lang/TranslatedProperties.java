package java.lang;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Properties;

/**
 * The translator's {@code System.getProperties()}: the properties of a plain {@code java Main}
 * run, each read with {@code System.getProperty}, which the translation answers as it answers any
 * read of a property (see {@code lower::system}). A key that the JDK does not set has no entry.
 * The object is made once, so that every caller sees the same one, as with the JVM's.
 *
 * <p>And the translator's {@code System.setProperty} and {@code clearProperty}, which keep what
 * they change here. A program that calls one of them reads every property through {@link #read},
 * which answers a changed key with its new value.
 */
final class TranslatedProperties {
    private static Properties all;

    /**
     * The keys that {@code setProperty} and {@code clearProperty} have changed, with their new
     * values, null for a cleared one. (A {@code LinkedHashMap}, so that its calls reach only that
     * class's methods.)
     */
    private static final LinkedHashMap<String, String> CHANGED = new LinkedHashMap<>();

    private TranslatedProperties() {}

    static synchronized Properties all() {
        if (all == null) {
            Properties p = new Properties();
            put(p, "file.encoding", System.getProperty("file.encoding"));
            put(p, "file.separator", System.getProperty("file.separator"));
            put(p, "java.awt.headless", System.getProperty("java.awt.headless"));
            put(p, "java.class.path", System.getProperty("java.class.path"));
            put(p, "java.class.version", System.getProperty("java.class.version"));
            put(p, "java.home", System.getProperty("java.home"));
            put(p, "java.io.tmpdir", System.getProperty("java.io.tmpdir"));
            put(p, "java.library.path", System.getProperty("java.library.path"));
            put(p, "java.runtime.name", System.getProperty("java.runtime.name"));
            put(p, "java.runtime.version", System.getProperty("java.runtime.version"));
            put(p, "java.specification.name", System.getProperty("java.specification.name"));
            put(p, "java.specification.vendor", System.getProperty("java.specification.vendor"));
            put(p, "java.specification.version", System.getProperty("java.specification.version"));
            put(p, "java.vendor", System.getProperty("java.vendor"));
            put(p, "java.vendor.url", System.getProperty("java.vendor.url"));
            put(p, "java.vendor.url.bug", System.getProperty("java.vendor.url.bug"));
            put(p, "java.version", System.getProperty("java.version"));
            put(p, "java.version.date", System.getProperty("java.version.date"));
            put(p, "java.vm.compressedOopsMode", System.getProperty("java.vm.compressedOopsMode"));
            put(p, "java.vm.info", System.getProperty("java.vm.info"));
            put(p, "java.vm.name", System.getProperty("java.vm.name"));
            put(p, "java.vm.specification.name", System.getProperty("java.vm.specification.name"));
            put(p, "java.vm.specification.vendor", System.getProperty("java.vm.specification.vendor"));
            put(p, "java.vm.specification.version", System.getProperty("java.vm.specification.version"));
            put(p, "java.vm.vendor", System.getProperty("java.vm.vendor"));
            put(p, "java.vm.version", System.getProperty("java.vm.version"));
            put(p, "jdk.debug", System.getProperty("jdk.debug"));
            put(p, "line.separator", System.getProperty("line.separator"));
            put(p, "native.encoding", System.getProperty("native.encoding"));
            put(p, "os.arch", System.getProperty("os.arch"));
            put(p, "os.name", System.getProperty("os.name"));
            put(p, "os.version", System.getProperty("os.version"));
            put(p, "path.separator", System.getProperty("path.separator"));
            put(p, "stderr.encoding", System.getProperty("stderr.encoding"));
            put(p, "stdin.encoding", System.getProperty("stdin.encoding"));
            put(p, "stdout.encoding", System.getProperty("stdout.encoding"));
            put(p, "sun.arch.data.model", System.getProperty("sun.arch.data.model"));
            put(p, "sun.boot.library.path", System.getProperty("sun.boot.library.path"));
            put(p, "sun.cpu.endian", System.getProperty("sun.cpu.endian"));
            put(p, "sun.io.unicode.encoding", System.getProperty("sun.io.unicode.encoding"));
            put(p, "sun.java.launcher", System.getProperty("sun.java.launcher"));
            put(p, "sun.jnu.encoding", System.getProperty("sun.jnu.encoding"));
            put(p, "sun.management.compiler", System.getProperty("sun.management.compiler"));
            put(p, "user.country", System.getProperty("user.country"));
            put(p, "user.dir", System.getProperty("user.dir"));
            put(p, "user.home", System.getProperty("user.home"));
            put(p, "user.language", System.getProperty("user.language"));
            put(p, "user.name", System.getProperty("user.name"));
            for (Map.Entry<String, String> change : CHANGED.entrySet()) {
                if (change.getValue() == null) {
                    p.remove(change.getKey());
                } else {
                    p.setProperty(change.getKey(), change.getValue());
                }
            }
            all = p;
        }
        return all;
    }

    /**
     * {@code System.getProperty(key, def)} where properties change: {@code value} is what the
     * translation answers for a key that no call has changed.
     */
    static synchronized String read(String key, String value, String def) {
        if (CHANGED.containsKey(key)) {
            value = CHANGED.get(key);
        }
        return value != null ? value : def;
    }

    /** {@code System.setProperty(key, value)}: the property's value before. */
    static synchronized String setProperty(String key, String value) {
        return change(key, value, false);
    }

    /** {@code System.clearProperty(key)}: the property's value before. */
    static synchronized String clearProperty(String key) {
        return change(key, null, true);
    }

    // The key is checked here, before it is read, which the read's switch over the keys of a plain
    // run then knows is not null.
    private static String change(String key, String value, boolean clearing) {
        if (key == null) {
            throw new NullPointerException("key can't be null");
        }
        if (key.isEmpty()) {
            throw new IllegalArgumentException("key can't be empty");
        }
        if (value == null && !clearing) {
            throw new NullPointerException();
        }
        String before = read(key, System.getProperty(key), null);
        CHANGED.put(key, value);
        Properties p = all;
        if (p != null) {
            if (value == null) {
                p.remove(key);
            } else {
                p.setProperty(key, value);
            }
        }
        return before;
    }

    private static void put(Properties p, String key, String value) {
        if (value != null) {
            p.setProperty(key, value);
        }
    }
}
