// The JDK's VarHandles of array elements and of byte arrays viewed as numbers. DataOutputStream
// and DataInputStream write and read ints, longs, shorts, chars, floats and doubles through
// `MethodHandles.byteArrayViewVarHandle` in big-endian order, and the atomic arrays update their
// elements through `MethodHandles.arrayElementVarHandle`, with compare-and-set and get-and-set
// holding the array for the whole update.
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.util.concurrent.atomic.AtomicIntegerArray;
import java.util.concurrent.atomic.AtomicLongArray;
import java.util.concurrent.atomic.AtomicReferenceArray;

public class VarHandleViews {
    public static void main(String[] args) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        DataOutputStream out = new DataOutputStream(bytes);
        out.writeInt(0x12345678);
        out.writeLong(-2L);
        out.writeShort(-3);
        out.writeChar('Z');
        out.writeDouble(1.5);
        out.writeFloat(-0.25f);
        byte[] written = bytes.toByteArray();
        System.out.println(written.length + " " + written[0] + " " + written[3] + " " + written[11]);
        DataInputStream in = new DataInputStream(new ByteArrayInputStream(written));
        System.out.println(in.readInt() + " " + in.readLong() + " " + in.readShort() + " " + in.readChar()
                + " " + in.readDouble() + " " + in.readFloat());

        AtomicIntegerArray ints = new AtomicIntegerArray(4);
        ints.incrementAndGet(1);
        ints.addAndGet(2, 5);
        System.out.println(ints + " " + ints.compareAndSet(2, 5, 7) + " " + ints.compareAndSet(2, 5, 9) + " " + ints.get(2));
        AtomicLongArray longs = new AtomicLongArray(new long[] {1, 2, 3});
        System.out.println(longs.getAndIncrement(0) + " " + longs.accumulateAndGet(2, 10, Math::max) + " " + longs);
        AtomicReferenceArray<StringBuilder> names = new AtomicReferenceArray<>(3);
        StringBuilder a = new StringBuilder("a");
        StringBuilder b = new StringBuilder("b");
        names.set(0, a);
        System.out.println(names.compareAndSet(1, null, b) + " " + names.compareAndSet(1, null, a) + " "
                + names.compareAndSet(1, b, a) + " " + names.getAndSet(0, b) + " " + names);
        try {
            ints.get(4);
        } catch (IndexOutOfBoundsException e) {
            System.out.println(e.getMessage());
        }
    }
}
