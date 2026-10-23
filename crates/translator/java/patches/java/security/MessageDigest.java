package java.security;

import sun.security.provider.TranslatedDigests;

/**
 * The translator's patch of the JDK's {@code MessageDigest} (see docs/jdk-overlays.md). The JDK
 * looks a digest up among the registered security providers, which make it by reflection from a
 * class name; the translation has no providers, so {@code getInstance(String)} makes the digests
 * of the JDK's {@code SUN} provider directly ({@code TranslatedDigests}). A digest so made has no
 * {@code Provider} object: {@code getProvider()} is null, and {@code toString} names {@code SUN},
 * as in Java. The other {@code getInstance} methods, which name a provider, stay rejected.
 */
class MessageDigest {
    // No provider is asked whether the algorithm is allowed, nor reports which one was used.
    @Edit(from = "if (!CryptoAlgorithmConstraints", to = "return md;", replace = """
            return Delegate.of(TranslatedDigests.spi(algorithm), algorithm, null);
            """)
    public static MessageDigest getInstance(String algorithm);

    // A provider's name is a call of `Provider`, which is not translated.
    @Replace
    private String getProviderName() {
        if (this instanceof Delegate delegate && TranslatedDigests.ofSun(delegate.digestSpi)) {
            return "SUN";
        }
        return "(no provider)";
    }

    @Patch
    private static class Delegate {
        // Only a digest of a PKCS#11 provider, which the translation does not have, may fail to
        // clone though it is `Cloneable`.
        @Edit(from = "if (isCloneable && p.getName()", to = "isCloneable = false; } }", replace = "")
        static Delegate of(MessageDigestSpi digestSpi, String algo, Provider p);
    }
}
