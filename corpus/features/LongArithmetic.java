public class LongArithmetic {
    public static void main(String[] args) {
        long big = 9223372036854775807L;
        long small = -9223372036854775808L;
        System.out.println(big + 1);
        System.out.println(small - 1);
        System.out.println(small / -1);
        int i = 2000000000;
        long widened = i + i;
        long promoted = (long) i + i;
        System.out.println(widened + " " + promoted);
        long product = 1;
        for (int k = 1; k <= 25; k++) {
            product *= k;
        }
        System.out.println(product);
        System.out.println(1L << 63);
        System.out.println(1L << 64);
        System.out.println(-1L >>> 1);
        System.out.println((int) 4294967297L);
        System.out.println((int) big);
        System.out.println(Math.max(i, 5L) + Math.abs(small));
        long mixed = i * 3L / 7;
        System.out.println(mixed);
    }
}
