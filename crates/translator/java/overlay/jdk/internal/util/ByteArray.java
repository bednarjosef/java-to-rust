package jdk.internal.util;

/**
 * The translator's {@code jdk.internal.util.ByteArray}: the values of the primitive types packed
 * into and unpacked from a {@code byte[]} in big endian order, as {@code DataInputStream},
 * {@code DataOutputStream} and {@code RandomAccessFile} use them. The JDK's class reads and writes
 * through views of the array as other primitive arrays ({@code MethodHandles.byteArrayViewVarHandle}),
 * which the translation does not have; this one shifts the bytes. An offset outside the array
 * throws what the view throws: {@code ArrayIndexOutOfBoundsException} with the view's message.
 */
public final class ByteArray {

    private ByteArray() {
    }

    /** Checks that {@code size} bytes from {@code offset} are inside {@code array}, as the views do. */
    private static void check(byte[] array, int offset, int size) {
        int length = array.length - (size - 1);
        if (offset < 0 || offset >= length) {
            throw new ArrayIndexOutOfBoundsException("Index " + offset + " out of bounds for length " + length);
        }
    }

    private static int unpack(byte[] array, int offset, int size) {
        check(array, offset, size);
        int value = 0;
        for (int i = 0; i < size; i++) {
            value = (value << 8) | (array[offset + i] & 0xFF);
        }
        return value;
    }

    private static void pack(byte[] array, int offset, int size, int value) {
        check(array, offset, size);
        for (int i = size - 1; i >= 0; i--) {
            array[offset + i] = (byte) value;
            value >>>= 8;
        }
    }

    public static boolean getBoolean(byte[] array, int offset) {
        return array[offset] != 0;
    }

    public static char getChar(byte[] array, int offset) {
        return (char) unpack(array, offset, 2);
    }

    public static short getShort(byte[] array, int offset) {
        return (short) unpack(array, offset, 2);
    }

    public static int getUnsignedShort(byte[] array, int offset) {
        return unpack(array, offset, 2);
    }

    public static int getInt(byte[] array, int offset) {
        return unpack(array, offset, 4);
    }

    public static float getFloat(byte[] array, int offset) {
        return Float.intBitsToFloat(getInt(array, offset));
    }

    public static float getFloatRaw(byte[] array, int offset) {
        return Float.intBitsToFloat(getInt(array, offset));
    }

    public static long getLong(byte[] array, int offset) {
        check(array, offset, 8);
        return ((long) unpack(array, offset, 4) << 32) | (unpack(array, offset + 4, 4) & 0xFFFFFFFFL);
    }

    public static double getDouble(byte[] array, int offset) {
        return Double.longBitsToDouble(getLong(array, offset));
    }

    public static double getDoubleRaw(byte[] array, int offset) {
        return Double.longBitsToDouble(getLong(array, offset));
    }

    public static void setBoolean(byte[] array, int offset, boolean value) {
        array[offset] = (byte) (value ? 1 : 0);
    }

    public static void setChar(byte[] array, int offset, char value) {
        pack(array, offset, 2, value);
    }

    public static void setShort(byte[] array, int offset, short value) {
        pack(array, offset, 2, value);
    }

    public static void setUnsignedShort(byte[] array, int offset, int value) {
        pack(array, offset, 2, (char) value);
    }

    public static void setInt(byte[] array, int offset, int value) {
        pack(array, offset, 4, value);
    }

    public static void setFloat(byte[] array, int offset, float value) {
        setInt(array, offset, Float.floatToIntBits(value));
    }

    public static void setFloatRaw(byte[] array, int offset, float value) {
        setInt(array, offset, Float.floatToRawIntBits(value));
    }

    public static void setLong(byte[] array, int offset, long value) {
        check(array, offset, 8);
        pack(array, offset, 4, (int) (value >>> 32));
        pack(array, offset + 4, 4, (int) value);
    }

    public static void setDouble(byte[] array, int offset, double value) {
        setLong(array, offset, Double.doubleToLongBits(value));
    }

    public static void setDoubleRaw(byte[] array, int offset, double value) {
        setLong(array, offset, Double.doubleToRawLongBits(value));
    }
}
