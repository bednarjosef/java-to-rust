package java.security;

import sun.security.provider.TranslatedRandoms;

/**
 * The translator's patch of the JDK's {@code SecureRandom} (see docs/jdk-overlays.md). The JDK
 * looks a generator up among the registered security providers, which make it by reflection from
 * a class name; the translation has no providers, so {@code new SecureRandom()} and
 * {@code getInstance(String)} make the generators of the JDK's {@code SUN} provider directly
 * ({@code TranslatedRandoms}), the default one as {@code SUN} chooses it ({@code NativePRNG} on
 * Linux). A generator so made has no {@code Provider} object: {@code getProvider()} is null, and
 * so it is not taken as thread-safe, which only makes its calls synchronized. The other
 * {@code getInstance} methods and {@code getInstanceStrong}, which name a provider or read the
 * security properties, stay rejected.
 */
class SecureRandom {
    @Replace
    private void getDefaultPRNG(boolean setSeed, byte[] seed) {
        String prngAlgorithm = TranslatedRandoms.defaultAlgorithm();
        try {
            this.secureRandomSpi = TranslatedRandoms.spi(prngAlgorithm);
        } catch (NoSuchAlgorithmException nsae) {
            throw new RuntimeException(nsae);
        }
        if (setSeed) {
            this.secureRandomSpi.engineSetSeed(seed);
        }
        if (getClass() == SecureRandom.class) {
            this.algorithm = prngAlgorithm;
        }
    }

    @Edit(from = "Instance instance", to = "instance.provider, algorithm);", replace = """
            return new SecureRandom(TranslatedRandoms.spi(algorithm), null, algorithm);
            """)
    public static SecureRandom getInstance(String algorithm);
}
