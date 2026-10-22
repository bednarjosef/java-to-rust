package java.net;

/**
 * The translator's patch of the JDK's {@code URI} (see docs/jdk-overlays.md). The JDK's decodes
 * each run of escaped octets ({@code %XX}) of a URI's part with a UTF-8 {@code CharsetDecoder}
 * that replaces malformed input, between buffers. Patched, it collects the octets in an array and
 * makes a {@code String} of them as UTF-8, which replaces malformed input the same way.
 */
class URI {
    @Edit(from = "ByteBuffer bb = ByteBuffer.allocate(n);", to = ".onUnmappableCharacter(CodingErrorAction.REPLACE);",
            replace = "byte[] bytes = new byte[n];")
    @Edit(find = "bb.clear();", replace = "int count = 0;")
    @Edit(find = "bb.put(decode(s.charAt(++i), s.charAt(++i)));",
            replace = "bytes[count++] = decode(s.charAt(++i), s.charAt(++i));")
    @Edit(from = "bb.flip();", to = "sb.append(cb.flip().toString());",
            replace = "sb.append(new String(bytes, 0, count, UTF_8.INSTANCE));")
    private static String decode(String s, boolean ignorePercentInBrackets);
}
