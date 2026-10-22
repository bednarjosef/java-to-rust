// Direct byte buffers, whose memory is outside the heap: typed reads and writes in both byte
// orders, bulk copies to and from arrays and heap buffers, compact, slices, views as char, int
// and long buffers, a read past the end, and equality with a heap buffer.
import java.nio.*;
import java.util.Arrays;
public class DirectBuffers {
    public static void main(String[] args) {
        ByteBuffer d = ByteBuffer.allocateDirect(64);
        System.out.println(d.isDirect() + " " + d.capacity() + " " + d.get(5));
        d.putInt(0x01020304).putLong(-2L).putChar('x').putDouble(1.5).put((byte) 7);
        d.flip();
        System.out.println(d.remaining() + " " + d.getInt() + " " + d.getLong() + " " + d.getChar() + " " + d.getDouble() + " " + d.get());
        d.clear();
        d.order(ByteOrder.LITTLE_ENDIAN).putShort(0, (short) 0x1234);
        System.out.println(d.get(0) + " " + d.get(1) + " " + d.order());
        byte[] src = {9, 8, 7, 6, 5, 4, 3, 2};
        d.position(10);
        d.put(src);
        byte[] back = new byte[8];
        d.position(10);
        d.get(back);
        System.out.println(Arrays.toString(back));
        ByteBuffer heap = ByteBuffer.allocate(16);
        d.position(10).limit(18);
        heap.put(d);
        System.out.println(Arrays.toString(heap.array()));
        heap.flip();
        ByteBuffer d2 = ByteBuffer.allocateDirect(16);
        d2.put(heap);
        d2.flip();
        System.out.println(d2.remaining() + " " + d2.get(3));
        d2.position(2);
        d2.compact();
        System.out.println(d2.position() + " " + d2.get(0));
        CharBuffer cb = ByteBuffer.allocateDirect(8).asCharBuffer();
        cb.put("hiya");
        cb.flip();
        System.out.println(cb.toString());
        IntBuffer ib = ByteBuffer.allocateDirect(16).order(ByteOrder.nativeOrder()).asIntBuffer();
        ib.put(new int[]{1, 2, 3, 4});
        System.out.println(ib.get(2));
        ByteBuffer slice = d.clear().slice(4, 8);
        System.out.println(slice.capacity() + " " + slice.isDirect() + " " + slice.get(6));
        try {
            d.get(64);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("out of bounds");
        }
        LongBuffer lb = ByteBuffer.allocateDirect(24).asLongBuffer();
        lb.put(new long[]{5, -6, 7});
        long[] longs = new long[3];
        lb.flip();
        lb.get(longs);
        System.out.println(Arrays.toString(longs) + " " + ByteBuffer.allocateDirect(4).equals(ByteBuffer.allocate(4)));
    }
}
