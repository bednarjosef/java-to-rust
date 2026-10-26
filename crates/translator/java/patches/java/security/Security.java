package java.security;

import java.io.StringReader;
import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code Security} (see docs/jdk-overlays.md). The security
 * properties are first the JDK's own, from the {@code java.security} file of the JDK that the
 * translator translates with, which the translation carries; then, as in the JDK, those of the
 * file that the {@code java.security.properties} property names, unless the JDK's file forbids
 * it. A file named by a URL that is not a {@code file:} one is not read, which keeps the URL
 * protocol handlers (and with them the {@code jar:} handler's signed jars) out of the
 * translation.
 */
class Security {
    // The JDK finds an engine's class by its name, `java.security.` and the type and `Spi`, with
    // `Class.forName`; these are the engines of `java.security` whose services `getImpl` makes,
    // and the translation names them, as a closed program has no classes found by a computed
    // name. (`Policy`, the other one, is not translated.)
    @Replace
    private static Class<?> getSpiClass(String type) {
        return switch (type) {
            case "AlgorithmParameterGenerator" -> AlgorithmParameterGeneratorSpi.class;
            case "AlgorithmParameters" -> AlgorithmParametersSpi.class;
            case "KeyFactory" -> KeyFactorySpi.class;
            case "KeyPairGenerator" -> KeyPairGeneratorSpi.class;
            case "KeyStore" -> KeyStoreSpi.class;
            case "MessageDigest" -> MessageDigestSpi.class;
            case "SecureRandom" -> SecureRandomSpi.class;
            case "Signature" -> SignatureSpi.class;
            default -> throw new AssertionError("Spi class not found",
                    new ClassNotFoundException("java.security." + type + "Spi"));
        };
    }

    @Patch
    private static final class SecPropLoader {
        // The JDK's own properties are those of the JDK the translator translates with, which
        // the translation carries: a translated program has no JDK to read them from.
        @Replace
        private static void loadMaster() {
            String master = TranslatedServices.securityProperties();
            try {
                if (master == null) {
                    throw new FileNotFoundException("the JDK has no conf/security/java.security");
                }
                props.load(new StringReader(master));
            } catch (IOException e) {
                throw new InternalError("Error loading java.security file", e);
            }
        }

        // A name that is neither a path nor a `file:` URL fails as one with no scheme does.
        @Edit(from = "// Try to interpret propFile as a URL", to = "loadFromUrl(url, mode);", replace = "throw error;")
        private static void loadExtraHelper(String propFile, LoadingMode mode) throws Exception;
    }
}
