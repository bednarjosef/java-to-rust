// Values a plain run of the JVM has without the program setting them: properties read as numbers,
// which are unset (the default, or null) or the JDK's, the release of the JDK, and a collection
// that is only a hint. And the exponents of doubles and floats.
public class JvmValues {
    static int calls;
    private static final int DEFAULT_TIMEOUT = 0;
    // A smaller default, which the call widens, unboxed into a field, as `LdapPoolManager` does.
    private static final long IDLE_TIMEOUT = Long.getLong("no.such.timeout", DEFAULT_TIMEOUT);

    static int sideEffect() {
        calls++;
        return 7;
    }

    public static void main(String[] args) {
        System.out.println(Integer.getInteger("no.such.number"));
        System.out.println(Integer.getInteger("no.such.number", 256));
        System.out.println(Integer.getInteger("no.such.number", Integer.valueOf(-3)));
        System.out.println(Integer.getInteger("java.class.version", 1));
        System.out.println(Integer.getInteger("java.specification.version", 1));
        System.out.println(Integer.getInteger("no.such.number", sideEffect()) + " " + calls);
        System.out.println(Long.getLong("java.vm.specification.version"));
        System.out.println(Long.getLong("no.such.number", 5L));
        System.out.println(Long.getLong("user.dir", 9L));
        short small = 12;
        System.out.println(IDLE_TIMEOUT + " " + Long.getLong("no.such.number", 'A') + " "
                + Integer.getInteger("no.such.number", small) + " " + Long.getLong("java.class.version", small));
        System.out.println(Runtime.version().feature() >= 17);
        System.out.println(Runtime.version().feature() == Integer.getInteger("java.specification.version"));
        System.gc();
        Runtime.getRuntime().gc();
        double[] doubles = {1.0, 0.75, 1024.5, -3e300, 4.9e-324, 0.0, Double.NaN, Double.POSITIVE_INFINITY};
        for (double d : doubles) {
            System.out.print(Math.getExponent(d) + " ");
        }
        System.out.println();
        float[] floats = {1.0f, 0.1f, 3e38f, 1e-45f, 0.0f, Float.NaN};
        for (float f : floats) {
            System.out.print(Math.getExponent(f) + " ");
        }
        System.out.println();
    }
}
