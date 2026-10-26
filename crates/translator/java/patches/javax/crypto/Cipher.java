package javax.crypto;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code Cipher} (see docs/jdk-overlays.md). The JDK looks a
 * cipher up among the registered security providers, which make it by reflection from a class
 * name, and may put the choice off until the cipher is initialized with a key; the translation
 * has no providers, so {@code getInstance(String)} makes the ciphers of the JDK's own providers
 * directly ({@code TranslatedServices}), with the transformation's mode and padding set as the
 * JDK sets them. A cipher so made has no {@code Provider} object: {@code getProvider()} is null.
 * The other {@code getInstance} methods, which name a provider, stay rejected. There are no
 * signed providers or jurisdiction policy files: every caller may make a cipher, and every key
 * length is allowed, as with the JDK's default {@code crypto.policy=unlimited}.
 */
class Cipher {
    // Nothing in the security properties disables an algorithm, and no provider is asked.
    @Edit(from = "if (!CryptoAlgorithmConstraints", to = "failure);", replace = "return made(transformation);")
    public static final Cipher getInstance(String transformation);

    /**
     * The cipher of the first of the transformation's forms that a provider of the JDK has and
     * that takes its mode and padding, as {@code getInstance} chooses among the providers' ones.
     */
    @Add
    private static Cipher made(String transformation) throws NoSuchAlgorithmException, NoSuchPaddingException {
        Exception last = null;
        for (Transform form : getTransforms(transformation)) {
            CipherSpi spi = TranslatedServices.cipher(form.transform);
            if (spi != null) {
                try {
                    form.setModePadding(spi);
                    return new Cipher(spi, transformation);
                } catch (Exception e) {
                    last = e;
                }
            }
        }
        if (last instanceof NoSuchPaddingException padding) {
            throw padding;
        }
        throw new NoSuchAlgorithmException("Cannot find any provider supporting " + transformation, last);
    }

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

    // A cipher made by name has its implementation from the start.
    @Replace
    void chooseFirstProvider() {}

    @Replace
    private void chooseProvider(int initType, int opmode, Key key,
            AlgorithmParameterSpec paramSpec,
            AlgorithmParameters params, SecureRandom random)
            throws InvalidKeyException, InvalidAlgorithmParameterException {
        implInit(spi, initType, opmode, key, paramSpec, params, random);
    }
}
