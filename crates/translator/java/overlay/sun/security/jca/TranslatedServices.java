package sun.security.jca;

import com.sun.crypto.provider.TranslatedSunJce;
import java.security.AlgorithmParametersSpi;
import java.security.KeyFactorySpi;
import java.security.KeyPairGeneratorSpi;
import java.security.NoSuchAlgorithmException;
import java.security.SignatureSpi;
import java.util.Locale;
import javax.crypto.CipherSpi;
import javax.crypto.KeyGeneratorSpi;
import sun.security.rsa.TranslatedRsa;
import sun.security.util.KnownOIDs;

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

    /** A new key factory of the algorithm named {@code algorithm}, as {@code SunRsaSign} makes it. */
    public static KeyFactorySpi keyFactory(String algorithm) throws NoSuchAlgorithmException {
        switch (name(algorithm)) {
            case "RSA", "1.2.840.113549.1.1.1": return TranslatedRsa.keyFactory();
            default: throw new NoSuchAlgorithmException(algorithm + " KeyFactory not available");
        }
    }

    /**
     * A new signature of the algorithm named {@code algorithm}, as {@code SunRsaSign} makes it:
     * its names are the standard names and object identifiers of {@code KnownOIDs}.
     */
    public static SignatureSpi signature(String algorithm) throws NoSuchAlgorithmException {
        String name = name(algorithm);
        KnownOIDs known = KnownOIDs.findMatch(name);
        SignatureSpi spi = TranslatedRsa.signature(known == null ? name : known.stdName().toUpperCase(Locale.ENGLISH));
        if (spi == null) {
            throw new NoSuchAlgorithmException(algorithm + " Signature not available");
        }
        return spi;
    }

    /**
     * A new cipher of the transformation {@code transform} as {@code SunJCE} registers it, before
     * its mode and padding are set: null if {@code SunJCE} has none of that name.
     */
    public static CipherSpi cipher(String transform) {
        switch (name(transform)) {
            case "AES", "2.16.840.1.101.3.4.1": return TranslatedSunJce.aes();
            case "RSA": return TranslatedSunJce.rsa();
            default: return null;
        }
    }

    /** A new secret key generator of the algorithm named {@code algorithm}, as {@code SunJCE} makes it. */
    public static KeyGeneratorSpi keyGenerator(String algorithm) throws NoSuchAlgorithmException {
        switch (name(algorithm)) {
            case "AES", "2.16.840.1.101.3.4.1": return TranslatedSunJce.aesKeys();
            default: throw new NoSuchAlgorithmException(algorithm + " KeyGenerator not available");
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
