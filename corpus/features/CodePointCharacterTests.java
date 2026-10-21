public class CodePointCharacterTests {
    // One line per code point: the int overloads of the Character tests and conversions.
    static void row(int cp) {
        System.out.println(Integer.toHexString(cp) + " " + Character.isLetter(cp) + " " + Character.isDigit(cp) + " " + Character.isLetterOrDigit(cp) + " " + Character.isWhitespace(cp) + " " + Character.isSpaceChar(cp) + " " + Character.isUpperCase(cp) + " " + Character.isLowerCase(cp) + " " + Character.isAlphabetic(cp) + " " + Character.toUpperCase(cp) + " " + Character.toLowerCase(cp) + " " + Character.getNumericValue(cp) + " " + Character.digit(cp, 16) + " " + Character.digit(cp, 5));
    }

    public static void main(String[] args) {
        // Basic Latin, Latin-1, Greek, Cyrillic, a title-case letter, full-width digits and a no-break space.
        row('a');
        row('7');
        row('É');
        row('ß');
        row('ǅ');
        row('Σ');
        row('ж');
        row('Ａ');
        row('９');
        row('²');
        row(' ');
        row(' ');
        row('\u0085');
        row('\t');
        row('\uD800');
        row(0xFFFF);
        // Supplementary: Deseret letters with a case mapping, mathematical letters and digits, a letter without one.
        row(0x10400);
        row(0x10428);
        row(0x1D400);
        row(0x1D41A);
        row(0x1D7CE);
        row(0x104A0);
        row(0x118A0);
        row(0x10000);
        row(0x20000);
        // Supplementary: a numeric value of 1, a fraction, a symbol and the last code point.
        row(0x10107);
        row(0x10140);
        row(0x1F600);
        row(0x10FFFF);
        // Invalid code points: every test is false, the case mappings give the input back, and the values are -1.
        row(-1);
        row(0x110000);
        row(Integer.MIN_VALUE);
        row(Integer.MAX_VALUE);
        // Every code point, counted and summed: any table entry that differs from the JDK changes a line here.
        int letters = 0;
        int digits = 0;
        int upper = 0;
        int lower = 0;
        int alphabetic = 0;
        int letterOrDigit = 0;
        int whitespace = 0;
        int spaceChar = 0;
        int numeric = 0;
        int fractions = 0;
        int decimal = 0;
        long upperSum = 0;
        long lowerSum = 0;
        for (int cp = 0; cp <= 0x10FFFF; cp++) {
            if (Character.isLetter(cp)) letters++;
            if (Character.isDigit(cp)) digits++;
            if (Character.isUpperCase(cp)) upper++;
            if (Character.isLowerCase(cp)) lower++;
            if (Character.isAlphabetic(cp)) alphabetic++;
            if (Character.isLetterOrDigit(cp)) letterOrDigit++;
            if (Character.isWhitespace(cp)) whitespace++;
            if (Character.isSpaceChar(cp)) spaceChar++;
            if (Character.getNumericValue(cp) >= 0) numeric++;
            if (Character.getNumericValue(cp) == -2) fractions++;
            if (Character.digit(cp, 10) >= 0) decimal++;
            upperSum += Character.toUpperCase(cp);
            lowerSum += Character.toLowerCase(cp);
        }
        System.out.println(letters + " " + digits + " " + upper + " " + lower + " " + alphabetic + " " + letterOrDigit);
        System.out.println(whitespace + " " + spaceChar + " " + numeric + " " + fractions + " " + decimal);
        System.out.println(upperSum + " " + lowerSum);
    }
}
