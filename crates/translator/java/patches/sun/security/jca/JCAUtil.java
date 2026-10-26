package sun.security.jca;

/**
 * The translator's patch of the JDK's {@code JCAUtil} (see docs/jdk-overlays.md): the event of a
 * new certificate is recorded for an X.509 certificate only, and X.509 certificates are not
 * translated, so no certificate of a translation has one.
 */
class JCAUtil {
    @Replace
    public static void tryCommitCertEvent(Certificate cert) {}
}
