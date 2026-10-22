package sun.util;

import java.io.IOException;
import java.io.InputStream;
import java.io.Reader;
import java.nio.charset.MalformedInputException;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

/**
 * The reader of a properties file that the constructor of {@code PropertyResourceBundle} from a
 * stream reads, in place of the JDK's {@code InputStreamReader} over a decoder of
 * {@code PropertyResourceBundleCharset}, a {@code Charset} of its own that the translation cannot
 * take (see docs/reflection-and-plugins.md).
 *
 * <p>It reads the stream's bytes and decodes them as Java's decoder does: as UTF-8, or, if they
 * are not UTF-8, as ISO-8859-1 from the start of the block of 8192 bytes that has the first
 * malformed byte, which is the whole text when that byte is in the first block. A later block
 * would be decoded in part as UTF-8, by the decoder's buffers, so that one is an
 * {@code UnsupportedOperationException}. A character that the end of the bytes cuts is a
 * {@code MalformedInputException}, as in Java. With the property
 * {@code java.util.PropertyResourceBundle.encoding} set to {@code ISO-8859-1}, the bytes are read
 * as ISO-8859-1 only; set to {@code UTF-8}, malformed UTF-8 is an
 * {@code UnsupportedOperationException}, where Java's would give the length of the input.
 */
public final class TranslatedPropertyFiles {
    // The encoding that the property gives, or "", in capitals as the JDK compares it.
    private static final String encoding =
            System.getProperty("java.util.PropertyResourceBundle.encoding", "").toUpperCase(Locale.ROOT);

    private TranslatedPropertyFiles() {
    }

    /** The reader of the properties file {@code stream}, decoded as the JDK's decoder does. */
    public static Reader reader(InputStream stream) throws IOException {
        return new TextReader(decode(bytesOf(stream)));
    }

    // The bytes of `stream`; a null stream fails as Java's does, without a message.
    private static byte[] bytesOf(InputStream stream) throws IOException {
        if (stream == null) {
            throw new NullPointerException();
        }
        return stream.readAllBytes();
    }

    // The text of the bytes of a properties file, as the JDK's decoder gives it.
    private static String decode(byte[] bytes) throws IOException {
        if ("ISO-8859-1".equals(encoding)) {
            return new String(bytes, StandardCharsets.ISO_8859_1);
        }
        int bad = malformed(bytes);
        if (bad < 0) {
            return new String(bytes, StandardCharsets.UTF_8);
        }
        // A character that the end of the stream cuts is malformed in either case, as the decoder
        // waits for its other bytes.
        if (cut(bytes, bad)) {
            throw new MalformedInputException(bytes.length - bad);
        }
        if ("UTF-8".equals(encoding)) {
            throw new UnsupportedOperationException("a properties file that is not UTF-8, with the"
                    + " encoding UTF-8: the translation cannot give the length of Java's malformed input");
        }
        if (bad >= 8192) {
            throw new UnsupportedOperationException("a properties file that is not UTF-8 after its first"
                    + " 8192 bytes: the translation cannot decode it as Java's decoder does");
        }
        return new String(bytes, StandardCharsets.ISO_8859_1);
    }

    // The index of the first byte that does not begin a well-formed UTF-8 character, or -1.
    private static int malformed(byte[] bytes) {
        int i = 0;
        while (i < bytes.length) {
            int length = length(bytes[i] & 0xff);
            if (length == 0 || i + length > bytes.length) {
                return i;
            }
            for (int k = 1; k < length; k++) {
                if (!continues(bytes[i] & 0xff, k, bytes[i + k] & 0xff)) {
                    return i;
                }
            }
            i += length;
        }
        return -1;
    }

    // Whether the character at `at` is well formed as far as the bytes go, which end before it does.
    private static boolean cut(byte[] bytes, int at) {
        int lead = bytes[at] & 0xff;
        if (length(lead) < 2 || at + length(lead) <= bytes.length) {
            return false;
        }
        for (int k = 1; at + k < bytes.length; k++) {
            if (!continues(lead, k, bytes[at + k] & 0xff)) {
                return false;
            }
        }
        return true;
    }

    // The length of the UTF-8 character that begins with `lead`, or 0 if no character does.
    private static int length(int lead) {
        if (lead < 0x80) {
            return 1;
        } else if (lead >= 0xc2 && lead <= 0xdf) {
            return 2;
        } else if (lead >= 0xe0 && lead <= 0xef) {
            return 3;
        } else if (lead >= 0xf0 && lead <= 0xf4) {
            return 4;
        }
        return 0;
    }

    // Whether `c` can be the byte `k` (from 1) of a character that begins with `lead`: no overlong
    // forms, surrogates or code points past U+10FFFF.
    private static boolean continues(int lead, int k, int c) {
        int low = 0x80;
        int high = 0xbf;
        if (k == 1) {
            if (lead == 0xe0) {
                low = 0xa0;
            } else if (lead == 0xed) {
                high = 0x9f;
            } else if (lead == 0xf0) {
                low = 0x90;
            } else if (lead == 0xf4) {
                high = 0x8f;
            }
        }
        return c >= low && c <= high;
    }

    /** A reader of a string, as a {@code StringReader}. */
    private static final class TextReader extends Reader {
        private final String text;
        private int next;

        TextReader(String text) {
            this.text = text;
        }

        @Override
        public int read(char[] buffer, int off, int len) {
            if (next >= text.length()) {
                return -1;
            }
            int n = Math.min(len, text.length() - next);
            text.getChars(next, next + n, buffer, off);
            next += n;
            return n;
        }

        @Override
        public void close() {
        }
    }
}
