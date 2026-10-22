package sun.nio.cs;

/**
 * The translator's patch of the JDK's {@code US_ASCII} (see docs/jdk-overlays.md): its decoder
 * and encoder can be made from {@code TranslatedCoders}, and each has the class's
 * {@code JavaLangAccess} of its own, as in the patch of {@code UTF_8}.
 */
@Edit(find = "private static class Decoder", replace = "static class Decoder")
@Edit(find = "private Decoder(Charset cs)", replace = "Decoder(Charset cs)")
@Edit(find = "private static class Encoder", replace = "static class Encoder")
@Edit(find = "private Encoder(Charset cs)", replace = "Encoder(Charset cs)")
class US_ASCII {
    @Patch
    private static class Decoder {
        @Add
        private static final JavaLangAccess JLA = SharedSecrets.getJavaLangAccess();
    }

    @Patch
    private static class Encoder {
        @Add
        private static final JavaLangAccess JLA = SharedSecrets.getJavaLangAccess();
    }
}
