public class NarrowingCasts {
    public static void main(String[] args) {
        double nan = 0.0 / 0.0;
        double inf = 1.0 / 0.0;
        double big = 1e20;
        double neg = -1e20;
        double frac = -3.99;
        System.out.println((int) nan);
        System.out.println((int) inf);
        System.out.println((int) -inf);
        System.out.println((int) big);
        System.out.println((int) neg);
        System.out.println((long) nan);
        System.out.println((long) inf);
        System.out.println((long) -inf);
        System.out.println((long) 1e19);
        System.out.println((long) frac);
        System.out.println((int) frac);
        System.out.println((short) 40000.7);
        System.out.println((byte) 200.9);
        System.out.println((char) 65.9);
        System.out.println((char) -1.0);
        System.out.println((short) nan);
        System.out.println((byte) inf);
        System.out.println((float) 1e300);
        System.out.println((float) -1e300);
        System.out.println((float) nan);
        System.out.println((double) 0.1f);
        System.out.println((float) 0.1);
        long L = 9007199254740993L;
        System.out.println((double) L);
        System.out.println((float) L);
        System.out.println((float) 16777217);
        System.out.println((int) 3000000000L);
        System.out.println((short) 70000);
        System.out.println((byte) 1000);
        System.out.println((char) 66000);
        System.out.println((int) 'z');
        System.out.println((char) 97);
        System.out.println((short) 'A' + 1);
        char c = (char) -1;
        System.out.println((int) c);
        System.out.println((short) c);
        System.out.println((byte) c);
        System.out.println((long) c);
        System.out.println((float) c);
        System.out.println((double) c);
        float f = 3.7f;
        System.out.println((int) f);
        System.out.println((long) -f);
        System.out.println((int) (f * 1e10f));
        System.out.println((long) 1e30f);
        // Values next to the bounds, where the conversion gives the type's least value.
        double[] edges = { -2147483648.0, -2147483648.9, -2147483649.0, 2147483647.9, 2147483648.0,
            -9.223372036854775808E18, -9.3E18, 9.3E18, -0.0, nan, -inf };
        for (double e : edges) {
            System.out.println((int) e + " " + (long) e + " " + (short) e + " "
                + (int) (char) e + " " + (int) (float) e + " " + (long) (float) e);
        }
        float fnan = (float) nan;
        float finf = (float) inf;
        System.out.println((int) fnan + " " + (long) fnan + " " + (int) -finf + " "
            + (long) -finf + " " + (byte) finf);
    }
}
