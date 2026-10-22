package jdk.internal.access;

import java.nio.charset.CharacterCodingException;
import java.nio.charset.Charset;
import java.nio.charset.MalformedInputException;
import java.nio.charset.StandardCharsets;
import java.nio.charset.UnmappableCharacterException;

/**
 * The methods of {@code JavaLangAccess} that the JDK's {@code java.io}, {@code java.nio.file} and
 * charsets' coders call on {@code SharedSecrets.getJavaLangAccess()}, which the JVM sets as it
 * starts and a translated program never has: each call of one of them is a call of the method of the same name here (see
 * {@code lower.rs}), which does what {@code String}'s own code does for it.
 */
final class TranslatedLangAccess {
    private TranslatedLangAccess() {}

    /** The number of bytes from {@code off} before the first negative one, at most {@code len}. */
    static int countPositives(byte[] ba, int off, int len) {
        int limit = off + len;
        for (int i = off; i < limit; i++) {
            if (ba[i] < 0) {
                return i - off;
            }
        }
        return len;
    }

    /** The number of characters at the start of {@code s} that are ASCII and not zero. */
    static int countNonZeroAscii(String s) {
        int n = s.length();
        for (int i = 0; i < n; i++) {
            char c = s.charAt(i);
            if (c == 0 || c >= 0x80) {
                return i;
            }
        }
        return n;
    }

    /** The string of the bytes as ISO-8859-1 characters. */
    static String uncheckedNewStringWithLatin1Bytes(byte[] bytes) {
        return new String(bytes, StandardCharsets.ISO_8859_1);
    }

    /** Copies {@code len} bytes from {@code src} to {@code dst} as ISO-8859-1 characters. */
    static void inflateBytesToChars(byte[] src, int srcOff, char[] dst, int dstOff, int len) {
        for (int i = 0; i < len; i++) {
            dst[dstOff + i] = (char) (src[srcOff + i] & 0xff);
        }
    }

    /**
     * Copies the ASCII bytes from {@code sp}, up to {@code len} and before the first that is not,
     * to {@code da} as characters, and returns their number, as {@code String.decodeASCII} does
     * for the charsets' decoders.
     */
    static int decodeASCII(byte[] sa, int sp, char[] da, int dp, int len) {
        int count = countPositives(sa, sp, len);
        inflateBytesToChars(sa, sp, da, dp, count);
        return count;
    }

    /**
     * Copies the ASCII characters from {@code sp}, up to {@code len} and before the first that
     * is not, to {@code da} as bytes, and returns their number, as
     * {@code StringCoding.implEncodeAsciiArray} does for the charsets' encoders.
     */
    static int uncheckedEncodeASCII(char[] sa, int sp, byte[] da, int dp, int len) {
        int i = 0;
        for (; i < len; i++) {
            char c = sa[sp++];
            if (c >= '\u0080') {
                break;
            }
            da[dp++] = (byte) c;
        }
        return i;
    }

    /**
     * Starts {@code thread} in {@code container}, as {@code ForkJoinPool}'s threads are started.
     * Containers only list their threads for monitoring tools, and track only virtual threads,
     * which a translated program does not have: the thread is started on its own. A null thread
     * fails as the JDK's {@code thread.start(container)} does.
     */
    static void start(Thread thread, jdk.internal.vm.ThreadContainer container) {
        if (thread == null) {
            throw new NullPointerException(
                    "Cannot invoke \"java.lang.Thread.start(jdk.internal.vm.ThreadContainer)\" because \"thread\" is null");
        }
        thread.start();
    }

    /**
     * The string of the bytes in {@code cs}, or a {@code MalformedInputException} where they are
     * not a string in it. The runtime's decoding replaces what is malformed, and bytes that are a
     * string in a standard charset are the bytes of that string again, so a decoding whose bytes
     * differ replaced something. The exception's length is 1, which is Java's for the usual
     * malformed byte of UTF-8 and ASCII, but may differ from Java's for a longer sequence.
     */
    static String uncheckedNewStringOrThrow(byte[] bytes, Charset cs) throws CharacterCodingException {
        String s = new String(bytes, cs);
        if (!java.util.Arrays.equals(s.getBytes(cs), bytes)) {
            throw new MalformedInputException(1);
        }
        return s;
    }

    /**
     * The bytes of the string in {@code cs}, or an {@code UnmappableCharacterException} where a
     * character has none: the runtime's encoding replaces such a character, so the bytes are not
     * the string again.
     */
    static byte[] uncheckedGetBytesOrThrow(String s, Charset cs) throws CharacterCodingException {
        byte[] bytes = s.getBytes(cs);
        if (!new String(bytes, cs).equals(s)) {
            throw new UnmappableCharacterException(1);
        }
        return bytes;
    }

    /**
     * The character at {@code index} of {@code bytes}, two bytes per character in the order of the
     * platform the translation runs on, little-endian (the JDK's {@code StringUTF16.getChar}).
     */
    static char uncheckedGetUTF16Char(byte[] bytes, int index) {
        int i = index << 1;
        return (char) ((bytes[i] & 0xff) | ((bytes[i + 1] & 0xff) << 8));
    }

    /** Sets the character at {@code index} of {@code bytes}, as {@link #uncheckedGetUTF16Char} reads it. */
    static void uncheckedPutCharUTF16(byte[] bytes, int index, int ch) {
        int i = index << 1;
        bytes[i] = (byte) ch;
        bytes[i + 1] = (byte) (ch >> 8);
    }

    /**
     * {@code prefix}, then {@code value} as {@code String.valueOf} gives it, then {@code suffix},
     * with a null prefix, value or suffix read as {@code "null"}, as the JDK's
     * {@code StringConcatHelper.concat} does for {@code ConstantUtils}.
     */
    static String concat(String prefix, Object value, String suffix) {
        return prefix + value + suffix;
    }
}
