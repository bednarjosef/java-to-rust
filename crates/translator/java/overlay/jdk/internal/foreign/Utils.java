package jdk.internal.foreign;

import jdk.internal.misc.Unsafe;
import sun.invoke.util.Wrapper;

import java.lang.foreign.AddressLayout;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.StructLayout;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.VarHandle;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Supplier;

/**
 * The translator's {@code jdk.internal.foreign.Utils}: the JDK's checks and helpers of memory
 * segments and layouts, without the method handles that its static initializer looks up to make
 * the handles of address layouts. A translation makes no segment handle: the accesses of a
 * segment through a value layout's handle call {@code TranslatedSegments} instead, and
 * {@code makeRawSegmentViewVarHandle} throws {@code UnsupportedOperationException}.
 */
public final class Utils {

    private Utils() {
    }

    public static long alignUp(long n, long alignment) {
        return (n + alignment - 1) & -alignment;
    }

    public static MemorySegment alignUp(MemorySegment ms, long alignment) {
        long offset = ms.address();
        return ms.asSlice(alignUp(offset, alignment) - offset);
    }

    public static VarHandle makeRawSegmentViewVarHandle(MemoryLayout enclosing, ValueLayout layout, boolean constantOffset, long offset) {
        throw new UnsupportedOperationException("the handles of memory layouts in a translated program");
    }

    public static boolean byteToBoolean(byte b) {
        return b != 0;
    }

    public static MemorySegment longToAddress(long addr) {
        return longToAddress(addr, 0, 1);
    }

    public static MemorySegment longToAddress(int addr) {
        return longToAddress(addr, 0, 1);
    }

    public static MemorySegment longToAddress(long addr, AddressLayout layout) {
        return longToAddress(addr, pointeeByteSize(layout), pointeeByteAlign(layout));
    }

    public static MemorySegment longToAddress(int addr, AddressLayout layout) {
        return longToAddress(addr, pointeeByteSize(layout), pointeeByteAlign(layout));
    }

    public static MemorySegment longToAddress(long addr, long size, long align) {
        if (!isAligned(addr, align)) {
            throw new IllegalArgumentException("Invalid alignment constraint for address: " + toHexString(addr));
        }
        return SegmentFactories.makeNativeSegmentUnchecked(addr, size);
    }

    public static MemorySegment longToAddress(long addr, long size, long align, MemorySessionImpl scope) {
        if (!isAligned(addr, align)) {
            throw new IllegalArgumentException("Invalid alignment constraint for address: " + toHexString(addr));
        }
        return SegmentFactories.makeNativeSegmentUnchecked(addr, size, scope);
    }

    public static boolean isAligned(long offset, long align) {
        return (offset & (align - 1)) == 0;
    }

    // Sizes and alignments are powers of two, so an alignment no larger than the size divides it.
    public static boolean isElementAligned(ValueLayout layout) {
        return layout.byteAlignment() <= layout.byteSize();
    }

    public static void checkElementAlignment(ValueLayout layout, String msg) {
        if (!isElementAligned(layout)) {
            throw new IllegalArgumentException(msg);
        }
    }

    public static void checkElementAlignment(MemoryLayout layout, String msg) {
        if (layout.byteSize() % layout.byteAlignment() != 0) {
            throw new IllegalArgumentException(msg);
        }
    }

    public static long pointeeByteSize(AddressLayout addressLayout) {
        return addressLayout.targetLayout().map(MemoryLayout::byteSize).orElse(0L);
    }

    public static long pointeeByteAlign(AddressLayout addressLayout) {
        return addressLayout.targetLayout().map(MemoryLayout::byteAlignment).orElse(1L);
    }

    public static void checkAllocationSizeAndAlign(long byteSize, long byteAlignment) {
        checkNonNegativeArgument(byteSize, "allocation size");
        checkAlign(byteAlignment);
    }

    public static void checkAlign(long byteAlignment) {
        if (byteAlignment <= 0 || ((byteAlignment & (byteAlignment - 1)) != 0L)) {
            throw new IllegalArgumentException("Invalid alignment constraint : " + byteAlignment);
        }
    }

    public static void checkNonNegativeArgument(long value, String name) {
        if (value < 0) {
            throw new IllegalArgumentException("The provided " + name + " is negative: " + value);
        }
    }

    public static void checkNonNegativeIndex(long value, String name) {
        if (value < 0) {
            throw new IndexOutOfBoundsException("The provided " + name + " is negative: " + value);
        }
    }

    private static long computePadding(long offset, long align) {
        if (offset == 0 || offset % align == 0) {
            return 0;
        }
        return align - offset % align;
    }

    public static StructLayout computePaddedStructLayout(MemoryLayout... elements) {
        long offset = 0L;
        List<MemoryLayout> layouts = new ArrayList<>();
        long align = 0;
        for (MemoryLayout l : elements) {
            long padding = computePadding(offset, l.byteAlignment());
            if (padding != 0) {
                layouts.add(MemoryLayout.paddingLayout(padding));
                offset += padding;
            }
            layouts.add(l);
            align = Math.max(align, l.byteAlignment());
            offset += l.byteSize();
        }
        long padding = computePadding(offset, align);
        if (padding != 0) {
            layouts.add(MemoryLayout.paddingLayout(padding));
        }
        return MemoryLayout.structLayout(layouts.toArray(MemoryLayout[]::new));
    }

    public static int byteWidthOfPrimitive(Class<?> primitive) {
        return Wrapper.forPrimitiveType(primitive).bitWidth() / 8;
    }

    public static boolean isPowerOfTwo(long value) {
        return (value & (value - 1)) == 0L;
    }

    public static <L extends MemoryLayout> L wrapOverflow(Supplier<L> layoutSupplier) {
        try {
            return layoutSupplier.get();
        } catch (ArithmeticException ex) {
            throw new IllegalArgumentException("Layout size exceeds Long.MAX_VALUE");
        }
    }

    public static boolean containsNullChars(String s) {
        return s.indexOf('\u0000') >= 0;
    }

    public static String toHexString(long value) {
        return "0x" + Long.toHexString(value);
    }

    public record BaseAndScale(long base, long scale) {

        public static final BaseAndScale BYTE =
                new BaseAndScale(Unsafe.ARRAY_BYTE_BASE_OFFSET, Unsafe.ARRAY_BYTE_INDEX_SCALE);
        public static final BaseAndScale CHAR =
                new BaseAndScale(Unsafe.ARRAY_CHAR_BASE_OFFSET, Unsafe.ARRAY_CHAR_INDEX_SCALE);
        public static final BaseAndScale SHORT =
                new BaseAndScale(Unsafe.ARRAY_SHORT_BASE_OFFSET, Unsafe.ARRAY_SHORT_INDEX_SCALE);
        public static final BaseAndScale INT =
                new BaseAndScale(Unsafe.ARRAY_INT_BASE_OFFSET, Unsafe.ARRAY_INT_INDEX_SCALE);
        public static final BaseAndScale FLOAT =
                new BaseAndScale(Unsafe.ARRAY_FLOAT_BASE_OFFSET, Unsafe.ARRAY_FLOAT_INDEX_SCALE);
        public static final BaseAndScale LONG =
                new BaseAndScale(Unsafe.ARRAY_LONG_BASE_OFFSET, Unsafe.ARRAY_LONG_INDEX_SCALE);
        public static final BaseAndScale DOUBLE =
                new BaseAndScale(Unsafe.ARRAY_DOUBLE_BASE_OFFSET, Unsafe.ARRAY_DOUBLE_INDEX_SCALE);

        public static BaseAndScale of(Object array) {
            return switch (array) {
                case byte[] _ -> BaseAndScale.BYTE;
                case char[] _ -> BaseAndScale.CHAR;
                case short[] _ -> BaseAndScale.SHORT;
                case int[] _ -> BaseAndScale.INT;
                case float[] _ -> BaseAndScale.FLOAT;
                case long[] _ -> BaseAndScale.LONG;
                case double[] _ -> BaseAndScale.DOUBLE;
                default -> throw new IllegalArgumentException("Not a supported array class: " + array.getClass().getSimpleName());
            };
        }
    }
}
