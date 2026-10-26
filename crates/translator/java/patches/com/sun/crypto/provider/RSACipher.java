package com.sun.crypto.provider;

import sun.security.provider.TranslatedDigests;

/**
 * The translator's patch of the JDK's {@code RSACipher} (see docs/jdk-overlays.md): OAEP padding
 * asks whether its digest is available of the translation's digests, not of the registered
 * providers, which the translation does not have.
 */
public final class RSACipher {
    @Edit(from = "if (Providers.getProviderList()", to = "== null) {", replace = """
            if (!TranslatedDigests.available(oaepHashAlgorithm)) {
            """)
    protected void engineSetPadding(String paddingName) throws NoSuchPaddingException;
}
