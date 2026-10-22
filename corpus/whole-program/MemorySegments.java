// Memory segments of arrays and of an arena's memory, read and written through value layouts:
// every primitive layout, both byte orders, unaligned layouts, a read past the end, a misaligned
// read, a heap segment that shares its array, and a closed arena.
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.ByteOrder;

public class MemorySegments {
    public static void main(String[] args) {
        MemorySegment heap = MemorySegment.ofArray(new byte[32]);
        heap.set(ValueLayout.JAVA_INT_UNALIGNED, 0, 0x01020304);
        heap.set(ValueLayout.JAVA_INT_UNALIGNED.withOrder(ByteOrder.BIG_ENDIAN), 4, 0x01020304);
        heap.set(ValueLayout.JAVA_LONG_UNALIGNED, 9, -2L);
        heap.set(ValueLayout.JAVA_DOUBLE_UNALIGNED, 17, 1.5);
        heap.set(ValueLayout.JAVA_SHORT_UNALIGNED, 25, (short) 300);
        heap.set(ValueLayout.JAVA_CHAR_UNALIGNED, 27, 'x');
        heap.set(ValueLayout.JAVA_BOOLEAN, 29, true);
        heap.set(ValueLayout.JAVA_FLOAT_UNALIGNED, 28, 2.25f);
        StringBuilder bytes = new StringBuilder();
        for (int i = 0; i < 32; i++) {
            bytes.append(heap.get(ValueLayout.JAVA_BYTE, i)).append(' ');
        }
        System.out.println(bytes.toString().trim());
        System.out.println(heap.get(ValueLayout.JAVA_INT_UNALIGNED, 0) + " " + heap.get(ValueLayout.JAVA_INT_UNALIGNED, 4));
        System.out.println(heap.get(ValueLayout.JAVA_LONG_UNALIGNED, 9) + " " + heap.get(ValueLayout.JAVA_DOUBLE_UNALIGNED, 17));
        System.out.println(heap.get(ValueLayout.JAVA_SHORT_UNALIGNED, 25) + " " + heap.get(ValueLayout.JAVA_FLOAT_UNALIGNED, 28)
                + " " + heap.get(ValueLayout.JAVA_BOOLEAN, 29) + " " + heap.get(ValueLayout.JAVA_CHAR_UNALIGNED, 26));
        try {
            heap.get(ValueLayout.JAVA_INT, 4);
        } catch (IllegalArgumentException e) {
            System.out.println("alignment: " + e.getMessage().substring(0, 59));
        }
        try {
            heap.get(ValueLayout.JAVA_INT_UNALIGNED, 30);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("out of bounds");
        }
        try {
            MemorySegment.ofArray(new int[4]).get(ValueLayout.JAVA_INT, 2);
        } catch (IllegalArgumentException e) {
            System.out.println("misaligned");
        }
        int[] ints = {7, 8, 9};
        MemorySegment of = MemorySegment.ofArray(ints);
        of.set(ValueLayout.JAVA_INT, 4, 80);
        System.out.println(ints[1] + " " + of.get(ValueLayout.JAVA_INT, 8) + " " + of.byteSize());
        Arena arena = Arena.ofConfined();
        MemorySegment off = arena.allocate(16);
        off.set(ValueLayout.JAVA_LONG, 0, 123456789012L);
        off.set(ValueLayout.JAVA_INT, 8, 42);
        off.set(ValueLayout.JAVA_SHORT.withOrder(ByteOrder.BIG_ENDIAN), 12, (short) 0x0102);
        System.out.println(off.get(ValueLayout.JAVA_LONG, 0) + " " + off.get(ValueLayout.JAVA_INT, 8) + " "
                + off.get(ValueLayout.JAVA_BYTE, 12) + " " + off.get(ValueLayout.JAVA_BYTE, 13) + " " + off.get(ValueLayout.JAVA_BYTE, 14));
        arena.close();
        try {
            off.get(ValueLayout.JAVA_INT, 8);
        } catch (IllegalStateException e) {
            System.out.println("closed: " + e.getMessage());
        }
    }
}
