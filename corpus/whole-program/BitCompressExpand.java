// `Integer.compress`, `Integer.expand`, `Long.compress` and `Long.expand`, with masks of no, some
// and every bit, the sign bit, and values read from variables and constants.
public class BitCompressExpand {
    public static void main(String[] args) {
        int[] ints = { 0, 1, -1, 0xCAFEBABE, 0x12345678, Integer.MIN_VALUE, Integer.MAX_VALUE };
        int[] intMasks = { 0, -1, 0xFF00, 0x80000001, 0x55555555, 0xF0F0F0F0, Integer.MIN_VALUE };
        for (int i : ints) {
            StringBuilder line = new StringBuilder();
            for (int m : intMasks) {
                line.append(Integer.compress(i, m)).append(' ').append(Integer.expand(i, m)).append(' ');
                if (Integer.expand(Integer.compress(i, m), m) != (i & m)) {
                    line.append("! ");
                }
            }
            System.out.println(line);
        }
        long[] longs = { 0L, 1L, -1L, 0xCAFEBABEDEADBEEFL, Long.MIN_VALUE, Long.MAX_VALUE, 42L };
        long[] longMasks = { 0L, -1L, 0xFFFF0000FFFF0000L, 0x8000000000000001L, 0x5555555555555555L, 0xFFL };
        for (long l : longs) {
            StringBuilder line = new StringBuilder();
            for (long m : longMasks) {
                line.append(Long.compress(l, m)).append(' ').append(Long.expand(l, m)).append(' ');
            }
            System.out.println(line);
        }
        System.out.println(Integer.compress(0b1011_0110, 0b0110_0110) + " " + Long.expand(0xABCDL, 0xF0F0F0F0L));
        System.out.println(Integer.compress(-2, 0xF0F) + " " + Integer.expand(-16, -256) + " "
            + Long.compress(-5L, Long.MIN_VALUE | 7) + " " + Long.expand(-3L, 0xFFL));
    }
}
