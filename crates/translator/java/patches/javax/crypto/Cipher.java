package javax.crypto;

/**
 * The translator's patch of the JDK's {@code Cipher} (see docs/jdk-overlays.md). There are no
 * signed providers or jurisdiction policy files: every caller may make a cipher, and every key
 * length is allowed, as with the JDK's default {@code crypto.policy=unlimited}.
 */
class Cipher {
    // The JDK asks whether the caller, found by walking the stack, may make a cipher.
    @Replace
    protected Cipher(CipherSpi cipherSpi, Provider provider, String transformation) {
        this.spi = cipherSpi;
        this.provider = provider;
        this.transformation = transformation;
        this.cryptoPerm = CryptoAllPermission.INSTANCE;
        this.lock = null;
    }

    @Replace
    private static CryptoPermission getConfiguredPermission(String transformation)
            throws NullPointerException, NoSuchAlgorithmException {
        if (transformation == null) throw new NullPointerException();
        tokenizeTransformation(transformation);
        return CryptoAllPermission.INSTANCE;
    }
}
