package jdk.internal.misc;

/**
 * The translator's forms of the methods of {@code sun.misc.Unsafe} that reach memory outside the
 * heap by address, or that tell its layout: the lowering calls these in place of a call of one of
 * them, after the warning that the method prints first (see {@code lower/sun_unsafe.rs}), since
 * that warning names the class whose code makes the call. Each does what the method does after
 * its warning: the same method of {@code jdk.internal.misc.Unsafe}.
 */
final class TranslatedUnsafeMemory {

    private static final Unsafe U = Unsafe.getUnsafe();

    private TranslatedUnsafeMemory() {
    }

    static byte getByte(long address) { return U.getByte(address); }
    static void putByte(long address, byte x) { U.putByte(address, x); }
    static short getShort(long address) { return U.getShort(address); }
    static void putShort(long address, short x) { U.putShort(address, x); }
    static char getChar(long address) { return U.getChar(address); }
    static void putChar(long address, char x) { U.putChar(address, x); }
    static int getInt(long address) { return U.getInt(address); }
    static void putInt(long address, int x) { U.putInt(address, x); }
    static long getLong(long address) { return U.getLong(address); }
    static void putLong(long address, long x) { U.putLong(address, x); }
    static float getFloat(long address) { return U.getFloat(address); }
    static void putFloat(long address, float x) { U.putFloat(address, x); }
    static double getDouble(long address) { return U.getDouble(address); }
    static void putDouble(long address, double x) { U.putDouble(address, x); }
    static long getAddress(long address) { return U.getAddress(address); }
    static void putAddress(long address, long x) { U.putAddress(address, x); }

    static long allocateMemory(long bytes) { return U.allocateMemory(bytes); }
    static long reallocateMemory(long address, long bytes) { return U.reallocateMemory(address, bytes); }
    static void freeMemory(long address) { U.freeMemory(address); }
    static void setMemory(Object o, long offset, long bytes, byte value) { U.setMemory(o, offset, bytes, value); }
    static void setMemory(long address, long bytes, byte value) { U.setMemory(address, bytes, value); }

    static void copyMemory(Object srcBase, long srcOffset, Object destBase, long destOffset, long bytes) {
        U.copyMemory(srcBase, srcOffset, destBase, destOffset, bytes);
    }

    static void copyMemory(long srcAddress, long destAddress, long bytes) {
        U.copyMemory(srcAddress, destAddress, bytes);
    }

    static int addressSize() { return U.addressSize(); }
    static int pageSize() { return U.pageSize(); }

    /** What {@code sun.misc.Unsafe.invokeCleaner} checks before its warning. */
    static void checkCleaner(java.nio.ByteBuffer directBuffer) {
        if (!directBuffer.isDirect())
            throw new IllegalArgumentException("Not a direct buffer");
    }

    static void invokeCleaner(java.nio.ByteBuffer directBuffer) { U.invokeCleaner(directBuffer); }
}
