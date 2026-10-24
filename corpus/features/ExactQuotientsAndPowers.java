public class ExactQuotientsAndPowers {
    public static void main(String[] args) {
        int zero = args.length;
        int minusOne = -1;

        // divideExact truncates and floorDivExact rounds toward negative infinity, where they fit.
        System.out.println(Math.divideExact(7, -2) + " " + Math.floorDivExact(7, -2) + " " + Math.floorDivExact(-7, 2));
        System.out.println(Math.divideExact(-6L, 3L) + " " + Math.floorDivExact(-7L, 2L) + " " + Math.floorDivExact(6L, 3L));
        System.out.println(StrictMath.divideExact(9, 2) + " " + StrictMath.floorDivExact(-9L, 2L));
        System.out.println(Math.floorDivExact(Integer.MIN_VALUE, 1) + " " + Math.divideExact(Long.MIN_VALUE, 2L));

        // MIN_VALUE / -1 overflows, and a zero divisor throws as a division does.
        try {
            System.out.println(Math.floorDivExact(Integer.MIN_VALUE, minusOne));
        } catch (ArithmeticException e) {
            System.out.println("floorDivExact: " + e.getMessage());
        }
        try {
            System.out.println(Math.divideExact(Long.MIN_VALUE, (long) minusOne));
        } catch (ArithmeticException e) {
            System.out.println("divideExact: " + e.getMessage());
        }
        try {
            System.out.println(Math.floorDivExact(1L, (long) zero));
        } catch (ArithmeticException e) {
            System.out.println("floorDivExact by zero: " + e.getMessage());
        }
        try {
            System.out.println(Math.divideExact(1, zero));
        } catch (ArithmeticException e) {
            System.out.println("divideExact by zero: " + e.getMessage());
        }

        // powExact gives the power where it fits, MIN_VALUE included.
        System.out.println(Math.powExact(3, 4) + " " + Math.powExact(-2, 31) + " " + Math.powExact(-1, 7) + " " + Math.powExact(0, 0));
        System.out.println(Math.powExact(10L, 18) + " " + Math.powExact(-2L, 63) + " " + StrictMath.powExact(7L, 1));
        int[] exponents = {31, 32, -1};
        for (int n : exponents) {
            try {
                System.out.println(Math.powExact(2, n));
            } catch (ArithmeticException e) {
                System.out.println("powExact(2, " + n + "): " + e.getMessage());
            }
            try {
                System.out.println(Math.powExact(2L, n + 32));
            } catch (ArithmeticException e) {
                System.out.println("powExact(2L, " + (n + 32) + "): " + e.getMessage());
            }
        }
        // An exception that no catch sees ends the program.
        System.out.println(Math.powExact(minusOne * 3, 21));
    }
}
