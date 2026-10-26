package java.security;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code Signature} (see docs/jdk-overlays.md). The JDK looks
 * a signature up among the registered security providers, which make it by reflection from a
 * class name, and may put the choice off until the signature is initialized with a key; the
 * translation has no providers, so {@code getInstance(String)} makes the signatures of the JDK's
 * own providers directly ({@code TranslatedServices}). A signature so made has no
 * {@code Provider} object: {@code getProvider()} is null. The other {@code getInstance} methods,
 * which name a provider, stay rejected.
 */
class Signature {
    // Nothing in the security properties disables an algorithm, and no provider is asked.
    @Edit(from = "if (!CryptoAlgorithmConstraints", to = "throw failure;", replace = """
            return Delegate.of(TranslatedServices.signature(algorithm), algorithm);
            """)
    public static Signature getInstance(String algorithm);

    @Patch
    private static class Delegate {
        // A signature made by name has its implementation from the start.
        @Replace
        void chooseFirstProvider() {}

        @Replace
        private void chooseProvider(int type, Key key,
                AlgorithmParameterSpec params, SecureRandom random)
                throws InvalidKeyException, InvalidAlgorithmParameterException {}
    }
}
