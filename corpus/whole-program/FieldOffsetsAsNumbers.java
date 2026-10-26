import java.lang.reflect.Field;
import sun.misc.Unsafe;

// Field offsets used as numbers, as joml's `MemUtilUnsafe` finds a matrix's offsets by name to
// check its layout before it reads memory at offsets of its own computing: what may be compared
// is the same as on the JVM (each field its own offset, the same however it is found), and a
// missing field throws.
public class FieldOffsetsAsNumbers {
    static final Unsafe UNSAFE;

    static {
        try {
            Field field = Unsafe.class.getDeclaredField("theUnsafe");
            field.setAccessible(true);
            UNSAFE = (Unsafe) field.get(null);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static class Matrix {
        float m00, m01, m10, m11;
        static int made;
    }

    static long offset(Class<?> c, String name) throws NoSuchFieldException {
        Field f = c.getDeclaredField(name);
        return UNSAFE.objectFieldOffset(f);
    }

    public static void main(String[] args) throws Exception {
        String[] names = {"m00", "m01", "m10", "m11"};
        long[] offsets = new long[names.length];
        for (int i = 0; i < names.length; i++) {
            offsets[i] = offset(Matrix.class, names[i]);
        }
        boolean distinct = true;
        for (int i = 0; i < offsets.length; i++) {
            for (int j = i + 1; j < offsets.length; j++) {
                distinct &= offsets[i] != offsets[j];
            }
        }
        System.out.println("distinct " + distinct);
        System.out.println("same again " + (offset(Matrix.class, "m10") == offsets[2]));
        System.out.println("positive " + (offsets[0] > 0));
        try {
            offset(Matrix.class, "m22");
        } catch (NoSuchFieldException e) {
            System.out.println("no field");
        }
        try {
            offset(Matrix.class, "made");
        } catch (IllegalArgumentException e) {
            System.out.println("static field");
        }
        try {
            UNSAFE.objectFieldOffset(null);
        } catch (NullPointerException e) {
            System.out.println("null field " + e.getMessage());
        }
    }
}
