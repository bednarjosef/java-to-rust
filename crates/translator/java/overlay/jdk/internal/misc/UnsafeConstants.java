package jdk.internal.misc;

/**
 * The translator's {@code jdk.internal.misc.UnsafeConstants}: the values that the JVM injects
 * into the JDK's class, whose own initializer leaves them all zero. Addresses are 8 bytes, as on
 * the 64-bit machines the translation runs on, which are little-endian and read unaligned
 * values, with the data cache lines of 64 bytes that the JVM reports on them; the page size is
 * the system's.
 */
final class UnsafeConstants {

    private UnsafeConstants() {
    }

    static final int ADDRESS_SIZE0 = 8;

    static final int PAGE_SIZE = pageSize0();

    static final boolean BIG_ENDIAN = false;

    static final boolean UNALIGNED_ACCESS = true;

    static final int DATA_CACHE_LINE_FLUSH_SIZE = 64;

    private static native int pageSize0();
}
