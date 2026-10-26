package com.sun.crypto.provider;

/**
 * The translator's patch of the JDK's {@code JceKeyStore}, the {@code JCEKS} key store (see
 * docs/jdk-overlays.md): a secret key entry holds its key as a serialized {@code SealedObject},
 * and serialization is not translated, so storing or loading a store with one throws an
 * {@code IOException} that says so. Private keys and certificates are stored and loaded as in the
 * JDK.
 */
class JceKeyStore {
    @Edit(from = "oos = new ObjectOutputStream(dos);", to = "sealedKey);",
            replace = "throw sealedKeys(\"stored\");")
    public void engineStore(OutputStream stream, char[] password);

    @Edit(from = "try { ois = new ObjectInputStream(dis);", to = "\"Invalid secret key format\"); }",
            replace = "if (dis != null) { throw sealedKeys(\"loaded\"); }")
    public void engineLoad(InputStream stream, char[] password);

    /** The exception of a secret key entry, which is {@code done} by serialization. */
    @Add
    private static IOException sealedKeys(String done) {
        return new IOException("a secret key entry is " + done + " by serialization, which is not translated");
    }
}
