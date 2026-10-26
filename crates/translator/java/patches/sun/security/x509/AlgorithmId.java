package sun.security.x509;

/**
 * The translator's patch of the JDK's {@code AlgorithmId} (see docs/jdk-overlays.md): the names of
 * algorithms by object identifier that providers other than the JDK's own register. The
 * translation has only the JDK's providers, whose names are in {@code KnownOIDs}, so there are
 * none.
 */
public class AlgorithmId {
    @Replace
    private static ConcurrentHashMap<String, String> collectOIDAliases() {
        return new ConcurrentHashMap<>();
    }
}
