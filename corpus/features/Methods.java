public class Methods {
    static int gcd(int a, int b) {
        while (b != 0) {
            int t = a % b;
            a = b;
            b = t;
        }
        return a;
    }

    static boolean isPrime(int n) {
        if (n < 2) return false;
        for (int d = 2; d * d <= n; d++) {
            if (n % d == 0) return false;
        }
        return true;
    }

    static String describe(int x) {
        return "int " + x;
    }

    static String describe(double x) {
        return "double " + x;
    }

    static String describe(long x) {
        return "long " + x;
    }

    static String describe(String s) {
        return "string \"" + s + "\" of length " + s.length();
    }

    static double average(double a, double b) {
        return (a + b) / 2;
    }

    static long power(long base, int exp) {
        long result = 1;
        while (exp > 0) {
            if ((exp & 1) == 1) result *= base;
            base *= base;
            exp >>= 1;
        }
        return result;
    }

    static void greet(String name, int times) {
        for (int i = 0; i < times; i++) {
            System.out.println("Hi " + name + " #" + (i + 1));
        }
    }

    static int ackermann(int m, int n) {
        if (m == 0) return n + 1;
        if (n == 0) return ackermann(m - 1, 1);
        return ackermann(m - 1, ackermann(m, n - 1));
    }

    public static void main(String[] args) {
        System.out.println(gcd(1071, 462));
        int primes = 0;
        for (int i = 0; i < 1000; i++) {
            if (isPrime(i)) primes++;
        }
        System.out.println(primes + " primes below 1000");
        System.out.println(describe(42));
        System.out.println(describe(42L));
        System.out.println(describe(4.2));
        System.out.println(describe("hello"));
        System.out.println(average(3, 4));
        System.out.println(power(3, 40));
        greet("Ada", 2);
        System.out.println(ackermann(2, 3));
        Methods.greet("Bob", 1);
    }
}
