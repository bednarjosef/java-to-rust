// The static methods and nested classes of Character that the runtime does not answer come from
// the JDK's Character and its CharacterData tables.
public class CharacterFromJdk {
    public static void main(String[] args) {
        int[] points = {'a', 'Z', '5', ' ', 0x01C5, 0x4E2D, 0x0416, 0x05D0, 0x1F600, 0x2028, '_', '$', 0x00AD};
        for (int p : points) {
            System.out.println(Integer.toHexString(p)
                    + " type " + Character.getType(p)
                    + " script " + Character.UnicodeScript.of(p)
                    + " block " + Character.UnicodeBlock.of(p)
                    + " title " + Character.isTitleCase(p)
                    + " ideographic " + Character.isIdeographic(p)
                    + " javaStart " + Character.isJavaIdentifierStart(p)
                    + " javaPart " + Character.isJavaIdentifierPart(p)
                    + " unicodeStart " + Character.isUnicodeIdentifierStart(p)
                    + " ignorable " + Character.isIdentifierIgnorable(p)
                    + " mirrored " + Character.isMirrored(p)
                    + " defined " + Character.isDefined(p)
                    + " emoji " + Character.isEmoji(p));
        }
        System.out.println(Character.getType('x') == Character.LOWERCASE_LETTER);
        System.out.println(Character.isISOControl('\n') + " " + Character.isSurrogatePair('\uD83D', '\uDE00'));
        System.out.println(Integer.toHexString(Character.reverseBytes('\u1234')));
        System.out.println(Character.TYPE + " " + Character.TYPE.isPrimitive());
        System.out.println(Character.codePointAt("a\uD83D\uDE00", 1) + " " + Character.codePointBefore("a\uD83D\uDE00", 3));
        int[] cps = {0x48, 0x1F600, 0x69};
        String s = new String(cps, 0, 3);
        System.out.println(s + " " + s.length());
        byte[] ascii = {72, 105};
        @SuppressWarnings("deprecation")
        String high = new String(ascii, 0, 0, 2);
        System.out.println(high);
        System.out.println(Character.isLetter(0x4E2D) + " " + Character.toUpperCase('q') + " " + Character.digit('7', 10));
    }
}
