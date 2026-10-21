public class IdiomaticMixedOperators {
    public static void main(String[] args) {
        int a = 6;
        int b = 5;
        int c = 3;
        System.out.println(a + b & 0xff);
        System.out.println(a * b | c);
        System.out.println(a + b ^ c * 2);
        System.out.println(a + b << c);
        System.out.println(a * b >> c - 1);
        System.out.println((a & b) + c);
        System.out.println(a - b >>> 1);
        long wide = 3000000000L;
        System.out.println(wide + a << 2);
        System.out.println(wide * 4 & 0xffffffffL);
        int max = Integer.MAX_VALUE;
        System.out.println(max + 1 >> 1);
        System.out.println(max * 2 | 1);
        char letter = 'c';
        System.out.println(letter + 1 & 0x7f);
        byte small = 100;
        System.out.println(small + small & 0xff);
        System.out.println(-a + b << 1);
    }
}
