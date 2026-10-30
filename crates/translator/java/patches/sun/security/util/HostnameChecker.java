package sun.security.util;

/**
 * The translator's patch of the JDK's {@code HostnameChecker} (see docs/jdk-overlays.md), which
 * the HTTPS client asks whether a server's certificate names the host it connected to. Checking a
 * name reads the certificate's X.500 names and its alternative names, and compares host names as
 * TLS does (IDN, Unicode normalization, the TLS code's logging): X.509 certificates, X.500 names
 * and TLS are not translated, so no translated program has a certificate to check, and a check
 * fails loudly, saying so.
 */
public class HostnameChecker {
    @Replace
    public void match(String expectedName, X509Certificate cert, boolean chainsToPublicCA)
            throws CertificateException {
        throw new UnsupportedOperationException(
                "checking the host name of an X.509 certificate is not supported by the translation");
    }
}
