package jdk.internal.foreign;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.ByteOrder;
import jdk.internal.misc.ScopedMemoryAccess;

/**
 * The translator's accesses of a memory segment through the {@code VarHandle} of a value layout,
 * {@code layout.varHandle().get(segment, offset)} and {@code set(segment, offset, value)}, as
 * {@code AbstractMemorySegmentImpl}'s typed accessors make them: the lowering calls these in
 * their place (see {@code lower/handles.rs}), since the handle is made at run time. Each checks
 * what the layout's handle checks (the segment's bounds and the alignment, and that a written
 * segment is not read-only), then reads or writes the value in the layout's byte order at the
 * segment's base and offset, as the handle does.
 */
final class TranslatedSegments {

    private static final ScopedMemoryAccess SCOPED = ScopedMemoryAccess.getScopedMemoryAccess();

    private TranslatedSegments() {
    }

    private static AbstractMemorySegmentImpl checked(MemorySegment segment, ValueLayout layout, long offset,
            boolean readOnly) {
        AbstractMemorySegmentImpl impl = (AbstractMemorySegmentImpl) segment;
        impl.checkEnclosingLayout(offset, layout, readOnly);
        return impl;
    }

    private static boolean bigEndian(ValueLayout layout) {
        return layout.order() == ByteOrder.BIG_ENDIAN;
    }

    static byte get(MemorySegment segment, ValueLayout.OfByte layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return SCOPED.getByte(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset);
    }

    static void set(MemorySegment segment, ValueLayout.OfByte layout, long offset, byte value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putByte(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, value);
    }

    static boolean get(MemorySegment segment, ValueLayout.OfBoolean layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return SCOPED.getBoolean(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset);
    }

    static void set(MemorySegment segment, ValueLayout.OfBoolean layout, long offset, boolean value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putBoolean(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, value);
    }

    static char get(MemorySegment segment, ValueLayout.OfChar layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return SCOPED.getCharUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, bigEndian(layout));
    }

    static void set(MemorySegment segment, ValueLayout.OfChar layout, long offset, char value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putCharUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, value, bigEndian(layout));
    }

    static short get(MemorySegment segment, ValueLayout.OfShort layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return SCOPED.getShortUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, bigEndian(layout));
    }

    static void set(MemorySegment segment, ValueLayout.OfShort layout, long offset, short value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putShortUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, value, bigEndian(layout));
    }

    static int get(MemorySegment segment, ValueLayout.OfInt layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return SCOPED.getIntUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, bigEndian(layout));
    }

    static void set(MemorySegment segment, ValueLayout.OfInt layout, long offset, int value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putIntUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, value, bigEndian(layout));
    }

    static float get(MemorySegment segment, ValueLayout.OfFloat layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return Float.intBitsToFloat(SCOPED.getIntUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, bigEndian(layout)));
    }

    static void set(MemorySegment segment, ValueLayout.OfFloat layout, long offset, float value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putIntUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, Float.floatToRawIntBits(value), bigEndian(layout));
    }

    static long get(MemorySegment segment, ValueLayout.OfLong layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return SCOPED.getLongUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, bigEndian(layout));
    }

    static void set(MemorySegment segment, ValueLayout.OfLong layout, long offset, long value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putLongUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, value, bigEndian(layout));
    }

    static double get(MemorySegment segment, ValueLayout.OfDouble layout, long offset) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, true);
        return Double.longBitsToDouble(SCOPED.getLongUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, bigEndian(layout)));
    }

    static void set(MemorySegment segment, ValueLayout.OfDouble layout, long offset, double value) {
        AbstractMemorySegmentImpl s = checked(segment, layout, offset, false);
        SCOPED.putLongUnaligned(s.sessionImpl(), s.unsafeGetBase(), s.unsafeGetOffset() + offset, Double.doubleToRawLongBits(value), bigEndian(layout));
    }
}
