package java.net;

import java.nio.charset.StandardCharsets;

/**
 * The translator's methods that calls of some of {@code URI}'s private methods call instead
 * ({@code program::REDIRECTED_CALLS}), each with the same type as the JDK's.
 */
final class TranslatedURI {
    private TranslatedURI() {}

    /**
     * {@code URI.decode(s, ignorePercentInBrackets)}: {@code s} with each run of escaped octets
     * ({@code %XX}) decoded as UTF-8, a malformed sequence replaced with {@code '�'}, and a
     * {@code %} between {@code []} (an IPv6 scope) left alone where
     * {@code ignorePercentInBrackets}. The JDK's decodes each run with a {@code CharsetDecoder}
     * that replaces malformed input, which is what {@code new String(bytes, UTF_8)} does.
     */
    static String decode(String s, boolean ignorePercentInBrackets) {
        if (s == null)
            return s;
        int n = s.length();
        if (n == 0)
            return s;
        if (s.indexOf('%') < 0)
            return s;

        StringBuilder sb = new StringBuilder(n);
        byte[] bytes = new byte[n];
        char c = s.charAt(0);
        boolean betweenBrackets = false;

        for (int i = 0; i < n;) {
            if (c == '[') {
                betweenBrackets = true;
            } else if (betweenBrackets && c == ']') {
                betweenBrackets = false;
            }
            if (c != '%' || (betweenBrackets && ignorePercentInBrackets)) {
                sb.append(c);
                if (++i >= n)
                    break;
                c = s.charAt(i);
                continue;
            }
            int count = 0;
            for (;;) {
                bytes[count++] = (byte) ((hex(s.charAt(++i)) << 4) | hex(s.charAt(++i)));
                if (++i >= n)
                    break;
                c = s.charAt(i);
                if (c != '%')
                    break;
            }
            sb.append(new String(bytes, 0, count, StandardCharsets.UTF_8));
        }

        return sb.toString();
    }

    /** The value of a hexadecimal digit, as {@code URI.decode(char)} gives it. */
    private static int hex(char c) {
        if ((c >= '0') && (c <= '9'))
            return c - '0';
        if ((c >= 'a') && (c <= 'f'))
            return c - 'a' + 10;
        if ((c >= 'A') && (c <= 'F'))
            return c - 'A' + 10;
        return -1 & 0xf;
    }
}
