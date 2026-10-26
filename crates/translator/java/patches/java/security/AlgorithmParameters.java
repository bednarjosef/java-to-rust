package java.security;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code AlgorithmParameters} (see docs/jdk-overlays.md): the
 * translation has no providers, so {@code getInstance(String)} asks {@code TranslatedServices},
 * which has none of the parameters of the JDK's providers yet. {@code getInstance(String,
 * Provider)}, which the JDK's own providers call with themselves, is that too: the translation
 * has no provider objects ({@code SunJCE.getInstance()} is null). The other {@code getInstance}
 * method, which names a provider, stays rejected.
 */
class AlgorithmParameters {
    @Edit(from = "try {", to = "not found\");\n        }", replace = """
            return new AlgorithmParameters(TranslatedServices.algorithmParameters(algorithm), null, algorithm);
            """)
    public static AlgorithmParameters getInstance(String algorithm);

    @Replace
    public static AlgorithmParameters getInstance(String algorithm, Provider provider)
            throws NoSuchAlgorithmException {
        return getInstance(algorithm);
    }
}
