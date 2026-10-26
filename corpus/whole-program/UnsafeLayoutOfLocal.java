import java.lang.reflect.Field;
import sun.misc.Unsafe;

// The layout of arrays asked of an `Unsafe` held in a local, as netty's `PlatformDependent0`
// asks the one it found by reflection (`unsafe.arrayIndexScale(byte[].class)`), and of a null
// one, which fails before it answers.
public class UnsafeLayoutOfLocal {
    static Object find() {
        try {
            Field field = Unsafe.class.getDeclaredField("theUnsafe");
            field.setAccessible(true);
            return field.get(null);
        } catch (Exception e) {
            return e;
        }
    }

    static int scale(Unsafe unsafe) {
        return unsafe.arrayIndexScale(long[].class);
    }

    public static void main(String[] args) {
        Object found = find();
        Unsafe unsafe = found instanceof Unsafe u ? u : null;
        System.out.println(unsafe.arrayIndexScale(byte[].class));
        System.out.println(scale(unsafe));
        System.out.println(unsafe.arrayIndexScale(Object[].class) > 0);
        System.out.println(unsafe.arrayBaseOffset(byte[].class) > 0);
        Unsafe none = args.length > 7 ? unsafe : null;
        try {
            System.out.println(none.arrayIndexScale(int[].class));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(scale(none));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
