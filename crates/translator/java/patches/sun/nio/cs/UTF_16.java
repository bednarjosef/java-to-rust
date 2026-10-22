package sun.nio.cs;

/**
 * The translator's patch of the JDK's {@code UTF_16} (see docs/jdk-overlays.md): its decoder and
 * encoder can be made from {@code TranslatedCoders}, as in the patch of {@code UTF_8}.
 */
@Edit(find = "private static class Decoder", replace = "static class Decoder")
@Edit(find = "private static class Encoder", replace = "static class Encoder")
class UTF_16 {
}
