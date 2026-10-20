public class NumberTheory {
    static int digitSum(long n) {
        int sum = 0;
        while (n > 0) {
            sum += n % 10;
            n /= 10;
        }
        return sum;
    }

    static boolean isPerfect(int n) {
        int sum = 1;
        for (int d = 2; d * d <= n; d++) {
            if (n % d == 0) {
                sum += d;
                if (d != n / d) sum += n / d;
            }
        }
        return n > 1 && sum == n;
    }

    static double newtonSqrt(double x) {
        double guess = x / 2;
        for (int i = 0; i < 20; i++) {
            guess = (guess + x / guess) / 2;
        }
        return guess;
    }

    static long modPow(long base, long exp, long mod) {
        long result = 1;
        base %= mod;
        while (exp > 0) {
            if (exp % 2 == 1) result = result * base % mod;
            base = base * base % mod;
            exp /= 2;
        }
        return result;
    }

    static int countPrimes(int limit) {
        int count = 0;
        for (int n = 2; n < limit; n++) {
            boolean prime = true;
            for (int d = 2; d * d <= n && prime; d++) {
                prime = n % d != 0;
            }
            if (prime) count++;
        }
        return count;
    }

    static String toBinary(int n) {
        if (n == 0) return "0";
        String bits = "";
        while (n != 0) {
            bits = (n & 1) + bits;
            n >>>= 1;
        }
        return bits;
    }

    public static void main(String[] args) {
        System.out.println("digit sum of 9876543210: " + digitSum(9876543210L));
        System.out.print("perfect numbers below 10000:");
        for (int i = 1; i < 10000; i++) {
            if (isPerfect(i)) System.out.print(" " + i);
        }
        System.out.println();
        System.out.println("sqrt(10) ~ " + newtonSqrt(10) + " vs " + Math.sqrt(10));
        System.out.println("3^200 mod 1000000007 = " + modPow(3, 200, 1000000007L));
        System.out.println("primes below 20000: " + countPrimes(20000));
        System.out.println("binary of 2026: " + toBinary(2026) + ", of -1: " + toBinary(-1));
        long hash = 7;
        for (int i = 0; i < 100; i++) {
            hash = hash * 31 + i;
        }
        System.out.println("hash: " + hash);
    }
}
