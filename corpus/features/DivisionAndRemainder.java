public class DivisionAndRemainder {
    static int zero() { return 0; }
    public static void main(String[] args) {
        System.out.println(7 / 2);
        System.out.println(-7 / 2);
        System.out.println(7 % -2);
        System.out.println(-7 % -2);
        System.out.println(7.0 / 0);
        System.out.println(-7.0 / 0);
        System.out.println(0.0 / 0);
        System.out.println(7.0 % 0);
        System.out.println(5 / 2.0);
        System.out.println((char) 65 / 2);
        System.out.println(5L / 2);
        System.out.println(-5L % 3L);
        byte b = 9;
        short s = -9;
        System.out.println(b / s);
        System.out.println(b % 4);
        System.out.println(Integer.MIN_VALUE / -1);
        System.out.println(Long.MIN_VALUE / -1L);
        int d = zero();
        System.out.println("before");
        System.out.println(10 / d);
        System.out.println("after");
    }
}
