package sun.security.provider;

import java.security.NoSuchAlgorithmException;
import java.security.ProviderException;
import java.security.SecureRandomSpi;
import java.util.Locale;

/**
 * The secure random generators of the translator's {@code SecureRandom} (see
 * docs/jdk-overlays.md): those of the JDK's own {@code SUN} provider, made directly by name, as
 * {@code TranslatedDigests} makes its digests. The JDK finds a generator through the registered
 * providers, which make it from its class name by reflection; the translation has no providers,
 * so the generators that {@code SUN} registers ({@code SunEntries}) are listed here, each only
 * while it is available, as {@code SUN} registers it. A name is matched ignoring case, as the
 * providers match it.
 */
public final class TranslatedRandoms {
    private TranslatedRandoms() {}

    /** A new generator of the algorithm named {@code algorithm}, as {@code SUN} makes it. */
    public static SecureRandomSpi spi(String algorithm) throws NoSuchAlgorithmException {
        switch (algorithm.toUpperCase(Locale.ENGLISH)) {
            case "NATIVEPRNG":
                if (NativePRNG.isAvailable()) {
                    return new NativePRNG(null);
                }
                break;
            case "NATIVEPRNGBLOCKING":
                if (NativePRNG.Blocking.isAvailable()) {
                    return new NativePRNG.Blocking(null);
                }
                break;
            case "NATIVEPRNGNONBLOCKING":
                if (NativePRNG.NonBlocking.isAvailable()) {
                    return new NativePRNG.NonBlocking(null);
                }
                break;
            case "SHA1PRNG":
                return new SecureRandom();
            case "DRBG":
                // `SUN` has it, so it cannot be reported missing; its mechanisms read the
                // security properties, which the translation does not have.
                throw new ProviderException("DRBG SecureRandom not supported yet");
            default:
                break;
        }
        throw new NoSuchAlgorithmException(algorithm + " SecureRandom not available");
    }

    /** The algorithm of {@code new SecureRandom()}, which {@code SUN} chooses. */
    public static String defaultAlgorithm() {
        return SunEntries.DEF_SECURE_RANDOM_ALGO;
    }
}
