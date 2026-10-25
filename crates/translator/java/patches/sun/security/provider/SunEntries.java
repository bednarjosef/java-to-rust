package sun.security.provider;

/**
 * The translator's patch of the JDK's {@code SunEntries} (see docs/jdk-overlays.md), of which
 * only the choice of the default secure random generator and its seed source are translated,
 * not the provider's registrations. The source is the {@code securerandom.source} security
 * property unless the {@code java.security.egd} system property sets it; the translation has no
 * security properties, so it is the value that the JDK's {@code java.security} file gives it.
 */
class SunEntries {
    @Edit(find = "Security.getProperty(PROP_RNDSOURCE)", replace = "URL_DEV_RANDOM")
    private static String getOverridableSeedSource();
}
