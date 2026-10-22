package sun.nio.cs;

import java.nio.charset.Charset;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.StandardCharsets;

/**
 * The decoders and encoders of the charsets that a translated program has, which the calls of
 * {@code newDecoder} and {@code newEncoder} on a charset make ({@code lower/charsets.rs}): a
 * charset is the runtime's value, so the JDK's classes of the standard charsets are not
 * translated, but their coders are. Each is the JDK's nested {@code Decoder} or {@code Encoder}
 * of {@code UTF_8}, {@code US_ASCII}, {@code ISO_8859_1}, {@code UTF_16}, {@code UTF_16BE} or
 * {@code UTF_16LE}, which the patches of those classes ({@code java/patches/sun/nio/cs}) let this
 * class make, and which reachability enters although it does not enter the class they are in
 * ({@code program::TRANSLATED_NESTED}). A charset without coders here (none, as the runtime has
 * only these) is refused with an {@code UnsupportedOperationException}.
 */
public final class TranslatedCoders {
    private TranslatedCoders() {}

    /** {@code cs.newDecoder()}. */
    public static CharsetDecoder newDecoder(Charset cs) {
        if (cs == StandardCharsets.UTF_8) {
            return new UTF_8.Decoder(cs);
        }
        if (cs == StandardCharsets.US_ASCII) {
            return new US_ASCII.Decoder(cs);
        }
        if (cs == StandardCharsets.ISO_8859_1) {
            return new ISO_8859_1.Decoder(cs);
        }
        if (cs == StandardCharsets.UTF_16) {
            return new UTF_16.Decoder(cs);
        }
        if (cs == StandardCharsets.UTF_16BE) {
            return new UTF_16BE.Decoder(cs);
        }
        if (cs == StandardCharsets.UTF_16LE) {
            return new UTF_16LE.Decoder(cs);
        }
        throw new UnsupportedOperationException("a decoder of " + cs);
    }

    /** {@code cs.newEncoder()}. */
    public static CharsetEncoder newEncoder(Charset cs) {
        if (cs == StandardCharsets.UTF_8) {
            return new UTF_8.Encoder(cs);
        }
        if (cs == StandardCharsets.US_ASCII) {
            return new US_ASCII.Encoder(cs);
        }
        if (cs == StandardCharsets.ISO_8859_1) {
            return new ISO_8859_1.Encoder(cs);
        }
        if (cs == StandardCharsets.UTF_16) {
            return new UTF_16.Encoder(cs);
        }
        if (cs == StandardCharsets.UTF_16BE) {
            return new UTF_16BE.Encoder(cs);
        }
        if (cs == StandardCharsets.UTF_16LE) {
            return new UTF_16LE.Encoder(cs);
        }
        throw new UnsupportedOperationException("an encoder of " + cs);
    }
}
