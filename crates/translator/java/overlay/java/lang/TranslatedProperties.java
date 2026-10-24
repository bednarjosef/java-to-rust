package java.lang;

import java.util.Properties;

/**
 * The translator's {@code System.getProperties()}: the properties of a plain {@code java Main}
 * run, each read with {@code System.getProperty}, which the translation answers as it answers any
 * read of a property (see {@code lower::system}). A key that the JDK does not set has no entry.
 * The object is made once, so that every caller sees the same one, as with the JVM's.
 */
final class TranslatedProperties {
    private static Properties all;

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
            all = p;
        }
        return all;
    }

    private static void put(Properties p, String key, String value) {
        if (value != null) {
            p.setProperty(key, value);
        }
    }
}
