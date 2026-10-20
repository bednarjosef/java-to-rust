public class ModularArithmetic {
    static final long MOD = 1_000_000_007L;

    static long gcd(long a, long b) {
        while (b != 0) {
            long t = a % b;
            a = b;
            b = t;
        }
        return a;
    }

    static long lcm(long a, long b) {
        return a / gcd(a, b) * b;
    }

    /** Returns {g, x, y} with a*x + b*y = g = gcd(a, b). */
    static long[] extendedGcd(long a, long b) {
        if (b == 0) {
            return new long[] {a, 1, 0};
        }
        long[] r = extendedGcd(b, a % b);
        return new long[] {r[0], r[2], r[1] - (a / b) * r[2]};
    }

    static long modPow(long base, long exp, long mod) {
        long result = 1 % mod;
        long b = ((base % mod) + mod) % mod;
        long e = exp;
        while (e > 0) {
            if ((e & 1) == 1) {
                result = result * b % mod;
            }
            b = b * b % mod;
            e >>= 1;
        }
        return result;
    }

    static long modInverse(long a, long mod) {
        long[] r = extendedGcd(a, mod);
        if (r[0] != 1) {
            return -1;
        }
        return ((r[1] % mod) + mod) % mod;
    }

    static long mulMod(long a, long b, long mod) {
        // Russian-peasant multiplication: avoids overflow for moduli up to 2^62.
        long result = 0;
        long x = a % mod;
        long y = b % mod;
        while (y > 0) {
            if ((y & 1) == 1) {
                result = (result + x) % mod;
            }
            x = (x << 1) % mod;
            y >>= 1;
        }
        return result;
    }

    /** Deterministic Miller-Rabin for 64-bit inputs with the first twelve prime bases. */
    static boolean isPrime(long n) {
        if (n < 2) {
            return false;
        }
        long[] bases = {2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37};
        for (long p : bases) {
            if (n % p == 0) {
                return n == p;
            }
        }
        long d = n - 1;
        int s = 0;
        while (d % 2 == 0) {
            d /= 2;
            s++;
        }
        for (long a : bases) {
            long x = modPowLarge(a, d, n);
            if (x == 1 || x == n - 1) {
                continue;
            }
            boolean composite = true;
            for (int r = 1; r < s; r++) {
                x = mulMod(x, x, n);
                if (x == n - 1) {
                    composite = false;
                    break;
                }
            }
            if (composite) {
                return false;
            }
        }
        return true;
    }

    static long modPowLarge(long base, long exp, long mod) {
        long result = 1;
        long b = base % mod;
        long e = exp;
        while (e > 0) {
            if ((e & 1) == 1) {
                result = mulMod(result, b, mod);
            }
            b = mulMod(b, b, mod);
            e >>= 1;
        }
        return result;
    }

    /** Number of divisors, from the prime factorisation found by trial division. */
    static int divisorCount(long n) {
        int count = 1;
        long m = n;
        for (long p = 2; p * p <= m; p++) {
            int e = 0;
            while (m % p == 0) {
                m /= p;
                e++;
            }
            count *= (e + 1);
        }
        if (m > 1) {
            count *= 2;
        }
        return count;
    }

    static long binomialMod(int n, int k) {
        long[] fact = new long[n + 1];
        fact[0] = 1;
        for (int i = 1; i <= n; i++) {
            fact[i] = fact[i - 1] * i % MOD;
        }
        long numerator = fact[n];
        long denominator = fact[k] * fact[n - k] % MOD;
        return numerator * modPow(denominator, MOD - 2, MOD) % MOD;
    }

    public static void main(String[] args) {
        System.out.println("gcd(462, 1071) = " + gcd(462, 1071));
        System.out.println("lcm(21, 6) = " + lcm(21, 6));
        long[] eg = extendedGcd(240, 46);
        System.out.println("ext gcd(240, 46): g=" + eg[0] + " x=" + eg[1] + " y=" + eg[2]);
        System.out.println("inverse of 3 mod 11: " + modInverse(3, 11));
        System.out.println("inverse of 4 mod 8: " + modInverse(4, 8));
        System.out.println("2^62 mod 1e9+7 = " + modPow(2, 62, MOD));
        System.out.println("3^(10^18) mod 1e9+7 = " + modPow(3, 1_000_000_000_000_000_000L, MOD));
        System.out.println("C(100, 50) mod p = " + binomialMod(100, 50));

        long[] candidates = {1, 2, 97, 561, 7919, 1000000007L, 999999999989L, 4294967297L, 9223372036854775783L};
        for (long c : candidates) {
            System.out.println(c + " prime? " + isPrime(c));
        }

        int primeCount = 0;
        long last = 0;
        for (long n = 1_000_000_000L; primeCount < 25; n++) {
            if (isPrime(n)) {
                primeCount++;
                last = n;
            }
        }
        System.out.println("25th prime from 1e9: " + last);
        System.out.println("divisors of 720720: " + divisorCount(720720));
        System.out.println("lcm of 1..30: " + lcmUpTo(30));
    }

    static long lcmUpTo(int n) {
        long result = 1;
        for (int i = 2; i <= n; i++) {
            result = lcm(result, i);
        }
        return result;
    }
}
