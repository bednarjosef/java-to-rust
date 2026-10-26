package javax.crypto;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code KeyGenerator} (see docs/jdk-overlays.md): the
 * translation has no providers, so {@code getInstance(String)} makes the generators of the JDK's
 * own providers directly ({@code TranslatedServices}), with no other provider's to fail over to.
 * A generator so made has no {@code Provider} object: {@code getProvider()} is null. The other
 * {@code getInstance} methods, which name a provider, stay rejected.
 */
class KeyGenerator {
    @Edit(find = "return new KeyGenerator(algorithm);", replace = """
            return new KeyGenerator(TranslatedServices.keyGenerator(algorithm), null, algorithm);
            """)
    public static final KeyGenerator getInstance(String algorithm);

    @Replace
    private KeyGeneratorSpi nextSpi(KeyGeneratorSpi oldSpi, boolean reinit) {
        return null;
    }
}
