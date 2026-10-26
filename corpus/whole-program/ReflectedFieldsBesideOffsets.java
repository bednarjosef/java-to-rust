import java.lang.reflect.Field;
import sun.misc.Unsafe;

// A class whose static field holds a field offset, beside a field that reflection looks up by a
// name the program computes: the offset's field, which is no field of the translation, is not
// one that reflection finds.
public class ReflectedFieldsBesideOffsets {
    static final Unsafe U;

    static {
        try {
            Field f = Unsafe.class.getDeclaredField("theUnsafe");
            f.setAccessible(true);
            U = (Unsafe) f.get(null);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static class Counter {
        static final long VALUE;

        static {
            try {
                VALUE = U.objectFieldOffset(Counter.class.getDeclaredField("value"));
            } catch (NoSuchFieldException e) {
                throw new Error(e);
            }
        }

        volatile int value;

        int increment() {
            return U.getAndAddInt(this, VALUE, 1) + 1;
        }
    }

    public static void main(String[] args) throws Exception {
        Counter c = new Counter();
        c.increment();
        Field f = Counter.class.getDeclaredField(args.length > 5 ? "VALUE" : "value");
        System.out.println(f.getName() + " " + f.get(c));
    }
}
