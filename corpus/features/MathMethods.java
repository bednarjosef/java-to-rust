public class MathMethods {
    public static void main(String[] args) {
        // Exact integer arithmetic on values in range; the overflows are in the programs of their own.
        System.out.println(Math.addExact(2, 3) + " " + Math.subtractExact(10L, 4) + " " + Math.multiplyExact(3L, 4));
        System.out.println(Math.multiplyExact(-7L, 6) + " " + Math.negateExact(-5) + " " + Math.absExact(-5));
        System.out.println(Math.incrementExact(41) + " " + Math.decrementExact(0L) + " " + Math.toIntExact(-7L));
        System.out.println(Math.addExact(Integer.MAX_VALUE, 0) + " " + Math.multiplyExact(Long.MIN_VALUE, 1) + " " + Math.absExact(Integer.MIN_VALUE + 1));

        // floorDiv and floorMod round toward negative infinity; the remainder takes the divisor's sign.
        System.out.println(Math.floorDiv(-7, 2) + " " + Math.floorMod(-7, 3) + " " + Math.floorMod(7, -3));
        System.out.println(Math.floorDiv(-7L, 2) + " " + Math.floorMod(-7L, 3) + " " + Math.floorMod(-7L, 3L));
        System.out.println(Math.floorDiv(Integer.MIN_VALUE, -1) + " " + Math.floorMod(Long.MIN_VALUE, -1L));

        // round is half-up, so -2.5 goes to -2; the float overload returns an int.
        System.out.println(Math.round(2.5) + " " + Math.round(-2.5) + " " + Math.round(-2.6) + " " + Math.round(0.49999999999999994));
        System.out.println(Math.round(2.5f) + " " + Math.round(-2.5f) + " " + Math.round(Float.NaN) + " " + Math.round(1e30));
        System.out.println(Math.round(Double.NaN) + " " + Math.round(Double.NEGATIVE_INFINITY) + " " + Math.round(-0.0));
        // rint rounds half to even, unlike round.
        System.out.println(Math.rint(2.5) + " " + Math.rint(3.5) + " " + Math.rint(-2.5) + " " + Math.rint(-0.2));
        System.out.println(Math.floor(-1.5) + " " + Math.ceil(-1.5) + " " + Math.ceil(-0.5) + " " + Math.floor(2.0f));

        // signum, copySign and the sign of zero.
        System.out.println(Math.signum(-3.0) + " " + Math.signum(0.0) + " " + Math.signum(-0.0) + " " + Math.signum(Double.NaN));
        System.out.println(Math.signum(-2.0f) + " " + Math.copySign(1.0, -0.0) + " " + Math.copySign(2.0f, Float.NEGATIVE_INFINITY));
        System.out.println(Math.copySign(-3.0, 4.0) + " " + Math.copySign(0.0, -1.0) + " " + Math.copySign(-1.5f, 2.0f));

        // nextUp and nextDown step to the neighbouring value; zeros and infinities behave as Java's do.
        System.out.println(Math.nextUp(1.0) == 1.0 + Math.ulp(1.0));
        System.out.println(Math.nextDown(1.0f) + " " + Math.nextDown(Float.MIN_VALUE) + " " + Math.nextUp(-0.0f));
        System.out.println(Math.nextDown(0.0) + " " + Math.nextDown(Double.MIN_VALUE) + " " + Math.nextDown(Double.NEGATIVE_INFINITY));
        System.out.println(Math.nextUp(Double.MAX_VALUE) + " " + Math.nextDown(Double.NaN) + " " + Math.nextUp(Float.MAX_VALUE));
        System.out.println(Math.ulp(1.0) + " " + Math.ulp(1.0f) + " " + Math.ulp(0.0) + " " + Math.ulp(Double.MAX_VALUE));

        // clamp keeps a value between two bounds.
        System.out.println(Math.clamp(5L, 1, 3) + " " + Math.clamp(-4L, -2, 2) + " " + Math.clamp(2L, 1L, 9L));
        System.out.println(Math.clamp(-0.0, 0.0, 1.0) + " " + Math.clamp(-0.0, -1.0, -0.0) + " " + Math.clamp(Double.NEGATIVE_INFINITY, -2, 2));
        System.out.println(Math.clamp(Double.NaN, 0.0, Double.POSITIVE_INFINITY) + " " + Math.clamp(-0.0f, 0.0f, 1.0f));
        System.out.println(Math.clamp(0.5f, 0.0f, 1.0f) + " " + Math.clamp(7.5, -1.0, 2.0));

        // fma computes a * b + c with one rounding; hypot avoids overflow.
        System.out.println(Math.fma(2.0, 3.0, 1.0) + " " + Math.fma(1e308, 10.0, -1e308) + " " + Math.fma(2.0f, 3.0f, 0.5f));
        System.out.println(Math.hypot(3.0, 4.0) + " " + Math.hypot(1e200, 1e200) + " " + Math.hypot(Double.NaN, Double.NEGATIVE_INFINITY));

        // max and min: NaN wins, and -0.0 is below 0.0.
        System.out.println(Math.max(-0.0, 0.0) + " " + Math.min(-0.0, 0.0) + " " + Math.max(Double.NaN, 1.0) + " " + Math.min(1.0, Double.NaN));
        System.out.println(Math.max(-0.0f, 0.0f) + " " + Math.min(2.5f, -7.0f) + " " + Math.max(3, -4) + " " + Math.min(3L, 9L));
        System.out.println(Math.max(Integer.MIN_VALUE, -1) + " " + Math.min(Long.MAX_VALUE, 0L) + " " + Math.max(Float.NaN, 0.0f));

        // Logarithms, angles and cbrt for the values where the libraries agree.
        System.out.println(Math.log1p(0.0) + " " + Math.expm1(0.0) + " " + Math.cbrt(27.0) + " " + Math.cbrt(-8.0));
        System.out.println(Math.toRadians(180.0) + " " + Math.toDegrees(Math.PI) + " " + Math.toDegrees(0.0));
        System.out.println(StrictMath.floor(-0.5) + " " + StrictMath.ceil(-0.5) + " " + StrictMath.round(-2.5) + " " + StrictMath.rint(4.5));
        System.out.println(StrictMath.signum(-9.0) + " " + StrictMath.hypot(5.0, 12.0) + " " + StrictMath.cbrt(64.0));
        System.out.println(StrictMath.toRadians(90.0) + " " + StrictMath.copySign(3.0, -0.0) + " " + StrictMath.ulp(2.0f));
    }
}
