public class SmallIntegers {
    static byte half(byte b) {
        return (byte) (b / 2);
    }

    static short square(short s) {
        return (short) (s * s);
    }

    static int widen(byte b, short s, char c) {
        return b + s + c;
    }

    static long total(short s) {
        return s;
    }

    static void show(byte b) {
        System.out.println("byte " + b);
    }

    static void show(short s) {
        System.out.println("short " + s);
    }

    static void show(int i) {
        System.out.println("int " + i);
    }

    static byte checksum(int n) {
        byte sum = 0;
        for (int i = 0; i < n; i++) {
            sum += i;
        }
        return sum;
    }

    public static void main(String[] args) {
        byte b = 10;
        short s = -300;
        System.out.println(b + " " + s + " " + (b + s) + " " + (b * s));

        // Narrowing casts keep the low bits.
        System.out.println((byte) 200 + " " + (byte) 128 + " " + (byte) -129 + " " + (byte) 255 + " " + (byte) 256);
        System.out.println((short) 70000 + " " + (short) 32768 + " " + (short) -32769 + " " + (short) 65535);
        System.out.println((byte) 1000000000L + " " + (short) -1L + " " + (byte) 'é' + " " + (short) '￿');
        int big = 300;
        long huge = 1L << 40;
        System.out.println((byte) big + " " + (short) (big * 1000) + " " + (byte) huge + " " + (short) (huge + 5));

        // Floating values reach byte and short through int.
        System.out.println((byte) 300.5 + " " + (byte) -129.9 + " " + (byte) 1e10 + " " + (byte) -1e10 + " " + (byte) Double.NaN);
        System.out.println((short) 1e10f + " " + (short) 40000.7f + " " + (short) Float.NEGATIVE_INFINITY);
        double d = 200.9;
        System.out.println((byte) d + " " + (short) (d * 1000));

        // MIN and MAX values, and wrapping at them.
        byte bmax = Byte.MAX_VALUE;
        byte bmin = Byte.MIN_VALUE;
        short smax = Short.MAX_VALUE;
        short smin = Short.MIN_VALUE;
        System.out.println(bmax + " " + bmin + " " + smax + " " + smin);
        bmax++;
        bmin--;
        smax += 1;
        smin -= 1;
        System.out.println(bmax + " " + bmin + " " + smax + " " + smin);
        System.out.println(-bmin + " " + (byte) -bmin + " " + -smin + " " + (short) -smin + " " + Math.abs(bmin));

        // Compound assignment narrows implicitly.
        byte x = 100;
        x += 100;
        System.out.println(x);
        x *= 3;
        System.out.println(x);
        x -= 200;
        System.out.println(x);
        x /= 2;
        System.out.println(x);
        x %= 7;
        System.out.println(x);
        x = -128;
        x /= -1;
        System.out.println(x);
        x = 100;
        x += 1.5;
        System.out.println(x);
        x += 1000L;
        System.out.println(x);
        x <<= 3;
        System.out.println(x);
        x >>= 10;
        System.out.println(x);
        x = -16;
        x >>>= 2;
        System.out.println(x);
        x = 0x0F;
        x |= 0xF0;
        System.out.println(x);
        x &= 0x3C;
        System.out.println(x);
        x ^= -1;
        System.out.println(x);
        short y = 1000;
        y *= y;
        System.out.println(y);
        y <<= 20;
        System.out.println(y);
        y = 12345;
        y >>= 4;
        System.out.println(y);
        y = -7;
        y %= 3;
        System.out.println(y + " " + (y++) + " " + (++y) + " " + (y--) + " " + y);

        // Arithmetic is int arithmetic after promotion.
        byte p = 127;
        byte q = 1;
        int r = p + q;
        byte t = (byte) (p + q);
        System.out.println(r + " " + t + " " + (p << 1) + " " + (p >> 1) + " " + ~p + " " + (p & q) + " " + (p | -q));
        System.out.println(half((byte) -7) + " " + square((short) 200) + " " + square((short) 181) + " " + widen((byte) -1, (short) -1, 'A'));
        System.out.println(total(Short.MIN_VALUE) + " " + checksum(100) + " " + checksum(10));
        show(b);
        show(s);
        show(b + b);
        show((byte) (b + b));

        // Comparisons.
        System.out.println((b < s) + " " + (b == 10) + " " + (s != -300) + " " + (bmin < 0) + " " + (b > 'a'));
        int count = 0;
        for (byte i = -128; i < 127; i++) {
            count++;
        }
        System.out.println(count);
        short steps = 0;
        for (short i = 0; i < 1000; i += 7) {
            steps++;
        }
        System.out.println(steps);
        short limit = 5;
        int sum = 0;
        for (short i = 0; i < limit; i++) {
            sum += i;
        }
        boolean flag = sum > 3;
        short mixed = flag ? b : s;
        long widened = flag ? s : 7L;
        System.out.println(sum + " " + mixed + " " + widened + " " + (flag ? b : 'c') + " " + (flag ? (byte) -1 : 200));
        System.out.println(Integer.MAX_VALUE + " " + Integer.MIN_VALUE + " " + Long.MAX_VALUE + " " + Long.MIN_VALUE);
    }
}
