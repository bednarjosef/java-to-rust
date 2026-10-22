package java.nio.file;

import java.util.Random;

/**
 * The translator's patch of the JDK's {@code TempFileHelper} (see docs/jdk-overlays.md). The
 * JDK's makes the names of temporary files from a {@code SecureRandom}, which is not translated
 * ({@code java.security}); patched, it makes them from a {@code java.util.Random}, whose seed is
 * the time and a counter. The names are as random to a program, but they are not a secret from
 * another process that knows the time; the files are created exclusively all the same, so another
 * process cannot take one over. The JDK's catches and throws again the
 * {@code InvalidPathException} of a name, which the patched class lets out as it is.
 */
class TempFileHelper {
    @Replace
    private static final Random random = new Random();

    @Edit(from = "Path f;", to = "throw e; }", replace = "Path f = generatePath(prefix, suffix, dir);")
    private static Path create(Path dir, String prefix, String suffix, boolean createDirectory, FileAttribute<?>[] attrs);
}
