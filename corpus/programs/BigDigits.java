public class BigDigits {
    static final int BASE = 10000;

    /** Non-negative big integers as little-endian arrays of base-10000 limbs. */
    static int[] fromLong(long value) {
        int count = 1;
        long v = value / BASE;
        while (v > 0) {
            count++;
            v /= BASE;
        }
        int[] limbs = new int[count];
        long rest = value;
        for (int i = 0; i < count; i++) {
            limbs[i] = (int) (rest % BASE);
            rest /= BASE;
        }
        return trim(limbs);
    }

    static int[] trim(int[] a) {
        int n = a.length;
        while (n > 1 && a[n - 1] == 0) {
            n--;
        }
        if (n == a.length) {
            return a;
        }
        int[] out = new int[n];
        for (int i = 0; i < n; i++) {
            out[i] = a[i];
        }
        return out;
    }

    static int[] add(int[] a, int[] b) {
        int n = Math.max(a.length, b.length) + 1;
        int[] out = new int[n];
        int carry = 0;
        for (int i = 0; i < n; i++) {
            int sum = carry;
            if (i < a.length) {
                sum += a[i];
            }
            if (i < b.length) {
                sum += b[i];
            }
            out[i] = sum % BASE;
            carry = sum / BASE;
        }
        return trim(out);
    }

    static int[] multiply(int[] a, int[] b) {
        int[] out = new int[a.length + b.length];
        for (int i = 0; i < a.length; i++) {
            long carry = 0;
            for (int j = 0; j < b.length; j++) {
                long cur = out[i + j] + (long) a[i] * b[j] + carry;
                out[i + j] = (int) (cur % BASE);
                carry = cur / BASE;
            }
            int k = i + b.length;
            while (carry > 0) {
                long cur = out[k] + carry;
                out[k] = (int) (cur % BASE);
                carry = cur / BASE;
                k++;
            }
        }
        return trim(out);
    }

    static int[] multiplySmall(int[] a, int factor) {
        return multiply(a, fromLong(factor));
    }

    static String toText(int[] a) {
        String text = "" + a[a.length - 1];
        for (int i = a.length - 2; i >= 0; i--) {
            String limb = "" + a[i];
            for (int pad = limb.length(); pad < 4; pad++) {
                text = text + "0";
            }
            text = text + limb;
        }
        return text;
    }

    static int digitSum(int[] a) {
        int sum = 0;
        for (int limb : a) {
            int v = limb;
            while (v > 0) {
                sum += v % 10;
                v /= 10;
            }
        }
        return sum;
    }

    static int[] factorial(int n) {
        int[] result = fromLong(1);
        for (int i = 2; i <= n; i++) {
            result = multiplySmall(result, i);
        }
        return result;
    }

    /** Fibonacci numbers by repeated addition; returns F(n). */
    static int[] fibonacci(int n) {
        int[] a = fromLong(0);
        int[] b = fromLong(1);
        for (int i = 0; i < n; i++) {
            int[] next = add(a, b);
            a = b;
            b = next;
        }
        return a;
    }

    static int compare(int[] a, int[] b) {
        if (a.length != b.length) {
            return a.length < b.length ? -1 : 1;
        }
        for (int i = a.length - 1; i >= 0; i--) {
            if (a[i] != b[i]) {
                return a[i] < b[i] ? -1 : 1;
            }
        }
        return 0;
    }

    public static void main(String[] args) {
        int[] f30 = factorial(30);
        System.out.println("30! = " + toText(f30));
        System.out.println("digit sum of 30! = " + digitSum(f30));

        int[] f100 = factorial(100);
        System.out.println("digit sum of 100! = " + digitSum(f100));
        System.out.println("100! has " + toText(f100).length() + " digits");

        int[] fib = fibonacci(150);
        System.out.println("F(150) = " + toText(fib));

        int[] two = fromLong(2);
        int[] power = fromLong(1);
        for (int i = 0; i < 200; i++) {
            power = multiply(power, two);
        }
        System.out.println("2^200 = " + toText(power));
        System.out.println("digit sum of 2^200 = " + digitSum(power));

        System.out.println("compare 2^200 and 100!: " + compare(power, f100));
        System.out.println("compare 100! and 100!: " + compare(f100, factorial(100)));

        // Squaring a 15-digit number: the product spans several limbs.
        int[] x = fromLong(123456789012345L);
        int[] sq = multiply(x, x);
        System.out.println("square = " + toText(sq));
        System.out.println("limbs of square: " + sq.length);
    }
}
