public class CodePoints {
    public static void main(String[] args) {
        int[] points = {0x41, 0xFFFF, 0x1F600, 0x10FFFF, -1};
        for (int cp : points) {
            System.out.println(Character.isValidCodePoint(cp) + " " + Character.isBmpCodePoint(cp) + " "
                + Character.isSupplementaryCodePoint(cp) + " " + Character.charCount(cp));
        }
        char[] pair = Character.toChars(0x1F600);
        System.out.println(pair.length + " " + (int) pair[0] + " " + (int) pair[1]);
        char[] single = Character.toChars(0x41);
        System.out.println(single.length + " " + single[0]);
        System.out.println((int) Character.highSurrogate(0x1F600) + " " + (int) Character.lowSurrogate(0x1F600));
        System.out.println(Character.toCodePoint('\uD83D', '\uDE00'));
        System.out.println(Character.toString(0x1F600));
        System.out.println(Character.toString('A') + Character.toString(0x42));
        System.out.println(Character.toString((short) 67));
        System.out.println(Character.toChars(0x110000).length);
    }
}
