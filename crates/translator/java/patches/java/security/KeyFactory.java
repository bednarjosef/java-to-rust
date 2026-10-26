package java.security;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code KeyFactory} (see docs/jdk-overlays.md): the
 * translation has no providers, so {@code getInstance(String)} makes the factories of the JDK's
 * own providers directly ({@code TranslatedServices}), with no other provider's to fail over to.
 * A factory so made has no {@code Provider} object: {@code getProvider()} is null. The other
 * {@code getInstance} methods, which name a provider, stay rejected.
 */
class KeyFactory {
    @Edit(find = "return new KeyFactory(algorithm);", replace = """
            return new KeyFactory(TranslatedServices.keyFactory(algorithm), null, algorithm);
            """)
    public static KeyFactory getInstance(String algorithm);

    @Replace
    private KeyFactorySpi nextSpi(KeyFactorySpi oldSpi) {
        return null;
    }
}
