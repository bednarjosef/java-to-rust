public class MultiplyHigh {
    static final long[] LONGS = {
        0, 1, -1, 2, -3, 1234567890123L, -987654321987L, Long.MIN_VALUE, Long.MAX_VALUE,
        Long.MIN_VALUE + 1, 1L << 32, -(1L << 32), 0x123456789abcdefL,
    };
    static final int[] INTS = {
        0, 1, -1, 2, -7, 46341, 65536, -123456789, Integer.MIN_VALUE, Integer.MAX_VALUE,
    };

    public static void main(String[] args) {
        long sum = 0;
        for (long x : LONGS) {
            for (long y : LONGS) {
                long high = Math.multiplyHigh(x, y);
                long unsigned = Math.unsignedMultiplyHigh(x, y);
                System.out.println(high + " " + unsigned + " " + StrictMath.multiplyHigh(x, y) + " "
                    + StrictMath.unsignedMultiplyHigh(x, y));
                sum += high ^ unsigned;
            }
        }
        System.out.println("sum " + sum);
        for (int x : INTS) {
            for (int y : INTS) {
                System.out.println(Math.multiplyFull(x, y) + " " + StrictMath.multiplyFull(x, y));
            }
        }
        System.out.println(Math.multiplyHigh(Long.MIN_VALUE, -1L));
        System.out.println(Math.multiplyHigh(Long.MIN_VALUE, Long.MIN_VALUE));
        System.out.println(Math.unsignedMultiplyHigh(-1L, -1L));
        System.out.println(Math.multiplyHigh(Long.MAX_VALUE, Long.MAX_VALUE));
        System.out.println(Math.multiplyHigh(1 << 30, 1 << 30));
        System.out.println(Math.multiplyFull(Integer.MIN_VALUE, Integer.MIN_VALUE));
        System.out.println(Math.multiplyFull(Integer.MIN_VALUE, -1));
        byte b = -100;
        short s = 300;
        char c = 'A';
        System.out.println(Math.multiplyFull(b, s) + Math.multiplyFull(c, c));
        System.out.println(Math.multiplyHigh(b, Long.MIN_VALUE) + Math.unsignedMultiplyHigh(s, -1));
        System.out.println(Math.multiplyFull(3, 4) * 5L + Math.multiplyHigh(3L, 4L));
    }
}
