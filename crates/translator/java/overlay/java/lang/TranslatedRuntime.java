package java.lang;

/**
 * The translator's {@code Runtime.version()}: the version of the JDK that a plain run has, parsed
 * from its {@code java.runtime.version} property, which the translation answers with the value of
 * the JDK it ran with (see {@code lower::system}), as the JVM makes it from the same numbers. The
 * version is made once, so that every caller sees the same one.
 */
final class TranslatedRuntime {
    private static Runtime.Version version;

    private TranslatedRuntime() {}

    static synchronized Runtime.Version version() {
        if (version == null) {
            version = Runtime.Version.parse(System.getProperty("java.runtime.version"));
        }
        return version;
    }
}
