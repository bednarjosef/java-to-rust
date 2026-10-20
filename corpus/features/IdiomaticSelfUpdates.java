public class IdiomaticSelfUpdates {
    public static void main(String[] args) {
        // Values that just fit are plain sums; values that just wrap must still wrap.
        int fits = 2147483000;
        fits = fits + 647;
        System.out.println(fits);
        int wraps = 2147483000;
        wraps = wraps + 648;
        System.out.println(wraps);
        int product = 7;
        product = product * 1103515245;
        System.out.println(product);
        int difference = -2147483648;
        difference = difference - 1;
        System.out.println(difference);
        int quotient = Integer.MIN_VALUE;
        quotient = quotient / -1;
        System.out.println(quotient);
        int remainder = -17;
        remainder = remainder % 5;
        System.out.println(remainder);

        int shifted = 1000;
        shifted = shifted << 33;
        System.out.println(shifted);
        shifted = shifted >> 1;
        System.out.println(shifted);
        shifted = shifted >>> 1;
        System.out.println(shifted);

        long big = 9000000000L;
        big = big * 3;
        System.out.println(big);
        big = big - 1;
        System.out.println(big);

        double x = 1.5;
        x = x * 2;
        x = x / 0.0;
        System.out.println(x);
        float f = 0.1f;
        f = f + 0.2f;
        System.out.println(f);

        boolean both = true;
        both = both & false;
        System.out.println(both);
        boolean either = false;
        either = either | true;
        System.out.println(either);
        boolean differ = true;
        differ = differ ^ true;
        System.out.println(differ);

        int bits = 13;
        bits = bits & 6;
        bits = bits | 8;
        bits = bits ^ 1;
        System.out.println(bits);

        // Compound shifts with literal counts: Java masks the count to the width of the type.
        int packed = 3;
        packed <<= 33;
        System.out.println(packed);
        packed >>= -1;
        System.out.println(packed);
        long wide = 1;
        wide <<= 65;
        System.out.println(wide);
        wide >>= 63;
        System.out.println(wide);
    }
}
