package java.lang;

import java.nio.charset.Charset;

/**
 * The translator's patch of the JDK's {@code ProcessEnvironment}, the map that
 * {@code System.getenv()} answers (see docs/jdk-overlays.md). The JVM's native {@code environ()}
 * gives the name and the value of each entry of the process's environment; here the runtime gives
 * the entries ({@code environ0}) and the patch splits them as the JVM does. The JDK's class reads
 * names and values in {@code ProcessImpl}'s charset, whose class prepares the launching of
 * processes as it initializes; the patched one finds the charset of the same property, as
 * {@code ProcessImpl} does, without it.
 */
class ProcessEnvironment {
    /**
     * Every entry of the environment, {@code name=value}, each ending in a zero byte, in the order
     * the process has them.
     */
    @Add
    private static native byte[] environ0();

    /**
     * The name and then the value of each entry that has an {@code =}, split at the first one, as
     * the JVM gives them; an entry without one is skipped.
     */
    @Replace
    private static byte[][] environ() {
        byte[] entries = environ0();
        int count = 0;
        for (int start = 0; start < entries.length; start = entryEnd(entries, start) + 1) {
            if (separator(entries, start) >= 0) {
                count++;
            }
        }
        byte[][] result = new byte[2 * count][];
        int next = 0;
        for (int start = 0; start < entries.length; start = entryEnd(entries, start) + 1) {
            int separator = separator(entries, start);
            if (separator >= 0) {
                result[next++] = Arrays.copyOfRange(entries, start, separator);
                result[next++] = Arrays.copyOfRange(entries, separator + 1, entryEnd(entries, start));
            }
        }
        return result;
    }

    /** The index of the zero byte that ends the entry at {@code start}. */
    @Add
    private static int entryEnd(byte[] entries, int start) {
        int end = start;
        while (entries[end] != 0) {
            end++;
        }
        return end;
    }

    /** The index of the first {@code =} of the entry at {@code start}, or -1 if it has none. */
    @Add
    private static int separator(byte[] entries, int start) {
        for (int i = start; entries[i] != 0; i++) {
            if (entries[i] == '=') {
                return i;
            }
        }
        return -1;
    }

    /** {@code ProcessImpl.JNU_CHARSET}: the charset that the property names. */
    @Add
    private static Charset jnuCharset() {
        return Charset.forName(System.getProperty("sun.jnu.encoding"));
    }

    @Patch
    private static class Variable {
        @Edit(find = "str.getBytes(JNU_CHARSET)", replace = "str.getBytes(jnuCharset())")
        public static Variable valueOfQueryOnly(String str);

        @Edit(find = "new String(bytes, JNU_CHARSET)", replace = "new String(bytes, jnuCharset())")
        public static Variable valueOf(byte[] bytes);
    }

    @Patch
    private static class Value {
        @Edit(find = "str.getBytes(JNU_CHARSET)", replace = "str.getBytes(jnuCharset())")
        public static Value valueOfQueryOnly(String str);

        @Edit(find = "new String(bytes, JNU_CHARSET)", replace = "new String(bytes, jnuCharset())")
        public static Value valueOf(byte[] bytes);
    }
}
