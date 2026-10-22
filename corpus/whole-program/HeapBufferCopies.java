// Buffers on the heap, which the JDK reads and writes through Unsafe at offsets into their arrays:
// copies between buffers with `Unsafe.copyMemory` (putting one char buffer into another,
// compacting one, the same with bytes, and a copy between a big-endian and a little-endian view,
// which swaps the bytes of each char), and the typed reads and writes of a byte buffer, aligned
// or not, in either byte order, which read the bytes of its array at a computed offset.
import java.lang.invoke.VarHandle;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.CharBuffer;

public class HeapBufferCopies {
    public static void main(String[] args) {
        CharBuffer from = CharBuffer.wrap("hello, world".toCharArray());
        CharBuffer to = CharBuffer.allocate(20);
        to.put('[').put(from).put(']');
        to.flip();
        System.out.println(to);
        CharBuffer partly = CharBuffer.wrap("abcdef".toCharArray());
        partly.get();
        partly.get();
        partly.compact();
        partly.flip();
        System.out.println(partly);
        ByteBuffer bytes = ByteBuffer.allocate(8);
        bytes.put(new byte[] {1, 2, 3, 4, 5, 6});
        bytes.flip();
        ByteBuffer copy = ByteBuffer.allocate(8);
        copy.put(bytes);
        System.out.println(copy.position() + " " + copy.get(0) + " " + copy.get(5));
        ByteBuffer big = ByteBuffer.allocate(4).order(ByteOrder.BIG_ENDIAN);
        ByteBuffer little = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN);
        big.asCharBuffer().put("hi");
        little.asCharBuffer().put(big.asCharBuffer());
        System.out.println(little.get(0) + " " + little.get(1) + " " + little.asCharBuffer());

        ByteBuffer typed = ByteBuffer.allocate(32);
        typed.putInt(1, 0x01020304).putLong(5, -2L).putShort(13, (short) 0x7f80).putChar(15, 'Z');
        typed.putFloat(17, 1.5f).putDouble(21, -0.25);
        System.out.println(typed.get(1) + " " + typed.get(4) + " " + typed.getInt(1) + " " + typed.getLong(5));
        System.out.println(typed.getShort(13) + " " + typed.getChar(15) + " " + typed.getFloat(17) + " " + typed.getDouble(21));
        typed.order(ByteOrder.LITTLE_ENDIAN);
        System.out.println(Integer.toHexString(typed.getInt(1)) + " " + typed.getShort(13) + " " + (int) typed.getChar(16));
        typed.putInt(0, 0x0a0b0c0d);
        System.out.println(typed.get(0) + " " + typed.get(3) + " " + typed.order(ByteOrder.BIG_ENDIAN).getInt(0));

        Object chars = new char[2];
        System.out.println(chars.getClass().getName() + " " + chars.getClass().getComponentType().isPrimitive());

        // The fences of VarHandle, which order memory and have no other effect.
        VarHandle.fullFence();
        VarHandle.acquireFence();
        VarHandle.releaseFence();
        VarHandle.loadLoadFence();
        VarHandle.storeStoreFence();
        System.out.println("fenced");
    }
}
