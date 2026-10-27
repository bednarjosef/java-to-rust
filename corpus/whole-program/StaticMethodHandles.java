import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.invoke.VarHandle;

// Handles of static methods, which `findStatic` finds and `invokeExact` calls, as netty's
// `ConcurrentSkipListIntObjMultimap` finds its fence: of `VarHandle`'s fence or of a method of its
// own, by the class that a helper casts (`cls`) and `Void.TYPE`, in a static block that picks one;
// and with arguments and results, through a local that may be null.
public class StaticMethodHandles {
    static int calls;

    static int twice(int x) {
        calls++;
        return 2 * x;
    }

    static String describe(String name, long count) {
        return name + " x" + count;
    }

    static int next(int value) {
        calls++;
        return value;
    }

    @SuppressWarnings("unchecked")
    private static <T> Class<T> cls(Class<?> c) {
        return (Class<T>) c;
    }

    // Read when `Fences` is initialized, after `main` has set it, as netty's map asks whether
    // the platform has `VarHandle`: the handle is either `VarHandle`'s fence or the class's own.
    static boolean hasVarHandle = true;

    static class Fences {
        static volatile int fenceVariable;
        static final MethodHandle FENCE;

        static {
            try {
                MethodHandles.Lookup lookup = MethodHandles.lookup();
                Class<Fences> own = cls(Fences.class);
                if (hasVarHandle) {
                    Class<VarHandle> varHandle = cls(Class.forName("java.lang.invoke.VarHandle"));
                    FENCE = lookup.findStatic(varHandle, "acquireFence", MethodType.methodType(Void.TYPE));
                } else {
                    FENCE = lookup.findStatic(own, "fenceFallback", MethodType.methodType(Void.TYPE));
                }
            } catch (ReflectiveOperationException e) {
                throw new ExceptionInInitializerError(e);
            }
        }

        private static void fenceFallback() {
            fenceVariable = 1;
            calls++;
        }

        static void fence() {
            try {
                FENCE.invokeExact();
            } catch (Throwable e) {
                LinkageError error = new LinkageError();
                error.initCause(e);
                throw error;
            }
        }
    }

    // `VarHandle`'s own fence, by its class literal.
    static class Releases {
        static final MethodHandle FENCE;

        static {
            try {
                FENCE = MethodHandles.lookup()
                        .findStatic(VarHandle.class, "releaseFence", MethodType.methodType(void.class));
            } catch (ReflectiveOperationException e) {
                throw new ExceptionInInitializerError(e);
            }
        }
    }

    static final MethodHandle TWICE;
    static final MethodHandle DESCRIBE;

    static {
        try {
            MethodHandles.Lookup lookup = MethodHandles.lookup();
            TWICE = lookup.findStatic(StaticMethodHandles.class, "twice", MethodType.methodType(int.class, int.class));
            DESCRIBE = lookup.findStatic(
                    StaticMethodHandles.class,
                    "describe",
                    MethodType.methodType(String.class, String.class, long.class));
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    public static void main(String[] args) throws Throwable {
        hasVarHandle = args.length > 0;
        Fences.fence();
        System.out.println("fell back, " + calls + " calls, " + Fences.fenceVariable);
        Releases.FENCE.invokeExact();
        System.out.println("released");

        int doubled = (int) TWICE.invokeExact(next(21));
        System.out.println(doubled + " after " + calls + " calls");
        String described = (String) DESCRIBE.invokeExact("handle", (long) doubled);
        System.out.println(described);

        // Through a local that may be null: the argument runs before the check.
        MethodHandle maybe = args.length > 0 ? TWICE : null;
        try {
            int none = (int) maybe.invokeExact(next(5));
            System.out.println(none);
        } catch (NullPointerException e) {
            System.out.println("null handle after " + calls + " calls");
        }
    }
}
