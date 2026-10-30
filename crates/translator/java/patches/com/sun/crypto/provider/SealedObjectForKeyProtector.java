package com.sun.crypto.provider;

/**
 * The translator's patch of the JDK's {@code SealedObjectForKeyProtector} (see
 * docs/jdk-overlays.md): the key it seals is read back by serialization, which is not
 * translated, so reading it throws an {@code IOException} that says so, as storing or loading a
 * {@code JCEKS} key store with a secret key entry does ({@code JceKeyStore}).
 */
class SealedObjectForKeyProtector {
    @Replace
    final Key getKey(Cipher c, int maxLength)
            throws IOException, ClassNotFoundException, IllegalBlockSizeException,
            BadPaddingException {
        throw new IOException("a sealed key is read by serialization, which is not translated");
    }
}
