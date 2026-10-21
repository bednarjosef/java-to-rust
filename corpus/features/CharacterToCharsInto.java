public class CharacterToCharsInto {
    static void show(String label, char[] dst) {
        StringBuilder text = new StringBuilder(label + ":");
        for (char c : dst) {
            text.append(" ").append((int) c);
        }
        System.out.println(text);
    }

    static void attempt(int cp, char[] dst, int index) {
        try {
            int count = Character.toChars(cp, dst, index);
            System.out.println("count " + count);
        } catch (RuntimeException e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        // A BMP code point writes one unit at the index.
        char[] bmp = new char[3];
        System.out.println("count " + Character.toChars('A', bmp, 1));
        show("bmp", bmp);
        System.out.println("count " + Character.toChars(0xFFFF, bmp, 0));
        show("bmp max", bmp);
        // A supplementary code point writes the high surrogate at the index and the low one after it.
        char[] pair = new char[4];
        System.out.println("count " + Character.toChars(0x1D400, pair, 1));
        show("pair", pair);
        System.out.println("count " + Character.toChars(0x10FFFF, pair, 2));
        show("pair max", pair);
        System.out.println("count " + Character.toChars(0x10000, pair, 0));
        show("pair min", pair);
        // Code points that are not valid throw before anything is written.
        char[] untouched = new char[3];
        attempt(-1, untouched, 0);
        attempt(0x110000, untouched, 0);
        attempt(Integer.MIN_VALUE, untouched, 0);
        attempt(Integer.MAX_VALUE, untouched, 1);
        show("untouched", untouched);
        // A supplementary code point that does not fit: Java writes the low surrogate first, so index is unchanged.
        char[] small = new char[2];
        attempt(0x1D400, small, 1);
        show("small", small);
        attempt(0x1D400, small, 2);
        show("small end", small);
        // A negative index and a BMP code point at the end of the array.
        attempt('B', small, -1);
        attempt('B', small, 2);
        show("bmp end", small);
        // Every code point survives a round trip through toChars and toCodePoint.
        char[] scratch = new char[2];
        int mismatches = 0;
        int ones = 0;
        int twos = 0;
        for (int cp = 0; cp <= 0x10FFFF; cp++) {
            if (Character.isSurrogate((char) cp) && cp <= 0xFFFF) {
                continue;
            }
            int count = Character.toChars(cp, scratch, 0);
            if (count == 1) ones++;
            if (count == 2) twos++;
            if (count == 2 && Character.toCodePoint(scratch[0], scratch[1]) != cp) mismatches++;
            if (count == 1 && scratch[0] != (char) cp) mismatches++;
        }
        System.out.println(ones + " " + twos + " " + mismatches);
    }
}
