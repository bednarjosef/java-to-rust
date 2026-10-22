package jdk.internal.misc;

/**
 * The translator's patch of the JDK's {@code jdk.internal.misc.UnsafeConstants} (see
 * docs/jdk-overlays.md): the values that the JVM injects into the class, whose own initializer
 * leaves them all zero. Addresses are 8 bytes, as on the 64-bit machines the translation runs
 * on, which are little-endian and read unaligned values, with the data cache lines of 64 bytes
 * that the JVM reports on them; the page size is the system's.
 */
class UnsafeConstants {
    @Replace
    static final int ADDRESS_SIZE0 = 8;

    @Replace
    static final int PAGE_SIZE = pageSize0();

    @Replace
    static final boolean BIG_ENDIAN = false;

    @Replace
    static final boolean UNALIGNED_ACCESS = true;

    @Replace
    static final int DATA_CACHE_LINE_FLUSH_SIZE = 64;

    // The JDK's sets every field to zero, for the JVM to set it after.
    static {}

    @Add
    private static native int pageSize0();
}
