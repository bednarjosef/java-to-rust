package jdk.internal.util;

/**
 * The translator's patch of the JDK's {@code jdk.internal.util.ByteArray} (see
 * docs/jdk-overlays.md): the values of the primitive types packed
 * into and unpacked from a {@code byte[]} in big endian order, as {@code DataInputStream},
 * {@code DataOutputStream} and {@code RandomAccessFile} use them. The JDK's class reads and writes
 * through views of the array as other primitive arrays ({@code MethodHandles.byteArrayViewVarHandle}),
 * which the translation does not have; patched, it shifts the bytes. An offset outside the array
 * throws what the view throws: {@code ArrayIndexOutOfBoundsException} with the view's message.
 */
class ByteArray {
    /** Checks that {@code size} bytes from {@code offset} are inside {@code array}, as the views do. */
    @Add
    private static void check(byte[] array, int offset, int size) {
        int length = array.length - (size - 1);
        if (offset < 0 || offset >= length) {
            throw new ArrayIndexOutOfBoundsException("Index " + offset + " out of bounds for length " + length);
        }
    }

    @Add
    private static int unpack(byte[] array, int offset, int size) {
        check(array, offset, size);
        int value = 0;
        for (int i = 0; i < size; i++) {
            value = (value << 8) | (array[offset + i] & 0xFF);
        }
        return value;
    }

    @Add
    private static void pack(byte[] array, int offset, int size, int value) {
        check(array, offset, size);
        for (int i = size - 1; i >= 0; i--) {
            array[offset + i] = (byte) value;
            value >>>= 8;
        }
    }

    @Replace
    public static char getChar(byte[] array, int offset) {
        return (char) unpack(array, offset, 2);
    }

    @Replace
    public static short getShort(byte[] array, int offset) {
        return (short) unpack(array, offset, 2);
    }

    @Replace
    public static int getUnsignedShort(byte[] array, int offset) {
        return unpack(array, offset, 2);
    }

    @Replace
    public static int getInt(byte[] array, int offset) {
        return unpack(array, offset, 4);
    }

    @Replace
    public static float getFloat(byte[] array, int offset) {
        return Float.intBitsToFloat(getInt(array, offset));
    }

    @Replace
    public static float getFloatRaw(byte[] array, int offset) {
        return Float.intBitsToFloat(getInt(array, offset));
    }

    @Replace
    public static long getLong(byte[] array, int offset) {
        check(array, offset, 8);
        return ((long) unpack(array, offset, 4) << 32) | (unpack(array, offset + 4, 4) & 0xFFFFFFFFL);
    }

    @Replace
    public static double getDouble(byte[] array, int offset) {
        return Double.longBitsToDouble(getLong(array, offset));
    }

    @Replace
    public static double getDoubleRaw(byte[] array, int offset) {
        return Double.longBitsToDouble(getLong(array, offset));
    }

    @Replace
    public static void setChar(byte[] array, int offset, char value) {
        pack(array, offset, 2, value);
    }

    @Replace
    public static void setShort(byte[] array, int offset, short value) {
        pack(array, offset, 2, value);
    }

    @Replace
    public static void setUnsignedShort(byte[] array, int offset, int value) {
        pack(array, offset, 2, (char) value);
    }

    @Replace
    public static void setInt(byte[] array, int offset, int value) {
        pack(array, offset, 4, value);
    }

    @Replace
    public static void setFloat(byte[] array, int offset, float value) {
        setInt(array, offset, Float.floatToIntBits(value));
    }

    @Replace
    public static void setFloatRaw(byte[] array, int offset, float value) {
        setInt(array, offset, Float.floatToRawIntBits(value));
    }

    @Replace
    public static void setLong(byte[] array, int offset, long value) {
        check(array, offset, 8);
        pack(array, offset, 4, (int) (value >>> 32));
        pack(array, offset + 4, 4, (int) value);
    }

    @Replace
    public static void setDouble(byte[] array, int offset, double value) {
        setLong(array, offset, Double.doubleToLongBits(value));
    }

    @Replace
    public static void setDoubleRaw(byte[] array, int offset, double value) {
        setLong(array, offset, Double.doubleToRawLongBits(value));
    }

    @Remove
    private static final VarHandle SHORT;

    @Remove
    private static final VarHandle CHAR;

    @Remove
    private static final VarHandle INT;

    @Remove
    private static final VarHandle FLOAT;

    @Remove
    private static final VarHandle LONG;

    @Remove
    private static final VarHandle DOUBLE;

    @Remove
    private static VarHandle create(Class<?> viewArrayClass);
}
