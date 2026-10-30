package sun.nio.cs;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.Charset;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;

/**
 * The decoders and encoders of the charsets that a translated program has, which the calls of
 * {@code newDecoder} and {@code newEncoder} on a charset make ({@code lower/charsets.rs}): a
 * charset is the runtime's value, so the JDK's classes of the standard charsets are not
 * translated, but their coders are. Each is the JDK's nested {@code Decoder} or {@code Encoder}
 * of {@code UTF_8}, {@code US_ASCII}, {@code ISO_8859_1}, {@code UTF_16}, {@code UTF_16BE} or
 * {@code UTF_16LE}, which the patches of those classes ({@code java/patches/sun/nio/cs}) let this
 * class make, and which reachability enters although it does not enter the class they are in
 * ({@code program::TRANSLATED_NESTED}), or for {@code UTF-32}, {@code UTF-32BE} and
 * {@code UTF-32LE} the JDK's {@code UTF_32Coder.Decoder} and {@code Encoder}, made as those
 * charsets' classes make them. Any other charset is a single-byte charset of the JDK
 * that the program carries as a table (see {@code jdk_charsets.rs}), whose coders are the JDK's
 * own {@code SingleByte.Decoder} and {@code Encoder}, made from what the charset decodes each byte
 * to and encodes each char to.
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
        if (cs == Charset.forName("UTF-32")) {
            return new UTF_32Coder.Decoder(cs, UTF_32Coder.NONE);
        }
        if (cs == Charset.forName("UTF-32BE")) {
            return new UTF_32Coder.Decoder(cs, UTF_32Coder.BIG);
        }
        if (cs == Charset.forName("UTF-32LE")) {
            return new UTF_32Coder.Decoder(cs, UTF_32Coder.LITTLE);
        }
        return new SingleByte.Decoder(cs, b2c(cs), false, false);
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
        if (cs == Charset.forName("UTF-32") || cs == Charset.forName("UTF-32BE")) {
            return new UTF_32Coder.Encoder(cs, UTF_32Coder.BIG, false);
        }
        if (cs == Charset.forName("UTF-32LE")) {
            return new UTF_32Coder.Encoder(cs, UTF_32Coder.LITTLE, false);
        }
        return tableEncoder(cs);
    }

    /**
     * What a single-byte charset decodes each byte to, as {@code SingleByte} has it: from byte
     * -128 to 127, U+FFFD where a byte has no char.
     */
    private static char[] b2c(Charset cs) {
        byte[] bytes = new byte[256];
        for (int i = 0; i < 256; i++) {
            bytes[i] = (byte) (i - 128);
        }
        return new String(bytes, cs).toCharArray();
    }

    /**
     * The JDK's encoder of a single-byte charset, with what it encodes each char to, page by page
     * of 256 chars: {@code getBytes} gives one byte per char, the replacement {@code ?} for one
     * that the charset cannot encode, which only the char of that byte encodes to otherwise
     * ({@code java/Charsets.java} makes no table of a charset where another does). U+FFFD in
     * {@code c2b} and {@code c2bIndex} is a char or a page that does not encode.
     */
    private static CharsetEncoder tableEncoder(Charset cs) {
        char replaced = b2c(cs)['?' + 128];
        char[] c2bIndex = new char[0x100];
        char[] pages = new char[0x100 * 0x100];
        int used = 0;
        char[] page = new char[0x100];
        for (int high = 0; high < 0x100; high++) {
            c2bIndex[high] = '\uFFFD';
            if (high >= 0xD8 && high < 0xE0) {
                continue;
            }
            for (int low = 0; low < 0x100; low++) {
                page[low] = (char) (high << 8 | low);
            }
            byte[] bytes = new String(page).getBytes(cs);
            boolean any = false;
            for (int low = 0; low < 0x100; low++) {
                boolean encodes = bytes[low] != '?' || page[low] == replaced;
                pages[used + low] = encodes ? (char) (bytes[low] & 0xff) : '\uFFFD';
                any |= encodes;
            }
            if (any) {
                c2bIndex[high] = (char) used;
                used += 0x100;
            }
        }
        char[] c2b = new char[used];
        System.arraycopy(pages, 0, c2b, 0, used);
        return new SingleByte.Encoder(cs, c2b, c2bIndex, false);
    }

    /**
     * The name that {@code getEncoding} of a reader or a writer gives for {@code cs}, the
     * historical name of the JDK's class of the charset, which the translation answers from the
     * runtime's charset ({@code CharsetOp::HistoricalName}): a call of it never runs this body.
     */
    public static String historicalName(Charset cs) {
        return cs.name();
    }

    /** {@code cs.decode(bb)}, which replaces malformed input and never fails on it. */
    public static CharBuffer decode(Charset cs, ByteBuffer bb) {
        try {
            return newDecoder(cs)
                .onMalformedInput(CodingErrorAction.REPLACE)
                .onUnmappableCharacter(CodingErrorAction.REPLACE)
                .decode(bb);
        } catch (CharacterCodingException x) {
            throw new Error(x);
        }
    }

    /** {@code cs.encode(cb)}, which replaces malformed input and never fails on it. */
    public static ByteBuffer encode(Charset cs, CharBuffer cb) {
        try {
            return newEncoder(cs)
                .onMalformedInput(CodingErrorAction.REPLACE)
                .onUnmappableCharacter(CodingErrorAction.REPLACE)
                .encode(cb);
        } catch (CharacterCodingException x) {
            throw new Error(x);
        }
    }

    /** {@code cs.encode(str)}, which is {@code encode(CharBuffer.wrap(str))}. */
    public static ByteBuffer encode(Charset cs, String str) {
        return encode(cs, CharBuffer.wrap(str));
    }
}
