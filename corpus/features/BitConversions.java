public class BitConversions {
    static void showDouble(double d) {
        System.out.println(d + " bits " + Double.doubleToLongBits(d) + " raw " + Double.doubleToRawLongBits(d));
    }

    static void showFloat(float f) {
        System.out.println(f + " bits " + Float.floatToIntBits(f) + " raw " + Float.floatToRawIntBits(f));
    }

    static double divide(double a, double b) {
        return a / b;
    }

    public static void main(String[] args) {
        double[] doubles = {0.0, -0.0, 1.0, -1.0, 0.1, Double.MIN_VALUE, Double.MAX_VALUE, 2.2250738585072014E-308,
            Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, Double.NaN, 1e-310, Math.PI};
        for (double d : doubles) {
            showDouble(d);
        }
        float[] floats = {0.0f, -0.0f, 1.0f, 0.1f, Float.MIN_VALUE, Float.MAX_VALUE, Float.NaN, 1e-40f, -3.5f};
        for (float f : floats) {
            showFloat(f);
        }

        // A NaN computed at run time: x86 makes it with the sign bit set, and Java keeps those
        // bits. Rust leaves the sign of an arithmetic NaN unspecified (an optimized build folds
        // 0.0 / 0.0 to the positive NaN), so only the canonical bits are printed for these.
        double zero = 0.0;
        double nan = divide(zero, zero);
        System.out.println(nan + " " + Double.doubleToLongBits(nan) + " " + Double.doubleToLongBits(Math.sqrt(-1.0)));
        System.out.println(Float.floatToIntBits((float) nan) + " " + Double.doubleToRawLongBits(Math.abs(nan)));
        showDouble((double) Float.NaN);

        // NaN payloads survive longBitsToDouble and doubleToRawLongBits; doubleToLongBits
        // collapses them.
        long[] patterns = {0x7ff0000000000001L, 0x7ff8000000000123L, 0xfff8000000000000L, 0xfff0000000000abcL,
            0x7ff0000000000000L, 1L, 0x8000000000000000L, 0x3ff0000000000001L, -1L};
        for (long bits : patterns) {
            double d = Double.longBitsToDouble(bits);
            System.out.println(bits + " -> " + d + " -> " + Double.doubleToRawLongBits(d) + " / "
                + Double.doubleToLongBits(d));
        }
        int[] floatPatterns = {0x7f800001, 0x7fc00123, 0xffc00000, 0x7f800000, 1, 0x80000000, 0x3f800001, -1};
        for (int bits : floatPatterns) {
            float f = Float.intBitsToFloat(bits);
            System.out.println(bits + " -> " + f + " -> " + Float.floatToRawIntBits(f) + " / " + Float.floatToIntBits(f));
        }

        // Taking a double apart and putting it back together.
        double x = 6.02214076e23;
        long bits = Double.doubleToLongBits(x);
        long exponent = (bits >> 52) & 0x7ff;
        long mantissa = bits & 0xfffffffffffffL;
        System.out.println("exponent " + (exponent - 1023) + " mantissa " + mantissa);
        double rebuilt = Double.longBitsToDouble((exponent << 52) | mantissa);
        System.out.println(rebuilt + " " + (rebuilt == x));
        double next = Double.longBitsToDouble(bits + 1);
        System.out.println(next + " " + (next - x));
        int fbits = Float.floatToIntBits(1.0f / 3.0f);
        System.out.println(fbits + " " + Float.intBitsToFloat(fbits + 1));
        // The bits in hexadecimal: normal, subnormal, zero, and the values that have no bits shown.
        double[] shown = {x, -2.0, 0.1, -0.0, Double.MAX_VALUE, Double.MIN_VALUE, next, Double.NaN, Double.NEGATIVE_INFINITY};
        for (double d : shown) {
            System.out.println(Double.toHexString(d) + " " + Math.getExponent(d) + " " + Math.getExponent((float) d));
        }
    }
}
