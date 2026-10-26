package com.sun.crypto.provider;

import javax.crypto.CipherSpi;
import javax.crypto.KeyGeneratorSpi;

/**
 * The services of the JDK's {@code SunJCE} provider that {@code TranslatedServices} makes, whose
 * classes are not all public.
 */
public final class TranslatedSunJce {
    private TranslatedSunJce() {}

    public static CipherSpi aes() {
        return new AESCipher.General();
    }

    public static CipherSpi rsa() {
        return new RSACipher();
    }

    public static KeyGeneratorSpi aesKeys() {
        return new AESKeyGenerator();
    }
}
