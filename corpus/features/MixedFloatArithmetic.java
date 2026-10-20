public class MixedFloatArithmetic {
    public static void main(String[] args) {
        long big = 123456789123L;
        float f = big;
        System.out.println(f);
        System.out.println(big + 0.5f);
        System.out.println(1 / 3f);
        System.out.println(1 / 3f * 3 == 1);
        double d = 1e21;
        System.out.println(d);
        System.out.println(123456789012.0);
        System.out.println(1e-7 * 3);
        System.out.println(0.1 * 3);
        System.out.println(0.1f * 3);
        System.out.println((float) 0.1 * 3);
        System.out.println(3.0e-45f);
        System.out.println(16777216f + 1);
        System.out.println(0.5f + 0.25);
        float g = 7;
        g /= 2;
        System.out.println(g);
        g %= 1.2;
        System.out.println(g);
        System.out.println(-0.0f + 0.0f);
        System.out.println(1e308 * 10);
        System.out.println(-1e308 * 10);
        System.out.println(2.0f == 2.0);
        System.out.println(0.1f == 0.1);
        System.out.println((int) 'A' * 1.5f);
        System.out.println(Float.MIN_VALUE / 2);
        System.out.println(Math.ulp(1.0f));
        System.out.println(Math.nextUp(1.0));
        System.out.println(Math.E);
        System.out.println((double) Float.MAX_VALUE);
        System.out.println(Long.MAX_VALUE * 1.0f);
        System.out.println(Integer.MIN_VALUE * 1.0);
        System.out.println(Math.sqrt(2) * Math.sqrt(2));
        System.out.println(1.0 - 0.9);
        System.out.println(100.0 / 3.0);
        System.out.println(2e-3f);
        System.out.println(1.0e-3f);
    }
}
