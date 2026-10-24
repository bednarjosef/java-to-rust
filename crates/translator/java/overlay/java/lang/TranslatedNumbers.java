package java.lang;

import java.util.Objects;

/**
 * The forms of {@code Integer.parseInt}, {@code Long.parseLong}, {@code parseUnsignedInt} and
 * {@code parseUnsignedLong} that parse a range of a {@code CharSequence}, which a call reaches here
 * when its text is not known to be a string (see {@code program::REDIRECTED_CALLS}; the runtime
 * parses a string's range itself). They behave as the JDK's: the sequence is asked for its length
 * once, its characters are read once each, from the first of the range on, up to the first that is
 * no digit or that overflows, and a failure throws what the JDK's throws, with its message (the
 * range, as {@code subSequence} gives it, for a malformed number).
 */
final class TranslatedNumbers {
    private TranslatedNumbers() {}

    static int parseInt(CharSequence s, int beginIndex, int endIndex, int radix) {
        return (int) parseSigned(s, beginIndex, endIndex, radix, Integer.MIN_VALUE);
    }

    static long parseLong(CharSequence s, int beginIndex, int endIndex, int radix) {
        return parseSigned(s, beginIndex, endIndex, radix, Long.MIN_VALUE);
    }

    static int parseUnsignedInt(CharSequence s, int beginIndex, int endIndex, int radix) {
        return (int) parseUnsigned(s, beginIndex, endIndex, radix, false);
    }

    static long parseUnsignedLong(CharSequence s, int beginIndex, int endIndex, int radix) {
        return parseUnsigned(s, beginIndex, endIndex, radix, true);
    }

    /** The checks before any character is read; the range is not empty when they pass. */
    private static void checkRange(CharSequence s, int beginIndex, int endIndex, int radix) {
        Objects.requireNonNull(s);
        Objects.checkFromToIndex(beginIndex, endIndex, s.length());
        if (radix < Character.MIN_RADIX) {
            throw new NumberFormatException("radix " + radix + " less than Character.MIN_RADIX");
        }
        if (radix > Character.MAX_RADIX) {
            throw new NumberFormatException("radix " + radix + " greater than Character.MAX_RADIX");
        }
        if (beginIndex == endIndex) {
            String under = radix == 10 ? "" : " under radix " + radix;
            throw new NumberFormatException("For input string: \"\"" + under);
        }
    }

    /** A malformed number: the character at {@code at} is where the range stops being one. */
    private static NumberFormatException malformed(CharSequence s, int beginIndex, int endIndex, int at) {
        CharSequence range = s.subSequence(beginIndex, endIndex);
        return new NumberFormatException("Error at index " + (at - beginIndex) + " in: \"" + range + "\"");
    }

    /**
     * A signed number whose smallest value is {@code min}: it is summed up negated, from 0 down,
     * so that {@code min} itself, which has no positive counterpart, can be reached.
     */
    private static long parseSigned(CharSequence s, int beginIndex, int endIndex, int radix, long min) {
        checkRange(s, beginIndex, endIndex, radix);
        char c = s.charAt(beginIndex);
        boolean negative = c == '-';
        // The most negative value the digits may sum to, and the most negative one that may still
        // be multiplied by the radix.
        long floor = negative ? min : min + 1;
        long beforeShift = floor / radix;
        int at = beginIndex;
        if (negative || c == '+') {
            at++;
            if (at == endIndex) {
                throw malformed(s, beginIndex, endIndex, at);
            }
            c = s.charAt(at);
        }
        long sum = 0;
        while (true) {
            int digit = Character.digit(c, radix);
            if (digit < 0 || sum < beforeShift || sum * radix < floor + digit) {
                throw malformed(s, beginIndex, endIndex, at);
            }
            sum = sum * radix - digit;
            at++;
            if (at == endIndex) {
                return negative ? sum : -sum;
            }
            c = s.charAt(at);
        }
    }

    /**
     * An unsigned number of 64 bits ({@code wide}) or 32. A leading minus sign and a value too
     * large have messages of their own, which name the whole sequence, as its {@code toString}
     * (for the value too large, as {@code %s} formats it).
     */
    private static long parseUnsigned(CharSequence s, int beginIndex, int endIndex, int radix, boolean wide) {
        checkRange(s, beginIndex, endIndex, radix);
        char c = s.charAt(beginIndex);
        if (c == '-') {
            throw new NumberFormatException("Illegal leading minus sign on unsigned string " + s + ".");
        }
        // The largest value, and the largest that may still be multiplied by the radix with the
        // largest digit that may follow then.
        long max = wide ? -1L : 0xFFFF_FFFFL;
        long beforeShift = Long.divideUnsigned(max, radix);
        long lastDigit = Long.remainderUnsigned(max, radix);
        int at = beginIndex;
        if (c == '+') {
            at++;
            if (at == endIndex) {
                throw malformed(s, beginIndex, endIndex, at);
            }
            c = s.charAt(at);
        }
        long sum = 0;
        while (true) {
            int digit = Character.digit(c, radix);
            if (digit < 0) {
                throw malformed(s, beginIndex, endIndex, at);
            }
            int order = Long.compareUnsigned(sum, beforeShift);
            if (order > 0 || order == 0 && digit > lastDigit) {
                String width = wide ? "long" : "int";
                String format = "String value %s exceeds range of unsigned " + width + ".";
                throw new NumberFormatException(String.format(format, s));
            }
            sum = sum * radix + digit;
            at++;
            if (at == endIndex) {
                return sum;
            }
            c = s.charAt(at);
        }
    }
}
