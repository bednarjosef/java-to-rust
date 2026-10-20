public class MathFunctions {
    static final int FUNCTIONS = 19;

    static String name(int f) {
        String[] names = {"sin", "cos", "tan", "asin", "acos", "atan", "exp", "log", "log10", "sqrt", "cbrt",
            "expm1", "log1p", "sinh", "cosh", "tanh", "toRadians", "toDegrees", "ulp"};
        return names[f];
    }

    static double math(int f, double x) {
        if (f == 0) return Math.sin(x);
        if (f == 1) return Math.cos(x);
        if (f == 2) return Math.tan(x);
        if (f == 3) return Math.asin(x);
        if (f == 4) return Math.acos(x);
        if (f == 5) return Math.atan(x);
        if (f == 6) return Math.exp(x);
        if (f == 7) return Math.log(x);
        if (f == 8) return Math.log10(x);
        if (f == 9) return Math.sqrt(x);
        if (f == 10) return Math.cbrt(x);
        if (f == 11) return Math.expm1(x);
        if (f == 12) return Math.log1p(x);
        if (f == 13) return Math.sinh(x);
        if (f == 14) return Math.cosh(x);
        if (f == 15) return Math.tanh(x);
        if (f == 16) return Math.toRadians(x);
        if (f == 17) return Math.toDegrees(x);
        return Math.ulp(x);
    }

    static double strict(int f, double x) {
        if (f == 0) return StrictMath.sin(x);
        if (f == 1) return StrictMath.cos(x);
        if (f == 2) return StrictMath.tan(x);
        if (f == 3) return StrictMath.asin(x);
        if (f == 4) return StrictMath.acos(x);
        if (f == 5) return StrictMath.atan(x);
        if (f == 6) return StrictMath.exp(x);
        if (f == 7) return StrictMath.log(x);
        if (f == 8) return StrictMath.log10(x);
        if (f == 9) return StrictMath.sqrt(x);
        if (f == 10) return StrictMath.cbrt(x);
        if (f == 11) return StrictMath.expm1(x);
        if (f == 12) return StrictMath.log1p(x);
        if (f == 13) return StrictMath.sinh(x);
        if (f == 14) return StrictMath.cosh(x);
        if (f == 15) return StrictMath.tanh(x);
        if (f == 16) return StrictMath.toRadians(x);
        if (f == 17) return StrictMath.toDegrees(x);
        return StrictMath.ulp(x);
    }

    /// HotSpot replaces these `Math` functions with its own code, which differs from fdlibm (and
    /// from every libm the translation can use) in the last bit for some arguments, so their
    /// results are printed only for arguments where the translation is known to agree.
    static boolean intrinsic(int f) {
        return f == 0 || f == 1 || f == 2 || f == 6 || f == 7 || f == 10 || f == 13 || f == 15;
    }

    public static void main(String[] args) {
        double[] xs = {0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 3.0, 10.0, 100.0, 0.1, 1e-8, 1e-300, 4.9E-324,
            2.2250738585072014E-308, 1.7976931348623157E308, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY,
            Double.NaN, Math.PI, Math.PI / 2, Math.PI / 4, Math.E, 0.7853981633974483, 1e22, 1e300, -1e300,
            709.78, 710.0, -745.1, -746.0, 22.0, 0.9999999999999999, 1.0000000000000002, 1e9, 123456.789,
            -2.5, 36.7, 0.25, 7.0, 1e-5};
        for (int f = 0; f < FUNCTIONS; f++) {
            System.out.println(name(f));
            for (double x : xs) {
                if (intrinsic(f)) {
                    System.out.println("  " + x + " " + strict(f, x));
                } else {
                    System.out.println("  " + x + " " + math(f, x) + " " + strict(f, x));
                }
            }
        }

        // Many arguments: sums of the results catch any one that differs in the last bit.
        for (int f = 0; f < FUNCTIONS; f++) {
            double sumMath = 0.0;
            double sumStrict = 0.0;
            for (int i = 1; i <= 2000; i++) {
                double x = i * 0.0137 - 7.5;
                if (f == 3 || f == 4) {
                    x = x / 7.6;
                }
                sumStrict += strict(f, x);
                if (!intrinsic(f)) {
                    sumMath += math(f, x);
                }
            }
            System.out.println(name(f) + " sums " + sumMath + " " + sumStrict);
        }

        double[] simple = {0.0, -0.0, 1.0, 0.5, 2.0, 10.0, 100.0, 1e-8, Double.NaN, Double.POSITIVE_INFINITY};
        for (int f = 0; f < FUNCTIONS; f++) {
            if (intrinsic(f)) {
                System.out.print(name(f));
                for (double x : simple) {
                    System.out.print(" " + math(f, x));
                }
                System.out.println();
            }
        }

        // Two-argument functions.
        double[] ys = {0.0, -0.0, 1.0, -1.0, 2.0, -3.0, 0.5, 1.0 / 3, 1e10, -1e10, 1075.0, 1e308,
            Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, Double.NaN, 4.9E-324, 9007199254740991.0};
        double[] bases = {0.0, -0.0, 1.0, -1.0, 2.0, -2.0, -8.0, 0.5, 10.0, 1e-300, Double.POSITIVE_INFINITY,
            Double.NEGATIVE_INFINITY, Double.NaN, 1.0000000000000002, 0.9999999999999999};
        for (double b : bases) {
            for (double y : ys) {
                System.out.println("pow " + b + " " + y + " " + StrictMath.pow(b, y) + " atan2 " + Math.atan2(b, y)
                    + " " + StrictMath.atan2(b, y) + " hypot " + Math.hypot(b, y) + " " + StrictMath.hypot(b, y)
                    + " rem " + Math.IEEEremainder(b, y) + " " + StrictMath.IEEEremainder(b, y));
            }
        }
        System.out.println(Math.pow(2.0, 10.0) + " " + Math.pow(2.0, -1074.0) + " " + Math.pow(-8.0, 1.0 / 3)
            + " " + Math.pow(10.0, 15.0) + " " + Math.pow(0.0, -1.0) + " " + Math.pow(Double.NaN, 0.0));

        // Huge arguments to the trigonometric functions need the full reduction modulo pi/2.
        double[] huge = {1e15, 1e100, 1e200, 1.7976931348623157E308, 2.0 * 1073741824.0 * 1073741824.0, 5e-324};
        for (double x : huge) {
            System.out.println(StrictMath.sin(x) + " " + StrictMath.cos(x) + " " + StrictMath.tan(x));
        }

        // Rounding and the integer functions.
        double[] rs = {0.5, -0.5, 1.5, 2.5, -2.5, 0.49999999999999994, -0.49999999999999994, 4503599627370497.0,
            1e19, -1e19, Double.NaN, Double.POSITIVE_INFINITY, -0.0, 3.7, -3.7};
        for (double x : rs) {
            System.out.println(x + " floor " + Math.floor(x) + " ceil " + Math.ceil(x) + " rint " + Math.rint(x)
                + " round " + Math.round(x) + " " + Math.round((float) x) + " signum " + Math.signum(x) + " "
                + Math.signum((float) x) + " nextUp " + Math.nextUp(x) + " " + Math.nextUp((float) x) + " ulp "
                + Math.ulp((float) x) + " abs " + Math.abs(x));
        }
        int[] ns = {7, -7, 0, Integer.MIN_VALUE, Integer.MAX_VALUE, 13, -13};
        int[] ds = {3, -3, 1, -1, 7};
        for (int n : ns) {
            for (int d : ds) {
                System.out.print(Math.floorDiv(n, d) + "," + Math.floorMod(n, d) + " ");
            }
            long ln = n * 3000000000L;
            System.out.println(Math.floorDiv(ln, 7L) + " " + Math.floorMod(ln, -7L) + " " + Math.floorMod(ln, 5));
        }
        System.out.println(Math.fma(0.1, 10.0, -1.0) + " " + (0.1 * 10.0 - 1.0) + " " + Math.fma(2.0f, 3.0f, 1.0f)
            + " " + Math.fma(1e308, 10.0, -1e308) + " " + StrictMath.fma(-0.0, 0.0, 0.0));
        System.out.println(StrictMath.floor(-1.5) + " " + StrictMath.ceil(-0.5) + " " + StrictMath.rint(0.5) + " "
            + StrictMath.round(-1.5) + " " + StrictMath.abs(-3) + " " + StrictMath.max(1.0, -0.0) + " "
            + StrictMath.min(-0.0, 0.0) + " " + StrictMath.signum(-2.0));
    }
}
