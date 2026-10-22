package jdk.internal.foreign;

/**
 * The translator's patch of the JDK's {@code jdk.internal.foreign.Utils} (see
 * docs/jdk-overlays.md): the JDK's checks and helpers of memory segments and layouts, without the
 * method handles that its static initializer looks up to make the handles of address layouts. A
 * translation makes no segment handle: the accesses of a segment through a value layout's handle
 * call {@code TranslatedSegments} instead, and {@code makeRawSegmentViewVarHandle} throws
 * {@code UnsupportedOperationException}.
 */
class Utils {
    @Replace
    public static VarHandle makeRawSegmentViewVarHandle(MemoryLayout enclosing, ValueLayout layout, boolean constantOffset, long offset) {
        throw new UnsupportedOperationException("the handles of memory layouts in a translated program");
    }

    // The method handles of address layouts, and what makes and uses them.
    static {}

    @Remove
    private static final Class<?> ADDRESS_CARRIER_TYPE;

    @Remove
    private static final MethodHandle LONG_TO_CARRIER;

    @Remove
    private static final MethodHandle LONG_TO_ADDRESS_TARGET;

    @Remove
    private static final MethodHandle LONG_TO_ADDRESS_NO_TARGET;

    @Remove
    private static VarHandle makeRawSegmentViewVarHandleInternal(MemoryLayout enclosing, ValueLayout layout, boolean constantOffset, long offset);

    @Remove
    private static byte booleanToByte(boolean b);
}
