public class CeilingDivision {
    public static void main(String[] args) {
        // ceilDiv rounds toward positive infinity; ceilMod's remainder has the opposite sign of the divisor.
        System.out.println(Math.ceilDiv(7, 2) + " " + Math.ceilDiv(-7, 2) + " " + Math.ceilDiv(7, -2) + " " + Math.ceilDiv(-7, -2));
        System.out.println(Math.ceilMod(7, 3) + " " + Math.ceilMod(-7, 3) + " " + Math.ceilMod(7, -3) + " " + Math.ceilMod(-7, -3));
        System.out.println(Math.ceilDiv(-7L, 2L) + " " + Math.ceilMod(-7L, 3L) + " " + Math.ceilDiv(7L, -2L) + " " + Math.ceilMod(7L, -3L));

        // Exact quotients have no rounding and a zero remainder.
        System.out.println(Math.ceilDiv(-6, 3) + " " + Math.ceilMod(-6, 3) + " " + Math.ceilDiv(0, -5) + " " + Math.ceilMod(0L, -5L));

        // The mixed forms: ceilDiv(long, int) is a long, ceilMod(long, int) an int.
        long big = 9_000_000_000L;
        System.out.println(Math.ceilDiv(big, 4) + " " + Math.ceilDiv(big + 1, 4) + " " + Math.ceilMod(big + 1, 4));
        System.out.println(Math.ceilDiv(big + 1, -4) + " " + Math.ceilMod(-big - 1, 4) + " " + Math.ceilMod(big, -7));

        // MIN_VALUE divided by -1 wraps for ceilDiv and ceilMod, as Java's does.
        System.out.println(Math.ceilDiv(Integer.MIN_VALUE, -1) + " " + Math.ceilMod(Integer.MIN_VALUE, -1));
        System.out.println(Math.ceilDiv(Long.MIN_VALUE, -1L) + " " + Math.ceilMod(Long.MIN_VALUE, -1L));
        System.out.println(Math.ceilDiv(Integer.MIN_VALUE, 2) + " " + Math.ceilMod(Integer.MIN_VALUE, 3));

        // ceilDivExact gives the quotient where it fits, and StrictMath has the same methods.
        System.out.println(Math.ceilDivExact(-6, 3) + " " + Math.ceilDivExact(7L, -2L) + " " + Math.ceilDivExact(Long.MAX_VALUE, 1L));
        System.out.println(StrictMath.ceilDiv(-7, 2) + " " + StrictMath.ceilMod(7L, -3) + " " + StrictMath.ceilDivExact(9L, 2L));

        // A zero divisor throws in every form; the message is the one of a division by zero.
        int zero = args.length;
        int minusOne = -1;
        long longMinusOne = -1L;
        try {
            System.out.println(Math.ceilDiv(1, zero));
        } catch (ArithmeticException e) {
            System.out.println("ceilDiv: " + e.getMessage());
        }
        try {
            System.out.println(Math.ceilMod(5L, zero));
        } catch (ArithmeticException e) {
            System.out.println("ceilMod: " + e.getMessage());
        }
        try {
            System.out.println(StrictMath.ceilMod(3, 0));
        } catch (ArithmeticException e) {
            System.out.println("StrictMath.ceilMod: " + e.getMessage());
        }

        // ceilDivExact throws where the quotient overflows, and with a zero divisor.
        try {
            System.out.println(Math.ceilDivExact(Integer.MIN_VALUE, minusOne));
        } catch (ArithmeticException e) {
            System.out.println("ceilDivExact: " + e.getMessage());
        }
        try {
            System.out.println(Math.ceilDivExact(Long.MIN_VALUE, longMinusOne));
        } catch (ArithmeticException e) {
            System.out.println("ceilDivExact: " + e.getMessage());
        }
        try {
            System.out.println(StrictMath.ceilDivExact(Integer.MIN_VALUE, -1));
        } catch (ArithmeticException e) {
            System.out.println("StrictMath.ceilDivExact: " + e.getMessage());
        }
        try {
            System.out.println(Math.ceilDivExact(1L, zero));
        } catch (ArithmeticException e) {
            System.out.println("ceilDivExact: " + e.getMessage());
        }
        try {
            System.out.println(Math.ceilDivExact(Integer.MIN_VALUE, 2));
        } catch (ArithmeticException e) {
            System.out.println("not thrown: " + e.getMessage());
        } finally {
            System.out.println("done");
        }
    }
}
