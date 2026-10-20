public class Doubles {
    public static void main(String[] args) {
        System.out.println(1.0);
        System.out.println(100.0);
        System.out.println(0.1 + 0.2);
        System.out.println(1.0 / 3);
        System.out.println(2.0 / 3);
        System.out.println(0.001);
        System.out.println(0.0001);
        System.out.println(1234567.0);
        System.out.println(12345678.9);
        System.out.println(1e7);
        System.out.println(1e21);
        System.out.println(1.0e-10);
        System.out.println(-0.0);
        System.out.println(0.0 / 0.0);
        System.out.println(1.0 / 0);
        System.out.println(-1 / 0.0);
        System.out.println(4.9e-324);
        System.out.println(1.7976931348623157e308);
        System.out.println(2e23);
        System.out.println(5 / 2 + 5 / 2.0);
        double d = 7.5;
        d %= 2;
        System.out.println(d + " " + -7.5 % 2 + " " + 7.5 % -2);
        System.out.println((int) 3.99 + " " + (int) -3.99 + " " + (int) 1e20 + " " + (int) (0.0 / 0.0));
        System.out.println((long) 1e19 + " " + (long) -2.5);
        System.out.println(Math.sqrt(2) + " " + Math.pow(2, 10) + " " + Math.pow(2, 0.5));
        System.out.println(Math.max(-0.0, 0.0) + " " + Math.min(-0.0, 0.0) + " " + Math.max(1.0, 0.0 / 0.0));
        System.out.println(Math.abs(-2.5) + " " + Math.max(1, 2.5));
        double sum = 0;
        for (int i = 1; i <= 10; i++) {
            sum += 1.0 / i;
        }
        System.out.println("harmonic(10) = " + sum);
        double x = 3;
        x++;
        x--;
        ++x;
        System.out.println(x + " " + -x + " " + (x > 3.5) + " " + (x == 4.0));
    }
}
