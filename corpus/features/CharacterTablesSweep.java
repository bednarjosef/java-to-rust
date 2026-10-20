public class CharacterTablesSweep {
    public static void main(String[] args) {
        // Every char where one of the Character methods changes its answer, from the previous char:
        // the category boundaries of the digit, letter, case, numeric and space tables.
        int boundaries = 0;
        printChar((char) 0);
        for (int code = 1; code <= 0xFFFF; code++) {
            if (differs((char) (code - 1), (char) code)) {
                boundaries++;
                printChar((char) code);
            }
        }
        System.out.println(boundaries + " boundaries");
        // forDigit for every digit below each radix, and the NUL character for the digit at the radix.
        for (int radix = 2; radix <= 36; radix++) {
            String line = "forDigit " + radix + ":";
            for (int digit = 0; digit <= radix; digit++) {
                line = line + " " + (int) Character.forDigit(digit, radix);
            }
            System.out.println(line);
        }
    }

    static boolean differs(char a, char b) {
        if (Character.isDigit(a) != Character.isDigit(b)
                || Character.isLetter(a) != Character.isLetter(b)
                || Character.isLetterOrDigit(a) != Character.isLetterOrDigit(b)
                || Character.isAlphabetic(a) != Character.isAlphabetic(b)
                || Character.isUpperCase(a) != Character.isUpperCase(b)
                || Character.isLowerCase(a) != Character.isLowerCase(b)
                || Character.isWhitespace(a) != Character.isWhitespace(b)
                || Character.isSpaceChar(a) != Character.isSpaceChar(b)
                || Character.isSurrogate(a) != Character.isSurrogate(b)
                || Character.isHighSurrogate(a) != Character.isHighSurrogate(b)
                || Character.isLowSurrogate(a) != Character.isLowSurrogate(b)
                || upperOffset(a) != upperOffset(b)
                || lowerOffset(a) != lowerOffset(b)
                || numericOffset(a) != numericOffset(b)) {
            return true;
        }
        for (int radix = 2; radix <= 36; radix++) {
            if (Character.digit(a, radix) != Character.digit(b, radix)) {
                return true;
            }
        }
        return false;
    }

    // The case mappings and numeric values as offsets from the char, so that a run of chars with the
    // same mapping is one value: a mapping that moves every char by one is not a change.
    static int upperOffset(char c) {
        return Character.toUpperCase(c) - c;
    }

    static int lowerOffset(char c) {
        return Character.toLowerCase(c) - c;
    }

    static int numericOffset(char c) {
        int value = Character.getNumericValue(c);
        return value < 0 ? value : value - c;
    }

    static void printChar(char c) {
        String line = (int) c + " " + Character.isDigit(c) + " " + Character.isLetter(c)
                + " " + Character.isLetterOrDigit(c) + " " + Character.isAlphabetic(c)
                + " " + Character.isUpperCase(c) + " " + Character.isLowerCase(c)
                + " " + Character.isWhitespace(c) + " " + Character.isSpaceChar(c)
                + " " + Character.isSurrogate(c) + " " + Character.isHighSurrogate(c)
                + " " + Character.isLowSurrogate(c)
                + " " + (int) Character.toUpperCase(c) + " " + (int) Character.toLowerCase(c)
                + " " + Character.getNumericValue(c);
        for (int radix = 2; radix <= 36; radix++) {
            line = line + " " + Character.digit(c, radix);
        }
        System.out.println(line);
    }
}
