package com.sun.crypto.provider;

/**
 * The translator's patch of the JDK's {@code SunJCE} provider (see docs/jdk-overlays.md): the
 * translation has no provider objects, so the provider's own code, which passes itself to
 * {@code AlgorithmParameters.getInstance} and the like, passes null; those methods do not look at
 * it (see the patch of {@code AlgorithmParameters}).
 */
final class SunJCE {
    @Replace
    static SunJCE getInstance() {
        return null;
    }
}
