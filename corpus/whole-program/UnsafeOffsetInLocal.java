import java.lang.reflect.Field;
import java.nio.Buffer;
import java.nio.ByteBuffer;
import sun.misc.Unsafe;

// A field read through the offset in a local, of a `Field` in a local, as netty's
// `PlatformDependent0` checks at start that it can read the address of a direct buffer: inside
// an anonymous class, the field that `getDeclaredField` found, its offset from `objectFieldOffset`,
// and `getLong` at that offset; and a field of the program's own read and written so.
public class UnsafeOffsetInLocal {
    static class Counter {
        long count = 41;
    }

    interface Probe {
        Object run();
    }

    public static void main(String[] args) throws Exception {
        Field theUnsafe = Unsafe.class.getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        final Unsafe unsafe = (Unsafe) theUnsafe.get(null);
        final ByteBuffer direct = ByteBuffer.allocateDirect(1);
        Object found = new Probe() {
            @Override
            public Object run() {
                try {
                    final Field field = Buffer.class.getDeclaredField("address");
                    final long offset = unsafe.objectFieldOffset(field);
                    final long address = unsafe.getLong(direct, offset);
                    if (address == 0) {
                        return null;
                    }
                    return field;
                } catch (NoSuchFieldException | SecurityException e) {
                    return e;
                }
            }
        }.run();
        System.out.println("address field: " + (found instanceof Field f ? f.getName() : found));

        Counter counter = new Counter();
        Field countField = Counter.class.getDeclaredField("count");
        long offset = unsafe.objectFieldOffset(countField);
        System.out.println(unsafe.getLong(counter, offset));
        unsafe.putLong(counter, offset, 42);
        System.out.println(counter.count);
    }
}
