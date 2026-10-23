package sun.security.provider;

import java.security.MessageDigestSpi;
import java.security.NoSuchAlgorithmException;
import java.util.Locale;

/**
 * The message digests of the translator's {@code MessageDigest.getInstance(String)} (see
 * docs/jdk-overlays.md): those of the JDK's own {@code SUN} provider, made directly by name. The
 * JDK finds a digest through the registered providers ({@code Security}, {@code Provider} and
 * {@code GetInstance}), which make the implementation from its class name by reflection; the
 * translation has no providers, so the names that {@code SUN} registers, its aliases and object
 * identifiers among them, are listed here. A name is matched ignoring case, as the providers
 * match it, and {@code OID.} before an object identifier is left out.
 */
public final class TranslatedDigests {
    private TranslatedDigests() {}

    /** A new digest of the algorithm named {@code algorithm}, as {@code SUN} makes it. */
    public static MessageDigestSpi spi(String algorithm) throws NoSuchAlgorithmException {
        String name = algorithm.toUpperCase(Locale.ENGLISH);
        if (name.startsWith("OID.")) {
            name = name.substring(4);
        }
        switch (name) {
            case "MD2", "1.2.840.113549.2.2": return new MD2();
            case "MD5", "1.2.840.113549.2.5": return new MD5();
            case "SHA-1", "SHA", "SHA1", "1.3.14.3.2.26": return new SHA();
            case "SHA-224", "SHA224", "2.16.840.1.101.3.4.2.4": return new SHA2.SHA224();
            case "SHA-256", "SHA256", "2.16.840.1.101.3.4.2.1": return new SHA2.SHA256();
            case "SHA-384", "SHA384", "2.16.840.1.101.3.4.2.2": return new SHA5.SHA384();
            case "SHA-512", "SHA512", "2.16.840.1.101.3.4.2.3": return new SHA5.SHA512();
            case "SHA-512/224", "SHA512/224", "2.16.840.1.101.3.4.2.5": return new SHA5.SHA512_224();
            case "SHA-512/256", "SHA512/256", "2.16.840.1.101.3.4.2.6": return new SHA5.SHA512_256();
            case "SHA3-224", "2.16.840.1.101.3.4.2.7": return new SHA3.SHA224();
            case "SHA3-256", "2.16.840.1.101.3.4.2.8": return new SHA3.SHA256();
            case "SHA3-384", "2.16.840.1.101.3.4.2.9": return new SHA3.SHA384();
            case "SHA3-512", "2.16.840.1.101.3.4.2.10": return new SHA3.SHA512();
            default: throw new NoSuchAlgorithmException(algorithm + " MessageDigest not available");
        }
    }

    /** Whether {@code spi} is one of the digests that {@link #spi} makes, which are SUN's. */
    public static boolean ofSun(MessageDigestSpi spi) {
        return spi instanceof DigestBase;
    }
}
