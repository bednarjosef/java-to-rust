package java.security;

import sun.security.jca.TranslatedServices;

/**
 * The translator's patch of the JDK's {@code AlgorithmParameters} (see docs/jdk-overlays.md): the
 * translation has no providers, so {@code getInstance(String)} asks {@code TranslatedServices},
 * which has none of the parameters of the JDK's providers yet. The other {@code getInstance}
 * methods, which name a provider, stay rejected.
 */
class AlgorithmParameters {
    @Edit(from = "try {", to = "not found\");\n        }", replace = """
            return new AlgorithmParameters(TranslatedServices.algorithmParameters(algorithm), null, algorithm);
            """)
    public static AlgorithmParameters getInstance(String algorithm);
}
