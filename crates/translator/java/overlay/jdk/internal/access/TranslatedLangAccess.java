package jdk.internal.access;

import java.nio.charset.StandardCharsets;

/**
 * The methods of {@code JavaLangAccess} that the JDK's {@code java.io} calls on
 * {@code SharedSecrets.getJavaLangAccess()}, which the JVM sets as it starts and a translated
 * program never has: each call of one of them is a call of the method of the same name here (see
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
}
