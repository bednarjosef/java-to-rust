public class FloatingRemainders {
    public static void main(String[] args) {
        double a = -7.5, b = 2.0;
        float fa = -7.5f, fb = 2f;
        System.out.println(a % b);
        System.out.println(fa % fb);
        System.out.println(7.5 % -2.0);
        System.out.println(1.0 % 0.0);
        System.out.println(5 % 0.0f);
        System.out.println(-0.0 % 3);
        System.out.println(Math.round(2.5) + " " + Math.round(-2.5) + " " + Math.round(2.5f));
        System.out.println(Math.round(Double.NaN) + " " + Math.round(1e30) + " " + Math.round(-1e30f));
        System.out.println(Math.floor(-0.5) + " " + Math.ceil(-0.5) + " " + Math.rint(2.5));
        System.out.println(-0.0 + 0.0);
        System.out.println(0.0 * -1);
        System.out.println(-(0.0));
        System.out.println(1 / -0.0);
        System.out.println(Math.abs(-0.0f));
        System.out.println(Math.sqrt(-1) + " " + (0.0 / 0.0 == 0.0 / 0.0));
        char c = 65;
        c += 1;
        System.out.println(c);
        System.out.println((char) -1 + 0);
        System.out.println((int) (char) -1);
        System.out.println('a' + 1 + "");
        System.out.println("" + 'a' + 1);
        System.out.println(c + c);
        System.out.println("" + (char) (c + 1) + (c + 1));
        long big = Long.MIN_VALUE;
        System.out.println(big / -1);
        System.out.println(big % -1);
        System.out.println(Integer.MIN_VALUE % -1);
        System.out.println(big + " " + (-big));
        System.out.println((float) big + " " + (double) Long.MAX_VALUE);
        System.out.println(0.1 + 0.2);
        System.out.println(1.0f / 3);
        System.out.println(100.0 / 3);
        System.out.println((float) 1e-46 + " " + 1e-300 * 1e-300);
    }
}
