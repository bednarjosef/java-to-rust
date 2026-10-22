package sun.nio.cs;

/**
 * The translator's patch of the JDK's {@code ISO_8859_1} (see docs/jdk-overlays.md): its decoder
 * and encoder can be made from {@code TranslatedCoders}, as in the patch of {@code UTF_8}.
 */
@Edit(find = "private static class Decoder", replace = "static class Decoder")
@Edit(find = "private Decoder(Charset cs)", replace = "Decoder(Charset cs)")
@Edit(find = "private static class Encoder", replace = "static class Encoder")
@Edit(find = "private Encoder(Charset cs)", replace = "Encoder(Charset cs)")
class ISO_8859_1 {
}
