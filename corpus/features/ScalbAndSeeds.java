import java.security.SecureRandom;

public class ScalbAndSeeds {
    public static void main(String[] args) {
        // Doubles: normal results, the largest and smallest powers of two, and rounding into the subnormals.
        System.out.println(Math.scalb(1.5, 10) + " " + Math.scalb(0.1, 3) + " " + Math.scalb(-7.25, -2));
        System.out.println(Math.scalb(1.0, 1023) + " " + Math.scalb(1.0, 1024) + " " + Math.scalb(-1.0, 2000));
        System.out.println(Math.scalb(1.0, -1022) + " " + Math.scalb(1.0, -1074) + " " + Math.scalb(1.0, -1075));
        System.out.println(Math.scalb(1.5, -1074) + " " + Math.scalb(3.0, -1075) + " " + Math.scalb(2.5, -1075));
        System.out.println(Math.scalb(Double.MAX_VALUE, -1) + " " + Math.scalb(Double.MIN_NORMAL, -1));
        System.out.println(Math.scalb(Double.MIN_VALUE, 1074) + " " + Math.scalb(Double.MIN_VALUE, 1075));
        System.out.println(Math.scalb(Double.MAX_VALUE, Integer.MIN_VALUE) + " " + Math.scalb(1.0, Integer.MAX_VALUE));
        System.out.println(Math.scalb(-0.0, 5) + " " + Math.scalb(0.0, -5000) + " " + Math.scalb(-0.0, 1100));
        System.out.println(Math.scalb(Double.NaN, 3) + " " + Math.scalb(Double.POSITIVE_INFINITY, -3));
        System.out.println(Math.scalb(Double.NEGATIVE_INFINITY, 3000) + " " + Math.scalb(Double.NaN, -3000));
        System.out.println(Math.scalb(4.9e-324, 1100) + " " + Math.scalb(1.0e-300, 2000) + " " + Math.scalb(1.0e300, -2000));
        // The same through StrictMath, which delegates to the same code.
        System.out.println(StrictMath.scalb(2.0, 3) + " " + StrictMath.scalb(1.0, -1075) + " " + StrictMath.scalb(3.0, -1075));
        // Floats: the same edges at float's exponent range.
        System.out.println(Math.scalb(1.5f, -3) + " " + Math.scalb(1.0f, 127) + " " + Math.scalb(1.0f, 128));
        System.out.println(Math.scalb(1.0f, -126) + " " + Math.scalb(1.0f, -149) + " " + Math.scalb(1.0f, -150));
        System.out.println(Math.scalb(3.0f, -150) + " " + Math.scalb(1.5f, -149) + " " + Math.scalb(-0.0f, 9));
        System.out.println(Math.scalb(Float.MAX_VALUE, 1) + " " + Math.scalb(Float.MAX_VALUE, -1) + " " + Math.scalb(Float.MIN_VALUE, 149));
        System.out.println(Math.scalb(Float.NaN, 4) + " " + Math.scalb(Float.NEGATIVE_INFINITY, -4) + " " + Math.scalb(0.1f, Integer.MIN_VALUE));
        System.out.println(StrictMath.scalb(0.75f, -2) + " " + StrictMath.scalb(1.0f, Integer.MAX_VALUE));
        // Seeds come from the operating system: only their lengths are printed.
        System.out.println(SecureRandom.getSeed(0).length + " " + SecureRandom.getSeed(16).length);
        try {
            SecureRandom.getSeed(-1);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        byte[] seed = SecureRandom.getSeed(32);
        System.out.println(seed.length + " " + (seed.length == 32));
        SecureRandom.getSeed(-2);
    }
}
