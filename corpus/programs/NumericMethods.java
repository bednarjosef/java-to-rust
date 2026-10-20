public class NumericMethods {
    static final long ONE = 1L << 16;

    /** Newton's method for square roots, starting from x itself. */
    static double newtonSqrt(double x) {
        double guess = x > 1 ? x : 1;
        for (int i = 0; i < 60; i++) {
            double next = 0.5 * (guess + x / guess);
            if (Math.abs(next - guess) < 1e-15 * guess) {
                return next;
            }
            guess = next;
        }
        return guess;
    }

    /** Bisection on f(x) = x^3 - 2x - 5, which has a root between 2 and 3. */
    static double bisect(double lo, double hi) {
        double a = lo;
        double b = hi;
        int iterations = 0;
        while (b - a > 1e-12) {
            double mid = (a + b) / 2;
            double fm = mid * mid * mid - 2 * mid - 5;
            double fa = a * a * a - 2 * a - 5;
            if (fa * fm <= 0) {
                b = mid;
            } else {
                a = mid;
            }
            iterations++;
        }
        lastIterations = iterations;
        return (a + b) / 2;
    }

    static int lastIterations;

    /** Composite Simpson's rule with n (even) subintervals. */
    static double simpson(int n, double a, double b) {
        double h = (b - a) / n;
        double sum = f(a) + f(b);
        for (int i = 1; i < n; i++) {
            double x = a + i * h;
            sum += (i % 2 == 1 ? 4 : 2) * f(x);
        }
        return sum * h / 3;
    }

    static double f(double x) {
        return x * x * Math.sqrt(x + 1);
    }

    /** Trapezoid rule, for comparison with Simpson. */
    static double trapezoid(int n, double a, double b) {
        double h = (b - a) / n;
        double sum = (f(a) + f(b)) / 2;
        for (int i = 1; i < n; i++) {
            sum += f(a + i * h);
        }
        return sum * h;
    }

    /** Fixed-point arithmetic with 16 fractional bits, stored in a long. */
    static long fromInt(long v) {
        return v * ONE;
    }

    static long fixedMul(long a, long b) {
        return (a * b) >> 16;
    }

    static long fixedDiv(long a, long b) {
        return (a << 16) / b;
    }

    static String fixedText(long v) {
        boolean negative = v < 0;
        long mag = negative ? -v : v;
        long whole = mag >> 16;
        long frac = ((mag & (ONE - 1)) * 10000) >> 16;
        String fracText = "" + frac;
        while (fracText.length() < 4) {
            fracText = "0" + fracText;
        }
        return (negative ? "-" : "") + whole + "." + fracText;
    }

    /** Rounds to six decimal places using integer arithmetic only. */
    static String sixPlaces(double v) {
        boolean negative = v < 0;
        double mag = negative ? -v : v;
        long scaled = (long) Math.floor(mag * 1000000 + 0.5);
        long whole = scaled / 1000000;
        long frac = scaled % 1000000;
        String fracText = "" + frac;
        while (fracText.length() < 6) {
            fracText = "0" + fracText;
        }
        return (negative ? "-" : "") + whole + "." + fracText;
    }

    /** Kahan compensated summation of many small values. */
    static double kahanSum(double[] values) {
        double sum = 0;
        double compensation = 0;
        for (double v : values) {
            double y = v - compensation;
            double t = sum + y;
            compensation = (t - sum) - y;
            sum = t;
        }
        return sum;
    }

    static double naiveSum(double[] values) {
        double sum = 0;
        for (double v : values) {
            sum += v;
        }
        return sum;
    }

    /** Compound growth: balance after years with yearly rate and monthly deposits. */
    static double balance(double principal, double yearlyRate, int years, double monthlyDeposit) {
        double b = principal;
        double monthly = yearlyRate / 12;
        for (int m = 0; m < years * 12; m++) {
            b = b * (1 + monthly) + monthlyDeposit;
        }
        return b;
    }

    static int collatzSteps(long n) {
        int steps = 0;
        long x = n;
        while (x != 1) {
            x = (x % 2 == 0) ? x / 2 : 3 * x + 1;
            steps++;
        }
        return steps;
    }

    public static void main(String[] args) {
        double[] inputs = {2, 10, 0.25, 12345.678};
        for (double x : inputs) {
            System.out.println("sqrt(" + x + ") ~ " + sixPlaces(newtonSqrt(x)) + ", Math.sqrt agrees: " + (newtonSqrt(x) == Math.sqrt(x)));
        }

        double root = bisect(2, 3);
        System.out.println("root of x^3 - 2x - 5 ~ " + sixPlaces(root) + " after " + lastIterations + " halvings");
        System.out.println("check: " + sixPlaces(root * root * root - 2 * root - 5));

        double exact = 1.3 * 1.3 * 1.3 / 3.0;
        System.out.println("simpson n=4:  " + sixPlaces(simpson(4, 0, 2)));
        System.out.println("simpson n=64: " + sixPlaces(simpson(64, 0, 2)));
        System.out.println("trapezoid n=64: " + sixPlaces(trapezoid(64, 0, 2)));
        System.out.println("error of simpson n=64 is tiny: " + (Math.abs(simpson(64, 0, 2) - simpson(128, 0, 2)) < 1e-6));

        long a = fromInt(3);
        long b = fixedDiv(fromInt(1), fromInt(7));
        System.out.println("3 = " + fixedText(a) + ", 1/7 = " + fixedText(b));
        System.out.println("3 * 1/7 = " + fixedText(fixedMul(a, b)));
        System.out.println("-2.5 * 4 = " + fixedText(fixedMul(-(ONE * 5 / 2), fromInt(4))));
        System.out.println("fixed sqrt(2) by Newton: " + fixedText(fixedSqrt2()));

        double[] tenths = new double[1000];
        for (int i = 0; i < tenths.length; i++) {
            tenths[i] = 0.1;
        }
        System.out.println("naive 1000 x 0.1 = " + naiveSum(tenths));
        System.out.println("kahan 1000 x 0.1 = " + kahanSum(tenths));
        System.out.println("0.1 + 0.2 = " + (0.1 + 0.2));
        System.out.println("1e20 + 1 - 1e20 = " + (1e20 + 1 - 1e20));

        System.out.println("balance after 10 years: " + sixPlaces(balance(1000, 0.05, 10, 50)));
        System.out.println("balance after 30 years: " + sixPlaces(balance(0, 0.07, 30, 100)));

        int best = 0;
        long bestStart = 0;
        for (long n = 1; n < 10000; n++) {
            int s = collatzSteps(n);
            if (s > best) {
                best = s;
                bestStart = n;
            }
        }
        System.out.println("longest Collatz chain below 10000 starts at " + bestStart + " with " + best + " steps");
        System.out.println("exact third: " + sixPlaces(exact));
    }

    static long fixedSqrt2() {
        long x = fromInt(1);
        for (int i = 0; i < 8; i++) {
            x = (x + fixedDiv(fromInt(2), x)) / 2;
        }
        return x;
    }
}
