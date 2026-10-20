public class DoubleAndFloatMethods {
    public static void main(String[] args) {
        // The tests of the special values, static and on a box.
        double nan = 0.0 / 0.0;
        double inf = 1.0 / 0.0;
        System.out.println(Double.isNaN(nan) + " " + Double.isNaN(inf) + " " + Double.isNaN(-0.0));
        System.out.println(Double.isInfinite(-inf) + " " + Double.isInfinite(Double.MAX_VALUE) + " " + Double.isInfinite(nan));
        System.out.println(Double.isFinite(Double.MIN_VALUE) + " " + Double.isFinite(inf) + " " + Double.isFinite(nan));
        Double boxed = nan;
        System.out.println(boxed.isNaN() + " " + boxed.isInfinite() + " " + Double.isFinite(boxed));
        Float f = Float.NEGATIVE_INFINITY;
        System.out.println(f.isInfinite() + " " + Float.isFinite(f) + " " + f.isNaN());
        System.out.println(Float.isNaN(Float.NaN) + " " + Float.isInfinite(Float.MAX_VALUE) + " " + Float.isFinite(Float.MIN_VALUE));

        // The constants.
        System.out.println(Double.MAX_VALUE + " " + Double.MIN_VALUE + " " + Double.MIN_NORMAL);
        System.out.println(Float.MAX_VALUE + " " + Float.MIN_VALUE + " " + Float.MIN_NORMAL);
        System.out.println(Double.POSITIVE_INFINITY + " " + Double.NEGATIVE_INFINITY + " " + Double.NaN);
        System.out.println(Float.POSITIVE_INFINITY + " " + Float.NEGATIVE_INFINITY + " " + Float.NaN);
        System.out.println(Double.MIN_NORMAL == Double.MIN_VALUE * 2048.0);

        // compare: by value, then -0.0 below 0.0, and NaN above everything.
        System.out.println(Double.compare(-0.0, 0.0) + " " + Double.compare(0.0, -0.0) + " " + Double.compare(nan, inf));
        System.out.println(Double.compare(nan, nan) + " " + Double.compare(1.5, 1.5) + " " + Double.compare(2.0, 1.0));
        System.out.println(Float.compare(-0.0f, 0.0f) + " " + Float.compare(Float.NaN, 1.0f) + " " + Float.compare(3.0f, 4.0f));

        // sum, max and min follow Math.max and Math.min.
        System.out.println(Double.sum(1.5, 2.25) + " " + Double.sum(inf, -inf) + " " + Float.sum(0.1f, 0.2f));
        System.out.println(Double.max(-0.0, 0.0) + " " + Double.min(-0.0, 0.0) + " " + Double.max(nan, 1.0));
        System.out.println(Float.max(1.0f, Float.NaN) + " " + Float.min(-0.0f, 0.0f) + " " + Float.max(2.5f, -7.0f));

        // toString prints as the string conversion does.
        System.out.println(Double.toString(100.0) + " " + Double.toString(1e7) + " " + Double.toString(1.0E-5));
        System.out.println(Float.toString(1.0e10f) + " " + Float.toString(0.1f) + " " + Double.toString(-0.0));

        // parseDouble and valueOf take Java's spellings, with surrounding white space.
        System.out.println(Double.parseDouble(" 1.5 ") + " " + Double.parseDouble("-0.0") + " " + Double.parseDouble("1e400"));
        System.out.println(Double.parseDouble("NaN") + " " + Double.parseDouble("-Infinity") + " " + Double.parseDouble("+.5d"));
        System.out.println(Double.valueOf("2.5f") + " " + Double.valueOf("1e-400") + " " + Double.parseDouble("0x1p3"));
        System.out.println(Double.valueOf(7).doubleValue() + " " + Double.valueOf("3.25").floatValue());
    }
}
