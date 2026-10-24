// VarHandles of a static field (`findStaticVarHandle`, as `SSLContext` keeps its default
// context) and of a field made by `ConstantBootstraps.fieldVarHandle` (as `SelectionKeyImpl`
// keeps its interest set): each access is one of the field itself, and the updates are atomic.
import java.lang.invoke.ConstantBootstraps;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;

public class StaticVarHandles {
    static final class Choice {
        final String name;

        Choice(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    private static volatile Choice chosen;
    private static long total;
    private volatile int bits;

    private static final VarHandle CHOSEN;
    private static final VarHandle TOTAL;
    private static final VarHandle BITS = ConstantBootstraps.fieldVarHandle(
            MethodHandles.lookup(), "bits", VarHandle.class, StaticVarHandles.class, int.class);

    static {
        try {
            MethodHandles.Lookup lookup = MethodHandles.lookup();
            CHOSEN = lookup.findStaticVarHandle(StaticVarHandles.class, "chosen", Choice.class);
            TOTAL = lookup.findStaticVarHandle(StaticVarHandles.class, "total", long.class);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    static Choice choose(String name) {
        Choice candidate = new Choice(name);
        return CHOSEN.compareAndSet(null, candidate) ? candidate : (Choice) CHOSEN.getVolatile();
    }

    public static void main(String[] args) throws InterruptedException {
        System.out.println(choose("first") + " " + choose("second") + " " + chosen);
        Choice third = new Choice("third");
        CHOSEN.setRelease(third);
        System.out.println(CHOSEN.compareAndSet(new Choice("third"), null) + " " + CHOSEN.compareAndSet(third, third)
                + " " + (Choice) CHOSEN.get() + " " + (Choice) CHOSEN.getAndSet(new Choice("fourth")) + " " + chosen);

        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            threads[t] = new Thread(() -> {
                for (int i = 0; i < 10_000; i++) {
                    TOTAL.getAndAdd(1L);
                }
            });
            threads[t].start();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        System.out.println((long) TOTAL.getAcquire() + " " + total);

        StaticVarHandles key = new StaticVarHandles();
        int before = (int) BITS.getAndBitwiseOr(key, 5);
        int middle = (int) BITS.getAndBitwiseAnd(key, 6);
        System.out.println(before + " " + middle + " " + (int) BITS.getAndSet(key, 9) + " " + key.bits);
    }
}
