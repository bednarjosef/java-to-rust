public class UnsignedArithmetic {
    public static void main(String[] args) {
        int big = -1;
        long huge = -2L;
        System.out.println(Integer.divideUnsigned(big, 7) + " " + Integer.remainderUnsigned(big, 10));
        System.out.println(Long.divideUnsigned(huge, 3L) + " " + Long.remainderUnsigned(huge, 1000L));
        System.out.println(Integer.compareUnsigned(big, 1) + " " + Integer.compareUnsigned(1, big));
        System.out.println(Long.compareUnsigned(5L, 5L) + " " + Long.compareUnsigned(-5L, 5L));
        System.out.println(Integer.toUnsignedString(big) + " " + Integer.toUnsignedString(big, 16));
        System.out.println(Integer.toUnsignedString(-8, 2) + " " + Integer.toUnsignedString(35, 36));
        System.out.println(Integer.toUnsignedString(42, 99) + " " + Long.toUnsignedString(huge, 36));
        System.out.println(Long.toUnsignedString(big) + " " + Integer.toUnsignedLong(big));
        System.out.println(Integer.parseUnsignedInt("4294967295") + " " + Integer.parseUnsignedInt("+ff", 16));
        System.out.println(Long.parseUnsignedLong("18446744073709551615") + " " + Long.parseUnsignedLong("77", 8));
        byte b = (byte) -1;
        byte small = (byte) -128;
        short s = (short) -1;
        short shortMinusTwo = (short) -2;
        System.out.println(Byte.toUnsignedInt(b) + " " + Byte.toUnsignedLong(small));
        System.out.println(Short.toUnsignedInt(s) + " " + Short.toUnsignedLong(shortMinusTwo));
        System.out.println(Integer.divideUnsigned(big, 2) + Integer.parseUnsignedInt("-1"));
    }
}
