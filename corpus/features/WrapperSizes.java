// The widths the wrapper classes give as constants: SIZE in bits and BYTES in bytes.
public class WrapperSizes {
    static int log2(int scale) {
        return Integer.numberOfTrailingZeros(scale);
    }

    public static void main(String[] args) {
        System.out.println(Byte.SIZE + " " + Byte.BYTES);
        System.out.println(Short.SIZE + " " + Short.BYTES);
        System.out.println(Character.SIZE + " " + Character.BYTES);
        System.out.println(Integer.SIZE + " " + Integer.BYTES);
        System.out.println(Long.SIZE + " " + Long.BYTES);
        System.out.println(Float.SIZE + " " + Float.BYTES);
        System.out.println(Double.SIZE + " " + Double.BYTES);
        int[] widths = new int[Long.BYTES];
        for (int i = 0; i < widths.length; i++) {
            widths[i] = (i + 1) * Byte.SIZE;
        }
        System.out.println(widths[Long.BYTES - 1] + " " + log2(Byte.SIZE) + " " + (Integer.SIZE - Integer.numberOfLeadingZeros(1000)));
    }
}
