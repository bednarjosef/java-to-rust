// The bit and radix methods of Integer and Long: unsigned hex, binary and octal text, counting
// and finding bits, reversing and rotating, and text in any radix. The last line parses text that
// is not a number in the radix, which throws NumberFormatException.
public class IntegerBits {
    static int log2(int n) {
        return 31 - Integer.numberOfLeadingZeros(n);
    }

    public static void main(String[] args) {
        int[] ints = {0, 1, -1, 255, 1 << 20, Integer.MIN_VALUE, Integer.MAX_VALUE, 0x12345678, -123456};
        for (int v : ints) {
            System.out.println(v + ": " + Integer.toHexString(v) + " " + Integer.toBinaryString(v) + " "
                    + Integer.toOctalString(v));
            System.out.println("  " + Integer.numberOfLeadingZeros(v) + " " + Integer.numberOfTrailingZeros(v)
                    + " " + Integer.bitCount(v) + " " + Integer.highestOneBit(v) + " " + Integer.lowestOneBit(v)
                    + " " + Integer.signum(v));
            System.out.println("  " + Integer.reverse(v) + " " + Integer.reverseBytes(v) + " "
                    + Integer.rotateLeft(v, 3) + " " + Integer.rotateRight(v, 3) + " " + Integer.rotateLeft(v, -1));
        }
        long[] longs = {0L, -1L, 1L << 40, Long.MIN_VALUE, Long.MAX_VALUE, 0x123456789ABCDEFL};
        for (long v : longs) {
            System.out.println(v + ": " + Long.toHexString(v) + " " + Long.toBinaryString(v).length() + " "
                    + Long.toOctalString(v));
            System.out.println("  " + Long.numberOfLeadingZeros(v) + " " + Long.numberOfTrailingZeros(v) + " "
                    + Long.bitCount(v) + " " + Long.highestOneBit(v) + " " + Long.lowestOneBit(v) + " "
                    + Long.signum(v));
            System.out.println("  " + Long.reverse(v) + " " + Long.reverseBytes(v) + " " + Long.rotateLeft(v, 7)
                    + " " + Long.rotateRight(v, 70));
        }
        char c = 'A';
        short s = -2;
        System.out.println(Integer.toHexString(c) + " " + Integer.toHexString(s) + " " + Integer.bitCount(c));
        System.out.println(log2(1) + " " + log2(1000) + " " + log2(1 << 30));
        System.out.println(Integer.toString(255, 16) + " " + Integer.toString(-255, 2) + " " + Integer.toString(35, 36)
                + " " + Integer.toString(10, 99) + " " + Long.toString(Long.MIN_VALUE, 7));
        System.out.println(Integer.parseInt("ff", 16) + " " + Integer.parseInt("-1010", 2) + " " + Integer.parseInt("+z", 36)
                + " " + Long.parseLong("7fffffffffffffff", 16) + " " + Integer.parseInt("-80000000", 16));
        System.out.println(Integer.MAX_VALUE + " " + Integer.MIN_VALUE + " " + Long.MAX_VALUE + " " + Long.MIN_VALUE);
        System.out.println(Integer.parseInt("12g", 16));
    }
}
