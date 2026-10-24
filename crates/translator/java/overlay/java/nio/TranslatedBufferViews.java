package java.nio;

import java.util.Objects;
import jdk.internal.misc.ScopedMemoryAccess;

/**
 * The translator's accesses of a buffer through a view handle,
 * {@code MethodHandles.byteBufferViewVarHandle(int[].class, order)}: the lowering calls these in
 * place of the handle's {@code get} and {@code set} in each memory mode (see
 * {@code lower/handles.rs}), with the handle's byte order last, since the translation has no
 * handles at run time. The index is absolute, as the handle's is, and the buffer's own order does
 * not matter. Each checks what the handle checks, in its order: that the buffer is not null; for
 * a mode other than the plain one, that it is not a heap buffer; for a write, that it is not
 * read-only; that the value's bytes end before the limit; and for a mode other than the plain
 * one, that the value's address is a multiple of its size.
 */
final class TranslatedBufferViews {

    private static final ScopedMemoryAccess SCOPED = ScopedMemoryAccess.getScopedMemoryAccess();

    private static final boolean NATIVE_BIG_ENDIAN = ByteOrder.nativeOrder() == ByteOrder.BIG_ENDIAN;

    private TranslatedBufferViews() {
    }

    /** The index of a value of {@code size} bytes at {@code index}, whose bytes end before the limit. */
    private static int index(ByteBuffer bb, int index, int size) {
        return Objects.checkIndex(index, bb.limit() - (size - 1));
    }

    /** {@code index}, of a buffer that a write may change. */
    private static int writable(ByteBuffer bb, int index, int size) {
        if (bb.isReadOnly) {
            throw new ReadOnlyBufferException();
        }
        return index(bb, index, size);
    }

    /** The heap array under a buffer that a mode other than the plain one accesses: none. */
    private static Object direct(ByteBuffer bb) {
        if (bb.hb != null) {
            throw new IllegalStateException("Atomic access not supported for heap buffer");
        }
        return null;
    }

    /** The address of a value of {@code size} bytes at {@code index}, which must be aligned to it. */
    private static long aligned(ByteBuffer bb, int index, int size) {
        long address = bb.address + index;
        if ((address & (size - 1)) != 0) {
            throw new IllegalStateException("Misaligned access at index: " + index);
        }
        return address;
    }

    static short getShort(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        return SCOPED.getShortUnaligned(bb.session(), bb.hb, bb.address + index(bb, index, 2), be);
    }

    static void setShort(ByteBuffer bb, int index, short value, boolean be) {
        Objects.requireNonNull(bb);
        SCOPED.putShortUnaligned(bb.session(), bb.hb, bb.address + writable(bb, index, 2), value, be);
    }

    /** {@code value} in the byte order {@code be} from the native one, or back. */
    private static short ordered(short value, boolean be) {
        return be == NATIVE_BIG_ENDIAN ? value : Short.reverseBytes(value);
    }

    static short getShortVolatile(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 2), 2);
        return ordered(SCOPED.getShortVolatile(bb.session(), base, address), be);
    }

    static void setShortVolatile(ByteBuffer bb, int index, short value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 2), 2);
        SCOPED.putShortVolatile(bb.session(), base, address, ordered(value, be));
    }

    static short getShortAcquire(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 2), 2);
        return ordered(SCOPED.getShortAcquire(bb.session(), base, address), be);
    }

    static void setShortRelease(ByteBuffer bb, int index, short value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 2), 2);
        SCOPED.putShortRelease(bb.session(), base, address, ordered(value, be));
    }

    static short getShortOpaque(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 2), 2);
        return ordered(SCOPED.getShortOpaque(bb.session(), base, address), be);
    }

    static void setShortOpaque(ByteBuffer bb, int index, short value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 2), 2);
        SCOPED.putShortOpaque(bb.session(), base, address, ordered(value, be));
    }

    static int getInt(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        return SCOPED.getIntUnaligned(bb.session(), bb.hb, bb.address + index(bb, index, 4), be);
    }

    static void setInt(ByteBuffer bb, int index, int value, boolean be) {
        Objects.requireNonNull(bb);
        SCOPED.putIntUnaligned(bb.session(), bb.hb, bb.address + writable(bb, index, 4), value, be);
    }

    /** {@code value} in the byte order {@code be} from the native one, or back. */
    private static int ordered(int value, boolean be) {
        return be == NATIVE_BIG_ENDIAN ? value : Integer.reverseBytes(value);
    }

    static int getIntVolatile(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 4), 4);
        return ordered(SCOPED.getIntVolatile(bb.session(), base, address), be);
    }

    static void setIntVolatile(ByteBuffer bb, int index, int value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 4), 4);
        SCOPED.putIntVolatile(bb.session(), base, address, ordered(value, be));
    }

    static int getIntAcquire(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 4), 4);
        return ordered(SCOPED.getIntAcquire(bb.session(), base, address), be);
    }

    static void setIntRelease(ByteBuffer bb, int index, int value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 4), 4);
        SCOPED.putIntRelease(bb.session(), base, address, ordered(value, be));
    }

    static int getIntOpaque(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 4), 4);
        return ordered(SCOPED.getIntOpaque(bb.session(), base, address), be);
    }

    static void setIntOpaque(ByteBuffer bb, int index, int value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 4), 4);
        SCOPED.putIntOpaque(bb.session(), base, address, ordered(value, be));
    }

    static long getLong(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        return SCOPED.getLongUnaligned(bb.session(), bb.hb, bb.address + index(bb, index, 8), be);
    }

    static void setLong(ByteBuffer bb, int index, long value, boolean be) {
        Objects.requireNonNull(bb);
        SCOPED.putLongUnaligned(bb.session(), bb.hb, bb.address + writable(bb, index, 8), value, be);
    }

    /** {@code value} in the byte order {@code be} from the native one, or back. */
    private static long ordered(long value, boolean be) {
        return be == NATIVE_BIG_ENDIAN ? value : Long.reverseBytes(value);
    }

    static long getLongVolatile(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 8), 8);
        return ordered(SCOPED.getLongVolatile(bb.session(), base, address), be);
    }

    static void setLongVolatile(ByteBuffer bb, int index, long value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 8), 8);
        SCOPED.putLongVolatile(bb.session(), base, address, ordered(value, be));
    }

    static long getLongAcquire(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 8), 8);
        return ordered(SCOPED.getLongAcquire(bb.session(), base, address), be);
    }

    static void setLongRelease(ByteBuffer bb, int index, long value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 8), 8);
        SCOPED.putLongRelease(bb.session(), base, address, ordered(value, be));
    }

    static long getLongOpaque(ByteBuffer bb, int index, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, index(bb, index, 8), 8);
        return ordered(SCOPED.getLongOpaque(bb.session(), base, address), be);
    }

    static void setLongOpaque(ByteBuffer bb, int index, long value, boolean be) {
        Objects.requireNonNull(bb);
        Object base = direct(bb);
        long address = aligned(bb, writable(bb, index, 8), 8);
        SCOPED.putLongOpaque(bb.session(), base, address, ordered(value, be));
    }

    // A `char`, `float` or `double` is accessed as the bits of a `short`, `int` or `long`, as the
    // handle accesses it.

    static char getChar(ByteBuffer bb, int index, boolean be) {
        return (char) getShort(bb, index, be);
    }

    static void setChar(ByteBuffer bb, int index, char value, boolean be) {
        setShort(bb, index, (short) value, be);
    }

    static char getCharVolatile(ByteBuffer bb, int index, boolean be) {
        return (char) getShortVolatile(bb, index, be);
    }

    static void setCharVolatile(ByteBuffer bb, int index, char value, boolean be) {
        setShortVolatile(bb, index, (short) value, be);
    }

    static char getCharAcquire(ByteBuffer bb, int index, boolean be) {
        return (char) getShortAcquire(bb, index, be);
    }

    static void setCharRelease(ByteBuffer bb, int index, char value, boolean be) {
        setShortRelease(bb, index, (short) value, be);
    }

    static char getCharOpaque(ByteBuffer bb, int index, boolean be) {
        return (char) getShortOpaque(bb, index, be);
    }

    static void setCharOpaque(ByteBuffer bb, int index, char value, boolean be) {
        setShortOpaque(bb, index, (short) value, be);
    }

    static float getFloat(ByteBuffer bb, int index, boolean be) {
        return Float.intBitsToFloat(getInt(bb, index, be));
    }

    static void setFloat(ByteBuffer bb, int index, float value, boolean be) {
        setInt(bb, index, Float.floatToRawIntBits(value), be);
    }

    static float getFloatVolatile(ByteBuffer bb, int index, boolean be) {
        return Float.intBitsToFloat(getIntVolatile(bb, index, be));
    }

    static void setFloatVolatile(ByteBuffer bb, int index, float value, boolean be) {
        setIntVolatile(bb, index, Float.floatToRawIntBits(value), be);
    }

    static float getFloatAcquire(ByteBuffer bb, int index, boolean be) {
        return Float.intBitsToFloat(getIntAcquire(bb, index, be));
    }

    static void setFloatRelease(ByteBuffer bb, int index, float value, boolean be) {
        setIntRelease(bb, index, Float.floatToRawIntBits(value), be);
    }

    static float getFloatOpaque(ByteBuffer bb, int index, boolean be) {
        return Float.intBitsToFloat(getIntOpaque(bb, index, be));
    }

    static void setFloatOpaque(ByteBuffer bb, int index, float value, boolean be) {
        setIntOpaque(bb, index, Float.floatToRawIntBits(value), be);
    }

    static double getDouble(ByteBuffer bb, int index, boolean be) {
        return Double.longBitsToDouble(getLong(bb, index, be));
    }

    static void setDouble(ByteBuffer bb, int index, double value, boolean be) {
        setLong(bb, index, Double.doubleToRawLongBits(value), be);
    }

    static double getDoubleVolatile(ByteBuffer bb, int index, boolean be) {
        return Double.longBitsToDouble(getLongVolatile(bb, index, be));
    }

    static void setDoubleVolatile(ByteBuffer bb, int index, double value, boolean be) {
        setLongVolatile(bb, index, Double.doubleToRawLongBits(value), be);
    }

    static double getDoubleAcquire(ByteBuffer bb, int index, boolean be) {
        return Double.longBitsToDouble(getLongAcquire(bb, index, be));
    }

    static void setDoubleRelease(ByteBuffer bb, int index, double value, boolean be) {
        setLongRelease(bb, index, Double.doubleToRawLongBits(value), be);
    }

    static double getDoubleOpaque(ByteBuffer bb, int index, boolean be) {
        return Double.longBitsToDouble(getLongOpaque(bb, index, be));
    }

    static void setDoubleOpaque(ByteBuffer bb, int index, double value, boolean be) {
        setLongOpaque(bb, index, Double.doubleToRawLongBits(value), be);
    }
}
