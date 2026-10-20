public class RuntimeShiftCounts {
    static int amount = 33;
    static long lamount = 65L;
    static byte sbyte(int v) { return (byte) v; }
    static char cval(int v) { return (char) v; }
    public static void main(String[] args) {
        int n = amount;
        long ln = lamount;
        System.out.println(1 << ln);
        System.out.println(1L << n);
        System.out.println(-8 >> ln);
        System.out.println(-8L >>> n);
        System.out.println(1 << (n + 1L));
        System.out.println(0x80000000 >>> (n - 1));
        byte b = -100;
        int sh = 9;
        System.out.println(b >> sh);
        System.out.println(b >>> sh);
        System.out.println((byte) (b >>> 1));
        char c = 'Z';
        System.out.println(c >> sh);
        System.out.println(~c);
        System.out.println(~b);
        System.out.println(+c);
        System.out.println(-b);
        short s = Short.MIN_VALUE;
        System.out.println(-s);
        System.out.println(s * s);
        System.out.println((short) (s * 3));
        System.out.println(sbyte(300) + " " + cval(65601));
        System.out.println(cval(-1) + 0);
        long mixed = 1L << 40 | 5;
        System.out.println(mixed);
        System.out.println(mixed >>> 38);
        System.out.println(5 & -2L);
        System.out.println(0xFF ^ (byte) -1);
        System.out.println(-1 >> 31 & 0xFF);
    }
}
