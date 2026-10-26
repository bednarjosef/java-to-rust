package sun.security.jca;

import java.security.AlgorithmParametersSpi;
import java.security.KeyPairGeneratorSpi;
import java.security.NoSuchAlgorithmException;
import java.util.Locale;
import sun.security.rsa.TranslatedRsa;

/**
 * The security services of the translator's {@code getInstance(String)} methods (see
 * docs/jdk-overlays.md): those of the JDK's own providers, made directly by name. The JDK finds a
 * service through the registered providers ({@code Security}, {@code Provider} and
 * {@code GetInstance}), which make the implementation from its class name by reflection; the
 * translation has no providers, so the names that the providers register, their aliases and
 * object identifiers among them, are listed here. A name is matched ignoring case, as the
 * providers match it, and {@code OID.} before an object identifier is left out.
 */
public final class TranslatedServices {
    private TranslatedServices() {}

    /** A new key pair generator of the algorithm named {@code algorithm}, as {@code SunRsaSign} makes it. */
    public static KeyPairGeneratorSpi keyPairGenerator(String algorithm) throws NoSuchAlgorithmException {
        switch (name(algorithm)) {
            case "RSA", "1.2.840.113549.1.1.1": return TranslatedRsa.keyPairGenerator();
            default: throw new NoSuchAlgorithmException(algorithm + " KeyPairGenerator not available");
        }
    }

    /** The parameters of the algorithm named {@code algorithm}: none of the providers' yet. */
    public static AlgorithmParametersSpi algorithmParameters(String algorithm) throws NoSuchAlgorithmException {
        throw new NoSuchAlgorithmException(algorithm + " AlgorithmParameters not available");
    }

    private static String name(String algorithm) {
        String name = algorithm.toUpperCase(Locale.ENGLISH);
        return name.startsWith("OID.") ? name.substring(4) : name;
    }
}
