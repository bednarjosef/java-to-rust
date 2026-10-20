// Overflows that the range analysis must not rule out; the translated Rust is built with
// overflow checks, so plain arithmetic in any of these places would panic.
public class OverflowAnalysis {
    static int next(int x) {
        return x + 1;
    }

    static int belowSix(int n) {
        if (n > 5) return 0;
        return n - 1;
    }

    static int lastSquareRoot(int n) {
        int d = 2;
        while (d * d <= n) {
            d++;
        }
        return d;
    }

    static long sumTo(long n) {
        long total = 0;
        for (long i = 0; i < n; i++) {
            total += i * 1000000000L;
        }
        return total;
    }

    public static void main(String[] args) {
        int counter = 0;
        for (int i = 0; i < 30000; i++) {
            counter += 100000;
        }
        System.out.println("counter: " + counter);

        int nested = 0;
        for (int i = 0; i < 1000; i++) {
            for (int j = 0; j < 1000; j++) {
                nested += 3000;
            }
        }
        System.out.println("nested: " + nested);

        int bounded = 0;
        for (int i = 0; i < 1000; i++) {
            if (i % 3 == 0) bounded += 2;
            else bounded++;
        }
        System.out.println("bounded: " + bounded);

        int p = 1;
        int doublings = 0;
        while (p > 0) {
            p *= 2;
            doublings++;
        }
        System.out.println("doublings until overflow: " + doublings + ", p = " + p);

        int down = 0;
        for (int i = 0; i < 10; i++) {
            down--;
        }
        int min = -2147483648;
        down += min;
        System.out.println("down: " + down);

        System.out.println("next(max): " + next(2147483647));
        System.out.println("belowSix(min): " + belowSix(-2147483648));
        System.out.println("lastSquareRoot(2147483000): " + lastSquareRoot(2147483000));
        System.out.println("lastSquareRoot(1000000): " + lastSquareRoot(1000000));
        System.out.println("sumTo(100000): " + sumTo(100000) + ", sumTo(200000): " + sumTo(200000));

        int minusOne = -1;
        int big = min;
        System.out.println("min / -1: " + big / minusOne + ", min % -1: " + big % minusOne);
        System.out.println("abs(min): " + Math.abs(big) + ", -min: " + -big);

        long fact = 1;
        for (int i = 1; i <= 30; i++) {
            fact *= i;
        }
        System.out.println("30! wrapped: " + fact);

        int mixed = 7;
        for (int i = 0; i < 100; i++) {
            mixed = mixed * 31 + i;
        }
        System.out.println("mixed: " + mixed);
    }
}
