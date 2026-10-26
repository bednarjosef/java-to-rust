package java.security;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code KeyPairGenerator} (see docs/jdk-overlays.md). The JDK
 * looks a generator up among the registered security providers, which make it by reflection from
 * a class name; the translation has no providers, so {@code getInstance(String)} makes the
 * generators of the JDK's own providers directly ({@code TranslatedServices}). A generator so made
 * has no {@code Provider} object: {@code getProvider()} is null. The other {@code getInstance}
 * methods, which name a provider, stay rejected.
 */
class KeyPairGenerator {
    @Edit(from = "Iterator<Service> t", to = "throw failure;", replace = """
            return new Delegate(TranslatedServices.keyPairGenerator(algorithm), algorithm);
            """)
    public static KeyPairGenerator getInstance(String algorithm);

    @Patch
    private static final class Delegate {
        // A generator made by name has no other provider's to fail over to.
        @Replace
        private KeyPairGeneratorSpi nextSpi(KeyPairGeneratorSpi oldSpi, boolean reinit) {
            return null;
        }
    }
}
