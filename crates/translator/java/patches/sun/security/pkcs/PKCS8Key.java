package sun.security.pkcs;

/**
 * The translator's patch of the JDK's {@code PKCS8Key} (see docs/jdk-overlays.md): EC keys are not
 * translated, so the public key that an EC private key's encoding may hold is not read.
 */
public class PKCS8Key {
    @Edit(find = "var bits = ECPrivateKeyImpl.parsePublicBits(privKeyMaterial);", replace = """
            BitArray bits = null;
            """)
    private void decode(DerValue val) throws InvalidKeyException;
}
