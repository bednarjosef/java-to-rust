import java.lang.reflect.Field;
import sun.misc.Unsafe;

// Fields found by a name the code computes, then read and written through `Unsafe` at their
// offsets, as netty's `PlatformDependent0` reads `java.nio.Bits`' `MAX_MEMORY` and `UNALIGNED`
// at start: the class from `Class.forName` of a name spelled out, the field's name one of two
// that a condition picks, and `staticFieldOffset` and `staticFieldBase` of the field it finds; an
// object's field likewise through `objectFieldOffset`; and the failures of a name the class does
// not have and of an offset of a static field used on an object.
public class UnsafeComputedFieldNames {
    static long limit = 1234;
    private static volatile boolean ready = true;

    static class Point {
        int x = 3;
        long y = 4;
        String label = "p";
    }

    public static void main(String[] args) throws Exception {
        Field theUnsafe = Unsafe.class.getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        Unsafe unsafe = (Unsafe) theUnsafe.get(null);
        int version = Runtime.version().feature();

        // A direct buffer initializes `Bits`, as netty makes one before it reads the fields.
        java.nio.ByteBuffer.allocateDirect(1);
        Class<?> bits = Class.forName("java.nio.Bits", false, ClassLoader.getSystemClassLoader());
        String fieldName = version >= 11 ? "MAX_MEMORY" : "maxMemory";
        Field maxMemory = bits.getDeclaredField(fieldName);
        if (maxMemory.getType() == long.class) {
            long offset = unsafe.staticFieldOffset(maxMemory);
            Object base = unsafe.staticFieldBase(maxMemory);
            System.out.println("Bits." + maxMemory.getName() + " > 0: " + (unsafe.getLong(base, offset) > 0));
        }
        fieldName = version >= 11 ? "UNALIGNED" : "unaligned";
        Field unaligned = bits.getDeclaredField(fieldName);
        if (unaligned.getType() == boolean.class) {
            long offset = unsafe.staticFieldOffset(unaligned);
            Object base = unsafe.staticFieldBase(unaligned);
            System.out.println("Bits." + unaligned.getName() + ": " + unsafe.getBoolean(base, offset));
        }

        Class<?> own = Class.forName("UnsafeComputedFieldNames");
        Field limitField = own.getDeclaredField(args.length > 0 ? "LIMIT" : "limit");
        long limitOffset = unsafe.staticFieldOffset(limitField);
        Object limitBase = unsafe.staticFieldBase(limitField);
        System.out.println("limit: " + unsafe.getLong(limitBase, limitOffset));
        unsafe.putLong(limitBase, limitOffset, 5678);
        System.out.println("limit now: " + limit);
        Field readyField = own.getDeclaredField(args.length > 0 ? "READY" : "ready");
        long readyOffset = unsafe.staticFieldOffset(readyField);
        Object readyBase = unsafe.staticFieldBase(readyField);
        unsafe.putBooleanVolatile(readyBase, readyOffset, false);
        System.out.println("ready now: " + ready);

        Point p = new Point();
        for (String name : new String[] {"x", "y", "label"}) {
            Field f = Point.class.getDeclaredField(name);
            long offset = unsafe.objectFieldOffset(f);
            if (f.getType() == int.class) {
                int x = unsafe.getInt(p, offset) * 10;
                unsafe.putInt(p, offset, x);
            } else if (f.getType() == long.class) {
                long y = unsafe.getLong(p, offset) + 100;
                unsafe.putLong(p, offset, y);
            } else {
                String label = unsafe.getObject(p, offset) + "!";
                unsafe.putObject(p, offset, label);
            }
        }
        System.out.println(p.x + " " + p.y + " " + p.label);

        try {
            own.getDeclaredField(args.length > 0 ? "limit" : "LIMIT");
            System.out.println("found LIMIT");
        } catch (NoSuchFieldException e) {
            System.out.println("no such field: " + e.getClass().getSimpleName());
        }
    }
}
