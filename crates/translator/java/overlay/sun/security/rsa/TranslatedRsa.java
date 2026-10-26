package sun.security.rsa;

import java.security.KeyPairGeneratorSpi;

/**
 * The services of the JDK's {@code SunRsaSign} provider that {@code TranslatedServices} makes,
 * whose classes are not public.
 */
public final class TranslatedRsa {
    private TranslatedRsa() {}

    public static KeyPairGeneratorSpi keyPairGenerator() {
        return new RSAKeyPairGenerator.Legacy();
    }
}
