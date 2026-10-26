package sun.security.jca;

/**
 * The translator's patch of the JDK's {@code ProviderConfig} (see docs/jdk-overlays.md). A
 * provider that the {@code java.security} file names and that is not one of the JDK's own is
 * found by {@code ServiceLoader}, among the providers that the translation's modules declare; the
 * JDK also loads one by its class name, by reflection, which a closed translation has nothing
 * for. Nor does it have the macOS {@code Apple} provider.
 */
class ProviderConfig {
    @Edit(from = "case \"Apple\", \"apple.security.AppleProvider\" -> {", to = "yield ap; }",
            replace = "case \"Apple\", \"apple.security.AppleProvider\" -> null;")
    Provider getProvider();

    @Patch
    private static final class ProviderLoader {
        @Replace
        private Provider legacyLoad(String classname) {
            return null;
        }
    }
}
