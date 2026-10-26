package sun.security.rsa;

import java.security.KeyFactorySpi;
import java.security.KeyPairGeneratorSpi;
import java.security.SignatureSpi;

/**
 * The services of the JDK's {@code SunRsaSign} provider that {@code TranslatedServices} makes,
 * whose classes are not all public.
 */
public final class TranslatedRsa {
    private TranslatedRsa() {}

    public static KeyPairGeneratorSpi keyPairGenerator() {
        return new RSAKeyPairGenerator.Legacy();
    }

    public static KeyFactorySpi keyFactory() {
        return new RSAKeyFactory.Legacy();
    }

    /** The signature with the RSA standard name {@code name}, null for another name. */
    public static SignatureSpi signature(String name) {
        switch (name) {
            case "MD2WITHRSA": return new RSASignature.MD2withRSA();
            case "MD5WITHRSA": return new RSASignature.MD5withRSA();
            case "SHA1WITHRSA": return new RSASignature.SHA1withRSA();
            case "SHA224WITHRSA": return new RSASignature.SHA224withRSA();
            case "SHA256WITHRSA": return new RSASignature.SHA256withRSA();
            case "SHA384WITHRSA": return new RSASignature.SHA384withRSA();
            case "SHA512WITHRSA": return new RSASignature.SHA512withRSA();
            case "SHA512/224WITHRSA": return new RSASignature.SHA512_224withRSA();
            case "SHA512/256WITHRSA": return new RSASignature.SHA512_256withRSA();
            case "SHA3-224WITHRSA": return new RSASignature.SHA3_224withRSA();
            case "SHA3-256WITHRSA": return new RSASignature.SHA3_256withRSA();
            case "SHA3-384WITHRSA": return new RSASignature.SHA3_384withRSA();
            case "SHA3-512WITHRSA": return new RSASignature.SHA3_512withRSA();
            default: return null;
        }
    }
}
