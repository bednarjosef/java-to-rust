package sun.nio.cs;

/**
 * The translator's patch of the JDK's {@code UTF_8} (see docs/jdk-overlays.md). A charset is the
 * runtime's value, so this class is not translated, but its decoder and encoder are: they are
 * what {@code newDecoder} and {@code newEncoder} of the charset make
 * ({@code overlay/sun/nio/cs/TranslatedCoders.java}). Patched, the two classes can be made from
 * there, and each has the class's members it uses of its own, so that nothing of the untranslated
 * class is left to reach.
 */
@Edit(find = "private static class Decoder", replace = "static class Decoder")
@Edit(find = "private Decoder(Charset cs)", replace = "Decoder(Charset cs)")
@Edit(find = "private static final class Encoder", replace = "static final class Encoder")
@Edit(find = "private Encoder(Charset cs)", replace = "Encoder(Charset cs)")
class UTF_8 {
    @Patch
    private static class Decoder {
        @Add
        private static final JavaLangAccess JLA = SharedSecrets.getJavaLangAccess();

        /** Sets the positions of both buffers from indexes of their arrays. */
        @Add
        private static void updatePositions(Buffer src, int sp, Buffer dst, int dp) {
            src.position(sp - src.arrayOffset());
            dst.position(dp - dst.arrayOffset());
        }
    }

    @Patch
    private static final class Encoder {
        @Add
        private static final JavaLangAccess JLA = SharedSecrets.getJavaLangAccess();

        /** Sets the positions of both buffers from indexes of their arrays. */
        @Add
        private static void updatePositions(Buffer src, int sp, Buffer dst, int dp) {
            src.position(sp - src.arrayOffset());
            dst.position(dp - dst.arrayOffset());
        }
    }
}
