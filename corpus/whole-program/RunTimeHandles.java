import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.invoke.VarHandle;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

/// Handles that a program finds at run time through `MethodHandle`s of the handle factories, the
/// way a library does that also runs where the factories are missing: each value is one handle or
/// null, and an access through null throws the `NullPointerException` Java throws.
public class RunTimeHandles {
    static final class Factory {
        private static final MethodHandle FIND_VAR_HANDLE;
        private static final MethodHandle FIND_STATIC_VAR_HANDLE;
        private static final MethodHandle PRIVATE_LOOKUP_IN;
        private static final VarHandle INT_BE_ARRAY_VIEW;
        private static final VarHandle LONG_LE_ARRAY_VIEW;
        private static final VarHandle SHORT_BE_BUFFER_VIEW;
        private static final Throwable CAUSE;

        static {
            MethodHandle find = null;
            MethodHandle findStatic = null;
            MethodHandle privateLookupIn = null;
            VarHandle intBe = null;
            VarHandle longLe = null;
            VarHandle shortBe = null;
            Throwable error = null;
            try {
                MethodHandles.Lookup lookup = MethodHandles.lookup();
                find = lookup.findVirtual(MethodHandles.Lookup.class, "findVarHandle",
                        MethodType.methodType(VarHandle.class, Class.class, String.class, Class.class));
                findStatic = lookup.findVirtual(MethodHandles.Lookup.class, "findStaticVarHandle",
                        MethodType.methodType(VarHandle.class, Class.class, String.class, Class.class));
                privateLookupIn = lookup.findStatic(MethodHandles.class, "privateLookupIn",
                        MethodType.methodType(MethodHandles.Lookup.class, Class.class, MethodHandles.Lookup.class));
                MethodHandle view = lookup.findStatic(MethodHandles.class, "byteArrayViewVarHandle",
                        MethodType.methodType(VarHandle.class, Class.class, ByteOrder.class));
                intBe = (VarHandle) view.invokeExact(int[].class, ByteOrder.BIG_ENDIAN);
                longLe = (VarHandle) view.invokeExact(long[].class, ByteOrder.LITTLE_ENDIAN);
                MethodHandle bufferView = lookup.findStatic(MethodHandles.class, "byteBufferViewVarHandle",
                        MethodType.methodType(VarHandle.class, Class.class, ByteOrder.class));
                shortBe = (VarHandle) bufferView.invokeExact(short[].class, ByteOrder.BIG_ENDIAN);
                error = null;
            } catch (Throwable e) {
                error = e;
                find = null;
                findStatic = null;
                intBe = null;
                longLe = null;
                shortBe = null;
            } finally {
                FIND_VAR_HANDLE = find;
                FIND_STATIC_VAR_HANDLE = findStatic;
                PRIVATE_LOOKUP_IN = privateLookupIn;
                INT_BE_ARRAY_VIEW = intBe;
                LONG_LE_ARRAY_VIEW = longLe;
                SHORT_BE_BUFFER_VIEW = shortBe;
                CAUSE = error;
            }
        }

        static boolean supported() {
            return CAUSE == null;
        }

        private static MethodHandles.Lookup privateLookup(MethodHandles.Lookup lookup, Class<?> c) {
            try {
                return (MethodHandles.Lookup) PRIVATE_LOOKUP_IN.invokeExact(c, lookup);
            } catch (Throwable e) {
                throw new RuntimeException(e);
            }
        }

        static VarHandle find(MethodHandles.Lookup lookup, Class<?> c, String name, Class<?> type) {
            try {
                return (VarHandle) FIND_VAR_HANDLE.invokeExact(privateLookup(lookup, c), c, name, type);
            } catch (Throwable e) {
                throw new RuntimeException(e);
            }
        }

        static VarHandle findStatic(MethodHandles.Lookup lookup, Class<?> c, String name, Class<?> type) {
            try {
                return (VarHandle) FIND_STATIC_VAR_HANDLE.invokeExact(lookup, c, name, type);
            } catch (Throwable e) {
                throw new RuntimeException(e);
            }
        }

        static VarHandle intBeArrayView() {
            return INT_BE_ARRAY_VIEW;
        }

        static VarHandle longLeArrayView() {
            return LONG_LE_ARRAY_VIEW;
        }

        static VarHandle shortBeBufferView() {
            return SHORT_BE_BUFFER_VIEW;
        }
    }

    static boolean enabled = System.getProperty("run.time.handles.off") == null;

    static VarHandle findIntField(MethodHandles.Lookup lookup, Class<?> c, String name) {
        if (enabled) {
            return Factory.find(lookup, c, name, int.class);
        }
        return null;
    }

    static VarHandle intView(boolean on) {
        if (!on || !enabled) {
            return null;
        }
        return Factory.intBeArrayView();
    }

    static VarHandle longView() {
        return enabled ? Factory.longLeArrayView() : null;
    }

    static VarHandle shortView() {
        if (enabled) {
            return Factory.shortBeBufferView();
        }
        return null;
    }

    static void setShort(ByteBuffer buffer, int index, int value) {
        shortView().set(buffer, index, (short) value);
    }

    static short getShort(ByteBuffer buffer, int index) {
        return (short) shortView().get(buffer, index);
    }

    volatile int value;
    static long total;

    static final class Counter {
        private static final VarHandle VALUE;
        private static final VarHandle TOTAL;

        static {
            VALUE = findIntField(MethodHandles.lookup(), RunTimeHandles.class, "value");
            TOTAL = Factory.findStatic(MethodHandles.lookup(), RunTimeHandles.class, "total", long.class);
        }

        static void init(RunTimeHandles o) {
            VALUE.set(o, 2);
        }

        static int get(RunTimeHandles o) {
            return (int) VALUE.getAcquire(o);
        }

        static int add(RunTimeHandles o, int d) {
            return (int) VALUE.getAndAdd(o, d);
        }

        static boolean cas(RunTimeHandles o, int e, int v) {
            return VALUE.compareAndSet(o, e, v);
        }

        static long addTotal(long d) {
            return (long) TOTAL.getAndAdd(d) + d;
        }
    }

    static final VarHandle INTS = intView(true);
    static final VarHandle LONGS = longView();
    static final VarHandle NONE = intView(false);

    public static void main(String[] args) {
        System.out.println(Factory.supported());
        RunTimeHandles o = new RunTimeHandles();
        Counter.init(o);
        System.out.println(Counter.add(o, 3) + " " + Counter.get(o) + " " + Counter.cas(o, 5, 9) + " " + o.value);
        System.out.println(Counter.addTotal(40) + " " + Counter.addTotal(2) + " " + total);

        byte[] bytes = new byte[16];
        int at = 4;
        INTS.set(bytes, at, 0x01020304);
        LONGS.set(bytes, 8, 0x1122334455667788L);
        System.out.println(java.util.Arrays.toString(bytes));
        System.out.println((int) INTS.get(bytes, at) + " " + (long) LONGS.get(bytes, 8));
        try {
            INTS.set(bytes, 14, 1);
        } catch (IndexOutOfBoundsException e) {
            System.out.println(e.getClass().getName());
        }
        try {
            NONE.set(bytes, 0, 1);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println((int) NONE.get(bytes, at));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            intView(false).set(bytes, at, 5);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }

        ByteBuffer buffer = ByteBuffer.allocate(8);
        setShort(buffer, 2, 0x1234);
        setShort(buffer, 4, -2);
        System.out.println(getShort(buffer, 2) + " " + getShort(buffer, 4) + " " + buffer.get(2) + " " + buffer.get(3));
        try {
            setShort(buffer, 7, 1);
        } catch (IndexOutOfBoundsException e) {
            System.out.println(e.getClass().getName());
        }
    }
}
