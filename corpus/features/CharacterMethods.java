public class CharacterMethods {
    public static void main(String[] args) {
        // isWhitespace: tabs and the file and group separators are white space, no-break spaces are not.
        System.out.println(Character.isWhitespace('\t') + " " + Character.isWhitespace('\u000B') + " " + Character.isWhitespace('\u001C') + " " + Character.isWhitespace('\u001F'));
        System.out.println(Character.isWhitespace(' ') + " " + Character.isWhitespace(' ') + " " + Character.isWhitespace(' ') + " " + Character.isWhitespace(' '));
        System.out.println(Character.isWhitespace('\u0085') + " " + Character.isWhitespace(' ') + " " + Character.isWhitespace(' ') + " " + Character.isWhitespace('x'));
        // isSpaceChar: the separators, no-break spaces included, and no control characters.
        System.out.println(Character.isSpaceChar(' ') + " " + Character.isSpaceChar(' ') + " " + Character.isSpaceChar(' ') + " " + Character.isSpaceChar(' '));
        System.out.println(Character.isSpaceChar('\t') + " " + Character.isSpaceChar('\u001C') + " " + Character.isSpaceChar('\u0085'));
        // isLetterOrDigit, isAlphabetic: superscripts are numeric but neither letters nor digits.
        System.out.println(Character.isLetterOrDigit('a') + " " + Character.isLetterOrDigit('7') + " " + Character.isLetterOrDigit('é') + " " + Character.isLetterOrDigit('_'));
        System.out.println(Character.isLetterOrDigit('²') + " " + Character.isLetterOrDigit('×') + " " + Character.isLetterOrDigit('\uD800'));
        System.out.println(Character.isAlphabetic('a') + " " + Character.isAlphabetic('1') + " " + Character.isAlphabetic('ª') + " " + Character.isAlphabetic('²'));
        // digit(char, radix): -1 when the digit is not below the radix or the radix is out of range.
        System.out.println(Character.digit('a', 16) + " " + Character.digit('7', 8) + " " + Character.digit('8', 8) + " " + Character.digit('z', 36));
        System.out.println(Character.digit('Z', 36) + " " + Character.digit('z', 37) + " " + Character.digit('A', 1) + " " + Character.digit('0', 2));
        System.out.println(Character.digit('Ａ', 16) + " " + Character.digit('Ａ', 10) + " " + Character.digit('９', 10) + " " + Character.digit('-', 10));
        // forDigit(int, int): a lowercase digit, or the NUL character when the digit or radix is out of range.
        System.out.println(Character.forDigit(11, 16) + " " + Character.forDigit(35, 36) + " " + Character.forDigit(9, 10));
        System.out.println((int) Character.forDigit(10, 10) + " " + (int) Character.forDigit(-1, 10) + " " + (int) Character.forDigit(3, 1) + " " + (int) Character.forDigit(5, 37));
        // Surrogates: a high surrogate is D800..DBFF and a low one DC00..DFFF.
        System.out.println(Character.isSurrogate('\uD800') + " " + Character.isSurrogate('\uDBFF') + " " + Character.isSurrogate('\uDC00') + " " + Character.isSurrogate('\uDFFF') + " " + Character.isSurrogate('a'));
        System.out.println(Character.isHighSurrogate('\uD800') + " " + Character.isHighSurrogate('\uDBFF') + " " + Character.isHighSurrogate('\uDC00'));
        System.out.println(Character.isLowSurrogate('\uDC00') + " " + Character.isLowSurrogate('\uDFFF') + " " + Character.isLowSurrogate('\uDBFF') + " " + Character.isLowSurrogate('\uD800'));
        // compare, toString and the bounds.
        System.out.println(Character.compare('a', 'b') + " " + Character.compare('b', 'a') + " " + Character.compare('a', 'a'));
        System.out.println(Character.toString('x') + Character.toString('é') + " " + Character.MIN_VALUE + " " + (int) Character.MAX_VALUE);
    }
}
