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
    @Edit(from = "if (!CryptoAlgorithmConstraints", to = "failure);", replace = """
            List<Transform> transforms = getTransforms(transformation);
            Exception failure = null;
            for (Transform tr : transforms) {
                CipherSpi spi = TranslatedServices.cipher(tr.transform);
                if (spi == null) {
                    continue;
                }
                try {
                    tr.setModePadding(spi);
                    return new Cipher(spi, transformation);
                } catch (Exception e) {
                    failure = e;
                }
            }
            if (failure instanceof NoSuchPaddingException nspe) {
                throw nspe;
            }
            throw new NoSuchAlgorithmException
                    ("Cannot find any provider supporting " + transformation,
                    failure);
            """)
    public static final Cipher getInstance(String transformation);

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
