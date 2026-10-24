// Atomic updates of `float`, `double` and `byte` fields and elements through VarHandles, which the
// JDK makes `Unsafe`'s float, double and byte forms: the compare forms compare raw bits, so `NaN`
// is itself and `-0.0` is not `0.0`, and the updates are atomic between threads.
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;

public class FloatHandles {
    private volatile float level;
    private volatile double total;
    private volatile byte state;
    private final double[] sums = new double[2];

    private static final VarHandle LEVEL;
    private static final VarHandle TOTAL;
    private static final VarHandle STATE;
    private static final VarHandle SUMS = MethodHandles.arrayElementVarHandle(double[].class);

    static {
        try {
            MethodHandles.Lookup lookup = MethodHandles.lookup();
            LEVEL = lookup.findVarHandle(FloatHandles.class, "level", float.class);
            TOTAL = lookup.findVarHandle(FloatHandles.class, "total", double.class);
            STATE = lookup.findVarHandle(FloatHandles.class, "state", byte.class);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        FloatHandles h = new FloatHandles();
        System.out.println(LEVEL.compareAndSet(h, -0.0f, 1.5f) + " " + LEVEL.compareAndSet(h, 0.0f, Float.NaN)
                + " " + LEVEL.compareAndSet(h, Float.NaN, 2.5f) + " " + h.level);
        System.out.println((float) LEVEL.getAndAdd(h, 0.25f) + " " + (float) LEVEL.compareAndExchange(h, 2.75f, -0.0f)
                + " " + (float) LEVEL.getAndSet(h, 4f) + " " + h.level);
        h.total = Double.NaN;
        System.out.println(TOTAL.weakCompareAndSet(h, Double.NaN, 0.0) + " " + TOTAL.compareAndSet(h, -0.0, 1.0)
                + " " + h.total);
        System.out.println((byte) STATE.compareAndExchange(h, (byte) 0, (byte) 7) + " "
                + STATE.compareAndSet(h, (byte) 0, (byte) 9) + " " + (byte) STATE.getAndSet(h, (byte) -1) + " " + h.state);
        System.out.println(SUMS.compareAndSet(h.sums, 1, -0.0, 3.0) + " " + (double) SUMS.getAndAdd(h.sums, 1, 0.5)
                + " " + h.sums[1]);

        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            threads[t] = new Thread(() -> {
                for (int i = 0; i < 10_000; i++) {
                    TOTAL.getAndAdd(h, 1.0);
                    SUMS.getAndAdd(h.sums, 0, 0.5);
                }
            });
            threads[t].start();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        System.out.println(h.total + " " + h.sums[0]);
    }
}
