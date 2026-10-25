package sun.security.provider;

/**
 * The translator's patch of the JDK's {@code SHA1PRNG} generator (see docs/jdk-overlays.md). It
 * asks for the {@code SUN} provider's SHA-1 digest by the provider's name, which the translated
 * {@code MessageDigest} does not take; the digest it makes by the algorithm's name alone is that
 * same one ({@code TranslatedDigests}), which the method's own fallback asks for.
 */
class SecureRandom {
    @Edit(find = "MessageDigest.getInstance(\"SHA\", \"SUN\");", replace = "MessageDigest.getInstance(\"SHA\");")
    @Edit(find = "NoSuchProviderException | NoSuchAlgorithmException", replace = "NoSuchAlgorithmException")
    private void init(byte[] seed);
}
