public class IntArithmetic {
    public static void main(String[] args) {
        int max = 2147483647;
        int min = -2147483648;
        System.out.println(max + 1);
        System.out.println(min - 1);
        System.out.println(max * 2);
        System.out.println(-min);
        System.out.println(min / -1);
        System.out.println(min % -1);
        System.out.println(7 / 2 + " " + -7 / 2 + " " + 7 % 3 + " " + -7 % 3 + " " + 7 % -3);
        int a = 17, b = 5;
        System.out.println(a / b + a % b * b);
        System.out.println(a & b);
        System.out.println(a | b);
        System.out.println(a ^ b);
        System.out.println(~a);
        System.out.println(1 << 31);
        System.out.println(1 << 32);
        System.out.println(a << 35);
        System.out.println(-17 >> 2);
        System.out.println(-17 >>> 2);
        System.out.println(-17 >>> 0);
        int s = 33;
        System.out.println(a << s);
        System.out.println(-a >>> s);
        System.out.println(0x7fffffff + 0xFF + 0b1010 + 017 + 1_000_000);
        System.out.println(0xFFFFFFFF);
        System.out.println(Math.abs(min) + " " + Math.abs(-5) + " " + Math.max(3, 9) + " " + Math.min(3, 9));
        int x = 10;
        x += 5;
        x -= 3;
        x *= 1000000;
        x /= 7;
        x %= 1000;
        x <<= 2;
        x >>= 1;
        x >>>= 1;
        x &= 0xFF;
        x |= 0x100;
        x ^= 3;
        System.out.println(x);
        x += 2.7;
        System.out.println(x);
        x = 100;
        x *= 0.5;
        System.out.println(x);
    }
}
