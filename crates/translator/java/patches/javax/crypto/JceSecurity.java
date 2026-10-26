package javax.crypto;

/**
 * The translator's patch of the JDK's {@code JceSecurity} (see docs/jdk-overlays.md). The JDK
 * checks that a provider's jar is signed before its ciphers, macs and the like are used, and
 * reads the jurisdiction policy files that limit key lengths. Every provider of a translation is
 * one of the JDK's own, which the JDK trusts without a check, and the policy is the JDK's default
 * {@code crypto.policy=unlimited}: nothing is restricted.
 */
@Edit(find = "setupJurisdictionPolicies(); isRestricted = defaultPolicy.implies( CryptoAllPermission.INSTANCE) ? false : true;",
        replace = "isRestricted = false;")
@Edit(find = "@SuppressWarnings(\"deprecation\") var _unused = NULL_URL = new URL(\"http://null.oracle.com/\");",
        replace = "NULL_URL = null;")
class JceSecurity {
    @Replace
    static Exception getVerificationResult(Provider p) {
        return null;
    }
}
