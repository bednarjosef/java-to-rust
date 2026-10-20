public class FinalConstantFolding {
    static final int LIMIT = 1 << 20;
    static final long BIG = 1L << 40;
    public static void main(String[] args) {
        final int a = Integer.MAX_VALUE;
        final int b = a + 1;
        final long c = a * 2L;
        final char ch = 'a' + 2;
        final byte by = 10 * 12;
        final short sh = (short) 70000;
        final double d = 1 / 3;
        final double e = 1 / 3.0;
        final float f = 0.1f + 0.2f;
        final int neg = -2147483648;
        final long lneg = -9223372036854775808L;
        System.out.println(b);
        System.out.println(c);
        System.out.println(ch);
        System.out.println(by);
        System.out.println(sh);
        System.out.println(d);
        System.out.println(e);
        System.out.println(f);
        System.out.println(neg);
        System.out.println(lneg);
        System.out.println(LIMIT * LIMIT);
        System.out.println(BIG * 1024 * 1024);
        System.out.println(Integer.MAX_VALUE + 1 == Integer.MIN_VALUE);
        System.out.println('a' * 'b');
        System.out.println((char) ('a' + 25));
        System.out.println(0x7fffffff + 0x7fffffff);
        System.out.println(0xFFFFFFFF);
        System.out.println(0xFFFFFFFFL);
        System.out.println(077 + 0b1011);
        System.out.println(1_000_000 * 3000);
        System.out.println(5 / 2 * 2.0);
        System.out.println(0.1f * 3);
        System.out.println(1e2f + 1e-2);
        System.out.println(Long.MIN_VALUE + " " + Byte.MIN_VALUE + " " + Short.MAX_VALUE);
        System.out.println(Character.MAX_VALUE + 0);
        System.out.println(Float.MAX_VALUE * 2);
        System.out.println(Double.MIN_VALUE / 2);
    }
}
