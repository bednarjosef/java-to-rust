// Numbers in the bytes of a ByteBuffer, read and written through the constant view handles of
// `MethodHandles.byteBufferViewVarHandle`. The handle's byte order decides, not the buffer's; the
// index is absolute and the value's bytes must end before the limit; a read-only buffer cannot be
// written; and the atomic modes reject a heap buffer and an address that is not aligned.
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

public class ByteBufferViews {
    static final VarHandle INT_BE = MethodHandles.byteBufferViewVarHandle(int[].class, ByteOrder.BIG_ENDIAN);
    static final VarHandle INT_LE = MethodHandles.byteBufferViewVarHandle(int[].class, ByteOrder.LITTLE_ENDIAN);
    static final VarHandle SHORT_LE = MethodHandles.byteBufferViewVarHandle(short[].class, ByteOrder.LITTLE_ENDIAN);
    static final VarHandle CHAR_BE = MethodHandles.byteBufferViewVarHandle(char[].class, ByteOrder.BIG_ENDIAN);
    static final VarHandle LONG_BE = MethodHandles.byteBufferViewVarHandle(long[].class, ByteOrder.BIG_ENDIAN);
    static final VarHandle FLOAT_LE = MethodHandles.byteBufferViewVarHandle(float[].class, ByteOrder.LITTLE_ENDIAN);
    static final VarHandle DOUBLE_BE = MethodHandles.byteBufferViewVarHandle(double[].class, ByteOrder.BIG_ENDIAN);

    static String bytes(ByteBuffer b) {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < b.capacity(); i++) {
            s.append(b.get(i) & 0xff).append(i + 1 < b.capacity() ? " " : "");
        }
        return s.toString();
    }

    static void tryRun(String what, Runnable r) {
        try {
            r.run();
            System.out.println(what + ": ok");
        } catch (RuntimeException e) {
            System.out.println(what + ": " + e.getClass().getName() + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        // The buffer's order is little-endian; the big-endian handle writes big-endian bytes.
        ByteBuffer heap = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN);
        INT_BE.set(heap, 0, 0x01020304);
        INT_LE.set(heap, 4, 0x01020304);
        SHORT_LE.set(heap, 8, (short) -2);
        CHAR_BE.set(heap, 10, 'A');
        System.out.println(bytes(heap));
        System.out.println((int) INT_BE.get(heap, 0) + " " + (int) INT_LE.get(heap, 0) + " "
                + (short) SHORT_LE.get(heap, 8) + " " + (char) CHAR_BE.get(heap, 10) + " "
                + Integer.toHexString((int) INT_BE.get(heap, 1)));
        // A read at a wider type, and as an object.
        long wide = (long) INT_LE.get(heap, 4);
        Object boxed = SHORT_LE.get(heap, 8);
        System.out.println(wide + " " + boxed);

        LONG_BE.set(heap, 8, 0x1122334455667788L);
        System.out.println(Long.toHexString((long) LONG_BE.get(heap, 8)) + " " + bytes(heap));
        FLOAT_LE.set(heap, 0, -1.5f);
        DOUBLE_BE.set(heap, 8, Math.PI);
        System.out.println((float) FLOAT_LE.get(heap, 0) + " " + (double) DOUBLE_BE.get(heap, 8) + " " + bytes(heap));

        // The index is absolute: neither the position nor the limit moves, and a slice starts at
        // its own index 0.
        heap.position(6);
        INT_BE.set(heap, 2, 7);
        System.out.println(heap.position() + " " + heap.limit() + " " + (int) INT_BE.get(heap, 2));
        ByteBuffer slice = heap.slice(4, 8);
        System.out.println((int) INT_BE.get(slice, 0) + " " + (int) INT_BE.get(heap, 4));

        // Bounds: the value's bytes must end before the limit, not the capacity.
        heap.position(0).limit(10);
        tryRun("get at 6", () -> INT_BE.get(heap, 6));
        tryRun("get at 7", () -> INT_BE.get(heap, 7));
        tryRun("get at -1", () -> INT_BE.get(heap, -1));
        tryRun("set at 7", () -> INT_BE.set(heap, 7, 1));
        tryRun("long at 3", () -> LONG_BE.get(heap, 3));
        ByteBuffer tiny = ByteBuffer.allocate(2);
        tryRun("int in two bytes", () -> INT_BE.get(tiny, 0));
        tryRun("short in two bytes", () -> SHORT_LE.get(tiny, 0));
        tryRun("null buffer", () -> INT_BE.get((ByteBuffer) null, 0));

        // A read-only buffer reads, and its writes throw before the index is checked.
        ByteBuffer readOnly = heap.asReadOnlyBuffer();
        tryRun("read-only get", () -> System.out.println((int) INT_BE.get(readOnly, 2)));
        tryRun("read-only set", () -> INT_BE.set(readOnly, 0, 1));
        tryRun("read-only set out of bounds", () -> INT_BE.set(readOnly, 100, 1));

        // The atomic modes need a direct buffer and an aligned address.
        tryRun("heap getVolatile", () -> INT_BE.getVolatile(heap, 0));
        tryRun("heap setRelease out of bounds", () -> INT_BE.setRelease(heap, 100, 1));
        ByteBuffer direct = ByteBuffer.allocateDirect(32);
        INT_BE.setVolatile(direct, 4, 0x0a0b0c0d);
        System.out.println(Integer.toHexString((int) INT_BE.getAcquire(direct, 4)) + " "
                + Integer.toHexString((int) INT_LE.getOpaque(direct, 4)) + " " + direct.get(4) + " " + direct.get(7));
        LONG_BE.setRelease(direct, 8, -5L);
        SHORT_LE.setOpaque(direct, 16, (short) 300);
        DOUBLE_BE.setVolatile(direct, 24, 2.5);
        System.out.println((long) LONG_BE.getVolatile(direct, 8) + " " + (short) SHORT_LE.getVolatile(direct, 16) + " "
                + (int) (char) CHAR_BE.getAcquire(direct, 16) + " " + (double) DOUBLE_BE.getOpaque(direct, 24));
        tryRun("misaligned getVolatile", () -> INT_BE.getVolatile(direct, 2));
        tryRun("misaligned setOpaque", () -> LONG_BE.setOpaque(direct, 4, 1L));
        tryRun("aligned plain get at 2", () -> System.out.println((int) INT_BE.get(direct, 2)));
        tryRun("misaligned out of bounds", () -> INT_BE.getVolatile(direct, 30));
        ByteBuffer directSlice = direct.slice(1, 16);
        tryRun("slice getVolatile at 3", () -> System.out.println((int) INT_BE.getVolatile(directSlice, 3)));
        tryRun("slice getVolatile at 0", () -> INT_BE.getVolatile(directSlice, 0));
        ByteBuffer directReadOnly = direct.asReadOnlyBuffer();
        tryRun("read-only direct setVolatile", () -> INT_BE.setVolatile(directReadOnly, 4, 1));
        tryRun("read-only direct getVolatile", () -> System.out.println((int) INT_BE.getVolatile(directReadOnly, 4)));
    }
}
